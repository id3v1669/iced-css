//! RFC9110 Content-Encoding list grammar; decoder support is a separate decision.
use super::http_syntax::{FieldError, Reader};
use std::{num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct ContentCodingLimits {
    pub fields: NonZeroUsize,
    /// Sum of borrowed input lengths, including ignored empty comma members.
    pub bytes: NonZeroUsize,
    pub members: NonZeroUsize,
    /// Actual retained record capacity; borrowed headers and growth overlap excluded.
    pub record_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentCodingError {
    Invalid { field: usize, offset: usize },
    Fields,
    Bytes,
    Members,
    Records,
    Work,
    Allocation,
}
impl std::fmt::Display for ContentCodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP Content-Encoding: {self:?}")
    }
}
impl std::error::Error for ContentCodingError {}
#[derive(Debug)]
pub struct ContentCoding<'a> {
    name: &'a [u8],
    field: usize,
    span: Range<usize>,
}
impl<'a> ContentCoding<'a> {
    /// Received spelling. Coding-name comparisons are ASCII case-insensitive.
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
pub struct ContentCodings<'a> {
    present: bool,
    codings: Vec<ContentCoding<'a>>,
}
impl<'a> ContentCodings<'a> {
    pub fn is_present(&self) -> bool {
        self.present
    }
    pub fn codings(&self) -> &[ContentCoding<'a>] {
        &self.codings
    }
    pub fn decoding_order(&self) -> impl Iterator<Item = &ContentCoding<'a>> {
        self.codings.iter().rev()
    }
}
/// Fields must be supplied in received order. Empty list elements are ignored
/// under byte/work bounds; unknown valid tokens remain present for the decoder
/// admission layer to reject or support explicitly. No body is decoded here.
pub fn parse_content_coding_fields<'a>(
    fields: &[&'a [u8]],
    limits: ContentCodingLimits,
) -> Result<ContentCodings<'a>, ContentCodingError> {
    if fields.len() > limits.fields.get() {
        return Err(ContentCodingError::Fields);
    }
    if limits.record_bytes.get() > isize::MAX as usize {
        return Err(ContentCodingError::Records);
    }
    let mut result = ContentCodings {
        present: !fields.is_empty(),
        codings: Vec::new(),
    };
    let mut work = limits.work.get();
    let mut bytes = 0usize;
    for (field, input) in fields.iter().enumerate() {
        bytes = bytes
            .checked_add(input.len())
            .filter(|n| *n <= limits.bytes.get())
            .ok_or(ContentCodingError::Bytes)?;
        let mut reader = Reader {
            input,
            offset: 0,
            work,
        };
        parse_field(&mut reader, field, &mut result.codings, limits).map_err(
            |error| match error {
                ListError::Field(FieldError::Invalid { offset }) => {
                    ContentCodingError::Invalid { field, offset }
                }
                ListError::Field(FieldError::Work) => ContentCodingError::Work,
                ListError::Field(FieldError::Bytes) => ContentCodingError::Bytes,
                ListError::Field(FieldError::Allocation) => ContentCodingError::Allocation,
                ListError::Limit(error) => error,
            },
        )?;
        work = reader.work;
    }
    Ok(result)
}
enum ListError {
    Field(FieldError),
    Limit(ContentCodingError),
}
impl From<FieldError> for ListError {
    fn from(e: FieldError) -> Self {
        Self::Field(e)
    }
}
impl From<ContentCodingError> for ListError {
    fn from(e: ContentCodingError) -> Self {
        Self::Limit(e)
    }
}
fn parse_field<'a>(
    reader: &mut Reader<'a>,
    field: usize,
    codings: &mut Vec<ContentCoding<'a>>,
    limits: ContentCodingLimits,
) -> Result<(), ListError> {
    reader.spend(1)?;
    loop {
        reader.ows()?;
        match reader.peek() {
            None => return Ok(()),
            Some(b',') => {
                reader.take()?;
                continue;
            }
            _ => {}
        }
        let span = reader.token()?;
        reader.ows()?;
        if !matches!(reader.peek(), None | Some(b',')) {
            return Err(reader.invalid().into());
        }
        if codings.len() == limits.members.get() {
            return Err(ContentCodingError::Members.into());
        }
        if codings.len() == codings.capacity() {
            let size = size_of::<ContentCoding<'_>>();
            let maximum = (limits.record_bytes.get() / size).min(limits.members.get());
            if codings.capacity() >= maximum {
                return Err(ContentCodingError::Records.into());
            }
            let next = codings.capacity().saturating_mul(2).max(1).min(maximum);
            reader.spend(
                codings
                    .len()
                    .checked_mul(size)
                    .ok_or(ContentCodingError::Work)?,
            )?;
            codings
                .try_reserve_exact(next - codings.len())
                .map_err(|_| ContentCodingError::Allocation)?;
            if codings.capacity() > maximum {
                return Err(ContentCodingError::Records.into());
            }
        }
        reader.spend(1)?;
        codings.push(ContentCoding {
            name: &reader.input[span.clone()],
            field,
            span,
        });
    }
}
