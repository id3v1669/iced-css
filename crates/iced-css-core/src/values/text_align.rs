//! Preserve CSS2.2's nameless initial alignment until used direction is known.
use super::{
    initial::InitialReference,
    keyword::ComputedKeyword,
    property::{ParsedProperty, SpecifiedValue},
    registry::PropertyId,
};
use crate::cascade::{DefaultedValueRef, DefaultingError, winner_table::PropertyWinners};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputedTextAlign {
    DirectionalInitial,
    Left,
    Right,
    Center,
    Justify,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsedTextAlign {
    Left,
    Right,
    Center,
    Justify,
}
#[derive(Debug, PartialEq, Eq)]
pub enum TextAlignError {
    Unresolved,
    MissingDirection,
    InvalidDirection,
    Defaulting(DefaultingError),
}
impl std::fmt::Display for TextAlignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "text-align computation: {self:?}")
    }
}
impl std::error::Error for TextAlignError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Defaulting(error) => Some(error),
            _ => None,
        }
    }
}
impl ComputedTextAlign {
    fn from_value(value: &SpecifiedValue) -> Result<Self, TextAlignError> {
        match value {
            SpecifiedValue::InitialReference(InitialReference::DirectionalTextAlign) => {
                Ok(Self::DirectionalInitial)
            }
            SpecifiedValue::Keyword(keyword) => match keyword.as_str() {
                "left" => Ok(Self::Left),
                "right" => Ok(Self::Right),
                "center" => Ok(Self::Center),
                "justify" => Ok(Self::Justify),
                _ => Err(TextAlignError::Unresolved),
            },
            _ => Err(TextAlignError::Unresolved),
        }
    }
    /// Resolve the nameless initial against this element's direction. Explicit
    /// alignments do not consult direction. Justification itself belongs to layout.
    pub fn resolve(
        self,
        direction: Option<ComputedKeyword>,
    ) -> Result<UsedTextAlign, TextAlignError> {
        Ok(match self {
            Self::Left => UsedTextAlign::Left,
            Self::Right => UsedTextAlign::Right,
            Self::Center => UsedTextAlign::Center,
            Self::Justify => UsedTextAlign::Justify,
            Self::DirectionalInitial => {
                let direction = direction.ok_or(TextAlignError::MissingDirection)?;
                if direction.property().descriptor().name != "direction" {
                    return Err(TextAlignError::InvalidDirection);
                }
                match direction.keyword().as_str() {
                    "ltr" => UsedTextAlign::Left,
                    "rtl" => UsedTextAlign::Right,
                    _ => return Err(TextAlignError::InvalidDirection),
                }
            }
        })
    }
}
impl TryFrom<&ParsedProperty> for ComputedTextAlign {
    type Error = TextAlignError;
    fn try_from(value: &ParsedProperty) -> Result<Self, Self::Error> {
        if value.property.descriptor().name != "text-align" {
            return Err(TextAlignError::Unresolved);
        }
        Self::from_value(&value.value)
    }
}
/// Default from the actual source parent's computed alignment, not its used side.
pub fn compute_text_align(
    winners: &PropertyWinners<'_>,
    parent: Option<ComputedTextAlign>,
) -> Result<ComputedTextAlign, TextAlignError> {
    let property = PropertyId::lookup("text-align").expect("CSS2.2 property is registered");
    match winners
        .defaulted(property, parent.is_some())
        .map_err(TextAlignError::Defaulting)?
    {
        DefaultedValueRef::Compute(value) => ComputedTextAlign::from_value(value),
        DefaultedValueRef::Initial(_) => Ok(ComputedTextAlign::DirectionalInitial),
        DefaultedValueRef::ParentComputed => {
            Ok(parent.expect("defaulting checked parent presence"))
        }
    }
}
