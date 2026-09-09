//! Borrow parsed HTTP fields for core processing; no filtering or cache admission.
use iced_css_core::resources::header_update::{HeaderField, HeaderFieldError, HeaderFieldLimits};
use reqwest::header::HeaderMap;
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy)]
pub struct HttpHeaderLimits {
    pub fields: NonZeroUsize,
    /// Sum of name and payload lengths, counting the name for every value.
    pub input_bytes: NonZeroUsize,
    /// Actual envelope vector capacity; borrowed HeaderMap storage is separate.
    pub record_bytes: NonZeroUsize,
    /// Metadata visits, name lookup bytes, validation and envelope moves.
    /// Upstream hashing/allocator internals are not an instruction bound.
    pub work: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpHeaderError {
    Fields,
    InputBytes,
    Records,
    Work,
    Allocation,
    Field {
        index: usize,
        error: HeaderFieldError,
    },
}
impl std::fmt::Display for HttpHeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "received HTTP field envelopes: {self:?}")
    }
}
impl std::error::Error for HttpHeaderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Field { error, .. } => Some(error),
            _ => None,
        }
    }
}

/// Per-name insertion order is preserved; cross-name order is unspecified.
/// Names/payloads are borrowed from an already parsed HeaderMap, not wire framing.
/// Core field spans and indexes refer to this input, not original network offsets.
pub struct HttpHeaderBlock<'a> {
    fields: Vec<HeaderField<'a>>,
}
impl std::fmt::Debug for HttpHeaderBlock<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpHeaderBlock")
            .field("fields", &self.fields.len())
            .field("retained_bytes", &self.retained_bytes())
            .finish()
    }
}
fn spend(work: &mut usize, cost: usize) -> Result<(), HttpHeaderError> {
    *work = work.checked_sub(cost).ok_or(HttpHeaderError::Work)?;
    Ok(())
}
impl<'a> HttpHeaderBlock<'a> {
    pub fn prepare(
        headers: &'a HeaderMap,
        limits: HttpHeaderLimits,
    ) -> Result<Self, HttpHeaderError> {
        let count = headers.len();
        if count > limits.fields.get() {
            return Err(HttpHeaderError::Fields);
        }
        if limits.record_bytes.get() > isize::MAX as usize
            || count
                .checked_mul(size_of::<HeaderField<'a>>())
                .is_none_or(|bytes| bytes > limits.record_bytes.get())
        {
            return Err(HttpHeaderError::Records);
        }
        let mut work = limits.work.get();
        spend(&mut work, count)?;
        // This pass uses arbitrary iteration only to sum lengths, never for order.
        headers.iter().try_fold(0usize, |bytes, (name, value)| {
            bytes
                .checked_add(name.as_str().len())
                .and_then(|bytes| bytes.checked_add(value.as_bytes().len()))
                .filter(|bytes| *bytes <= limits.input_bytes.get())
                .ok_or(HttpHeaderError::InputBytes)
        })?;
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(count)
            .map_err(|_| HttpHeaderError::Allocation)?;
        if fields
            .capacity()
            .checked_mul(size_of::<HeaderField<'a>>())
            .is_none_or(|bytes| bytes > limits.record_bytes.get())
        {
            return Err(HttpHeaderError::Records);
        }
        for name in headers.keys() {
            spend(
                &mut work,
                name.as_str()
                    .len()
                    .checked_add(1)
                    .ok_or(HttpHeaderError::Work)?,
            )?;
            // Unlike general HeaderMap iteration, GetAll promises insertion order.
            for value in headers.get_all(name).iter() {
                let cost = value
                    .as_bytes()
                    .len()
                    .checked_mul(2)
                    .and_then(|cost| cost.checked_add(name.as_str().len()))
                    .and_then(|cost| cost.checked_add(1))
                    .ok_or(HttpHeaderError::Work)?;
                spend(
                    &mut work,
                    cost.checked_add(size_of::<HeaderField<'a>>())
                        .ok_or(HttpHeaderError::Work)?,
                )?;
                let field = HeaderField::new(
                    name.as_str().as_bytes(),
                    value.as_bytes(),
                    HeaderFieldLimits {
                        bytes: limits.input_bytes,
                        work: NonZeroUsize::new(cost).ok_or(HttpHeaderError::Work)?,
                    },
                )
                .map_err(|error| HttpHeaderError::Field {
                    index: fields.len(),
                    error,
                })?;
                // The immutable input supplies exactly count values; capacity was
                // reserved and admitted above, so this push does not reallocate.
                fields.push(field);
            }
        }
        Ok(Self { fields })
    }
    pub fn fields(&self) -> &[HeaderField<'a>] {
        &self.fields
    }
    pub fn retained_bytes(&self) -> usize {
        self.fields.capacity() * size_of::<HeaderField<'a>>()
    }
}
