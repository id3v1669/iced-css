//! Animation-only internal angles for CSS2.2 keyword font-style endpoints.
use super::{
    combine::{Combination, CombinationError, Endpoint, discrete_choice},
    number::{Clamp, CombinedNumber, combine_number},
};
use crate::values::property::{ParsedProperty, SpecifiedValue};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyleKeyword {
    Normal,
    Italic,
    Oblique,
}
impl TryFrom<&ParsedProperty> for FontStyleKeyword {
    type Error = CombinationError;
    fn try_from(property: &ParsedProperty) -> Result<Self, Self::Error> {
        if property.property.descriptor().name != "font-style" {
            return Err(CombinationError::UnresolvedFontStyle);
        }
        let SpecifiedValue::Keyword(value) = &property.value else {
            return Err(CombinationError::UnresolvedFontStyle);
        };
        match value.as_str() {
            "normal" => Ok(Self::Normal),
            "italic" => Ok(Self::Italic),
            "oblique" => Ok(Self::Oblique),
            _ => Err(CombinationError::UnresolvedFontStyle),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontStyleValue {
    Italic,
    ObliqueDegrees(f32),
}
/// A computed keyword or captured final presentation value. This is not authored
/// CSS angle syntax and must not replace an unbounded composition intermediate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontStyleEndpoint(FontStyleValue);
impl From<FontStyleKeyword> for FontStyleEndpoint {
    fn from(value: FontStyleKeyword) -> Self {
        Self(match value {
            FontStyleKeyword::Normal => FontStyleValue::ObliqueDegrees(0.0),
            FontStyleKeyword::Oblique => FontStyleValue::ObliqueDegrees(14.0),
            FontStyleKeyword::Italic => FontStyleValue::Italic,
        })
    }
}
impl TryFrom<FontStyleValue> for FontStyleEndpoint {
    type Error = CombinationError;
    fn try_from(value: FontStyleValue) -> Result<Self, Self::Error> {
        match value {
            FontStyleValue::Italic => Ok(Self(value)),
            FontStyleValue::ObliqueDegrees(angle) => {
                if !angle.is_finite() || !(-90.0..=90.0).contains(&angle) {
                    return Err(CombinationError::InvalidFontStyleAngle);
                }
                Ok(Self(FontStyleValue::ObliqueDegrees(if angle == 0.0 {
                    0.0
                } else {
                    angle
                })))
            }
        }
    }
}
impl FontStyleEndpoint {
    fn degrees(self) -> Option<f32> {
        match self.0 {
            FontStyleValue::Italic => None,
            FontStyleValue::ObliqueDegrees(angle) => Some(angle),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontStyleSample {
    pub value: FontStyleValue,
    pub clamped: Option<Clamp>,
}
#[derive(Debug, Clone, Copy)]
enum StyleResult {
    Italic,
    Angle(CombinedNumber),
}
/// Pair result before final range handling. It is not an iced native Font Style.
#[derive(Debug, Clone, Copy)]
pub struct CombinedFontStyle(StyleResult);
impl CombinedFontStyle {
    /// Resolve the final angle against the selected Fonts4 -90..90 degree range.
    /// This does not select a font face or synthesize/render its slant.
    pub fn resolve(self) -> FontStyleSample {
        match self.0 {
            StyleResult::Italic => FontStyleSample {
                value: FontStyleValue::Italic,
                clamped: None,
            },
            StyleResult::Angle(value) => {
                let sample = value
                    .resolve(-90.0, 90.0)
                    .expect("fixed finite ordered angle bounds");
                FontStyleSample {
                    value: FontStyleValue::ObliqueDegrees(sample.value),
                    clamped: sample.clamped,
                }
            }
        }
    }
}
/// Combine already computed CSS2.2 keywords using internal angle semantics.
/// Normal/oblique normalize to0/14 degrees; incompatible italic/angle pairs use
/// the discrete rule, including its non-additive first-operand fallback.
///
/// # Errors
/// Rejects nonfinite interpolation progress, including identical italic endpoints.
pub fn combine_font_style(
    start: FontStyleKeyword,
    end: FontStyleKeyword,
    operation: Combination,
) -> Result<CombinedFontStyle, CombinationError> {
    combine_font_style_endpoints(start.into(), end.into(), operation)
}
/// Combine validated final presentation endpoints without snapping captured
/// angles to CSS2.2 keywords. Final range handling remains in `resolve`.
pub fn combine_font_style_endpoints(
    start: FontStyleEndpoint,
    end: FontStyleEndpoint,
    operation: Combination,
) -> Result<CombinedFontStyle, CombinationError> {
    let choice = discrete_choice(operation)?;
    let result = match (start.degrees(), end.degrees()) {
        (Some(a), Some(b)) => StyleResult::Angle(
            combine_number(a, b, operation).expect("validated finite angles and progress"),
        ),
        _ => {
            let endpoint = match choice {
                Endpoint::Start => start,
                Endpoint::End => end,
            };
            match endpoint.degrees() {
                None => StyleResult::Italic,
                Some(angle) => StyleResult::Angle(
                    combine_number(angle, 0.0, Combination::Add).expect("validated finite angle"),
                ),
            }
        }
    };
    Ok(CombinedFontStyle(result))
}
