//! Border style combination and dependent presentation-width suppression.
use super::{
    combine::{Combination, CombinationError, Endpoint, discrete_choice},
    number::{Clamp, CombinedNumber, NumberError, combine_number},
};
use crate::formatting::table_borders::BorderStyle;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderAnimationError {
    InvalidEndpoint,
    Number(NumberError),
}
impl std::fmt::Display for BorderAnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "border animation: {self:?}")
    }
}
impl std::error::Error for BorderAnimationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Number(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct CombinedBorderWidth(CombinedNumber);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderWidthSample {
    pub value: f32,
    pub suppressed_by_style: bool,
    pub clamped: Option<Clamp>,
}
impl CombinedBorderWidth {
    /// Resolve after presentation style selection. Suppression is distinct from
    /// a numeric range clamp and does not mutate or control an animation track.
    pub fn resolve(
        self,
        style: BorderStyle,
        maximum: f32,
    ) -> Result<BorderWidthSample, NumberError> {
        if !maximum.is_finite() || maximum < 0.0 {
            return Err(NumberError::InvalidRange);
        }
        if matches!(style, BorderStyle::None | BorderStyle::Hidden) {
            return Ok(BorderWidthSample {
                value: 0.0,
                suppressed_by_style: true,
                clamped: None,
            });
        }
        let sample = self.0.resolve(0.0, maximum)?;
        Ok(BorderWidthSample {
            value: sample.value,
            suppressed_by_style: false,
            clamped: sample.clamped,
        })
    }
}
/// Both endpoints are resolved nonnegative absolute widths. Capturing a start,
/// refreshing a typed target and checking side identity belong to the owner.
pub fn combine_border_width(
    start: f32,
    end: f32,
    operation: Combination,
) -> Result<CombinedBorderWidth, BorderAnimationError> {
    if [start, end].iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(BorderAnimationError::InvalidEndpoint);
    }
    combine_number(start, end, operation)
        .map(CombinedBorderWidth)
        .map_err(BorderAnimationError::Number)
}
pub fn combine_border_style(
    start: BorderStyle,
    end: BorderStyle,
    operation: Combination,
) -> Result<BorderStyle, CombinationError> {
    match discrete_choice(operation)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
