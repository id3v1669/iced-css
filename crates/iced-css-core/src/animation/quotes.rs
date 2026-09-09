//! Whole-property combination of computed quote marks, before logical traversal.
use super::combine::{Combination, CombinationError, Endpoint, discrete_choice};
use crate::formatting::quotes::QuotePairs;

/// All components are textual and discrete. Different lengths or strings require
/// whole-property fallback; equal lists produce the same result with this selection.
/// This operation borrows the selected storage and does not advance quote depth.
pub fn combine_quotes<'a>(
    start: QuotePairs<'a>,
    end: QuotePairs<'a>,
    operation: Combination,
) -> Result<QuotePairs<'a>, CombinationError> {
    match discrete_choice(operation)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
