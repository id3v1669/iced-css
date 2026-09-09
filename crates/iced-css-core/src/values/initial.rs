use super::{
    color::ColorLimits,
    property::{PropertyError, SpecifiedValue},
    registry::PropertyId,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialReference {
    CurrentColor,
    UserAgentColor,
    UserAgentFontFamily,
    UserAgentQuotes,
    DirectionalTextAlign,
}
#[derive(Debug, PartialEq, Eq)]
pub enum InitialError {
    Shorthand(PropertyId),
    InvalidDefinition(PropertyId),
    Property(PropertyError),
}
pub fn initial_value(
    property: PropertyId,
    limits: ColorLimits,
) -> Result<SpecifiedValue, InitialError> {
    if property.descriptor().shorthand {
        return Err(InitialError::Shorthand(property));
    }
    let reference = match property.descriptor().name {
        "border-top-color" | "border-right-color" | "border-bottom-color" | "border-left-color" => {
            Some(InitialReference::CurrentColor)
        }
        "color" => Some(InitialReference::UserAgentColor),
        "font-family" => Some(InitialReference::UserAgentFontFamily),
        "quotes" => Some(InitialReference::UserAgentQuotes),
        "text-align" => Some(InitialReference::DirectionalTextAlign),
        _ => None,
    };
    if let Some(reference) = reference {
        return Ok(SpecifiedValue::InitialReference(reference));
    }
    let mut work = limits.syntax.work.get();
    match super::property::inner(
        property,
        property.descriptor().initial,
        false,
        false,
        limits,
        &mut work,
    )
    .map_err(InitialError::Property)?
    {
        super::property::PropertyResult::Parsed(value) => Ok(value.value),
        _ => Err(InitialError::InvalidDefinition(property)),
    }
}

impl std::fmt::Display for InitialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shorthand(property) => write!(
                f,
                "shorthand has no independent initial value: {}",
                property.descriptor().name
            ),
            Self::InvalidDefinition(property) => write!(
                f,
                "invalid initial definition: {}",
                property.descriptor().name
            ),
            Self::Property(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for InitialError {}
