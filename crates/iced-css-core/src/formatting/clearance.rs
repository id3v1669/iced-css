//! CSS 2.2 §9.5.2's first clearance amount algorithm for a nonfloating block.
//!
//! The caller determines presence, selects relevant earlier same-BFC floats, and
//! recomputes margin adjacency. This primitive neither classifies equal edges nor
//! rebuilds margin groups. Coordinates increase downward and share one origin.
use super::arithmetic::Exact;

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
