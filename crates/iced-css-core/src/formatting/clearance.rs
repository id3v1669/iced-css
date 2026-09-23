//! CSS 2.2 §9.5.2 float selection, strict-case presence and first amount algorithm.
//!
//! The caller establishes earlier same-BFC membership and recomputes margin
//! adjacency after presence classification. Equal-edge introduction remains
//! unsupported; the explicit amount API can also consume caller-supplied presence.
//! Coordinates increase downward and share one origin.
use super::{
    arithmetic::Exact,
    floats::{FloatSide, PlacedFloat},
};
use std::num::NonZeroUsize;

/// Whether the owner must rebuild clearance-affected margin adjacency.
/// `Required` does not supply the recomputed baseline for the amount solver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearanceRequirement {
    Absent,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearancePresenceError {
    InvalidInput(&'static str),
    /// Equal hypothetical and relevant float edges remain unsupported pending
    /// stronger specification evidence. This is not absence or zero clearance.
    UnsupportedEquality,
}
impl std::fmt::Display for ClearancePresenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "clearance presence: {self:?}")
    }
}
impl std::error::Error for ClearancePresenceError {}

/// Classify a nonfloating block's clearance requirement before rebuilding margins.
///
/// `hypothetical_top` is the border top as if `clear` were `none`.
/// `lowest_float_bottom` is the selector's result over caller-established eligible
/// floats. Propagate selection errors rather than turning them into `None`.
/// Coordinates share one downward-increasing origin. Finite strict comparisons
/// use no tolerance or subtraction; signed zeros compare equal. If upstream
/// rounding produces equality, this API reports the unsupported case.
/// A hypothetical top below the bottom numerically (`H < F`) returns `Required`;
/// `H > F`, or no selected bottom, returns `Absent`.
///
/// `Required` means the caller must derive the post-barrier zero-clearance baseline
/// before invoking [`solve_clearance`]. This fixed-cost function does not own the
/// formatting tree, rebuild adjacency, or manufacture that baseline.
///
/// # Errors
/// Validates the hypothetical top first (even with no selected bottom), then any
/// bottom, rejecting nonfinite values. Equal finite coordinates return
/// [`ClearancePresenceError::UnsupportedEquality`]; callers must not silently
/// convert this into either requirement state.
pub fn classify_clearance(
    hypothetical_top: f32,
    lowest_float_bottom: Option<f32>,
) -> Result<ClearanceRequirement, ClearancePresenceError> {
    if !hypothetical_top.is_finite() {
        return Err(ClearancePresenceError::InvalidInput("hypothetical_top"));
    }
    let Some(bottom) = lowest_float_bottom else {
        return Ok(ClearanceRequirement::Absent);
    };
    if !bottom.is_finite() {
        return Err(ClearancePresenceError::InvalidInput("lowest_float_bottom"));
    }
    if hypothetical_top < bottom {
        Ok(ClearanceRequirement::Required)
    } else if hypothetical_top > bottom {
        Ok(ClearanceRequirement::Absent)
    } else {
        Err(ClearancePresenceError::UnsupportedEquality)
    }
}

/// The side constraint supplied by the caller's resolved `clear` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearSide {
    None,
    Left,
    Right,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearanceFloatError {
    /// The entire supplied slice exceeds the prepaid record allowance.
    Work,
    /// A bottom is nonfinite, including on an unselected side.
    InvalidBottom { index: usize },
}
impl std::fmt::Display for ClearanceFloatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "clearance float selection: {self:?}")
    }
}
impl std::error::Error for ClearanceFloatError {}

