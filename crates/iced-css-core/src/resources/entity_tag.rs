//! RFC9110 entity-tag grammar and section 8.8.3.2 comparisons. Opaque bytes
//! are never unescaped, decoded, case-folded or interpreted as a version number.
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct EntityTagLimits {
    pub bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityTagError {
    Invalid { offset: usize },
    MultipleFields { count: usize },
    Bytes,
    Work,
}
impl std::fmt::Display for EntityTagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP entity tag: {self:?}")
    }
}
impl std::error::Error for EntityTagError {}
#[derive(Debug, Clone, Copy)]
pub struct EntityTag<'a> {
    raw: &'a [u8],
    opaque: &'a [u8],
    weak: bool,
}
impl<'a> EntityTag<'a> {
    pub fn as_bytes(self) -> &'a [u8] {
        self.raw
    }
    pub fn opaque(self) -> &'a [u8] {
        self.opaque
    }
    pub fn is_weak(self) -> bool {
        self.weak
    }
    pub fn weak_eq(self, other: Self, work: NonZeroUsize) -> Result<bool, EntityTagError> {
        charge(self.opaque.len().min(other.opaque.len()), 1, work)?;
        Ok(self.opaque == other.opaque)
    }
    pub fn strong_eq(self, other: Self, work: NonZeroUsize) -> Result<bool, EntityTagError> {
        let equal = self.weak_eq(other, work)?;
        Ok(!self.weak && !other.weak && equal)
    }
}
fn charge(bytes: usize, multiplier: usize, work: NonZeroUsize) -> Result<(), EntityTagError> {
    let needed = bytes
        .checked_mul(multiplier)
        .and_then(|x| x.checked_add(1))
        .ok_or(EntityTagError::Work)?;
    if needed > work.get() {
        Err(EntityTagError::Work)
    } else {
        Ok(())
    }
}
/// Strip envelope OWS and validate exactly one singleton field. Multiple fields
/// are rejected by project policy, including equal duplicates. No owned allocation.
pub fn parse_entity_tag_fields<'a>(
    fields: &[&'a [u8]],
    limits: EntityTagLimits,
) -> Result<Option<EntityTag<'a>>, EntityTagError> {
    if fields.is_empty() {
        return Ok(None);
    }
    if fields.len() != 1 {
        return Err(EntityTagError::MultipleFields {
            count: fields.len(),
        });
    }
    let input = fields[0];
    if input.len() > limits.bytes.get() {
        return Err(EntityTagError::Bytes);
    }
    charge(input.len(), 3, limits.work)?;
    let mut start = 0;
    let mut end = input.len();
    while start < end && matches!(input[start], b' ' | b'\t') {
        start += 1;
    }
    while end > start && matches!(input[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    let raw = &input[start..end];
    let weak = raw.starts_with(b"W/");
    let mut at = start + if weak { 2 } else { 0 };
    if input.get(at) != Some(&b'"') || at >= end {
        return Err(EntityTagError::Invalid { offset: at });
    }
    at += 1;
    let opaque_start = at;
    while at < end && input[at] != b'"' {
        if !matches!(input[at],0x21|0x23..=0x7e|0x80..=0xff) {
            return Err(EntityTagError::Invalid { offset: at });
        }
        at += 1;
    }
    if at == end {
        return Err(EntityTagError::Invalid { offset: at });
    }
    let opaque = &input[opaque_start..at];
    at += 1;
    if at != end {
        return Err(EntityTagError::Invalid { offset: at });
    }
    Ok(Some(EntityTag { raw, opaque, weak }))
}
