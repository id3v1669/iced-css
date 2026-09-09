//! Bounded ownership conversion of already prepared header fields.
use super::{HeaderField, HeaderOrigin, PreparedHeaderUpdate};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct HeaderOwnershipLimits {
    pub entries: NonZeroUsize,
    /// Actual owned record, name and raw-payload Vec capacities combined.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderOwnershipError {
    Entries,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for HeaderOwnershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "owned HTTP headers: {self:?}")
    }
}
impl std::error::Error for HeaderOwnershipError {}
#[derive(Debug)]
pub struct OwnedUpdatedHeader {
    name: Vec<u8>,
    raw_value: Vec<u8>,
    start: usize,
    end: usize,
    origin: HeaderOrigin,
}
impl OwnedUpdatedHeader {
    pub fn field(&self) -> HeaderField<'_> {
        HeaderField {
            name: &self.name,
            raw_value: &self.raw_value,
            start: self.start,
            end: self.end,
        }
    }
    pub fn origin(&self) -> HeaderOrigin {
        self.origin
    }
}
#[derive(Debug)]
pub struct OwnedHeaderUpdate {
    fields: Vec<OwnedUpdatedHeader>,
    retained_bytes: usize,
}
impl OwnedHeaderUpdate {
    pub fn fields(&self) -> &[OwnedUpdatedHeader] {
        &self.fields
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}
fn spend(work: &mut usize, count: usize) -> Result<(), HeaderOwnershipError> {
    *work = work.checked_sub(count).ok_or(HeaderOwnershipError::Work)?;
    Ok(())
}
fn copy_bytes(input: &[u8], allowance: usize) -> Result<Vec<u8>, HeaderOwnershipError> {
    if input.len() > allowance {
        return Err(HeaderOwnershipError::Bytes);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(input.len())
        .map_err(|_| HeaderOwnershipError::Allocation)?;
    if result.capacity() > allowance {
        return Err(HeaderOwnershipError::Bytes);
    }
    result.extend_from_slice(input);
    Ok(result)
}
impl PreparedHeaderUpdate<'_> {
    /// Own only selected field bytes, retaining original spans and provenance.
    /// Failure preserves this preparation; source response/preparation capacities
    /// and allocator metadata are outside the new object's ownership allowance.
    pub fn try_to_owned(
        &self,
        limits: HeaderOwnershipLimits,
    ) -> Result<OwnedHeaderUpdate, HeaderOwnershipError> {
        if self.fields.len() > limits.entries.get() {
            return Err(HeaderOwnershipError::Entries);
        }
        if limits.retained_bytes.get() > isize::MAX as usize {
            return Err(HeaderOwnershipError::Bytes);
        }
        let mut work = limits.work.get();
        let size = size_of::<OwnedUpdatedHeader>();
        let mut nominal = self
            .fields
            .len()
            .checked_mul(size)
            .ok_or(HeaderOwnershipError::Bytes)?;
        if nominal > limits.retained_bytes.get() {
            return Err(HeaderOwnershipError::Bytes);
        }
        for entry in &self.fields {
            spend(&mut work, 1)?;
            nominal = nominal
                .checked_add(entry.field.name.len())
                .and_then(|n| n.checked_add(entry.field.raw_value.len()))
                .ok_or(HeaderOwnershipError::Bytes)?;
            if nominal > limits.retained_bytes.get() {
                return Err(HeaderOwnershipError::Bytes);
            }
        }
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(self.fields.len())
            .map_err(|_| HeaderOwnershipError::Allocation)?;
        let mut retained = fields
            .capacity()
            .checked_mul(size)
            .ok_or(HeaderOwnershipError::Bytes)?;
        if retained > limits.retained_bytes.get() {
            return Err(HeaderOwnershipError::Bytes);
        }
        for entry in &self.fields {
            let cost = entry
                .field
                .name
                .len()
                .checked_add(entry.field.raw_value.len())
                .and_then(|n| n.checked_add(size))
                .ok_or(HeaderOwnershipError::Work)?;
            spend(&mut work, cost)?;
            let name = copy_bytes(entry.field.name, limits.retained_bytes.get() - retained)?;
            retained = retained
                .checked_add(name.capacity())
                .ok_or(HeaderOwnershipError::Bytes)?;
            let raw_value = copy_bytes(
                entry.field.raw_value,
                limits.retained_bytes.get() - retained,
            )?;
            retained = retained
                .checked_add(raw_value.capacity())
                .ok_or(HeaderOwnershipError::Bytes)?;
            fields.push(OwnedUpdatedHeader {
                name,
                raw_value,
                start: entry.field.start,
                end: entry.field.end,
                origin: entry.origin,
            });
        }
        Ok(OwnedHeaderUpdate {
            fields,
            retained_bytes: retained,
        })
    }
}
