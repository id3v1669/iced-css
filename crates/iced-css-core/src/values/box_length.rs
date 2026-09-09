//! Property-identified computed lengths for the explicit nineteen-property subset.
use super::{
    Decimal, LengthTarget, NumericLimits,
    geometry::{LengthContext, LengthEvaluation, LengthEvaluationError, evaluate_length},
    property::{ParsedProperty, SpecifiedValue},
    registry::PropertyId,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Policy {
    signed: bool,
    auto: bool,
    none: bool,
}
fn policy(property: PropertyId) -> Option<Policy> {
    let (signed, auto, none) = match property.descriptor().name {
        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" | "top" | "right"
        | "bottom" | "left" => (true, true, false),
        "text-indent" => (true, false, false),
        "width" | "height" => (false, true, false),
        "max-width" | "max-height" => (false, false, true),
        "min-width" | "min-height" | "padding-top" | "padding-right" | "padding-bottom"
        | "padding-left" => (false, false, false),
        _ => return None,
    };
    Some(Policy { signed, auto, none })
}
#[derive(Debug, PartialEq)]
enum Value {
    Pixels(f32),
    Percentage(Decimal),
    Auto,
    None,
}
#[derive(Debug, PartialEq)]
pub struct ComputedBoxLength {
    property: PropertyId,
    policy: Policy,
    value: Value,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxLengthComponent<'a> {
    Pixels(f32),
    Percentage(&'a Decimal),
    Auto,
    None,
}
impl ComputedBoxLength {
    pub fn property(&self) -> PropertyId {
        self.property
    }
    pub fn allows_negative(&self) -> bool {
        self.policy.signed
    }
    pub fn component(&self) -> BoxLengthComponent<'_> {
        match &self.value {
            Value::Pixels(v) => BoxLengthComponent::Pixels(*v),
            Value::Percentage(v) => BoxLengthComponent::Percentage(v),
            Value::Auto => BoxLengthComponent::Auto,
            Value::None => BoxLengthComponent::None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxLengthError {
    UnsupportedProperty,
    Unresolved,
    InvalidValue,
    Length(LengthEvaluationError),
}
impl std::fmt::Display for BoxLengthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "computed box length: {self:?}")
    }
}
impl std::error::Error for BoxLengthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Length(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug, PartialEq)]
pub struct BoxLengthComputation {
    pub value: ComputedBoxLength,
    pub rounded_to_zero: bool,
}
/// Compute lengths under the supplied font context and move percentages unchanged.
/// Auto/none are retained for the later property-specific used-value algorithm.
/// Authored negative values fail before conversion for nonnegative properties.
pub fn compute_box_length(
    property: ParsedProperty,
    context: LengthContext,
    limits: NumericLimits,
) -> Result<BoxLengthComputation, BoxLengthError> {
    let policy = policy(property.property).ok_or(BoxLengthError::UnsupportedProperty)?;
    let (value, rounded_to_zero) = match property.value {
        SpecifiedValue::Length(LengthTarget::Auto) => {
            if !policy.auto {
                return Err(BoxLengthError::InvalidValue);
            }
            (Value::Auto, false)
        }
        SpecifiedValue::Length(value) => {
            if !policy.signed && value.is_negative() {
                return Err(BoxLengthError::InvalidValue);
            }
            if let LengthTarget::Percentage(value) = value {
                (Value::Percentage(value), false)
            } else {
                match evaluate_length(&value, context, limits).map_err(BoxLengthError::Length)? {
                    LengthEvaluation::Value(v) => (Value::Pixels(v.value), v.rounded_to_zero),
                    LengthEvaluation::Auto => return Err(BoxLengthError::InvalidValue),
                }
            }
        }
        SpecifiedValue::Keyword(k) if k.as_str() == "none" && policy.none => (Value::None, false),
        SpecifiedValue::Inherit
        | SpecifiedValue::Initial
        | SpecifiedValue::InitialReference(_)
        | SpecifiedValue::SystemFont(_) => return Err(BoxLengthError::Unresolved),
        _ => return Err(BoxLengthError::InvalidValue),
    };
    Ok(BoxLengthComputation {
        value: ComputedBoxLength {
            property: property.property,
            policy,
            value,
        },
        rounded_to_zero,
    })
}
