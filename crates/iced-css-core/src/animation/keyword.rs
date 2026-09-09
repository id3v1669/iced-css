//! Discrete combination of validated, property-identified keyword values.
use super::combine::{Combination, Endpoint, discrete_choice};
use crate::values::keyword::ComputedKeyword;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordAnimationError {
    PropertyMismatch,
    NonfiniteProgress,
}
impl std::fmt::Display for KeywordAnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "keyword animation: {self:?}")
    }
}
impl std::error::Error for KeywordAnimationError {}
/// Select a whole keyword value. Addition/accumulation retain the first operand.
/// This does not perform cascade defaulting, dependency propagation or rendering.
pub fn combine_keyword(
    start: ComputedKeyword,
    end: ComputedKeyword,
    operation: Combination,
) -> Result<ComputedKeyword, KeywordAnimationError> {
    if start.property() != end.property() {
        return Err(KeywordAnimationError::PropertyMismatch);
    }
    match discrete_choice(operation).map_err(|_| KeywordAnimationError::NonfiniteProgress)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
