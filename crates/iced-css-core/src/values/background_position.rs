//! CSS2.2 computed background-position, preserving length/percentage tags.
use super::{
    Decimal, LengthTarget, NumericLimits,
    geometry::{LengthContext, LengthEvaluation, LengthEvaluationError, evaluate_length},
    property::{ParsedProperty, SpecifiedValue},
};
#[derive(Debug, PartialEq)]
enum Axis {
    Pixels(f32),
    Percentage(Decimal),
}
#[derive(Debug, PartialEq)]
pub struct ComputedBackgroundPosition {
    axes: [Axis; 2],
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionComponent<'a> {
    Pixels(f32),
    Percentage(&'a Decimal),
}
impl ComputedBackgroundPosition {
    pub fn components(&self) -> [PositionComponent<'_>; 2] {
        self.axes.each_ref().map(|a| match a {
            Axis::Pixels(v) => PositionComponent::Pixels(*v),
            Axis::Percentage(v) => PositionComponent::Percentage(v),
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundPositionError {
    Unresolved,
    InvalidValue,
    Length(LengthEvaluationError),
}
impl std::fmt::Display for BackgroundPositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "background-position computation: {self:?}")
    }
}
impl std::error::Error for BackgroundPositionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Length(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug, PartialEq)]
pub struct BackgroundPositionComputation {
    pub value: ComputedBackgroundPosition,
    pub rounded_to_zero: [bool; 2],
}
/// Defaulting is already resolved; parsed keywords have become percentages.
/// Limits apply separately to each length evaluation; percentages move unchanged.
pub fn compute_background_position(
    property: ParsedProperty,
    context: LengthContext,
    limits: NumericLimits,
) -> Result<BackgroundPositionComputation, BackgroundPositionError> {
    if property.property.descriptor().name != "background-position" {
        return Err(BackgroundPositionError::Unresolved);
    }
    let SpecifiedValue::LengthPair(a, b) = property.value else {
        return Err(BackgroundPositionError::Unresolved);
    };
    if matches!(a, LengthTarget::Auto) || matches!(b, LengthTarget::Auto) {
        return Err(BackgroundPositionError::InvalidValue);
    }
    let component = |value: LengthTarget| -> Result<(Axis, bool), BackgroundPositionError> {
        if let LengthTarget::Percentage(value) = value {
            return Ok((Axis::Percentage(value), false));
        }
        match evaluate_length(&value, context, limits).map_err(BackgroundPositionError::Length)? {
            LengthEvaluation::Value(v) => Ok((Axis::Pixels(v.value), v.rounded_to_zero)),
            LengthEvaluation::Auto => Err(BackgroundPositionError::InvalidValue),
        }
    };
    let (a, az) = component(a)?;
    let (b, bz) = component(b)?;
    Ok(BackgroundPositionComputation {
        value: ComputedBackgroundPosition { axes: [a, b] },
        rounded_to_zero: [az, bz],
    })
}
