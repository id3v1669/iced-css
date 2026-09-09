//! CSS22 17.6.2.1 winner selection for one already-segmented collapsed edge.
use crate::values::property::SpecifiedValue;
use std::{cmp::Ordering, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    None,
    Hidden,
    Inset,
    Groove,
    Outset,
    Ridge,
    Dotted,
    Dashed,
    Solid,
    Double,
}
impl BorderStyle {
    /// Convert an already-defaulted border-side style, not an unresolved inherit.
    pub fn from_resolved(value: &SpecifiedValue) -> Result<Self, BorderConflictError> {
        let SpecifiedValue::Keyword(keyword) = value else {
            return Err(BorderConflictError::Style);
        };
        match keyword.as_str() {
            "none" => Ok(Self::None),
            "hidden" => Ok(Self::Hidden),
            "inset" => Ok(Self::Inset),
            "groove" => Ok(Self::Groove),
            "outset" => Ok(Self::Outset),
            "ridge" => Ok(Self::Ridge),
            "dotted" => Ok(Self::Dotted),
            "dashed" => Ok(Self::Dashed),
            "solid" => Ok(Self::Solid),
            "double" => Ok(Self::Double),
            _ => Err(BorderConflictError::Style),
        }
    }
    fn priority(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Inset => 1,
            Self::Groove => 2,
            Self::Outset => 3,
            Self::Ridge => 4,
            Self::Dotted => 5,
            Self::Dashed => 6,
            Self::Solid => 7,
            Self::Double => 8,
            Self::Hidden => 9,
        }
    }
    fn paint(self) -> Self {
        match self {
            Self::Inset => Self::Ridge,
            Self::Outset => Self::Groove,
            other => other,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderOrigin {
    Table,
    ColumnGroup,
    Column,
    RowGroup,
    Row,
    Cell,
}
impl BorderOrigin {
    fn priority(self) -> u8 {
        match self {
            Self::Table => 0,
            Self::ColumnGroup => 1,
            Self::Column => 2,
            Self::RowGroup => 3,
            Self::Row => 4,
            Self::Cell => 5,
        }
    }
}
/// Grid-supplied ranks. Lower inline rank means farther left in LTR, farther
/// right in RTL; lower block rank means farther up. These are not source IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BorderOrder {
    pub inline_start: usize,
    pub block_start: usize,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderCandidate {
    style: BorderStyle,
    width: f32,
    origin: BorderOrigin,
    order: BorderOrder,
}
impl BorderCandidate {
    /// Validate a computed candidate; hidden and none have zero computed width.
    pub fn new(
        style: BorderStyle,
        width: f32,
        origin: BorderOrigin,
        order: BorderOrder,
    ) -> Result<Self, BorderConflictError> {
        if !width.is_finite() || width < 0.0 {
            return Err(BorderConflictError::Width);
        }
        let width = if matches!(style, BorderStyle::None | BorderStyle::Hidden) || width == 0.0 {
            0.0
        } else {
            width
        };
        Ok(Self {
            style,
            width,
            origin,
            order,
        })
    }
    pub fn style(self) -> BorderStyle {
        self.style
    }
    pub fn width(self) -> f32 {
        self.width
    }
    fn stronger_than(self, other: Self) -> bool {
        // Constructors admit only finite nonnegative widths, with canonical zero.
        self.width
            .total_cmp(&other.width)
            .then_with(|| self.style.priority().cmp(&other.style.priority()))
            .then_with(|| self.origin.priority().cmp(&other.origin.priority()))
            .then_with(|| other.order.cmp(&self.order))
            == Ordering::Greater
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BorderConflictLimits {
    pub candidates: NonZeroUsize,
    /// One charged scan step per candidate, checked before inspecting any entry.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderConflictError {
    Width,
    Style,
    Candidates,
    Work,
}
impl std::fmt::Display for BorderConflictError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "collapsed table border: {self:?}")
    }
}
impl std::error::Error for BorderConflictError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapsedBorder {
    Absent,
    Suppressed,
    /// Index preserves the caller's source/color payload. Relief remapping is
    /// deliberately later than comparison of original styles.
    Winner {
        index: usize,
        paint_style: BorderStyle,
    },
}
/// Select one edge winner without allocation. Candidates must already belong to
/// the same geometric segment. Exact duplicate priorities retain the first input.
pub fn resolve_collapsed_border(
    candidates: &[BorderCandidate],
    limits: BorderConflictLimits,
) -> Result<CollapsedBorder, BorderConflictError> {
    if candidates.len() > limits.candidates.get() {
        return Err(BorderConflictError::Candidates);
    }
    if candidates.len() > limits.work.get() {
        return Err(BorderConflictError::Work);
    }
    Ok(resolve_indexed(candidates.iter().enumerate()))
}
fn resolve_indexed<'a>(
    candidates: impl Iterator<Item = (usize, &'a BorderCandidate)>,
) -> CollapsedBorder {
    let mut best: Option<(usize, &BorderCandidate)> = None;
    for (index, candidate) in candidates {
        match candidate.style {
            BorderStyle::Hidden => return CollapsedBorder::Suppressed,
            BorderStyle::None => continue,
            _ => {}
        }
        if best.is_none_or(|(_, old)| candidate.stronger_than(*old)) {
            best = Some((index, candidate));
        }
    }
    best.map_or(CollapsedBorder::Absent, |(index, candidate)| {
        CollapsedBorder::Winner {
            index,
            paint_style: candidate.style.paint(),
        }
    })
}

#[path = "table_border_segments.rs"]
mod segments;
pub use segments::{
    BorderEdge, BorderSegmentError, BorderSegmentLimits, CollapsedBorderSegment,
    split_collapsed_borders,
};
