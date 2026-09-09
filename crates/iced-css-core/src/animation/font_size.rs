//! Final computed font-size pair combination with range-event classification.
use super::{
    combine::Combination,
    number::{Clamp, NumberError, combine_number},
};
use crate::values::RangeEvent;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSizeAnimationError {
    InvalidEndpoint,
    Number(NumberError),
}
impl std::fmt::Display for FontSizeAnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "font-size animation: {self:?}")
    }
}
impl std::error::Error for FontSizeAnimationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Number(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontSizeSample {
    pub value: f32,
    pub event: Option<RangeEvent>,
    pub clamped: Option<Clamp>,
}
/// Combine resolved absolute nonnegative font sizes and apply the approved
/// final0..f32::MAX domain. This is not a multi-effect intermediate value.
/// Diagnostic identity/coalescing/delivery and target refresh belong to the host.
pub fn combine_font_size(
    start: f32,
    end: f32,
    operation: Combination,
) -> Result<FontSizeSample, FontSizeAnimationError> {
    if [start, end].iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(FontSizeAnimationError::InvalidEndpoint);
    }
    let (sample, positive_rounded_to_zero) = combine_number(start, end, operation)
        .map_err(FontSizeAnimationError::Number)?
        .resolve_font_size();
    let event = if sample.clamped == Some(Clamp::Upper) {
        Some(RangeEvent::UpperSaturation)
    } else if positive_rounded_to_zero {
        Some(RangeEvent::PositiveRoundedToZero)
    } else {
        None
    };
    Ok(FontSizeSample {
        value: sample.value,
        event,
        clamped: sample.clamped,
    })
}
