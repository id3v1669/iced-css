//! Outline-only style admission with shared exact width arithmetic.
use super::{
    border::{BorderAnimationError, BorderWidthSample, CombinedBorderWidth, combine_border_width},
    combine::{Combination, CombinationError, Endpoint, discrete_choice},
    number::NumberError,
};
use crate::values::outline::OutlineStyle;

#[derive(Debug, Clone, Copy)]
pub struct CombinedOutlineWidth(CombinedBorderWidth);
impl CombinedOutlineWidth {
    /// Final width sampling, after selecting outline style. The returned sample
    /// uses the shared width result shape; none suppression is not a range clamp.
    pub fn resolve(
        self,
        style: OutlineStyle,
        maximum: f32,
    ) -> Result<BorderWidthSample, NumberError> {
        self.0.resolve(style.border_style(), maximum)
    }
}
/// Endpoints are computed nonnegative absolute outline widths. Target ownership,
/// refresh and contour painting belong to their respective providers.
pub fn combine_outline_width(
    start: f32,
    end: f32,
    operation: Combination,
) -> Result<CombinedOutlineWidth, BorderAnimationError> {
    combine_border_width(start, end, operation).map(CombinedOutlineWidth)
}
pub fn combine_outline_style(
    start: OutlineStyle,
    end: OutlineStyle,
    operation: Combination,
) -> Result<OutlineStyle, CombinationError> {
    match discrete_choice(operation)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
