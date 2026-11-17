use crate::values::{
    color::ColorLimits,
    initial::InitialError,
    property::{ParsedProperty, SpecifiedValue},
    registry::PropertyId,
};
// Fixed inline specified values avoid an infallible allocation at this boundary.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq, Eq)]
pub enum DefaultedValue {
    Compute(SpecifiedValue),
    ParentComputed,
}
#[derive(Debug, PartialEq, Eq)]
pub enum DefaultingError {
    Shorthand(PropertyId),
    WrongProperty {
        expected: PropertyId,
        actual: PropertyId,
    },
    UnexpandedShorthand,
    Initial(InitialError),
}
/// Defaulting decision borrowing immutable declaration storage. Initial recipes
/// are not materialized here; callers use `initial_value` with explicit limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultedValueRef<'a> {
    Compute(&'a SpecifiedValue),
    Initial(PropertyId),
    ParentComputed,
}
/// Classify one expanded longhand without allocation or value cloning.
pub fn default_value_ref(
    property: PropertyId,
    winner: Option<&ParsedProperty>,
    has_parent: bool,
) -> Result<DefaultedValueRef<'_>, DefaultingError> {
    if property.descriptor().shorthand {
        return Err(DefaultingError::Shorthand(property));
    }
    if let Some(winner) = winner {
        if winner.property != property {
            return Err(DefaultingError::WrongProperty {
                expected: property,
                actual: winner.property,
            });
        }
        match &winner.value {
            SpecifiedValue::Expanded(_) => Err(DefaultingError::UnexpandedShorthand),
            SpecifiedValue::Inherit if has_parent => Ok(DefaultedValueRef::ParentComputed),
            // A `revert` no cascade resolved acts as `unset`.
            SpecifiedValue::Revert
                if has_parent
                    && property.descriptor().inheritance
                        == crate::values::registry::Inheritance::Yes =>
            {
                Ok(DefaultedValueRef::ParentComputed)
            }
            SpecifiedValue::Revert => Ok(DefaultedValueRef::Initial(property)),
            SpecifiedValue::Inherit | SpecifiedValue::Initial => {
                Ok(DefaultedValueRef::Initial(property))
            }
            value => Ok(DefaultedValueRef::Compute(value)),
        }
    } else if has_parent
        && property.descriptor().inheritance == crate::values::registry::Inheritance::Yes
    {
        Ok(DefaultedValueRef::ParentComputed)
    } else {
        Ok(DefaultedValueRef::Initial(property))
    }
}
pub fn default_value(
    property: PropertyId,
    winner: Option<ParsedProperty>,
    has_parent: bool,
    limits: ColorLimits,
) -> Result<DefaultedValue, DefaultingError> {
    match default_value_ref(property, winner.as_ref(), has_parent)? {
        DefaultedValueRef::Compute(_) => Ok(DefaultedValue::Compute(
            winner
                .expect("a borrowed specified value requires a winner")
                .value,
        )),
        DefaultedValueRef::ParentComputed => Ok(DefaultedValue::ParentComputed),
        DefaultedValueRef::Initial(property) => {
            crate::values::initial::initial_value(property, limits)
                .map(DefaultedValue::Compute)
                .map_err(DefaultingError::Initial)
        }
    }
}

impl std::fmt::Display for DefaultingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shorthand(property) => write!(
                f,
                "defaulting requires expanded longhand: {}",
                property.descriptor().name
            ),
            Self::WrongProperty { expected, actual } => write!(
                f,
                "defaulting owner mismatch: expected {}, received {}",
                expected.descriptor().name,
                actual.descriptor().name
            ),
            Self::UnexpandedShorthand => {
                f.write_str("defaulting received an unexpanded shorthand value")
            }
            Self::Initial(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for DefaultingError {}
