//! CSS22 12.2 computed content. Counter strings and quote-depth changes
//! remain logical traversal operations.
use super::property::{ContentItem, ParsedProperty, SpecifiedValue};
use crate::source::Metadata;
use crate::{
    formatting::{counters::CounterRead, quotes::QuoteOperation},
    resources::uri,
};
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentTarget {
    Element,
    Before,
    After,
}
#[derive(Debug, Clone, Copy)]
pub enum ContentSelection<'a> {
    Normal,
    Suppressed,
    /// Specified items needing per-item computation, not a completed computed value.
    Items(&'a [ContentItem]),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentComputationError {
    WrongProperty,
    Unresolved,
    InvalidValue,
    Work,
    Items,
    Bytes,
    Allocation,
    MissingBase,
    Uri(uri::UriError),
}
impl std::fmt::Display for ContentComputationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "content computation: {self:?}")
    }
}
impl std::error::Error for ContentComputationError {}

pub fn select_content(
    property: &ParsedProperty,
    target: ContentTarget,
) -> Result<ContentSelection<'_>, ContentComputationError> {
    if property.property.descriptor().name != "content" {
        return Err(ContentComputationError::WrongProperty);
    }
    if target == ContentTarget::Element {
        return Ok(ContentSelection::Normal);
    }
    match &property.value {
        SpecifiedValue::Keyword(keyword) if matches!(keyword.as_str(), "normal" | "none") => {
            Ok(ContentSelection::Suppressed)
        }
        SpecifiedValue::Content(items) => Ok(ContentSelection::Items(items)),
        SpecifiedValue::Inherit | SpecifiedValue::Initial | SpecifiedValue::InitialReference(_) => {
            Err(ContentComputationError::Unresolved)
        }
        _ => Err(ContentComputationError::InvalidValue),
    }
}

/// The caller supplies the originating source element's immutable metadata revision.
/// This document language uses exact attribute names. Missing means empty text;
/// returned text is not interpreted as CSS or as generated-content tokens.
pub fn resolve_content_attribute<'a>(
    name: &str,
    origin: &'a Metadata,
    work: NonZeroUsize,
) -> Result<&'a str, ContentComputationError> {
    attribute_with_work(name, origin, &mut work.get())
}
fn attribute_with_work<'a>(
    name: &str,
    origin: &'a Metadata,
    work: &mut usize,
) -> Result<&'a str, ContentComputationError> {
    let required = name
        .len()
        .checked_add(1)
        .and_then(|cost| cost.checked_mul(origin.attribute_count()))
        .and_then(|cost| cost.checked_add(1))
        .ok_or(ContentComputationError::Work)?;
    spend(work, required)?;
    Ok(origin.attribute(name).unwrap_or(""))
}

#[derive(Debug, Clone, Copy)]
pub struct ContentLimits {
    pub items: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug)]
pub enum ComputedContent<'a> {
    Normal,
    Suppressed,
    Items(Vec<ComputedContentItem<'a>>),
}
#[derive(Debug)]
pub enum ComputedContentItem<'a> {
    Text(&'a str),
    /// An absolute generic URI; scheme conversion/acquisition are later operations.
    Uri(String),
    Counter(CounterRead<'a>),
    Quote(QuoteOperation),
}
/// Compute ordered content items against an immutable originating source revision.
/// Owned URI and vector capacities share the byte bound; borrowed input strings do
/// not. Counters and quotes remain logical operations and are not sampled here.
pub fn compute_content<'a>(
    property: &'a ParsedProperty,
    target: ContentTarget,
    origin: &'a Metadata,
    stylesheet_base: Option<&str>,
    limits: ContentLimits,
) -> Result<ComputedContent<'a>, ContentComputationError> {
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(ContentComputationError::Bytes);
    }
    let items = match select_content(property, target)? {
        ContentSelection::Normal => return Ok(ComputedContent::Normal),
        ContentSelection::Suppressed => return Ok(ComputedContent::Suppressed),
        ContentSelection::Items(items) => items,
    };
    if items.len() > limits.items.get() {
        return Err(ContentComputationError::Items);
    }
    let mut work = limits.work.get();
    spend(
        &mut work,
        items
            .len()
            .checked_add(1)
            .ok_or(ContentComputationError::Work)?,
    )?;
    let size = size_of::<ComputedContentItem<'_>>();
    let available = limits.retained_bytes.get() / size;
    if items.len() > available {
        return Err(ContentComputationError::Bytes);
    }
    let mut computed = Vec::new();
    computed
        .try_reserve_exact(items.len())
        .map_err(|_| ContentComputationError::Allocation)?;
    if computed.capacity() > available {
        return Err(ContentComputationError::Bytes);
    }
    let mut owned_bytes = computed.capacity() * size;
    for item in items {
        computed.push(match item {
            ContentItem::Text(text) => ComputedContentItem::Text(text),
            ContentItem::Attribute(name) => {
                ComputedContentItem::Text(attribute_with_work(name, origin, &mut work)?)
            }
            ContentItem::Uri(reference) => {
                let base = stylesheet_base.ok_or(ContentComputationError::MissingBase)?;
                let remaining = NonZeroUsize::new(limits.retained_bytes.get() - owned_bytes)
                    .ok_or(ContentComputationError::Bytes)?;
                let uri = uri::resolve_with_work(base, reference, remaining, &mut work)
                    .map_err(ContentComputationError::Uri)?;
                owned_bytes += uri.capacity();
                ComputedContentItem::Uri(uri)
            }
            ContentItem::Counter { .. } => ComputedContentItem::Counter(
                CounterRead::from_content(item).ok_or(ContentComputationError::InvalidValue)?,
            ),
            ContentItem::Quote(_) => ComputedContentItem::Quote(
                QuoteOperation::from_content(item).ok_or(ContentComputationError::InvalidValue)?,
            ),
        });
    }
    Ok(ComputedContent::Items(computed))
}
fn spend(work: &mut usize, amount: usize) -> Result<(), ContentComputationError> {
    *work = work
        .checked_sub(amount)
        .ok_or(ContentComputationError::Work)?;
    Ok(())
}
