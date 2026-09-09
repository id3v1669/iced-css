//! Bounded exact Vary evidence. No cache storage, reuse or network authorization.
use super::{
    http_syntax::{field_octet, tchar},
    vary::Vary,
};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct RequestField<'a> {
    pub name: &'a [u8],
    /// One separated field-line payload, before mandatory outer OWS exclusion.
    pub value: &'a [u8],
}
#[derive(Debug, Clone, Copy)]
pub struct VaryRequestLimits {
    pub fields: NonZeroUsize,
    pub bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestSide {
    Stored,
    Incoming,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaryRequestError {
    Fields,
    Bytes,
    Work,
    InvalidName { side: RequestSide, index: usize },
    InvalidValue { side: RequestSide, index: usize },
}
impl std::fmt::Display for VaryRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Vary request fields: {self:?}")
    }
}
impl std::error::Error for VaryRequestError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaryComparison {
    /// Only the Vary portion is satisfied, not other cache reuse conditions.
    ExactMatch,
    /// Exact evidence unavailable; field-specific equivalence is not disproved.
    DifferentFieldLines,
    Wildcard,
}
fn spend(work: &mut usize, count: usize) -> Result<(), VaryRequestError> {
    *work = work.checked_sub(count).ok_or(VaryRequestError::Work)?;
    Ok(())
}
fn validate(
    stored: &[RequestField<'_>],
    incoming: &[RequestField<'_>],
    limits: VaryRequestLimits,
    work: &mut usize,
) -> Result<(), VaryRequestError> {
    let count = stored
        .len()
        .checked_add(incoming.len())
        .ok_or(VaryRequestError::Fields)?;
    if count > limits.fields.get() {
        return Err(VaryRequestError::Fields);
    }
    let mut bytes = 0usize;
    for (side, fields) in [
        (RequestSide::Stored, stored),
        (RequestSide::Incoming, incoming),
    ] {
        for (index, field) in fields.iter().enumerate() {
            let length = field
                .name
                .len()
                .checked_add(field.value.len())
                .ok_or(VaryRequestError::Bytes)?;
            bytes = bytes.checked_add(length).ok_or(VaryRequestError::Bytes)?;
            if bytes > limits.bytes.get() {
                return Err(VaryRequestError::Bytes);
            }
            spend(work, length.checked_add(1).ok_or(VaryRequestError::Work)?)?;
            if field.name.is_empty() || !field.name.iter().copied().all(tchar) {
                return Err(VaryRequestError::InvalidName { side, index });
            }
            if !field.value.iter().copied().all(field_octet) {
                return Err(VaryRequestError::InvalidValue { side, index });
            }
        }
    }
    Ok(())
}
fn next_value<'a>(
    fields: &mut std::slice::Iter<'_, RequestField<'a>>,
    name: &[u8],
    work: &mut usize,
) -> Result<Option<&'a [u8]>, VaryRequestError> {
    for field in fields {
        let count = field
            .name
            .len()
            .checked_add(name.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(VaryRequestError::Work)?;
        spend(work, count)?;
        if field.name.eq_ignore_ascii_case(name) {
            return Ok(Some(field.value));
        }
    }
    Ok(None)
}
fn trim_ows(mut value: &[u8]) -> &[u8] {
    while matches!(value.first(), Some(b' ' | b'\t')) {
        value = &value[1..];
    }
    while matches!(value.last(), Some(b' ' | b'\t')) {
        value = &value[..value.len() - 1];
    }
    value
}
/// Require exact selected values after RFC9110 §5.5 outer OWS exclusion. Selected
/// field names are ASCII case-insensitive; per-name line order remains significant.
/// Does not combine lines or apply field-specific semantic normalization. Input and
/// work errors are never represented as a match, even for absent or wildcard Vary.
pub fn compare_vary_exact(
    vary: &Vary<'_>,
    stored: &[RequestField<'_>],
    incoming: &[RequestField<'_>],
    limits: VaryRequestLimits,
) -> Result<VaryComparison, VaryRequestError> {
    let mut work = limits.work.get();
    validate(stored, incoming, limits, &mut work)?;
    if vary.is_wildcard() {
        return Ok(VaryComparison::Wildcard);
    }
    for member in vary.members() {
        spend(
            &mut work,
            member
                .name()
                .len()
                .checked_add(1)
                .ok_or(VaryRequestError::Work)?,
        )?;
        let mut old = stored.iter();
        let mut new = incoming.iter();
        loop {
            match (
                next_value(&mut old, member.name(), &mut work)?,
                next_value(&mut new, member.name(), &mut work)?,
            ) {
                (None, None) => break,
                (Some(old), Some(new)) => {
                    let count = old
                        .len()
                        .checked_add(new.len())
                        .and_then(|n| n.checked_mul(2))
                        .and_then(|n| n.checked_add(1))
                        .ok_or(VaryRequestError::Work)?;
                    spend(&mut work, count)?;
                    if trim_ows(old) != trim_ows(new) {
                        return Ok(VaryComparison::DifferentFieldLines);
                    }
                }
                _ => return Ok(VaryComparison::DifferentFieldLines),
            }
        }
    }
    Ok(VaryComparison::ExactMatch)
}
