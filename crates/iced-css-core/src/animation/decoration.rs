//! Whole-value combination for the CSS2.2 decoration keyword set.
use super::combine::{Combination, CombinationError, Endpoint, discrete_choice};
use crate::values::property::TextDecoration;
/// Unequal discrete components select the whole endpoint. Addition/accumulation
/// retain the first set; they never union decoration bits. Propagation and paint
/// are separate from this computed-value component operation.
pub fn combine_text_decoration(
    start: TextDecoration,
    end: TextDecoration,
    operation: Combination,
) -> Result<TextDecoration, CombinationError> {
    match discrete_choice(operation)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
