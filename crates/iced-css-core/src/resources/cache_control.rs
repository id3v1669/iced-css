//! RFC9111 Cache-Control directive-list syntax. Successful parsing does not
//! authorize storage or reuse; directive-specific semantics belong to the cache.
use super::http_syntax::{FieldError, Reader};
use std::{borrow::Cow, num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct CacheControlLimits {
    pub fields: NonZeroUsize,
    pub bytes: NonZeroUsize,
    pub directives: NonZeroUsize,
    /// Combined directive-vector and copied argument capacities.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheControlError {
    Invalid { field: usize, offset: usize },
    Fields,
    Bytes,
    Directives,
    Work,
    Allocation,
}
impl std::fmt::Display for CacheControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP Cache-Control: {self:?}")
    }
}
impl std::error::Error for CacheControlError {}
fn located(error: FieldError, field: usize) -> CacheControlError {
    match error {
        FieldError::Invalid { offset } => CacheControlError::Invalid { field, offset },
        FieldError::Bytes => CacheControlError::Bytes,
        FieldError::Work => CacheControlError::Work,
        FieldError::Allocation => CacheControlError::Allocation,
    }
}
#[derive(Debug)]
pub struct CacheDirective<'a> {
    name: &'a [u8],
    argument: Option<Cow<'a, [u8]>>,
    field: usize,
    span: Range<usize>,
    argument_span: Option<Range<usize>>,
}
impl<'a> CacheDirective<'a> {
    pub fn name(&self) -> &'a [u8] {
        self.name
    }
    pub fn argument(&self) -> Option<&[u8]> {
        self.argument.as_deref()
    }
    pub fn field(&self) -> usize {
        self.field
    }
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }
    pub fn argument_span(&self) -> Option<Range<usize>> {
        self.argument_span.clone()
    }
}
#[derive(Debug)]
pub struct CacheControl<'a> {
    present: bool,
    directives: Vec<CacheDirective<'a>>,
    retained_bytes: usize,
}
impl<'a> CacheControl<'a> {
    pub fn is_present(&self) -> bool {
        self.present
    }
    pub fn directives(&self) -> &[CacheDirective<'a>] {
        &self.directives
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}
pub fn parse_cache_control_fields<'a>(
    fields: &[&'a [u8]],
    limits: CacheControlLimits,
) -> Result<CacheControl<'a>, CacheControlError> {
    if fields.len() > limits.fields.get() {
        return Err(CacheControlError::Fields);
    }
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(CacheControlError::Bytes);
    }
    let mut work = limits.work.get();
    let mut bytes = 0usize;
    let mut copied = 0usize;
    let mut result = CacheControl {
        present: !fields.is_empty(),
        directives: Vec::new(),
        retained_bytes: 0,
    };
    for (field, input) in fields.iter().enumerate() {
        bytes = bytes
            .checked_add(input.len())
            .ok_or(CacheControlError::Bytes)?;
        if bytes > limits.bytes.get() {
            return Err(CacheControlError::Bytes);
        }
        work = work.checked_sub(1).ok_or(CacheControlError::Work)?;
        let mut reader = Reader {
            input,
            offset: 0,
            work,
        };
        let map = |e| located(e, field);
        loop {
            reader.ows().map_err(map)?;
            match reader.peek() {
                None => break,
                Some(b',') => {
                    reader.take().map_err(map)?;
                    continue;
                }
                _ => {}
            }
            let start = reader.offset;
            let name = reader.token().map_err(map)?;
            let value = if reader.peek() == Some(b'=') {
                reader.take().map_err(map)?;
                Some(reader.value().map_err(map)?)
            } else {
                None
            };
            let end = reader.offset;
            reader.ows().map_err(map)?;
            if !matches!(reader.peek(), None | Some(b',')) {
                return Err(map(reader.invalid()));
            }
            if result.directives.len() == limits.directives.get() {
                return Err(CacheControlError::Directives);
            }
            let size = std::mem::size_of::<CacheDirective<'_>>();
            if result.directives.len() == result.directives.capacity() {
                let maximum =
                    ((limits.retained_bytes.get() - copied) / size).min(limits.directives.get());
                let old = result.directives.capacity();
                if old >= maximum {
                    return Err(CacheControlError::Bytes);
                }
                let next = old.saturating_mul(2).max(1).min(maximum);
                reader
                    .spend(
                        result
                            .directives
                            .len()
                            .checked_mul(size)
                            .ok_or(CacheControlError::Work)?,
                    )
                    .map_err(map)?;
                result
                    .directives
                    .try_reserve_exact(next - result.directives.len())
                    .map_err(|_| CacheControlError::Allocation)?;
                if result.directives.capacity() > maximum {
                    return Err(CacheControlError::Bytes);
                }
            }
            let records = result.directives.capacity() * size;
            let argument_span = value.as_ref().map(|v| v.span.clone());
            let argument = match value {
                None => None,
                Some(value) => Some(
                    value
                        .decode(&mut reader, limits.retained_bytes.get() - records - copied)
                        .map_err(map)?,
                ),
            };
            if let Some(Cow::Owned(value)) = &argument {
                copied = copied
                    .checked_add(value.capacity())
                    .ok_or(CacheControlError::Bytes)?;
            }
            result.retained_bytes = records + copied;
            result.directives.push(CacheDirective {
                name: &input[name],
                argument,
                field,
                span: start..end,
                argument_span,
            });
        }
        work = reader.work;
    }
    Ok(result)
}
