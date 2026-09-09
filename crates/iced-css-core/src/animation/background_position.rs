//! Two-axis background-position arithmetic and offsets relative to a padding box.
use super::{
    combine::Combination,
    length_percentage::{LengthPercentage, MixedValueError, combine_length_percentage},
    number::Clamp,
};
use crate::values::{
    DecimalScalingError, NumericLimits,
    background_position::{ComputedBackgroundPosition, PositionComponent},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundPositionAnimationError {
    InvalidSize,
    Mixed(MixedValueError),
    Scaling(DecimalScalingError),
}
impl std::fmt::Display for BackgroundPositionAnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "background-position animation: {self:?}")
    }
}
impl std::error::Error for BackgroundPositionAnimationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mixed(e) => Some(e),
            Self::Scaling(e) => Some(e),
            _ => None,
        }
    }
}
impl From<MixedValueError> for BackgroundPositionAnimationError {
    fn from(e: MixedValueError) -> Self {
        Self::Mixed(e)
    }
}
#[derive(Debug)]
pub struct BackgroundPositionValue {
    axes: [LengthPercentage; 2],
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackgroundPositionSample {
    pub offset: [f32; 2],
    pub clamped: [Option<Clamp>; 2],
    pub rounded_to_zero: [bool; 2],
}
impl BackgroundPositionValue {
    pub fn from_computed(
        value: &ComputedBackgroundPosition,
        limits: NumericLimits,
    ) -> Result<Self, BackgroundPositionAnimationError> {
        let axis = |a| -> Result<LengthPercentage, BackgroundPositionAnimationError> {
            match a {
                PositionComponent::Pixels(v) => Ok(LengthPercentage::from_pixels(v, limits)?),
                PositionComponent::Percentage(v) => Ok(LengthPercentage::from_percentage(
                    v.checked_scale_f32(1.0, limits)
                        .map_err(BackgroundPositionAnimationError::Scaling)?,
                )),
            }
        };
        let [a, b] = value.components();
        Ok(Self {
            axes: [axis(a)?, axis(b)?],
        })
    }
    /// Caller supplies used padding-box and rendered-image sizes. This computes
    /// offsets only, not attachment, repeat tiling, box origin or paint publication.
    pub fn resolve(
        &self,
        padding_size: [f32; 2],
        image_size: [f32; 2],
        minimum: [f32; 2],
        maximum: [f32; 2],
        limits: NumericLimits,
    ) -> Result<BackgroundPositionSample, BackgroundPositionAnimationError> {
        if padding_size
            .into_iter()
            .chain(image_size)
            .any(|v| !v.is_finite() || v < 0.0)
        {
            return Err(BackgroundPositionAnimationError::InvalidSize);
        }
        let x = self.axes[0].resolve_difference_basis(
            padding_size[0],
            image_size[0],
            minimum[0],
            maximum[0],
            limits,
        )?;
        let y = self.axes[1].resolve_difference_basis(
            padding_size[1],
            image_size[1],
            minimum[1],
            maximum[1],
            limits,
        )?;
        Ok(BackgroundPositionSample {
            offset: [x.value, y.value],
            clamped: [x.clamped, y.clamped],
            rounded_to_zero: [x.rounded_to_zero, y.rounded_to_zero],
        })
    }
}
pub fn combine_background_position(
    start: &ComputedBackgroundPosition,
    end: &ComputedBackgroundPosition,
    operation: Combination,
    limits: NumericLimits,
) -> Result<BackgroundPositionValue, BackgroundPositionAnimationError> {
    if let Combination::Interpolate(p) = operation
        && !p.is_finite()
    {
        return Err(MixedValueError::NonfiniteProgress.into());
    }
    let a = BackgroundPositionValue::from_computed(start, limits)?;
    let b = BackgroundPositionValue::from_computed(end, limits)?;
    Ok(BackgroundPositionValue {
        axes: [
            combine_length_percentage(&a.axes[0], &b.axes[0], operation, limits)?,
            combine_length_percentage(&a.axes[1], &b.axes[1], operation, limits)?,
        ],
    })
}
