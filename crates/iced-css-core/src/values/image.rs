//! CSS2.2 image properties compute to an absolute URI or none.
use super::{
    property::{ParsedProperty, SpecifiedValue},
    registry::PropertyId,
};
use crate::resources::uri::{self, UriError, UriLimits};

#[derive(Debug)]
pub struct ComputedImage {
    property: PropertyId,
    uri: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageComponent<'a> {
    None,
    Uri(&'a str),
}
impl ComputedImage {
    pub fn property(&self) -> PropertyId {
        self.property
    }
    pub fn component(&self) -> ImageComponent<'_> {
        match self.uri.as_deref() {
            Some(uri) => ImageComponent::Uri(uri),
            None => ImageComponent::None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageComputationError {
    UnsupportedProperty,
    Unresolved,
    Uri(UriError),
}
impl std::fmt::Display for ImageComputationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "image URI computation: {self:?}")
    }
}
impl std::error::Error for ImageComputationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(error) => Some(error),
            _ => None,
        }
    }
}
/// Resolve against the owning stylesheet's absolute ASCII base URI. Input byte,
/// output capacity and cumulative work limits are enforced by the URI resolver.
/// Raw non-ASCII conversion, acquisition and image decoding are separate stages.
/// None performs no URI work and does not inspect the unused base.
pub fn compute_image(
    property: &ParsedProperty,
    stylesheet_base: &str,
    limits: UriLimits,
) -> Result<ComputedImage, ImageComputationError> {
    if !matches!(
        property.property.descriptor().name,
        "background-image" | "list-style-image"
    ) {
        return Err(ImageComputationError::UnsupportedProperty);
    }
    let uri = match &property.value {
        SpecifiedValue::Keyword(keyword) if keyword.as_str() == "none" => None,
        SpecifiedValue::Uri(reference) => Some(
            uri::resolve(stylesheet_base, reference, limits).map_err(ImageComputationError::Uri)?,
        ),
        _ => return Err(ImageComputationError::Unresolved),
    };
    Ok(ComputedImage {
        property: property.property,
        uri,
    })
}
