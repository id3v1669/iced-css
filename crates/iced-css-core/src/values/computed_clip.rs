//! Computed clip values; auto sides are not used border-box coordinates yet.
use super::{
    LengthTarget, NumericLimits,
    geometry::{LengthContext, LengthEvaluation, LengthEvaluationError, evaluate_length},
    property::{ParsedProperty, SpecifiedValue},
};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipSides([Option<f32>; 4]);
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComputedClip {
    Auto,
    Rectangle(ClipSides),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputedClipError {
    Unresolved,
    Percentage,
    NonfiniteCoordinate,
    Length(LengthEvaluationError),
}
impl std::fmt::Display for ComputedClipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "computed clip: {self:?}")
    }
}
impl std::error::Error for ComputedClipError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Length(error) => Some(error),
            _ => None,
        }
    }
}
impl ComputedClip {
    /// Top/right/bottom/left, with None representing a side's auto keyword.
    pub fn rectangle(mut sides: [Option<f32>; 4]) -> Result<Self, ComputedClipError> {
        for value in sides.iter_mut().flatten() {
            if !value.is_finite() {
                return Err(ComputedClipError::NonfiniteCoordinate);
            }
            if *value == 0.0 {
                *value = 0.0;
            }
        }
        Ok(Self::Rectangle(ClipSides(sides)))
    }
    /// None is whole auto; Some(`[None; 4]`) is a rectangle with four auto sides.
    pub fn sides(self) -> Option<[Option<f32>; 4]> {
        match self {
            Self::Auto => None,
            Self::Rectangle(sides) => Some(sides.0),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipComputation {
    pub value: ComputedClip,
    pub rounded_to_zero: [bool; 4],
}
/// Compute resolved authored values. Numeric limits apply separately to each
/// of the at most four lengths; percentages fail before length evaluation.
pub fn compute_clip(
    property: &ParsedProperty,
    context: LengthContext,
    per_component_limits: NumericLimits,
) -> Result<ClipComputation, ComputedClipError> {
    if property.property.descriptor().name != "clip" {
        return Err(ComputedClipError::Unresolved);
    }
    let operands = match &property.value {
        SpecifiedValue::Keyword(k) if k.as_str() == "auto" => {
            return Ok(ClipComputation {
                value: ComputedClip::Auto,
                rounded_to_zero: [false; 4],
            });
        }
        SpecifiedValue::ClipRect(sides) => sides,
        _ => return Err(ComputedClipError::Unresolved),
    };
    if operands
        .iter()
        .any(|v| matches!(v, LengthTarget::Percentage(_)))
    {
        return Err(ComputedClipError::Percentage);
    }
    let mut sides = [None; 4];
    let mut rounded_to_zero = [false; 4];
    for (i, operand) in operands.iter().enumerate() {
        if let LengthEvaluation::Value(value) =
            evaluate_length(operand, context, per_component_limits)
                .map_err(ComputedClipError::Length)?
        {
            sides[i] = Some(value.value);
            rounded_to_zero[i] = value.rounded_to_zero;
        }
    }
    Ok(ClipComputation {
        value: ComputedClip::rectangle(sides)?,
        rounded_to_zero,
    })
}
