//! CSS2.2 color computation preserves specified numeric and system-color values.
use super::{
    color::Color,
    initial::InitialReference,
    property::{ParsedProperty, SpecifiedValue},
    registry::PropertyId,
};
use crate::cascade::{DefaultedValueRef, DefaultingError, winner_table::PropertyWinners};

#[derive(Debug, Clone, Copy)]
pub struct ComputedColor<'a> {
    property: PropertyId,
    value: &'a Color,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct ColorComputationContext<'a> {
    /// Actual source parent's same-property computed value, when a parent exists.
    pub parent: Option<ComputedColor<'a>>,
    /// This element's computed foreground color, needed for border initials.
    pub current_color: Option<ComputedColor<'a>>,
    pub user_agent_color: Option<&'a Color>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ColorComputationError {
    UnsupportedProperty,
    Unresolved,
    InvalidValue,
    MissingUserAgentColor,
    MissingCurrentColor,
    InvalidCurrentColor,
    ParentPropertyMismatch,
    Defaulting(DefaultingError),
}
impl std::fmt::Display for ColorComputationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "computed color: {self:?}")
    }
}
impl std::error::Error for ColorComputationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Defaulting(error) => Some(error),
            _ => None,
        }
    }
}
fn border(property: PropertyId) -> bool {
    matches!(
        property.descriptor().name,
        "border-top-color" | "border-right-color" | "border-bottom-color" | "border-left-color"
    )
}
pub(crate) fn supported(property: PropertyId) -> bool {
    border(property)
        || matches!(
            property.descriptor().name,
            "color" | "background-color" | "outline-color"
        )
}
impl<'a> ComputedColor<'a> {
    pub fn property(self) -> PropertyId {
        self.property
    }
    pub fn value(self) -> &'a Color {
        self.value
    }
    fn new(property: PropertyId, value: &'a Color) -> Result<Self, ColorComputationError> {
        if !supported(property) {
            return Err(ColorComputationError::UnsupportedProperty);
        }
        // CSS Color 3: `transparent` is a color on every property. `currentColor` is
        // resolved before a computed color is made.
        match value {
            Color::CurrentColor => return Err(ColorComputationError::Unresolved),
            Color::Invert if property.descriptor().name != "outline-color" => {
                return Err(ColorComputationError::InvalidValue);
            }
            _ => {}
        }
        Ok(Self { property, value })
    }
}
/// Direct resolved color admission borrows its source storage:
/// ```compile_fail
/// use iced_css_core::values::{computed_color::ComputedColor,property::ParsedProperty};
/// fn escape(value: ParsedProperty) -> ComputedColor<'static> {
///     ComputedColor::try_from(&value).unwrap()
/// }
/// ```
impl<'a> TryFrom<&'a ParsedProperty> for ComputedColor<'a> {
    type Error = ColorComputationError;
    fn try_from(property: &'a ParsedProperty) -> Result<Self, Self::Error> {
        let SpecifiedValue::Color(value) = &property.value else {
            return Err(ColorComputationError::Unresolved);
        };
        Self::new(property.property, value)
    }
}
fn reference<'a>(
    property: PropertyId,
    value: InitialReference,
    context: ColorComputationContext<'a>,
) -> Result<&'a Color, ColorComputationError> {
    match value {
        InitialReference::CurrentColor if border(property) => {
            let foreground = context
                .current_color
                .ok_or(ColorComputationError::MissingCurrentColor)?;
            if foreground.property.descriptor().name != "color" {
                return Err(ColorComputationError::InvalidCurrentColor);
            }
            Ok(foreground.value)
        }
        InitialReference::UserAgentColor if property.descriptor().name == "color" => context
            .user_agent_color
            .ok_or(ColorComputationError::MissingUserAgentColor),
        _ => Err(ColorComputationError::Unresolved),
    }
}
static TRANSPARENT: Color = Color::Transparent;
static INVERT: Color = Color::Invert;
/// Compute from actual cascade winners. This does not normalize numeric channels,
/// resolve a system palette, perform gamut mapping or record dependency revisions.
/// The caller must provide immutable source/context revisions and refresh them.
pub fn compute_color<'a>(
    winners: &PropertyWinners<'a>,
    property: PropertyId,
    context: ColorComputationContext<'a>,
) -> Result<ComputedColor<'a>, ColorComputationError> {
    if !supported(property) {
        return Err(ColorComputationError::UnsupportedProperty);
    }
    let value = match winners
        .defaulted(property, context.parent.is_some())
        .map_err(ColorComputationError::Defaulting)?
    {
        // CSS Color 3, 4.4: the element's `color`; on `color` itself it is `inherit`.
        DefaultedValueRef::Compute(SpecifiedValue::Color(Color::CurrentColor)) => {
            if property.descriptor().name != "color" {
                let foreground = context
                    .current_color
                    .ok_or(ColorComputationError::MissingCurrentColor)?;
                if foreground.property.descriptor().name != "color" {
                    return Err(ColorComputationError::InvalidCurrentColor);
                }
                foreground.value
            } else if let Some(parent) = context.parent {
                if parent.property != property {
                    return Err(ColorComputationError::ParentPropertyMismatch);
                }
                parent.value
            } else {
                reference(property, InitialReference::UserAgentColor, context)?
            }
        }
        DefaultedValueRef::Compute(SpecifiedValue::Color(value)) => value,
        DefaultedValueRef::Compute(SpecifiedValue::InitialReference(value)) => {
            reference(property, *value, context)?
        }
        DefaultedValueRef::Compute(_) => return Err(ColorComputationError::Unresolved),
        DefaultedValueRef::ParentComputed => {
            let parent = context.parent.expect("defaulting checked parent presence");
            if parent.property != property {
                return Err(ColorComputationError::ParentPropertyMismatch);
            }
            parent.value
        }
        DefaultedValueRef::Initial(_) => match property.descriptor().name {
            "background-color" => &TRANSPARENT,
            "outline-color" => &INVERT,
            "color" => reference(property, InitialReference::UserAgentColor, context)?,
            _ => reference(property, InitialReference::CurrentColor, context)?,
        },
    };
    ComputedColor::new(property, value)
}
