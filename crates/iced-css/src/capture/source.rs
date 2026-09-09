//! Borrowed selector integration before host identity allocation/publication.
use super::Construction;
use iced_css_core::{
    selectors::{ElementFacts, LinkState, SelectorSource},
    source::selector_tree::{DocumentLanguage, SelectorTreeLimits},
};

/// Explicit source-local state supplied by the owner. Default means no link or
/// active interaction; this view does not acquire state from a native window.
#[derive(Debug, Clone, Copy)]
pub struct ConstructionState {
    pub link: LinkState,
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
}
impl Default for ConstructionState {
    fn default() -> Self {
        Self {
            link: LinkState::NotLink,
            hovered: false,
            active: false,
            focused: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionSourceError {
    StateCount,
    EmptyDocumentLanguage,
    DuplicateCssId,
    Entries,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for ConstructionSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "captured selector source: {self:?}")
    }
}
impl std::error::Error for ConstructionSourceError {}

#[derive(Clone, Copy, Default)]
struct Indexed<'a> {
    previous: Option<usize>,
    last_child: Option<usize>,
    language: Option<&'a str>,
    explicit_language: bool,
    css_id: Option<&'a str>,
    state: ConstructionState,
}

/// Positions are local to this borrowed view, not source identities or tokens.
/// Source topology comes from private construction records, never native Trees.
pub struct ConstructionSource<'a> {
    construction: &'a Construction,
    indexed: Vec<Indexed<'a>>,
}
fn charge(work: &mut usize, cost: usize) -> Result<(), ConstructionSourceError> {
    *work = work
        .checked_sub(cost)
        .ok_or(ConstructionSourceError::Work)?;
    Ok(())
}

impl<'a> ConstructionSource<'a> {
    pub fn new(
        construction: &'a Construction,
        states: &[ConstructionState],
        language: DocumentLanguage<'a>,
        limits: SelectorTreeLimits,
    ) -> Result<Self, ConstructionSourceError> {
        let document_language = match language {
            DocumentLanguage::Unknown => None,
            DocumentLanguage::Known("") => {
                return Err(ConstructionSourceError::EmptyDocumentLanguage);
            }
            DocumentLanguage::Known(language) => Some(language),
        };
        let nodes = construction.nodes();
        if nodes.len() != states.len() {
            return Err(ConstructionSourceError::StateCount);
        }
        if nodes.len() > limits.entries.get() {
            return Err(ConstructionSourceError::Entries);
        }
        let bytes = nodes
            .len()
            .checked_mul(size_of::<Indexed<'_>>())
            .ok_or(ConstructionSourceError::Bytes)?;
        if bytes > limits.retained_bytes.get() {
            return Err(ConstructionSourceError::Bytes);
        }
        let mut work = limits.work.get();
        charge(
            &mut work,
            nodes
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(1))
                .ok_or(ConstructionSourceError::Work)?,
        )?;
        let mut indexed: Vec<Indexed<'a>> = Vec::new();
        indexed
            .try_reserve_exact(nodes.len())
            .map_err(|_| ConstructionSourceError::Allocation)?;
        if indexed
            .capacity()
            .checked_mul(size_of::<Indexed<'_>>())
            .ok_or(ConstructionSourceError::Bytes)?
            > limits.retained_bytes.get()
        {
            return Err(ConstructionSourceError::Bytes);
        }
        indexed.resize(nodes.len(), Indexed::default());
        for (position, node) in nodes.iter().enumerate() {
            let metadata = node.metadata();
            // Two exact-name linear lookups, bounded by the fixed key lengths.
            charge(
                &mut work,
                metadata
                    .attribute_count()
                    .checked_mul(8)
                    .ok_or(ConstructionSourceError::Work)?,
            )?;
            let css_id = metadata.css_id();
            if let Some(id) = css_id {
                for earlier in &indexed[..position] {
                    charge(&mut work, 1)?;
                    if let Some(other) = earlier.css_id {
                        charge(
                            &mut work,
                            id.len()
                                .min(other.len())
                                .checked_add(1)
                                .ok_or(ConstructionSourceError::Work)?,
                        )?;
                        if id == other {
                            return Err(ConstructionSourceError::DuplicateCssId);
                        }
                    }
                }
            }
            let local_language = metadata.attribute("lang");
            let previous = node
                .parent()
                .and_then(|parent| indexed[parent].last_child.replace(position));
            // Preserve last_child recorded by already visited postorder children.
            indexed[position].previous = previous;
            indexed[position].language = local_language.filter(|value| !value.is_empty());
            indexed[position].explicit_language = local_language.is_some();
            indexed[position].css_id = css_id;
            indexed[position].state = states[position];
        }
        // Every parent follows its children in this private postorder recipe.
        for position in (0..nodes.len()).rev() {
            if !indexed[position].explicit_language {
                indexed[position].language = nodes[position]
                    .parent()
                    .map_or(document_language, |parent| indexed[parent].language);
            }
        }
        for (position, node) in nodes.iter().enumerate() {
            if let Some(parent) = node.parent() {
                indexed[parent].state.hovered |= indexed[position].state.hovered;
                indexed[parent].state.active |= indexed[position].state.active;
            }
        }
        Ok(Self {
            construction,
            indexed,
        })
    }
    pub fn root(&self) -> usize {
        self.construction.root()
    }
    pub fn retained_bytes(&self) -> usize {
        self.indexed.capacity() * size_of::<Indexed<'_>>()
    }
}

impl SelectorSource for ConstructionSource<'_> {
    type Node = usize;
    fn facts(&self, position: usize) -> Option<ElementFacts<'_>> {
        let node = self.construction.nodes().get(position)?;
        let indexed = &self.indexed[position];
        Some(ElementFacts {
            type_name: node.canonical_type(),
            metadata: node.metadata(),
            has_parent: node.parent().is_some(),
            has_previous_element: indexed.previous.is_some(),
            language: indexed.language,
            link: indexed.state.link,
            hovered: indexed.state.hovered,
            active: indexed.state.active,
            focused: indexed.state.focused,
        })
    }
    fn parent(&self, position: usize) -> Option<usize> {
        self.construction.nodes().get(position)?.parent()
    }
    fn previous_element(&self, position: usize) -> Option<usize> {
        self.indexed.get(position)?.previous
    }
}
