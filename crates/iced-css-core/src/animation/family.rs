//! Whole-list discrete combination for already admitted CSS2.2 font families.
use super::combine::{Combination, CombinationError, Endpoint, discrete_choice};
use crate::values::property::FontFamily;

/// Select an entire endpoint without allocating, merging entries or matching fonts.
/// The caller owns property admission, inheritance/defaulting and list validity.
/// Quoted names and generic families keep their existing representations.
///
/// The selected slice cannot outlive its endpoint storage:
/// ```compile_fail
/// use iced_css_core::{animation::{family::combine_font_family, combine::Combination},
///     values::property::FontFamily};
/// let selected;
/// {
///     let start = vec![FontFamily::Quoted("Example".into())];
///     let end = vec![FontFamily::Quoted("Other".into())];
///     selected = combine_font_family(&start, &end, Combination::Add).unwrap();
/// }
/// println!("{:?}", selected);
/// ```
pub fn combine_font_family<'a>(
    start: &'a [FontFamily],
    end: &'a [FontFamily],
    operation: Combination,
) -> Result<&'a [FontFamily], CombinationError> {
    match discrete_choice(operation)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
