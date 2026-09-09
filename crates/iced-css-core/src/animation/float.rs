//! Float combination after CSS display/position/float endpoint computation.
use super::combine::{Combination, CombinationError, Endpoint, discrete_choice};
use crate::formatting::display::Float;

/// Combine computed float keywords. The display resolver owns absolute/fixed
/// suppression and display transformation, both for endpoint computation and
/// presentation reconciliation. This function performs no box generation.
pub fn combine_float(
    start: Float,
    end: Float,
    operation: Combination,
) -> Result<Float, CombinationError> {
    match discrete_choice(operation)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
