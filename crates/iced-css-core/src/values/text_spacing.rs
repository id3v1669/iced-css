//! CSS2.2 letter/word spacing retain different computed meanings of normal.
use super::{
    LengthTarget, NumericLimits,
    geometry::{LengthContext, LengthEvaluation, LengthEvaluationError, evaluate_length},
    property::{ParsedProperty, SpecifiedValue},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSpacingProperty {
    Letter,
    Word,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextSpacingValue {
    Normal,
    Pixels(f32),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComputedTextSpacing {
    property: TextSpacingProperty,
    value: TextSpacingValue,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSpacingError {
    Unresolved,
    InvalidValue,
    InvalidComputed,
    Length(LengthEvaluationError),
}
impl std::fmt::Display for TextSpacingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "text spacing computation: {self:?}")
    }
}
impl std::error::Error for TextSpacingError {}
impl ComputedTextSpacing {
    pub fn from_pixels(
        property: TextSpacingProperty,
        value: f32,
    ) -> Result<Self, TextSpacingError> {
        if !value.is_finite() {
            return Err(TextSpacingError::InvalidComputed);
        }
        Ok(Self {
            property,
            value: TextSpacingValue::Pixels(if value == 0.0 { 0.0 } else { value }),
        })
    }
    pub fn normal(property: TextSpacingProperty) -> Self {
        Self {
            property,
            value: match property {
                TextSpacingProperty::Letter => TextSpacingValue::Normal,
                TextSpacingProperty::Word => TextSpacingValue::Pixels(0.0),
            },
        }
    }
    pub fn property(self) -> TextSpacingProperty {
        self.property
    }
    pub fn value(self) -> TextSpacingValue {
        self.value
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSpacingComputation {
    pub value: ComputedTextSpacing,
    pub rounded_to_zero: bool,
}
/// Compute resolved specified values after cascade/defaulting. Signed lengths
/// remain signed; percentages are not accepted even with a supplied basis.
pub fn compute_text_spacing(
    property: &ParsedProperty,
    context: LengthContext,
    limits: NumericLimits,
) -> Result<TextSpacingComputation, TextSpacingError> {
    let kind = match property.property.descriptor().name {
        "letter-spacing" => TextSpacingProperty::Letter,
        "word-spacing" => TextSpacingProperty::Word,
        _ => return Err(TextSpacingError::Unresolved),
    };
    let length = match &property.value {
        SpecifiedValue::Keyword(keyword) if keyword.as_str() == "normal" => {
            return Ok(TextSpacingComputation {
                value: ComputedTextSpacing::normal(kind),
                rounded_to_zero: false,
            });
        }
        SpecifiedValue::Length(
            value @ (LengthTarget::Absolute(_) | LengthTarget::Relative(..)),
        ) => value,
        SpecifiedValue::Length(_) => return Err(TextSpacingError::InvalidValue),
        _ => return Err(TextSpacingError::Unresolved),
    };
    let LengthEvaluation::Value(value) =
        evaluate_length(length, context, limits).map_err(TextSpacingError::Length)?
    else {
        return Err(TextSpacingError::InvalidValue);
    };
    Ok(TextSpacingComputation {
        value: ComputedTextSpacing::from_pixels(kind, value.value)?,
        rounded_to_zero: value.rounded_to_zero,
    })
}
