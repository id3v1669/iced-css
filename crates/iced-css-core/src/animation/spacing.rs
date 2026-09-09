//! By-computed-value pair combination for border-spacing absolute lengths.
use super::{
    combine::Combination,
    number::{Clamp, CombinedNumber, NumberError, combine_number},
};
use crate::values::border_spacing::BorderSpacing;
#[derive(Debug, Clone, Copy)]
pub struct CombinedBorderSpacing {
    horizontal: CombinedNumber,
    vertical: CombinedNumber,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderSpacingSample {
    pub value: BorderSpacing,
    pub clamped: [Option<Clamp>; 2],
}
impl CombinedBorderSpacing {
    /// Apply the nonnegative property range and caller's explicit finite working
    /// maximum only at the final computed/used stage.
    pub fn resolve(self, maximum: f32) -> Result<BorderSpacingSample, NumberError> {
        let h = self.horizontal.resolve(0.0, maximum)?;
        let v = self.vertical.resolve(0.0, maximum)?;
        Ok(BorderSpacingSample {
            value: BorderSpacing::from_pixels(h.value, v.value)
                .expect("final numeric bounds ensure valid spacing"),
            clamped: [h.clamped, v.clamped],
        })
    }
}
/// Both endpoints are computed absolute lengths. Each component retains its exact
/// pair result; this function does not clamp or create a luminate track.
pub fn combine_border_spacing(
    start: BorderSpacing,
    end: BorderSpacing,
    operation: Combination,
) -> Result<CombinedBorderSpacing, NumberError> {
    Ok(CombinedBorderSpacing {
        horizontal: combine_number(start.horizontal(), end.horizontal(), operation)?,
        vertical: combine_number(start.vertical(), end.vertical(), operation)?,
    })
}
