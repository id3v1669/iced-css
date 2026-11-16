use super::{ConversionError, NonnegativeF32, NumericLimits, property::SpecifiedValue};
#[derive(Debug, Clone, Copy)]
pub struct BorderFontContext {
    pub size: f32,
    pub x_height: f32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum BorderWidthError {
    UnresolvedWidth,
    UnresolvedStyle,
    Conversion(ConversionError),
}
pub fn compute_border_width(
    width: &SpecifiedValue,
    style: &SpecifiedValue,
    context: BorderFontContext,
    limits: NumericLimits,
) -> Result<NonnegativeF32, BorderWidthError> {
    use super::{LengthTarget, RelativeUnit};
    let constant = match width {
        SpecifiedValue::Keyword(keyword) => Some(match keyword.as_str() {
            "thin" => 1.0,
            "medium" => 3.0,
            "thick" => 5.0,
            _ => return Err(BorderWidthError::UnresolvedWidth),
        }),
        SpecifiedValue::Length(
            length @ (LengthTarget::Absolute(_) | LengthTarget::Relative(_, _)),
        ) => {
            if length.is_negative() {
                return Err(BorderWidthError::Conversion(
                    ConversionError::InvalidNegative,
                ));
            }
            None
        }
        _ => return Err(BorderWidthError::UnresolvedWidth),
    };
    let SpecifiedValue::Keyword(style) = style else {
        return Err(BorderWidthError::UnresolvedStyle);
    };
    match style.as_str() {
        "none" | "hidden" => {
            return Ok(NonnegativeF32 {
                value: 0.0,
                event: None,
            });
        }
        "dotted" | "dashed" | "solid" | "double" | "groove" | "ridge" | "inset" | "outset" => {}
        _ => return Err(BorderWidthError::UnresolvedStyle),
    }
    if let Some(value) = constant {
        return Ok(NonnegativeF32 { value, event: None });
    }
    let SpecifiedValue::Length(length) = width else {
        unreachable!("validated width");
    };
    match length {
        LengthTarget::Absolute(value) => value.to_nonnegative_f32(limits),
        LengthTarget::Relative(value, RelativeUnit::Em) => {
            value.scaled_nonnegative_f32(context.size, limits)
        }
        LengthTarget::Relative(value, RelativeUnit::Ex) => {
            value.scaled_nonnegative_f32(context.x_height, limits)
        }
        LengthTarget::Relative(value, RelativeUnit::Ch) => {
            value.scaled_nonnegative_f32(context.size * 0.5, limits)
        }
        // This context holds no root font size: the caller resolves `rem` first.
        LengthTarget::Relative(_, RelativeUnit::Rem) => {
            return Err(BorderWidthError::UnresolvedWidth);
        }
        _ => unreachable!("validated border length"),
    }
    .map_err(BorderWidthError::Conversion)
}

impl std::fmt::Display for BorderWidthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnresolvedWidth => f.write_str("border width requires a resolved width value"),
            Self::UnresolvedStyle => f.write_str("border width requires a resolved border style"),
            Self::Conversion(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for BorderWidthError {}
