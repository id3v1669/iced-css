//! Outline-specific admission around shared CSS2.2 width computation.
use super::{
    NonnegativeF32, NumericLimits,
    border_width::{BorderFontContext, BorderWidthError, compute_border_width},
    property::ParsedProperty,
};
use crate::formatting::table_borders::BorderStyle;

/// Nine legal outline styles. Hidden cannot be constructed through this API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutlineStyle(BorderStyle);
#[derive(Debug, PartialEq, Eq)]
pub enum OutlineComputationError {
    WrongWidthProperty,
    InvalidStyle,
    Width(BorderWidthError),
}
impl std::fmt::Display for OutlineComputationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "outline computation: {self:?}")
    }
}
impl std::error::Error for OutlineComputationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Width(error) => Some(error),
            _ => None,
        }
    }
}
impl TryFrom<&ParsedProperty> for OutlineStyle {
    type Error = OutlineComputationError;
    fn try_from(value: &ParsedProperty) -> Result<Self, Self::Error> {
        if value.property.descriptor().name != "outline-style" {
            return Err(OutlineComputationError::InvalidStyle);
        }
        let style = BorderStyle::from_resolved(&value.value)
            .map_err(|_| OutlineComputationError::InvalidStyle)?;
        if style == BorderStyle::Hidden {
            return Err(OutlineComputationError::InvalidStyle);
        }
        Ok(Self(style))
    }
}
impl OutlineStyle {
    /// Preserve the style for outline painting; no collapsed-table substitution.
    pub fn border_style(self) -> BorderStyle {
        self.0
    }
}
/// Both inputs must be their resolved outline properties. Width conversion and
/// the repository's 1/3/5px keyword policy are shared with borders.
pub fn compute_outline_width(
    width: &ParsedProperty,
    style: &ParsedProperty,
    context: BorderFontContext,
    limits: NumericLimits,
) -> Result<NonnegativeF32, OutlineComputationError> {
    if width.property.descriptor().name != "outline-width" {
        return Err(OutlineComputationError::WrongWidthProperty);
    }
    OutlineStyle::try_from(style)?;
    compute_border_width(&width.value, &style.value, context, limits)
        .map_err(OutlineComputationError::Width)
}
