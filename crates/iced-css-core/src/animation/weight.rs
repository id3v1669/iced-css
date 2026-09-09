//! Final legacy CSS2.2 font-weight pair combination.
use super::{
    combine::Combination,
    number::{NumberError, combine_number},
};
use crate::values::font_weight::FontWeight;
/// Combine already computed weights, then round to hundred steps (ties upward)
/// and apply the final100..900 range. Does not resolve relative keywords, choose
/// native faces, or implement a sequence of multiple property effects.
///
/// # Errors
/// Rejects nonfinite interpolation progress.
pub fn combine_font_weight(
    start: FontWeight,
    end: FontWeight,
    operation: Combination,
) -> Result<FontWeight, NumberError> {
    // Every allowed hundred-step endpoint is exactly representable in f32.
    Ok(combine_number(
        f32::from(start.number()),
        f32::from(end.number()),
        operation,
    )?
    .legacy_weight())
}
