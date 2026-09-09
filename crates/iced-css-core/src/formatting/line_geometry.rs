//! CSS22 10.8 vertical line geometry over resolved inline/subtree bounds.
//! Font selection, horizontal line breaking and subtree construction are separate.
use super::arithmetic::Exact;
use std::{mem::size_of, num::NonZeroUsize};

/// Exact vertical interval relative to a participant's original coordinate origin.
/// Font bounds use the baseline as origin. No intermediate rounded edges escape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlineBounds {
    top: Exact,
    bottom: Exact,
}
impl InlineBounds {
    /// Include half-leading around resolved font metrics. Negative leading is
    /// valid; ascent, descent and line-height themselves must be nonnegative.
    pub fn font(ascent: f32, descent: f32, line_height: f32) -> Result<Self, LineGeometryError> {
        if [ascent, descent, line_height]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(LineGeometryError::Metrics);
        }
        let difference = Exact::from_f32(descent).sub(Exact::from_f32(ascent));
        Ok(Self {
            top: difference.sub(Exact::from_f32(line_height)).half(),
            bottom: difference.add(Exact::from_f32(line_height)).half(),
        })
    }
    /// Bounds of a previously resolved aligned subtree or atomic margin box.
    /// Signed edges are allowed; they must be finite and ordered.
    pub fn edges(top: f32, bottom: f32) -> Result<Self, LineGeometryError> {
        if !top.is_finite() || !bottom.is_finite() || top > bottom {
            return Err(LineGeometryError::Bounds);
        }
        Ok(Self {
            top: Exact::from_f32(top),
            bottom: Exact::from_f32(bottom),
        })
    }
    fn height(self) -> Exact {
        self.bottom.sub(self.top)
    }
}
/// Exact local property offset; construction is restricted to the resolver.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedRaise(Exact);
#[derive(Debug, Clone, Copy)]
pub enum LineAlignment {
    /// Already-resolved numeric shift from the parent baseline. Positive raises.
    Baseline {
        raise: f32,
    },
    /// Property-derived offset retained exactly until line placement.
    Resolved(ResolvedRaise),
    Top,
    Bottom,
}
#[derive(Debug, Clone, Copy)]
pub struct LineParticipant {
    pub bounds: InlineBounds,
    pub alignment: LineAlignment,
}
#[derive(Debug, Clone, Copy)]
pub struct LineGeometryLimits {
    pub participants: NonZeroUsize,
    /// Output Vec capacity only; borrowed input and fixed stack storage separate.
    pub retained_bytes: NonZeroUsize,
    /// Two participant passes, charged before either begins.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineGeometryError {
    Metrics,
    Bounds,
    Alignment,
    Participants,
    Bytes,
    Work,
    Allocation,
    Unrepresentable,
}
impl std::fmt::Display for LineGeometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "inline line geometry: {self:?}")
    }
}
impl std::error::Error for LineGeometryError {}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinePlacement {
    pub top: f32,
    pub bottom: f32,
    /// Translation from the participant's original origin to line-relative space.
    pub origin: f32,
}
#[derive(Debug, PartialEq)]
pub struct LineGeometry {
    pub height: f32,
    /// Parent baseline position relative to the line's top. May lie outside it.
    pub baseline: f32,
    /// Input participant order, retaining the caller's source/fragment mapping.
    pub placements: Vec<LinePlacement>,
}
fn rounded(value: Exact) -> Result<f32, LineGeometryError> {
    value
        .to_f32()
        .map_err(|_| LineGeometryError::Unrepresentable)
}
/// Union normal participants with the parent strut, then fit top/bottom subtrees.
/// DP06 preserves the normal top and adds any extra height below it.
///
/// # Errors
/// Rejects invalid raises, limits, allocation failures and unrepresentable final
/// geometry. No partial placement vector escapes. Empty input still has a strut;
/// phantom-line suppression must be decided by the containing line builder.
pub fn layout_line_geometry(
    strut: InlineBounds,
    participants: &[LineParticipant],
    limits: LineGeometryLimits,
) -> Result<LineGeometry, LineGeometryError> {
    if participants.len() > limits.participants.get() {
        return Err(LineGeometryError::Participants);
    }
    let work = participants
        .len()
        .checked_mul(2)
        .ok_or(LineGeometryError::Work)?;
    if work > limits.work.get() {
        return Err(LineGeometryError::Work);
    }
    let bytes = participants
        .len()
        .checked_mul(size_of::<LinePlacement>())
        .ok_or(LineGeometryError::Bytes)?;
    if limits.retained_bytes.get() > isize::MAX as usize || bytes > limits.retained_bytes.get() {
        return Err(LineGeometryError::Bytes);
    }
    let (height, baseline) = exact_extents(strut, participants)?;
    let rounded_height = rounded(height)?;
    let rounded_baseline = rounded(baseline)?;
    let mut placements = Vec::new();
    placements
        .try_reserve_exact(participants.len())
        .map_err(|_| LineGeometryError::Allocation)?;
    if placements
        .capacity()
        .checked_mul(size_of::<LinePlacement>())
        .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(LineGeometryError::Bytes);
    }
    for participant in participants {
        let origin = exact_origin(*participant, height, baseline);
        let placed_top = participant.bounds.top.add(origin);
        let placed_bottom = participant.bounds.bottom.add(origin);
        placements.push(LinePlacement {
            top: rounded(placed_top)?,
            bottom: rounded(placed_bottom)?,
            origin: rounded(origin)?,
        });
    }
    Ok(LineGeometry {
        height: rounded_height,
        baseline: rounded_baseline,
        placements,
    })
}

#[path = "vertical_align.rs"]
mod vertical_align;
pub use vertical_align::{
    ResolvedVerticalAlign, VerticalAlignContext, VerticalAlignError, VerticalMetric,
    resolve_computed_vertical_align, resolve_vertical_align,
};

fn exact_extents(
    strut: InlineBounds,
    participants: &[LineParticipant],
) -> Result<(Exact, Exact), LineGeometryError> {
    let (mut top, mut bottom) = (strut.top, strut.bottom);
    let mut aligned_height = Exact::ZERO;
    for participant in participants {
        match participant.alignment {
            LineAlignment::Baseline { raise } => {
                if !raise.is_finite() {
                    return Err(LineGeometryError::Alignment);
                }
                let raise = Exact::from_f32(raise);
                top = top.min(participant.bounds.top.sub(raise));
                bottom = bottom.max(participant.bounds.bottom.sub(raise));
            }
            LineAlignment::Resolved(ResolvedRaise(raise)) => {
                top = top.min(participant.bounds.top.sub(raise));
                bottom = bottom.max(participant.bounds.bottom.sub(raise));
            }
            LineAlignment::Top | LineAlignment::Bottom => {
                aligned_height = aligned_height.max(participant.bounds.height());
            }
        }
    }
    let height = bottom.sub(top).max(aligned_height);
    let baseline = Exact::ZERO.sub(top);
    Ok((height, baseline))
}
fn exact_origin(participant: LineParticipant, height: Exact, baseline: Exact) -> Exact {
    match participant.alignment {
        LineAlignment::Baseline { raise } => baseline.sub(Exact::from_f32(raise)),
        LineAlignment::Resolved(ResolvedRaise(raise)) => baseline.sub(raise),
        LineAlignment::Top => Exact::ZERO.sub(participant.bounds.top),
        LineAlignment::Bottom => height.sub(participant.bounds.bottom),
    }
}
#[path = "inline_tree.rs"]
mod inline_tree;
pub use inline_tree::{InlineNode, InlineTreeError, InlineTreeLimits, layout_inline_tree};
