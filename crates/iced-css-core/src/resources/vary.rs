//! Vary recipient grammar and wildcard interpretation; cache ownership is separate.
pub use super::field_names::{
    FieldNameError as VaryError, FieldNameLimits as VaryLimits, FieldNameMember as VaryMember,
};
use super::field_names::{FieldNames, parse_field_name_list};
use std::num::NonZeroUsize;
#[derive(Debug)]
pub struct Vary<'a> {
    names: FieldNames<'a>,
}
impl<'a> Vary<'a> {
    pub fn is_present(&self) -> bool {
        self.names.is_present()
    }
    /// RFC9111 §4.1: a list containing '*' always fails to match a stored response.
    pub fn is_wildcard(&self) -> bool {
        self.names.has_star_member()
    }
    pub fn members(&self) -> &[VaryMember<'a>] {
        self.names.members()
    }
    pub fn selects(&self, name: &[u8], work: NonZeroUsize) -> Result<bool, VaryError> {
        self.names.contains(name, work)
    }
}
pub fn parse_vary_fields<'a>(
    fields: &[&'a [u8]],
    limits: VaryLimits,
) -> Result<Vary<'a>, VaryError> {
    parse_field_name_list(fields, limits).map(|names| Vary { names })
}
