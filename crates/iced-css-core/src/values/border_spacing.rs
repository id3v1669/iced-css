//! CSS2.2 border-spacing computes to horizontal and vertical absolute lengths.
use super::{
    LengthTarget, NumericLimits,
    geometry::{LengthContext, LengthEvaluation, LengthEvaluationError, evaluate_length},
    property::{ParsedProperty, SpecifiedValue},
};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderSpacing {
    horizontal: f32,
    vertical: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderSpacingError {
    Unresolved,
    InvalidValue,
    InvalidComputed,
    Length(LengthEvaluationError),
}
impl std::fmt::Display for BorderSpacingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "border-spacing computation: {self:?}")
    }
}
impl std::error::Error for BorderSpacingError {}
impl BorderSpacing {
    /// Admit already computed pixel values, not authored numeric syntax.
    pub fn from_pixels(horizontal: f32, vertical: f32) -> Result<Self, BorderSpacingError> {
        if [horizontal, vertical]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(BorderSpacingError::InvalidComputed);
        }
        Ok(Self {
            horizontal: if horizontal == 0.0 { 0.0 } else { horizontal },
            vertical: if vertical == 0.0 { 0.0 } else { vertical },
        })
    }
    pub fn horizontal(self) -> f32 {
        self.horizontal
    }
    pub fn vertical(self) -> f32 {
        self.vertical
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderSpacingComputation {
    pub value: BorderSpacing,
    pub rounded_to_zero: [bool; 2],
}
/// Compute after defaulting/inheritance resolution. Limits apply independently
/// to each of the exactly two length evaluations. Missing font bases and numeric
/// conversion failures remain explicit; percentages and negative values fail
/// before representation loss can hide them.
pub fn compute_border_spacing(
    property: &ParsedProperty,
    context: LengthContext,
    per_component_limits: NumericLimits,
) -> Result<BorderSpacingComputation, BorderSpacingError> {
    if property.property.descriptor().name != "border-spacing" {
        return Err(BorderSpacingError::Unresolved);
    }
    let SpecifiedValue::LengthPair(horizontal, vertical) = &property.value else {
        return Err(BorderSpacingError::Unresolved);
    };
    for value in [horizontal, vertical] {
        if !matches!(
            value,
            LengthTarget::Absolute(_) | LengthTarget::Relative(..)
        ) || value.is_negative()
        {
            return Err(BorderSpacingError::InvalidValue);
        }
    }
    let component = |value| -> Result<_, BorderSpacingError> {
        match evaluate_length(value, context, per_component_limits)
            .map_err(BorderSpacingError::Length)?
        {
            LengthEvaluation::Value(v) => Ok(v),
            LengthEvaluation::Auto => Err(BorderSpacingError::InvalidValue),
        }
    };
    let h = component(horizontal)?;
    let v = component(vertical)?;
    Ok(BorderSpacingComputation {
        value: BorderSpacing::from_pixels(h.value, v.value)?,
        rounded_to_zero: [h.rounded_to_zero, v.rounded_to_zero],
    })
}
