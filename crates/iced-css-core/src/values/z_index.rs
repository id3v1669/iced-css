//! Computed CSS2.2 z-index values, before stacking-context participation rules.
use super::{
    Integer,
    property::{ParsedProperty, SpecifiedValue},
};
#[derive(Debug, PartialEq, Eq)]
pub enum ComputedZIndex {
    Auto,
    Integer(Integer),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZIndexError {
    Unresolved,
}
impl std::fmt::Display for ZIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("z-index requires a resolved auto or integer property")
    }
}
impl std::error::Error for ZIndexError {}
/// Move the exact integer without narrowing or cloning. Inheritance/defaulting
/// and source order belong to the cascade/stacking owner, not this value helper.
pub fn compute_z_index(property: ParsedProperty) -> Result<ComputedZIndex, ZIndexError> {
    if property.property.descriptor().name != "z-index" {
        return Err(ZIndexError::Unresolved);
    }
    match property.value {
        SpecifiedValue::Integer(value) => Ok(ComputedZIndex::Integer(value)),
        SpecifiedValue::Keyword(keyword) if keyword.as_str() == "auto" => Ok(ComputedZIndex::Auto),
        _ => Err(ZIndexError::Unresolved),
    }
}
