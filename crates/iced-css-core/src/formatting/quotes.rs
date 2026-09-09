//! Logical CSS quote-depth operations (Q272), independent of visual ordering.
use crate::values::property::{ContentItem, ParsedProperty, SpecifiedValue};
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteOperation {
    Open,
    Close,
    NoOpen,
    NoClose,
}
impl QuoteOperation {
    /// Select a logical quote operation without interpreting other content items.
    pub fn from_content(item: &ContentItem) -> Option<Self> {
        let ContentItem::Quote(keyword) = item else {
            return None;
        };
        match keyword.as_str() {
            "open-quote" => Some(Self::Open),
            "close-quote" => Some(Self::Close),
            "no-open-quote" => Some(Self::NoOpen),
            "no-close-quote" => Some(Self::NoClose),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteError {
    Depth,
}
impl std::fmt::Display for QuoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "quote operation: {self:?}")
    }
}
impl std::error::Error for QuoteError {}

/// Quote marks. An empty list or None suppresses marks, not depth operations.
#[derive(Debug, Clone, Copy)]
pub struct QuotePairs<'a>(Option<&'a [[String; 2]]>);
impl<'a> QuotePairs<'a> {
    /// Borrow an explicit quotes value. The caller must resolve inheritance and
    /// the user-agent initial recipe first; unresolved values return None.
    pub fn from_property(property: &'a ParsedProperty) -> Option<Self> {
        if property.property.descriptor().name != "quotes" {
            return None;
        }
        match &property.value {
            SpecifiedValue::Quotes(pairs) => Some(Self::new(pairs)),
            SpecifiedValue::Keyword(keyword) if keyword.as_str() == "none" => Some(Self::none()),
            _ => None,
        }
    }
    pub fn new(pairs: &'a [[String; 2]]) -> Self {
        if pairs.is_empty() {
            return Self::none();
        }
        Self(Some(pairs))
    }
    pub const fn none() -> Self {
        Self(None)
    }
    fn mark(self, depth: usize, closing: bool) -> Option<&'a str> {
        let pairs = self.0?;
        Some(pairs[depth.min(pairs.len() - 1)][usize::from(closing)].as_str())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QuoteDepth {
    depth: usize,
    limit: NonZeroUsize,
}
impl QuoteDepth {
    pub const fn new(limit: NonZeroUsize) -> Self {
        Self { depth: 0, limit }
    }
    pub fn depth(&self) -> usize {
        self.depth
    }
    /// One logical token, with no allocation or caller callback. A failed
    /// increment produces neither a mark nor a changed depth.
    pub fn apply<'a>(
        &mut self,
        operation: QuoteOperation,
        pairs: QuotePairs<'a>,
    ) -> Result<Option<&'a str>, QuoteError> {
        match operation {
            QuoteOperation::Open | QuoteOperation::NoOpen => {
                let next = self
                    .depth
                    .checked_add(1)
                    .filter(|depth| *depth <= self.limit.get())
                    .ok_or(QuoteError::Depth)?;
                let text = if operation == QuoteOperation::Open {
                    pairs.mark(self.depth, false)
                } else {
                    None
                };
                self.depth = next;
                Ok(text)
            }
            QuoteOperation::Close | QuoteOperation::NoClose => {
                if self.depth == 0 {
                    return Ok(None);
                }
                self.depth -= 1;
                Ok(if operation == QuoteOperation::Close {
                    pairs.mark(self.depth, true)
                } else {
                    None
                })
            }
        }
    }
}
