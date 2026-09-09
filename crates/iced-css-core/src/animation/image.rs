//! Discrete absolute-URI/none combination with image-property identity.
use super::combine::{Combination, Endpoint, discrete_choice};
use crate::values::image::ComputedImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageAnimationError {
    PropertyMismatch,
    NonfiniteProgress,
}
impl std::fmt::Display for ImageAnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "image URI animation: {self:?}")
    }
}
impl std::error::Error for ImageAnimationError {}
/// Borrow the entire selected endpoint. No resolution, retrieval or allocation
/// occurs during sampling. This operates on CSS2.2 URI/none computed values.
///
/// Endpoint ownership is retained by the caller:
/// ```compile_fail
/// use iced_css_core::{animation::{image::combine_image, combine::Combination},
///     values::image::ComputedImage};
/// fn escape(start: ComputedImage, end: ComputedImage) -> &'static ComputedImage {
///     combine_image(&start, &end, Combination::Add).unwrap()
/// }
/// ```
pub fn combine_image<'a>(
    start: &'a ComputedImage,
    end: &'a ComputedImage,
    operation: Combination,
) -> Result<&'a ComputedImage, ImageAnimationError> {
    if start.property() != end.property() {
        return Err(ImageAnimationError::PropertyMismatch);
    }
    match discrete_choice(operation).map_err(|_| ImageAnimationError::NonfiniteProgress)? {
        Endpoint::Start => Ok(start),
        Endpoint::End => Ok(end),
    }
}
