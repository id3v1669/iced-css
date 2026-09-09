//! Whole computed cursor selection without URL copying or native callbacks.
use super::combine::{Combination, CombinationError, Endpoint, discrete_choice};
use crate::values::computed_cursor::ComputedCursor;

/// Select the complete ordered URL list and fallback. Different discrete textual
/// components or list counts require whole-property selection; equal endpoints
/// yield the same value without scanning or copying their contents.
pub fn combine_cursor<'a>(
    start: &'a ComputedCursor,
    end: &'a ComputedCursor,
    operation: Combination,
) -> Result<&'a ComputedCursor, CombinationError> {
    match discrete_choice(operation)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
