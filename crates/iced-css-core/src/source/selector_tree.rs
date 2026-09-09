//! Borrowed validated selector view. Host capture, canonical type registration,
//! token liveness and owned snapshot publication remain the owner's responsibility.
use super::{Metadata, SourceNode};
use crate::{
    identity::InstanceId,
    selectors::{ElementFacts, LinkState, SelectorSource},
};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct SelectorElement<'a> {
    pub node: &'a SourceNode,
    pub type_name: &'a str,
    pub link: LinkState,
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
}
#[derive(Debug, Clone, Copy)]
pub enum DocumentLanguage<'a> {
    Unknown,
    Known(&'a str),
}
#[derive(Debug, Clone, Copy)]
pub struct SelectorTreeLimits {
    pub entries: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorTreeError {
    Entries,
    Bytes,
    Work,
    Allocation,
    EmptyDocumentLanguage,
    Root,
    DuplicateInstance,
    DuplicateCssId,
    UnknownInstance,
    DuplicateChild,
    Nonreciprocal,
    Cycle,
}
impl std::fmt::Display for SelectorTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "selector source view: {self:?}")
    }
}
impl std::error::Error for SelectorTreeError {}
/// A position scoped to this view, not a globally unique identity or capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementIndex(usize);
#[derive(Clone, Copy, Default)]
struct Indexed<'a> {
    parent: Option<ElementIndex>,
    previous: Option<ElementIndex>,
    local_language: Option<Option<&'a str>>,
    language: Option<&'a str>,
    css_id: Option<&'a str>,
    hovered: bool,
    active: bool,
}
pub struct SelectorTree<'a> {
    elements: &'a [SelectorElement<'a>],
    indexed: Vec<Indexed<'a>>,
    root: ElementIndex,
    retained_bytes: usize,
}
fn charge(work: &mut usize, count: usize) -> Result<(), SelectorTreeError> {
    *work = work.checked_sub(count).ok_or(SelectorTreeError::Work)?;
    Ok(())
}
fn attribute<'a>(
    metadata: &'a Metadata,
    key: &str,
    work: &mut usize,
) -> Result<Option<&'a str>, SelectorTreeError> {
    // Conservatively pay a linear bound over map entries; each comparison with
    // these fixed short keys inspects at most key.len()+1 units.
    let cost = metadata
        .attributes
        .len()
        .checked_add(1)
        .and_then(|count| count.checked_mul(key.len() + 1))
        .ok_or(SelectorTreeError::Work)?;
    charge(work, cost)?;
    Ok(metadata.attribute(key))
}
fn find(
    elements: &[SelectorElement<'_>],
    id: InstanceId,
    work: &mut usize,
) -> Result<ElementIndex, SelectorTreeError> {
    for (index, element) in elements.iter().enumerate() {
        charge(work, 1)?;
        if element.node.instance == id {
            return Ok(ElementIndex(index));
        }
    }
    Err(SelectorTreeError::UnknownInstance)
}
impl<'a> SelectorTree<'a> {
    /// Own only the bounded index. Source records, child lists, attributes and
    /// labels are borrowed unchanged. Construction is work-bounded, not a promise
    /// of linear-time identity lookup; all matcher callbacks are constant-time.
    pub fn new(
        elements: &'a [SelectorElement<'a>],
        document_language: DocumentLanguage<'a>,
        limits: SelectorTreeLimits,
    ) -> Result<Self, SelectorTreeError> {
        let document_language = match document_language {
            DocumentLanguage::Unknown => None,
            DocumentLanguage::Known("") => return Err(SelectorTreeError::EmptyDocumentLanguage),
            DocumentLanguage::Known(language) => Some(language),
        };
        if elements.len() > limits.entries.get() {
            return Err(SelectorTreeError::Entries);
        }
        let mut work = limits.work.get();
        charge(
            &mut work,
            elements
                .len()
                .checked_add(1)
                .ok_or(SelectorTreeError::Work)?,
        )?;
        let requested = elements
            .len()
            .checked_mul(size_of::<Indexed<'_>>())
            .ok_or(SelectorTreeError::Bytes)?;
        if requested > limits.retained_bytes.get().min(isize::MAX as usize) {
            return Err(SelectorTreeError::Bytes);
        }
        let mut indexed: Vec<Indexed<'a>> = Vec::new();
        indexed
            .try_reserve_exact(elements.len())
            .map_err(|_| SelectorTreeError::Allocation)?;
        let retained_bytes = indexed
            .capacity()
            .checked_mul(size_of::<Indexed<'_>>())
            .ok_or(SelectorTreeError::Bytes)?;
        if retained_bytes > limits.retained_bytes.get() {
            return Err(SelectorTreeError::Bytes);
        }
        let mut root = None;
        for (position, element) in elements.iter().enumerate() {
            if element.node.parent.is_none() && root.replace(ElementIndex(position)).is_some() {
                return Err(SelectorTreeError::Root);
            }
            let css_id =
                attribute(&element.node.metadata, "id", &mut work)?.filter(|id| !id.is_empty());
            for earlier in 0..position {
                charge(&mut work, 1)?;
                if elements[earlier].node.instance == element.node.instance {
                    return Err(SelectorTreeError::DuplicateInstance);
                }
                if let (Some(id), Some(previous)) = (css_id, indexed[earlier].css_id) {
                    charge(
                        &mut work,
                        id.len()
                            .checked_add(previous.len())
                            .and_then(|count| count.checked_add(1))
                            .ok_or(SelectorTreeError::Work)?,
                    )?;
                    if id == previous {
                        return Err(SelectorTreeError::DuplicateCssId);
                    }
                }
            }
            let local_language = attribute(&element.node.metadata, "lang", &mut work)?
                .map(|lang| (!lang.is_empty()).then_some(lang));
            indexed.push(Indexed {
                css_id,
                local_language,
                hovered: element.hovered,
                active: element.active,
                ..Default::default()
            });
        }
        let root = root.ok_or(SelectorTreeError::Root)?;
        let mut entries = elements.len();
        for (position, element) in elements.iter().enumerate() {
            if let Some(parent) = element.node.parent {
                indexed[position].parent = Some(find(elements, parent, &mut work)?);
            }
            let mut previous = None;
            for (child_position, child_id) in element.node.children.iter().enumerate() {
                entries = entries
                    .checked_add(1)
                    .filter(|entries| *entries <= limits.entries.get())
                    .ok_or(SelectorTreeError::Entries)?;
                charge(
                    &mut work,
                    child_position
                        .checked_add(1)
                        .ok_or(SelectorTreeError::Work)?,
                )?;
                if element.node.children[..child_position].contains(child_id) {
                    return Err(SelectorTreeError::DuplicateChild);
                }
                let child = find(elements, *child_id, &mut work)?;
                if elements[child.0].node.parent != Some(element.node.instance) {
                    return Err(SelectorTreeError::Nonreciprocal);
                }
                indexed[child.0].previous = previous;
                previous = Some(child);
            }
        }
        for (position, element) in elements.iter().enumerate() {
            if let Some(parent) = indexed[position].parent {
                let children = &elements[parent.0].node.children;
                charge(
                    &mut work,
                    children
                        .len()
                        .checked_add(1)
                        .ok_or(SelectorTreeError::Work)?,
                )?;
                if !children.contains(&element.node.instance) {
                    return Err(SelectorTreeError::Nonreciprocal);
                }
            }
            let mut cursor = Some(ElementIndex(position));
            let mut language = None;
            let mut traversed = 0;
            while let Some(current) = cursor {
                charge(&mut work, 1)?;
                if traversed == elements.len() {
                    return Err(SelectorTreeError::Cycle);
                }
                traversed += 1;
                if language.is_none() {
                    language = indexed[current.0].local_language;
                }
                indexed[current.0].hovered |= element.hovered;
                indexed[current.0].active |= element.active;
                cursor = indexed[current.0].parent;
            }
            indexed[position].language = language.unwrap_or(document_language);
        }
        Ok(Self {
            elements,
            indexed,
            root,
            retained_bytes,
        })
    }
    pub fn root(&self) -> ElementIndex {
        self.root
    }
    pub fn node(&self, position: usize) -> Option<ElementIndex> {
        (position < self.elements.len()).then_some(ElementIndex(position))
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}
impl SelectorSource for SelectorTree<'_> {
    type Node = ElementIndex;
    fn facts(&self, node: ElementIndex) -> Option<ElementFacts<'_>> {
        let element = self.elements.get(node.0)?;
        let indexed = self.indexed.get(node.0)?;
        Some(ElementFacts {
            type_name: element.type_name,
            metadata: &element.node.metadata,
            has_parent: indexed.parent.is_some(),
            has_previous_element: indexed.previous.is_some(),
            language: indexed.language,
            link: element.link,
            hovered: indexed.hovered,
            active: indexed.active,
            focused: element.focused,
        })
    }
    fn parent(&self, node: ElementIndex) -> Option<ElementIndex> {
        self.indexed.get(node.0)?.parent
    }
    fn previous_element(&self, node: ElementIndex) -> Option<ElementIndex> {
        self.indexed.get(node.0)?.previous
    }
}
