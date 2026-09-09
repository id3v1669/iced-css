//! Integer component animation, before property-specific final ranges.
use super::combine::Combination;
use crate::values::{Integer, IntegerInterpolationError, NumericLimits};
/// Preserve exact integer endpoints. Addition and default accumulation sum them;
/// interpolation rounds only its exact final real-number result, ties upward.
pub fn combine_integer(
    start: &Integer,
    end: &Integer,
    operation: Combination,
    limits: NumericLimits,
) -> Result<Integer, IntegerInterpolationError> {
    match operation {
        Combination::Interpolate(p) => start.checked_interpolate(end, p, limits),
        Combination::Add | Combination::Accumulate => {
            start.checked_add(end, limits).map_err(Into::into)
        }
    }
}
