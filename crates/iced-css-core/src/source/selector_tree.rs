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
    next: Option<ElementIndex>,
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
/// Comparisons one binary search or one sort pass over `count` items pays.
fn log_cost(count: usize) -> usize {
    (usize::BITS - count.leading_zeros()).max(1) as usize
}
/// Sorts `items`, first paying `unit` work per comparison over the sort's
/// `count * log2(count)` bound.
fn sort_charged<T: Ord>(
    items: &mut [T],
    unit: usize,
    work: &mut usize,
) -> Result<(), SelectorTreeError> {
    let cost = items
        .len()
        .checked_mul(log_cost(items.len()))
        .and_then(|count| count.checked_mul(unit))
        .ok_or(SelectorTreeError::Work)?;
    charge(work, cost)?;
    items.sort_unstable();
    Ok(())
}
fn find(
    by_instance: &[(InstanceId, usize)],
    id: InstanceId,
    work: &mut usize,
) -> Result<ElementIndex, SelectorTreeError> {
    charge(work, log_cost(by_instance.len()))?;
    by_instance
        .binary_search_by_key(&id, |(instance, _)| *instance)
        .map(|found| ElementIndex(by_instance[found].1))
        .map_err(|_| SelectorTreeError::UnknownInstance)
}
impl<'a> SelectorTree<'a> {
    /// Own only the bounded index. Source records, child lists, attributes and
    /// labels are borrowed unchanged. Construction is work-bounded: identity and
    /// CSS id checks sort transient indexes (n log n), and each element still
    /// walks its ancestor chain (n x depth). All matcher callbacks are
    /// constant-time.
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
        let mut by_instance: Vec<(InstanceId, usize)> = Vec::new();
        by_instance
            .try_reserve_exact(elements.len())
            .map_err(|_| SelectorTreeError::Allocation)?;
        by_instance.extend(
            elements
                .iter()
                .enumerate()
                .map(|(position, element)| (element.node.instance, position)),
        );
        sort_charged(&mut by_instance, 1, &mut work)?;
        if by_instance.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(SelectorTreeError::DuplicateInstance);
        }
        let mut css_ids: Vec<&str> = Vec::new();
        css_ids
            .try_reserve_exact(elements.len())
            .map_err(|_| SelectorTreeError::Allocation)?;
        css_ids.extend(indexed.iter().filter_map(|indexed| indexed.css_id));
        // Each string comparison inspects at most the longest id plus one.
        let longest = css_ids.iter().map(|id| id.len()).max().unwrap_or(0);
        sort_charged(&mut css_ids, longest + 1, &mut work)?;
        if css_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(SelectorTreeError::DuplicateCssId);
        }
        drop(css_ids);
        let root = root.ok_or(SelectorTreeError::Root)?;
        let mut entries = elements.len();
        // Whether an element was already linked as a child. A linked child's
        // parent owns the list it was found in, so each element can be
        // linked at most once.
        let mut linked: Vec<bool> = Vec::new();
        linked
            .try_reserve_exact(elements.len())
            .map_err(|_| SelectorTreeError::Allocation)?;
        linked.resize(elements.len(), false);
        for (position, element) in elements.iter().enumerate() {
            if let Some(parent) = element.node.parent {
                indexed[position].parent = Some(find(&by_instance, parent, &mut work)?);
            }
            let mut previous = None;
            for child_id in &element.node.children {
                entries = entries
                    .checked_add(1)
                    .filter(|entries| *entries <= limits.entries.get())
                    .ok_or(SelectorTreeError::Entries)?;
                charge(&mut work, 1)?;
                let child = find(&by_instance, *child_id, &mut work)?;
                if elements[child.0].node.parent != Some(element.node.instance) {
                    return Err(SelectorTreeError::Nonreciprocal);
                }
                if linked[child.0] {
                    return Err(SelectorTreeError::DuplicateChild);
                }
                linked[child.0] = true;
                indexed[child.0].previous = previous;
                if let Some(previous) = previous {
                    indexed[previous.0].next = Some(child);
                }
                previous = Some(child);
            }
        }
        for (position, element) in elements.iter().enumerate() {
            // Every linked child sits in its parent's list, so a parented
            // element that was never linked is missing from that list.
            charge(&mut work, 1)?;
            if indexed[position].parent.is_some() && !linked[position] {
                return Err(SelectorTreeError::Nonreciprocal);
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
            // This view holds elements only: an element's own text is not known to it.
            empty: element.node.children.is_empty(),
            position: None,
        })
    }
    fn parent(&self, node: ElementIndex) -> Option<ElementIndex> {
        self.indexed.get(node.0)?.parent
    }
    fn previous_element(&self, node: ElementIndex) -> Option<ElementIndex> {
        self.indexed.get(node.0)?.previous
    }
    fn next_element(&self, node: ElementIndex) -> Option<ElementIndex> {
        self.indexed.get(node.0)?.next
    }
}
