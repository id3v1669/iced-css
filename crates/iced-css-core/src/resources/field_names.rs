//! Shared HTTP comma-list of tokens with source spans and bounded storage/work.
use super::http_syntax::tchar as token;
use std::{num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct FieldNameLimits {
    pub fields: NonZeroUsize,
    pub bytes: NonZeroUsize,
    pub members: NonZeroUsize,
    pub record_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldNameError {
    Invalid { field: usize, offset: usize },
    Fields,
    Bytes,
    Members,
    Records,
    Work,
    Allocation,
}
impl std::fmt::Display for FieldNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP token list: {self:?}")
    }
}
impl std::error::Error for FieldNameError {}
#[derive(Debug)]
pub struct FieldNameMember<'a> {
    name: &'a [u8],
    field: usize,
    span: Range<usize>,
}
impl<'a> FieldNameMember<'a> {
    pub fn name(&self) -> &'a [u8] {
        self.name
    }
    pub fn field(&self) -> usize {
        self.field
    }
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }
}
#[derive(Debug)]
pub struct FieldNames<'a> {
    present: bool,
    literal_star: bool,
    members: Vec<FieldNameMember<'a>>,
}
impl<'a> FieldNames<'a> {
    pub fn is_present(&self) -> bool {
        self.present
    }
    /// Lexical evidence only; the consuming field defines any star semantics.
    pub(super) fn has_star_member(&self) -> bool {
        self.literal_star
    }
    pub fn members(&self) -> &[FieldNameMember<'a>] {
        &self.members
    }
    pub fn contains(&self, name: &[u8], work: NonZeroUsize) -> Result<bool, FieldNameError> {
        let mut work = work.get();
        self.contains_with_work(name, &mut work)
    }
    pub(super) fn contains_with_work(
        &self,
        name: &[u8],
        work: &mut usize,
    ) -> Result<bool, FieldNameError> {
        for member in &self.members {
            spend(
                work,
                member
                    .name
                    .len()
                    .checked_add(1)
                    .ok_or(FieldNameError::Work)?,
            )?;
            if member.name.eq_ignore_ascii_case(name) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
fn spend(work: &mut usize, n: usize) -> Result<(), FieldNameError> {
    *work = work.checked_sub(n).ok_or(FieldNameError::Work)?;
    Ok(())
}
fn ows(c: u8) -> bool {
    matches!(c, b' ' | b'\t')
}
pub fn parse_field_name_list<'a>(
    fields: &[&'a [u8]],
    limits: FieldNameLimits,
) -> Result<FieldNames<'a>, FieldNameError> {
    if fields.len() > limits.fields.get() {
        return Err(FieldNameError::Fields);
    }
    if limits.record_bytes.get() > isize::MAX as usize {
        return Err(FieldNameError::Records);
    }
    let mut work = limits.work.get();
    let mut bytes = 0usize;
    let mut result = FieldNames {
        present: !fields.is_empty(),
        literal_star: false,
        members: Vec::new(),
    };
    for (field, input) in fields.iter().enumerate() {
        bytes = bytes
            .checked_add(input.len())
            .ok_or(FieldNameError::Bytes)?;
        if bytes > limits.bytes.get() {
            return Err(FieldNameError::Bytes);
        }
        spend(
            &mut work,
            input
                .len()
                .checked_mul(3)
                .and_then(|n| n.checked_add(1))
                .ok_or(FieldNameError::Work)?,
        )?;
        let mut at = 0;
        while at < input.len() {
            while at < input.len() && (ows(input[at]) || input[at] == b',') {
                at += 1;
            }
            if at == input.len() {
                break;
            }
            let start = at;
            while at < input.len() && token(input[at]) {
                at += 1;
            }
            if start == at {
                return Err(FieldNameError::Invalid { field, offset: at });
            }
            let end = at;
            while at < input.len() && ows(input[at]) {
                at += 1;
            }
            if at < input.len() && input[at] != b',' {
                return Err(FieldNameError::Invalid { field, offset: at });
            }
            if result.members.len() == limits.members.get() {
                return Err(FieldNameError::Members);
            }
            if result.members.len() == result.members.capacity() {
                let size = std::mem::size_of::<FieldNameMember<'_>>();
                let maximum = (limits.record_bytes.get() / size).min(limits.members.get());
                let old = result.members.capacity();
                if old >= maximum {
                    return Err(FieldNameError::Records);
                }
                let next = old.saturating_mul(2).max(1).min(maximum);
                spend(
                    &mut work,
                    result
                        .members
                        .len()
                        .checked_mul(size)
                        .ok_or(FieldNameError::Work)?,
                )?;
                result
                    .members
                    .try_reserve_exact(next - result.members.len())
                    .map_err(|_| FieldNameError::Allocation)?;
                if result.members.capacity() > maximum {
                    return Err(FieldNameError::Records);
                }
            }
            spend(&mut work, 1)?;
            let name = &input[start..end];
            result.literal_star |= name == b"*";
            result.members.push(FieldNameMember {
                name,
                field,
                span: start..end,
            });
        }
    }
    Ok(result)
}