/// Select the lowest matching outer bottom from caller-established eligible floats.
///
/// The caller must supply only earlier source floats in the same block formatting
/// context, excluding the clearing element's descendants. Coordinates share one
/// origin and increase downward. This function cannot verify source membership.
/// Zero-height and zero-width floats remain eligible; no occupancy test is applied.
/// Only sides and bottoms are consumed, so other geometry is not validated.
///
/// `None` means no selected constraint, not a decision about clearance presence.
/// `Some(0.0)` is a real bottom and is normalized to positive zero. `ClearSide::None`
/// returns without inspecting floats. Other sides prepay one complete scan before
/// validation. This function allocates no storage and performs no coordinate sums.
///
/// # Errors
/// For `clear != ClearSide::None`, returns [`ClearanceFloatError::Work`] before
/// traversal if `floats.len()` exceeds `work`. Otherwise returns the index of the
/// first nonfinite bottom in slice order, even on the unselected side.
pub fn lowest_clearance_float_bottom(
    floats: &[PlacedFloat],
    clear: ClearSide,
    work: NonZeroUsize,
) -> Result<Option<f32>, ClearanceFloatError> {
    if clear == ClearSide::None {
        return Ok(None);
    }
    if floats.len() > work.get() {
        return Err(ClearanceFloatError::Work);
    }
    let mut bottom: Option<f32> = None;
    for (index, float) in floats.iter().enumerate() {
        if !float.bottom.is_finite() {
            return Err(ClearanceFloatError::InvalidBottom { index });
        }
        if matches!(
            (clear, float.side),
            (ClearSide::Both, _)
                | (ClearSide::Left, FloatSide::Left)
                | (ClearSide::Right, FloatSide::Right)
        ) {
            bottom = Some(bottom.map_or(float.bottom, |current| current.max(float.bottom)));
        }
    }
    Ok(bottom.map(zero))
}

/// Presence is supplied by the formatting owner, never inferred from the amount.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClearanceState {
    Absent,
    Required {
        /// Border top after clearance-induced adjacency changes and remaining
        /// permitted collapse, evaluated with the numerical clearance set to zero.
        /// This is not the hypothetical top, nor generally a sum of two margins.
        zero_clearance_border_top: f32,
        /// Bottom outer coordinate of the lowest relevant float for this calculation.
        lowest_float_bottom: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearancePlacement {
    /// Use this coordinate directly: adding the rounded clearance to the baseline
    /// may round differently from the selected border coordinate.
    pub border_top: f32,
    /// `Some(0.0)` and negative amounts still represent present clearance, with
    /// the associated margin-adjacency barrier. Only `None` means absent.
    pub clearance: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearanceError {
    InvalidInput(&'static str),
    Unrepresentable,
}
impl std::fmt::Display for ClearanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "clearance calculation: {self:?}")
    }
}
impl std::error::Error for ClearanceError {}

fn finite(value: f32, field: &'static str) -> Result<(), ClearanceError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ClearanceError::InvalidInput(field))
    }
}
fn zero(value: f32) -> f32 {
    if value == 0.0 { 0.0 } else { value }
}

/// Preserve the border top computed as if `clear` were `none`. If the caller has
/// determined clearance is required, use the greater of the amounts to reach the
/// float bottom and preserve this hypothetical position. Select the target first,
/// then subtract the recomputed zero-clearance baseline exactly and round once.
///
/// This is fixed-cost, allocation-free arithmetic. It does not select floats,
/// decide whether clearance exists, or implement `clear` on a floating element.
///
/// # Errors
/// Rejects nonfinite input and an amount outside finite binary32 representation.
pub fn solve_clearance(
    hypothetical_top: f32,
    state: ClearanceState,
) -> Result<ClearancePlacement, ClearanceError> {
    finite(hypothetical_top, "hypothetical_top")?;
    match state {
        ClearanceState::Absent => Ok(ClearancePlacement {
            border_top: zero(hypothetical_top),
            clearance: None,
        }),
        ClearanceState::Required {
            zero_clearance_border_top,
            lowest_float_bottom,
        } => {
            finite(zero_clearance_border_top, "zero_clearance_border_top")?;
            finite(lowest_float_bottom, "lowest_float_bottom")?;
            let border_top = zero(hypothetical_top.max(lowest_float_bottom));
            // max(F-B, H-B) == max(F,H)-B, without rounding either candidate.
            let clearance = Exact::from_f32(border_top)
                .sub(Exact::from_f32(zero_clearance_border_top))
                .to_f32()
                .map_err(|_| ClearanceError::Unrepresentable)?;
            Ok(ClearancePlacement {
                border_top,
                clearance: Some(clearance),
            })
        }
    }
}
