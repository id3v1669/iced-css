//! Borrowed header-field admission and atomic update preparation, not cache admission.
use super::{
    http_syntax::{field_octet, tchar},
    storage_fields::{
        HeaderUpdateAction, StorageFieldClassifier, StorageFieldError, StorageFieldRole,
        StoredProcessing,
    },
};
use std::{num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct HeaderFieldLimits {
    pub bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderFieldError {
    Bytes,
    Work,
    InvalidName { offset: usize },
    InvalidValue { offset: usize },
}
impl std::fmt::Display for HeaderFieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP header field: {self:?}")
    }
}
impl std::error::Error for HeaderFieldError {}
/// Generic field-line envelope only; field-specific syntax/admission is separate.
#[derive(Debug, Clone, Copy)]
pub struct HeaderField<'a> {
    name: &'a [u8],
    raw_value: &'a [u8],
    start: usize,
    end: usize,
}
impl<'a> HeaderField<'a> {
    pub fn new(
        name: &'a [u8],
        raw_value: &'a [u8],
        limits: HeaderFieldLimits,
    ) -> Result<Self, HeaderFieldError> {
        let bytes = name
            .len()
            .checked_add(raw_value.len())
            .ok_or(HeaderFieldError::Bytes)?;
        if bytes > limits.bytes.get() {
            return Err(HeaderFieldError::Bytes);
        }
        let cost = raw_value
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(name.len()))
            .and_then(|n| n.checked_add(1))
            .ok_or(HeaderFieldError::Work)?;
        if cost > limits.work.get() {
            return Err(HeaderFieldError::Work);
        }
        if name.is_empty() {
            return Err(HeaderFieldError::InvalidName { offset: 0 });
        }
        if let Some(offset) = name.iter().position(|&byte| !tchar(byte)) {
            return Err(HeaderFieldError::InvalidName { offset });
        }
        if let Some(offset) = raw_value.iter().position(|&byte| !field_octet(byte)) {
            return Err(HeaderFieldError::InvalidValue { offset });
        }
        let mut start = 0;
        let mut end = raw_value.len();
        while start < end && matches!(raw_value[start], b' ' | b'\t') {
            start += 1;
        }
        while end > start && matches!(raw_value[end - 1], b' ' | b'\t') {
            end -= 1;
        }
        Ok(Self {
            name,
            raw_value,
            start,
            end,
        })
    }
    pub fn name(self) -> &'a [u8] {
        self.name
    }
    pub fn value(self) -> &'a [u8] {
        &self.raw_value[self.start..self.end]
    }
    pub fn raw_value(self) -> &'a [u8] {
        self.raw_value
    }
    pub fn value_span(self) -> Range<usize> {
        self.start..self.end
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderOrigin {
    Stored(usize),
    Received(usize),
}
#[derive(Debug, Clone, Copy)]
pub struct UpdatedHeader<'a> {
    field: HeaderField<'a>,
    origin: HeaderOrigin,
}
impl<'a> UpdatedHeader<'a> {
    pub fn field(self) -> HeaderField<'a> {
        self.field
    }
    pub fn origin(self) -> HeaderOrigin {
        self.origin
    }
}
#[derive(Debug)]
pub struct PreparedHeaderUpdate<'a> {
    fields: Vec<UpdatedHeader<'a>>,
}
impl<'a> PreparedHeaderUpdate<'a> {
    pub fn fields(&self) -> &[UpdatedHeader<'a>] {
        &self.fields
    }
    pub fn retained_bytes(&self) -> usize {
        self.fields.capacity() * size_of::<UpdatedHeader<'a>>()
    }
}
#[derive(Debug, Clone, Copy)]
pub struct HeaderUpdateLimits {
    pub fields: NonZeroUsize,
    pub input_bytes: NonZeroUsize,
    pub record_bytes: NonZeroUsize,
    /// Preparation only; each classifier retains its own shared work budget.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderUpdateError {
    Fields,
    InputBytes,
    Records,
    Work,
    Allocation,
    Classification {
        origin: HeaderOrigin,
        error: StorageFieldError,
    },
}
impl std::fmt::Display for HeaderUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP header update: {self:?}")
    }
}
impl std::error::Error for HeaderUpdateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Classification { error, .. } => Some(error),
            _ => None,
        }
    }
}
fn spend(work: &mut usize, count: usize) -> Result<(), HeaderUpdateError> {
    *work = work.checked_sub(count).ok_or(HeaderUpdateError::Work)?;
    Ok(())
}
/// Caller has established raw cache restrictions and update eligibility, and pairs
/// each classifier with its input block. Names/values remain borrowed and inputs
/// never change. Failure exposes no partial list; classifier budgets may be consumed.
pub fn prepare_header_update<'a>(
    stored: &[HeaderField<'a>],
    received: &[HeaderField<'a>],
    stored_classifier: &mut StorageFieldClassifier<'_, '_>,
    received_classifier: &mut StorageFieldClassifier<'_, '_>,
    processing: StoredProcessing,
    limits: HeaderUpdateLimits,
) -> Result<PreparedHeaderUpdate<'a>, HeaderUpdateError> {
    let count = stored
        .len()
        .checked_add(received.len())
        .ok_or(HeaderUpdateError::Fields)?;
    if count > limits.fields.get() {
        return Err(HeaderUpdateError::Fields);
    }
    let mut work = limits.work.get();
    let mut input_bytes = 0usize;
    for field in stored.iter().chain(received) {
        spend(&mut work, 1)?;
        input_bytes = input_bytes
            .checked_add(field.name.len())
            .and_then(|n| n.checked_add(field.raw_value.len()))
            .ok_or(HeaderUpdateError::InputBytes)?;
        if input_bytes > limits.input_bytes.get() {
            return Err(HeaderUpdateError::InputBytes);
        }
    }
    let size = size_of::<UpdatedHeader<'a>>();
    if limits.record_bytes.get() > isize::MAX as usize
        || count
            .checked_mul(size)
            .is_none_or(|bytes| bytes > limits.record_bytes.get())
    {
        return Err(HeaderUpdateError::Records);
    }
    spend(
        &mut work,
        stored
            .len()
            .checked_mul(size)
            .ok_or(HeaderUpdateError::Work)?,
    )?;
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(count)
        .map_err(|_| HeaderUpdateError::Allocation)?;
    if fields
        .capacity()
        .checked_mul(size)
        .is_none_or(|bytes| bytes > limits.record_bytes.get())
    {
        return Err(HeaderUpdateError::Records);
    }
    for (index, field) in stored.iter().enumerate() {
        let origin = HeaderOrigin::Stored(index);
        let role = stored_classifier
            .classify(field.name)
            .map_err(|error| HeaderUpdateError::Classification { origin, error })?;
        if role == StorageFieldRole::Candidate {
            fields.push(UpdatedHeader {
                field: *field,
                origin,
            });
        }
    }
    for (index, field) in received.iter().enumerate() {
        spend(&mut work, 1)?;
        let origin = HeaderOrigin::Received(index);
        let action = received_classifier
            .classify_update(field.name, processing)
            .map_err(|error| HeaderUpdateError::Classification { origin, error })?;
        if action != HeaderUpdateAction::ReplaceFromReceived {
            continue;
        }
        // Pay possible movement and all comparisons before allocation-free retain.
        spend(
            &mut work,
            fields
                .len()
                .checked_mul(size)
                .ok_or(HeaderUpdateError::Work)?,
        )?;
        for entry in &fields {
            spend(&mut work, 1)?;
            if matches!(entry.origin, HeaderOrigin::Stored(_)) {
                spend(
                    &mut work,
                    entry
                        .field
                        .name
                        .len()
                        .checked_add(field.name.len())
                        .and_then(|n| n.checked_add(1))
                        .ok_or(HeaderUpdateError::Work)?,
                )?;
            }
        }
        fields.retain(|entry| {
            !matches!(entry.origin, HeaderOrigin::Stored(_))
                || !entry.field.name.eq_ignore_ascii_case(field.name)
        });
        spend(&mut work, 1)?;
        fields.push(UpdatedHeader {
            field: *field,
            origin,
        });
    }
    Ok(PreparedHeaderUpdate { fields })
}

#[path = "header_ownership.rs"]
mod ownership;
pub use ownership::{
    HeaderOwnershipError, HeaderOwnershipLimits, OwnedHeaderUpdate, OwnedUpdatedHeader,
};
