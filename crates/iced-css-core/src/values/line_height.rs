//! Property-defined computed and used stages, independent of line-box layout.
use super::{
    ConversionError, Decimal, LengthTarget, NonnegativeF32, NumericLimits, RangeEvent,
    RelativeUnit, property::SpecifiedValue,
};
#[derive(Debug, Clone, Copy)]
pub struct LineHeightFont {
    pub size: f32,
    pub x_height: f32,
}
#[derive(Debug)]
enum Kind {
    Normal,
    Number(Decimal),
    Absolute(f32),
}
/// Private representation keeps the computed normal/number/length distinction.
/// Numbers retain their already bounded exact storage by move, without a clone.
#[derive(Debug)]
pub struct ComputedLineHeight(Kind);
#[derive(Debug)]
pub struct LineHeightResult {
    pub value: ComputedLineHeight,
    pub event: Option<RangeEvent>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum LineHeightError {
    UnresolvedValue,
    Conversion(ConversionError),
}
impl From<ConversionError> for LineHeightError {
    fn from(error: ConversionError) -> Self {
        Self::Conversion(error)
    }
}
impl std::fmt::Display for LineHeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnresolvedValue => {
                f.write_str("line-height requires a resolved normal, number or length value")
            }
            Self::Conversion(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for LineHeightError {}

pub fn compute_line_height(
    value: SpecifiedValue,
    font: LineHeightFont,
    limits: NumericLimits,
) -> Result<LineHeightResult, LineHeightError> {
    let converted = match value {
        SpecifiedValue::Keyword(keyword) if keyword.as_str() == "normal" => {
            return Ok(LineHeightResult {
                value: ComputedLineHeight(Kind::Normal),
                event: None,
            });
        }
        SpecifiedValue::Number(number) => {
            if number.is_negative() {
                return Err(ConversionError::InvalidNegative.into());
            }
            return Ok(LineHeightResult {
                value: ComputedLineHeight(Kind::Number(number)),
                event: None,
            });
        }
        SpecifiedValue::Length(LengthTarget::Absolute(length)) => length.to_nonnegative_f32(limits),
        SpecifiedValue::Length(LengthTarget::Relative(number, RelativeUnit::Em)) => {
            number.scaled_nonnegative_f32(font.size, limits)
        }
        SpecifiedValue::Length(LengthTarget::Relative(number, RelativeUnit::Ex)) => {
            number.scaled_nonnegative_f32(font.x_height, limits)
        }
        SpecifiedValue::Length(LengthTarget::Relative(number, RelativeUnit::Ch)) => {
            number.scaled_nonnegative_f32(font.size * 0.5, limits)
        }
        SpecifiedValue::Length(LengthTarget::Percentage(number)) => {
            number.percentage_nonnegative_f32(font.size, limits)
        }
        _ => return Err(LineHeightError::UnresolvedValue),
    }?;
    Ok(LineHeightResult {
        value: ComputedLineHeight(Kind::Absolute(converted.value)),
        event: converted.event,
    })
}

/// Resolve against the using element. `normal_height` must come from the font
/// owner's selected metrics policy; it is only inspected for computed normal.
/// Absolute lengths already reported their computed conversion event and do not
/// repeat it here. Numeric products use the existing exact-to-f32 boundary.
pub fn used_line_height(
    computed: &ComputedLineHeight,
    font_size: f32,
    normal_height: f32,
    limits: NumericLimits,
) -> Result<NonnegativeF32, LineHeightError> {
    match &computed.0 {
        Kind::Number(number) => number
            .scaled_nonnegative_f32(font_size, limits)
            .map_err(Into::into),
        Kind::Absolute(value) => Ok(NonnegativeF32 {
            value: *value,
            event: None,
        }),
        Kind::Normal => {
            if !normal_height.is_finite() || normal_height < 0.0 {
                return Err(ConversionError::InvalidContext.into());
            }
            Ok(NonnegativeF32 {
                value: if normal_height == 0.0 {
                    0.0
                } else {
                    normal_height
                },
                event: None,
            })
        }
    }
}

pub(crate) enum LineHeightKindRef<'a> {
    Normal,
    Number(&'a Decimal),
    Absolute(f32),
}
impl ComputedLineHeight {
    pub(crate) fn animation_kind(&self) -> LineHeightKindRef<'_> {
        match &self.0 {
            Kind::Normal => LineHeightKindRef::Normal,
            Kind::Number(value) => LineHeightKindRef::Number(value),
            Kind::Absolute(value) => LineHeightKindRef::Absolute(*value),
        }
    }
    pub(crate) fn from_animation_number(value: Decimal) -> Result<Self, LineHeightError> {
        if value.is_negative() {
            return Err(ConversionError::InvalidNegative.into());
        }
        Ok(Self(Kind::Number(value)))
    }
    pub(crate) fn from_animation_absolute(value: f32) -> Result<Self, LineHeightError> {
        if !value.is_finite() || value < 0.0 {
            return Err(ConversionError::InvalidContext.into());
        }
        Ok(Self(Kind::Absolute(if value == 0.0 { 0.0 } else { value })))
    }
}
