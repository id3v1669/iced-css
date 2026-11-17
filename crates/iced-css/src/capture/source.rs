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
    next: Option<usize>,
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
    work: usize,
    #[cfg(test)]
    work_receipt: (usize, usize),
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
        let mut seen_ids = std::collections::HashSet::new();
        seen_ids
            .try_reserve(nodes.len())
            .map_err(|_| ConstructionSourceError::Allocation)?;
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
                // Hashes the id, so this costs its length and not the ids before it.
                charge(
                    &mut work,
                    id.len()
                        .checked_add(1)
                        .ok_or(ConstructionSourceError::Work)?,
                )?;
                if !seen_ids.insert(id) {
                    return Err(ConstructionSourceError::DuplicateCssId);
                }
            }
            let local_language = metadata.attribute("lang");
            let previous = node
                .parent()
                .and_then(|parent| indexed[parent].last_child.replace(position));
            // Preserve last_child recorded by already visited postorder children.
            indexed[position].previous = previous;
            if let Some(previous) = previous {
                indexed[previous].next = Some(position);
            }
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
            work: limits.work.get() - work,
            #[cfg(test)]
            work_receipt: (limits.work.get(), work),
        })
    }
    /// Checked structural work already consumed by this exact view construction.
    /// Includes duplicate-ID comparisons; an allowance is not a use metric.
    pub fn work(&self) -> usize {
        self.work
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
            empty: indexed.last_child.is_none() && node.text().is_none_or(str::is_empty),
            position: None,
        })
    }
    fn parent(&self, position: usize) -> Option<usize> {
        self.construction.nodes().get(position)?.parent()
    }
    fn next_element(&self, position: usize) -> Option<usize> {
        self.indexed.get(position)?.next
    }
    fn previous_element(&self, position: usize) -> Option<usize> {
        self.indexed.get(position)?.previous
    }
    fn last_child(&self, position: usize) -> Option<usize> {
        self.indexed.get(position)?.last_child
    }
}

#[cfg(test)]
mod work_tests {
    use super::*;
    use crate::capture::{CaptureLimits, CapturedElement, adapters};
    use iced_css_core::source::AttributeLimits;
    use std::num::NonZeroUsize;
    fn nz(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }
    fn capture() -> CaptureLimits {
        CaptureLimits {
            nodes: nz(100),
            depth: nz(100),
            retained_bytes: nz(100000),
            work: nz(100000),
        }
    }
    fn attributes() -> AttributeLimits {
        AttributeLimits {
            entries: nz(100),
            retained_bytes: nz(100000),
            work: nz(100000),
        }
    }
    fn leaf(id: Option<&str>) -> CapturedElement<'static, (), iced::Theme, ()> {
        let value = adapters::text::<iced::Theme, ()>("x", capture())
            .unwrap()
            .erase();
        if let Some(id) = id {
            value.style_id(id, attributes()).unwrap()
        } else {
            value
        }
    }
    #[test]
    fn actual_view_work_and_observed_terminal_allowance() {
        let cases = [
            (vec![None], None, false),
            (vec![None, None], None, false),
            (vec![Some("a")], None, false),
            (vec![Some("a"), None], Some("root"), false),
            (vec![Some("a"), Some("bb")], Some("root"), false),
            (vec![Some("a"), Some("bb")], Some("root"), true),
            (vec![Some("a"), Some("")], Some("root"), false),
        ];
        for (ids, root_id, lang) in cases {
            let mut element = if ids.len() == 1 {
                leaf(ids[0])
            } else {
                adapters::row_from_vec(ids.iter().map(|&id| leaf(id)).collect(), capture())
                    .unwrap()
                    .erase()
            };
            if let Some(id) = root_id {
                element = element.style_id(id, attributes()).unwrap();
            }
            if lang {
                element = element.style_attribute("lang", "tr", attributes()).unwrap();
            }
            let construction = element.into_parts().1;
            let nodes = construction.nodes();
            let expected_attrs: Vec<_> = if ids.len() == 1 {
                vec![usize::from(ids[0].is_some())]
            } else {
                vec![
                    usize::from(ids[0].is_some()),
                    usize::from(ids[1].is_some()),
                    usize::from(root_id.is_some()) + usize::from(lang),
                ]
            };
            assert_eq!(
                nodes
                    .iter()
                    .map(|n| n.metadata().attribute_count())
                    .collect::<Vec<_>>(),
                expected_attrs
            );
            let mut expected_ids: Vec<_> =
                ids.iter().map(|&id| id.filter(|s| !s.is_empty())).collect();
            if ids.len() > 1 {
                expected_ids.push(root_id);
            }
            assert_eq!(
                nodes
                    .iter()
                    .map(|n| n.metadata().css_id())
                    .collect::<Vec<_>>(),
                expected_ids
            );
            let states = vec![ConstructionState::default(); nodes.len()];
            // Four per node and one, eight per attribute, and each id's length and one.
            let expected = 4 * nodes.len()
                + 1
                + nodes
                    .iter()
                    .map(|node| {
                        8 * node.metadata().attribute_count()
                            + node.metadata().css_id().map_or(0, |id| id.len() + 1)
                    })
                    .sum::<usize>();
            for work in [expected, 100000, usize::MAX] {
                let view = ConstructionSource::new(
                    &construction,
                    &states,
                    DocumentLanguage::Unknown,
                    SelectorTreeLimits {
                        entries: nz(100),
                        retained_bytes: nz(100000),
                        work: nz(work),
                    },
                )
                .unwrap();
                assert_eq!(view.work(), expected);
                assert_eq!(view.work_receipt.0, work);
                assert_eq!(view.work_receipt.0 - view.work_receipt.1, expected);
            }
            assert!(matches!(
                ConstructionSource::new(
                    &construction,
                    &states,
                    DocumentLanguage::Unknown,
                    SelectorTreeLimits {
                        entries: nz(100),
                        retained_bytes: nz(100000),
                        work: nz(expected - 1)
                    }
                ),
                Err(ConstructionSourceError::Work)
            ));
        }
    }
}
