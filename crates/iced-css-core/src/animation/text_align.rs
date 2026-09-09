//! Text alignment combines before the nameless initial resolves against direction.
use super::combine::{Combination, CombinationError, Endpoint, discrete_choice};
use crate::values::text_align::ComputedTextAlign;

pub fn combine_text_align(
    start: ComputedTextAlign,
    end: ComputedTextAlign,
    operation: Combination,
) -> Result<ComputedTextAlign, CombinationError> {
    match discrete_choice(operation)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
