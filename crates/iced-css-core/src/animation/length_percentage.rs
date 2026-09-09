//! Exact arithmetic components for mixed lengths and percentages.
//! This is not a property's canonical identity, eligibility or dependency record.
use super::{combine::Combination, number::Clamp};
use crate::values::{
    ConversionError, Decimal, DecimalArithmeticError, DecimalScalingError, NumericError,
    NumericLimits,
};
#[derive(Debug)]
pub struct LengthPercentage {
    pixels: Decimal,
    percentage: Decimal,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixedValueError {
    InvalidPixels,
    InvalidBasis,
    InvalidRange,
    NonfiniteProgress,
    Numeric(NumericError),
    Conversion(ConversionError),
}
impl std::fmt::Display for MixedValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mixed length/percentage: {self:?}")
    }
}
impl std::error::Error for MixedValueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Numeric(e) => Some(e),
            Self::Conversion(e) => Some(e),
            _ => None,
        }
    }
}
impl From<NumericError> for MixedValueError {
    fn from(e: NumericError) -> Self {
        Self::Numeric(e)
    }
}
impl From<DecimalArithmeticError> for MixedValueError {
    fn from(e: DecimalArithmeticError) -> Self {
        match e {
            DecimalArithmeticError::NonfiniteProgress => Self::NonfiniteProgress,
            DecimalArithmeticError::Numeric(e) => Self::Numeric(e),
        }
    }
}
fn scaling(e: DecimalScalingError) -> MixedValueError {
    match e {
        DecimalScalingError::NonfiniteValue => MixedValueError::InvalidBasis,
        DecimalScalingError::Numeric(e) => MixedValueError::Numeric(e),
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixedValueSample {
    pub value: f32,
    pub clamped: Option<Clamp>,
    pub rounded_to_zero: bool,
}
impl LengthPercentage {
    /// A previously computed finite absolute pixel value, with no percentage term.
    pub fn from_pixels(value: f32, limits: NumericLimits) -> Result<Self, MixedValueError> {
        if !value.is_finite() {
            return Err(MixedValueError::InvalidPixels);
        }
        Ok(Self {
            pixels: Decimal::from_finite_f32(value, limits).map_err(scaling)?,
            percentage: Decimal::zero(),
        })
    }
    /// Move an admitted percentage coefficient:100 represents100%, not1%.
    /// Property sign restrictions and source/dependency identity remain external.
    pub fn from_percentage(value: Decimal) -> Self {
        Self {
            pixels: Decimal::zero(),
            percentage: value,
        }
    }
    pub fn components(&self) -> (&Decimal, &Decimal) {
        (&self.pixels, &self.percentage)
    }
    /// Bind the final sum to the caller's finite signed basis, clamp against exact
    /// bounds, then round once. Limits apply separately to at most seven numeric
    /// calls, not to an unbounded chain of user operations.
    pub fn resolve(
        &self,
        basis: f32,
        minimum: f32,
        maximum: f32,
        limits: NumericLimits,
    ) -> Result<MixedValueSample, MixedValueError> {
        if !basis.is_finite() {
            return Err(MixedValueError::InvalidBasis);
        }
        if !minimum.is_finite() || !maximum.is_finite() || minimum > maximum {
            return Err(MixedValueError::InvalidRange);
        }
        let percentage = self
            .percentage
            .checked_percentage_of_f32(basis, limits)
            .map_err(scaling)?;
        let value = self.pixels.checked_add(&percentage, limits)?;
        resolve_exact(value, minimum, maximum, limits)
    }
    /// Resolve against first-second without rounding that basis difference.
    /// At most nine separately bounded numeric calls are needed. The caller
    /// selects the meaning and admissible signs of the two finite terms.
    pub fn resolve_difference_basis(
        &self,
        first: f32,
        second: f32,
        minimum: f32,
        maximum: f32,
        limits: NumericLimits,
    ) -> Result<MixedValueSample, MixedValueError> {
        if !first.is_finite() || !second.is_finite() {
            return Err(MixedValueError::InvalidBasis);
        }
        if !minimum.is_finite() || !maximum.is_finite() || minimum > maximum {
            return Err(MixedValueError::InvalidRange);
        }
        let first = self
            .percentage
            .checked_percentage_of_f32(first, limits)
            .map_err(scaling)?;
        let second = self
            .percentage
            .checked_percentage_of_f32(-second, limits)
            .map_err(scaling)?;
        let value = self
            .pixels
            .checked_add(&first, limits)?
            .checked_add(&second, limits)?;
        resolve_exact(value, minimum, maximum, limits)
    }
}
fn resolve_exact(
    value: Decimal,
    minimum: f32,
    maximum: f32,
    limits: NumericLimits,
) -> Result<MixedValueSample, MixedValueError> {
    let lower = Decimal::from_finite_f32(minimum, limits).map_err(scaling)?;
    let upper = Decimal::from_finite_f32(maximum, limits).map_err(scaling)?;
    if value.checked_cmp(&lower, limits)?.is_lt() {
        return Ok(MixedValueSample {
            value: if minimum == 0.0 { 0.0 } else { minimum },
            clamped: Some(Clamp::Lower),
            rounded_to_zero: false,
        });
    }
    if value.checked_cmp(&upper, limits)?.is_gt() {
        return Ok(MixedValueSample {
            value: if maximum == 0.0 { 0.0 } else { maximum },
            clamped: Some(Clamp::Upper),
            rounded_to_zero: false,
        });
    }
    let sample = value
        .scaled_finite_f32(1.0, limits)
        .map_err(MixedValueError::Conversion)?;
    Ok(MixedValueSample {
        value: sample.value,
        clamped: None,
        rounded_to_zero: sample.rounded_to_zero,
    })
}
/// Combine corresponding exact components without clamping or f32 conversion.
/// This helper assumes the owning property permits length/percentage mixing;
/// auto/none/normal and canonical source/dependency identity are not represented.
pub fn combine_length_percentage(
    start: &LengthPercentage,
    end: &LengthPercentage,
    operation: Combination,
    limits: NumericLimits,
) -> Result<LengthPercentage, MixedValueError> {
    if let Combination::Interpolate(p) = operation
        && !p.is_finite()
    {
        return Err(MixedValueError::NonfiniteProgress);
    }
    let combine = |a: &Decimal, b: &Decimal| -> Result<Decimal, MixedValueError> {
        match operation {
            Combination::Interpolate(p) => a.checked_interpolate(b, p, limits).map_err(Into::into),
            Combination::Add | Combination::Accumulate => {
                a.checked_add(b, limits).map_err(Into::into)
            }
        }
    };
    Ok(LengthPercentage {
        pixels: combine(&start.pixels, &end.pixels)?,
        percentage: combine(&start.percentage, &end.percentage)?,
    })
}
