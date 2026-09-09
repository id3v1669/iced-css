//! RFC9110 §7.6.1 option grammar and nomination lookup, not forwarding/cache policy.
pub use super::field_names::{
    FieldNameError as ConnectionError, FieldNameLimits as ConnectionLimits,
    FieldNameMember as ConnectionOption,
};
use super::field_names::{FieldNames, parse_field_name_list};
use std::num::NonZeroUsize;
#[derive(Debug)]
pub struct ConnectionOptions<'a> {
    names: FieldNames<'a>,
}
impl<'a> ConnectionOptions<'a> {
    pub fn is_present(&self) -> bool {
        self.names.is_present()
    }
    pub fn options(&self) -> &[ConnectionOption<'a>] {
        self.names.members()
    }
    /// Case-insensitive literal lookup. '*' has no wildcard nomination semantics.
    pub fn nominates(&self, name: &[u8], work: NonZeroUsize) -> Result<bool, ConnectionError> {
        self.names.contains(name, work)
    }
    pub(super) fn nominates_with_work(
        &self,
        name: &[u8],
        work: &mut usize,
    ) -> Result<bool, ConnectionError> {
        self.names.contains_with_work(name, work)
    }
}
/// Preserve lexical evidence, including names a sender must not nominate. This
/// parser does not grant protocol-version validity or remove cache restrictions.
pub fn parse_connection_fields<'a>(
    fields: &[&'a [u8]],
    limits: ConnectionLimits,
) -> Result<ConnectionOptions<'a>, ConnectionError> {
    parse_field_name_list(fields, limits).map(|names| ConnectionOptions { names })
}
