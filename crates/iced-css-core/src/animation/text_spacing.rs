//! By-computed-value letter/word spacing pair combination.
use super::{
    combine::{Combination, Endpoint, discrete_choice},
    number::{Clamp, CombinedNumber, NumberError, combine_number},
};
use crate::values::text_spacing::{ComputedTextSpacing, TextSpacingProperty, TextSpacingValue};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSpacingAnimationError {
    PropertyMismatch,
    Number(NumberError),
}
impl std::fmt::Display for TextSpacingAnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "text spacing animation: {self:?}")
    }
}
impl std::error::Error for TextSpacingAnimationError {}
#[derive(Debug, Clone, Copy)]
enum Pair {
    Normal,
    Length(CombinedNumber),
}
#[derive(Debug, Clone, Copy)]
pub struct CombinedTextSpacing {
    property: TextSpacingProperty,
    value: Pair,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSpacingSample {
    pub property: TextSpacingProperty,
    pub value: TextSpacingValue,
    pub clamped: Option<Clamp>,
}
impl CombinedTextSpacing {
    /// Explicit final working bounds apply to length results. Normal is retained
    /// as a keyword; its justification behavior is not replaced by numeric zero.
    pub fn resolve(self, minimum: f32, maximum: f32) -> Result<TextSpacingSample, NumberError> {
        if !minimum.is_finite() || !maximum.is_finite() || minimum > maximum {
            return Err(NumberError::InvalidRange);
        }
        let (value, clamped) = match self.value {
            Pair::Normal => (TextSpacingValue::Normal, None),
            Pair::Length(value) => {
                let result = value.resolve(minimum, maximum)?;
                (TextSpacingValue::Pixels(result.value), result.clamped)
            }
        };
        Ok(TextSpacingSample {
            property: self.property,
            value,
            clamped,
        })
    }
}
/// Numeric pairs combine exactly; incompatible normal/length pairs use the
/// whole-value discrete rule. The typed word-spacing endpoint normalizes to0
/// before this function, as required by its computed-value definition.
pub fn combine_text_spacing(
    start: ComputedTextSpacing,
    end: ComputedTextSpacing,
    operation: Combination,
) -> Result<CombinedTextSpacing, TextSpacingAnimationError> {
    if start.property() != end.property() {
        return Err(TextSpacingAnimationError::PropertyMismatch);
    }
    if let Combination::Interpolate(p) = operation
        && !p.is_finite()
    {
        return Err(TextSpacingAnimationError::Number(
            NumberError::NonfiniteProgress,
        ));
    }
    let value = match (start.value(), end.value()) {
        (TextSpacingValue::Pixels(a), TextSpacingValue::Pixels(b)) => Pair::Length(
            combine_number(a, b, operation).map_err(TextSpacingAnimationError::Number)?,
        ),
        _ => {
            let choice = discrete_choice(operation).expect("progress was validated");
            match match choice {
                Endpoint::Start => start.value(),
                Endpoint::End => end.value(),
            } {
                TextSpacingValue::Normal => Pair::Normal,
                TextSpacingValue::Pixels(value) => Pair::Length(
                    combine_number(value, 0.0, Combination::Add).expect("typed endpoint is finite"),
                ),
            }
        }
    };
    Ok(CombinedTextSpacing {
        property: start.property(),
        value,
    })
}
