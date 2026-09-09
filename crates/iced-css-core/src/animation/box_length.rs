//! Numeric and whole-keyword combination for property-identified box lengths.
use super::{
    combine::{Combination, Endpoint, discrete_choice},
    length_percentage::{LengthPercentage, MixedValueError, combine_length_percentage},
    number::Clamp,
};
use crate::values::{
    DecimalScalingError, NumericLimits,
    box_length::{BoxLengthComponent, ComputedBoxLength},
    registry::PropertyId,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxLengthAnimationError {
    PropertyMismatch,
    Mixed(MixedValueError),
    Scaling(DecimalScalingError),
}
impl std::fmt::Display for BoxLengthAnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "box length animation: {self:?}")
    }
}
impl std::error::Error for BoxLengthAnimationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mixed(e) => Some(e),
            Self::Scaling(e) => Some(e),
            _ => None,
        }
    }
}
impl From<MixedValueError> for BoxLengthAnimationError {
    fn from(e: MixedValueError) -> Self {
        Self::Mixed(e)
    }
}
#[derive(Debug)]
enum Value {
    Numeric(LengthPercentage),
    Auto,
    None,
}
#[derive(Debug)]
pub struct CombinedBoxLength {
    property: PropertyId,
    signed: bool,
    value: Value,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxLengthSampleValue {
    Pixels(f32),
    Auto,
    None,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxLengthSample {
    pub property: PropertyId,
    pub value: BoxLengthSampleValue,
    pub clamped: Option<Clamp>,
    pub rounded_to_zero: bool,
}
impl CombinedBoxLength {
    pub fn from_computed(
        value: &ComputedBoxLength,
        limits: NumericLimits,
    ) -> Result<Self, BoxLengthAnimationError> {
        let component = match value.component() {
            BoxLengthComponent::Pixels(v) => {
                Value::Numeric(LengthPercentage::from_pixels(v, limits)?)
            }
            BoxLengthComponent::Percentage(v) => Value::Numeric(LengthPercentage::from_percentage(
                v.checked_scale_f32(1.0, limits)
                    .map_err(BoxLengthAnimationError::Scaling)?,
            )),
            BoxLengthComponent::Auto => Value::Auto,
            BoxLengthComponent::None => Value::None,
        };
        Ok(Self {
            property: value.property(),
            signed: value.allows_negative(),
            value: component,
        })
    }
    /// The caller has already selected the applicable definite nonnegative basis
    /// or used-value fallback. This does not solve auto/none or percentage cycles.
    pub fn resolve(
        &self,
        basis: f32,
        coordinate_limit: f32,
        limits: NumericLimits,
    ) -> Result<BoxLengthSample, BoxLengthAnimationError> {
        if !basis.is_finite() || basis < 0.0 {
            return Err(MixedValueError::InvalidBasis.into());
        }
        if !coordinate_limit.is_finite() || coordinate_limit < 0.0 {
            return Err(MixedValueError::InvalidRange.into());
        }
        let (value, clamped, rounded_to_zero) = match &self.value {
            Value::Auto => (BoxLengthSampleValue::Auto, None, false),
            Value::None => (BoxLengthSampleValue::None, None, false),
            Value::Numeric(value) => {
                let minimum = if self.signed { -coordinate_limit } else { 0.0 };
                let sample = value.resolve(basis, minimum, coordinate_limit, limits)?;
                (
                    BoxLengthSampleValue::Pixels(sample.value),
                    sample.clamped,
                    sample.rounded_to_zero,
                )
            }
        };
        Ok(BoxLengthSample {
            property: self.property,
            value,
            clamped,
            rounded_to_zero,
        })
    }
}
/// Check identity and component compatibility before copying numeric operands.
/// A selected auto/none endpoint therefore performs no unnecessary numeric work.
pub fn combine_box_length(
    start: &ComputedBoxLength,
    end: &ComputedBoxLength,
    operation: Combination,
    limits: NumericLimits,
) -> Result<CombinedBoxLength, BoxLengthAnimationError> {
    if start.property() != end.property() {
        return Err(BoxLengthAnimationError::PropertyMismatch);
    }
    let selected = discrete_choice(operation).map_err(|_| MixedValueError::NonfiniteProgress)?;
    let numeric = |v| {
        matches!(
            v,
            BoxLengthComponent::Pixels(_) | BoxLengthComponent::Percentage(_)
        )
    };
    if numeric(start.component()) && numeric(end.component()) {
        let a = CombinedBoxLength::from_computed(start, limits)?;
        let b = CombinedBoxLength::from_computed(end, limits)?;
        let (Value::Numeric(a), Value::Numeric(b)) = (a.value, b.value) else {
            unreachable!("both components were classified numeric");
        };
        return Ok(CombinedBoxLength {
            property: start.property(),
            signed: start.allows_negative(),
            value: Value::Numeric(combine_length_percentage(&a, &b, operation, limits)?),
        });
    }
    CombinedBoxLength::from_computed(
        match selected {
            Endpoint::Start => start,
            Endpoint::End => end,
        },
        limits,
    )
}
