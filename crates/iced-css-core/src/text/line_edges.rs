//! CSS22 16.6.1 initial/final space removal on an already selected logical line.
use crate::values::property::SpecifiedValue;
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    Nowrap,
    Pre,
    PreWrap,
    PreLine,
}
impl WhiteSpace {
    /// The parser canonicalizes keywords; unresolved defaulting is upstream.
    pub fn from_resolved(value: &SpecifiedValue) -> Option<Self> {
        let SpecifiedValue::Keyword(keyword) = value else {
            return None;
        };
        match keyword.as_str() {
            "normal" => Some(Self::Normal),
            "nowrap" => Some(Self::Nowrap),
            "pre" => Some(Self::Pre),
            "pre-wrap" => Some(Self::PreWrap),
            "pre-line" => Some(Self::PreLine),
            _ => None,
        }
    }
    pub fn collapses_spaces(self) -> bool {
        matches!(self, Self::Normal | Self::Nowrap | Self::PreLine)
    }
    pub fn allows_soft_wrap(self) -> bool {
        matches!(self, Self::Normal | Self::PreWrap | Self::PreLine)
    }
    pub fn preserves_newlines(self) -> bool {
        matches!(self, Self::Pre | Self::PreWrap | Self::PreLine)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEdgeKind {
    Scalar(char),
    Atomic,
    /// Classified by the text service, retained in source/UBA data, no glyph.
    BidiFormatting,
}
impl LineEdgeKind {
    /// Classify the original scalar using the pinned Unicode Bidi_Control property.
    /// Its original value/position remains in the caller's source mapping.
    pub fn from_scalar(scalar: char) -> Self {
        if super::unicode::is_bidi_control(scalar) {
            Self::BidiFormatting
        } else {
            Self::Scalar(scalar)
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineEdgeItem {
    pub kind: LineEdgeKind,
    pub white_space: WhiteSpace,
}
impl LineEdgeItem {
    fn collapsible(self) -> bool {
        self.kind == LineEdgeKind::Scalar(' ') && self.white_space.collapses_spaces()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDisposition {
    Keep,
    RemovedLeading,
    RemovedTrailing,
}
#[derive(Debug, Clone, Copy)]
pub struct LineEdgeLimits {
    pub items: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    /// Initialization and two edge passes, prepaid at three times item count.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEdgeError {
    Items,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for LineEdgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line-edge whitespace: {self:?}")
    }
}
impl std::error::Error for LineEdgeError {}
/// Classify removed spaces without deleting source items or bidi controls.
/// Input must follow newline/whitespace preprocessing and logical line selection.
/// Preserved-mode whitespace is retained, including optional pre-wrap trailing
/// whitespace. This stage does not decide whether a line is a phantom line.
///
/// # Errors
/// Rejects admission limits and allocation failures without partial output.
pub fn trim_line_edges(
    items: &[LineEdgeItem],
    limits: LineEdgeLimits,
) -> Result<Vec<EdgeDisposition>, LineEdgeError> {
    if items.len() > limits.items.get() {
        return Err(LineEdgeError::Items);
    }
    if items
        .len()
        .checked_mul(3)
        .is_none_or(|n| n > limits.work.get())
    {
        return Err(LineEdgeError::Work);
    }
    if limits.retained_bytes.get() > isize::MAX as usize
        || items
            .len()
            .checked_mul(size_of::<EdgeDisposition>())
            .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(LineEdgeError::Bytes);
    }
    let mut dispositions = Vec::new();
    dispositions
        .try_reserve_exact(items.len())
        .map_err(|_| LineEdgeError::Allocation)?;
    if dispositions
        .capacity()
        .checked_mul(size_of::<EdgeDisposition>())
        .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(LineEdgeError::Bytes);
    }
    dispositions.resize(items.len(), EdgeDisposition::Keep);
    for (item, disposition) in items.iter().zip(&mut dispositions) {
        if item.kind == LineEdgeKind::BidiFormatting {
            continue;
        }
        if !item.collapsible() {
            break;
        }
        *disposition = EdgeDisposition::RemovedLeading;
    }
    for (item, disposition) in items.iter().zip(&mut dispositions).rev() {
        if item.kind == LineEdgeKind::BidiFormatting {
            continue;
        }
        if !item.collapsible() {
            break;
        }
        if *disposition == EdgeDisposition::Keep {
            *disposition = EdgeDisposition::RemovedTrailing;
        }
    }
    Ok(dispositions)
}
