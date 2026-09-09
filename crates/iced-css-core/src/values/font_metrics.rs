//! Exact signed design-unit scaling with one final binary32 rounding.
use super::{FiniteF32, float_ratio::Ratio};
use std::num::NonZeroU16;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontMetricScaleError {
    FontSize,
    UnitsPerEm,
    Unrepresentable,
}
impl std::fmt::Display for FontMetricScaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "font metric scaling: {self:?}")
    }
}
impl std::error::Error for FontMetricScaleError {}
/// Scale any signed i32 design-unit value without first rounding it to f32.
/// No allocation; final nearest/ties-even rounding takes at most31 search steps.
///
/// # Errors
/// Requires finite nonnegative font size and OpenType head units_per_em16..16384.
/// Rejects unrepresentable final output, preserving signed finite geometry.
pub fn scale_font_units(
    units: i32,
    units_per_em: u16,
    font_size: f32,
) -> Result<FiniteF32, FontMetricScaleError> {
    if !font_size.is_finite() || font_size < 0.0 {
        return Err(FontMetricScaleError::FontSize);
    }
    if !(16..=16384).contains(&units_per_em) {
        return Err(FontMetricScaleError::UnitsPerEm);
    }
    let divisor = NonZeroU16::new(units_per_em).ok_or(FontMetricScaleError::UnitsPerEm)?;
    let value = Ratio::scaled_integer(font_size, units.unsigned_abs(), divisor)
        .round()
        .map_err(|_| FontMetricScaleError::Unrepresentable)?;
    let rounded_to_zero = value == 0.0 && units != 0 && font_size != 0.0;
    let value = if units < 0 && value != 0.0 {
        -value
    } else {
        value
    };
    Ok(FiniteF32 {
        value,
        rounded_to_zero,
    })
}
