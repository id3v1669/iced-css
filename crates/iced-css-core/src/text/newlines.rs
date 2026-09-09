//! CSS22 16.6 default newline normalization, including generated content.
use std::{mem::size_of, num::NonZeroUsize, ops::Range};

#[derive(Debug, Clone, Copy)]
pub struct NewlineLimits {
    pub input_bytes: NonZeroUsize,
    /// Combined output text and scalar-map actual capacities; source is borrowed.
    pub retained_bytes: NonZeroUsize,
    /// Two scans, prepaid using the source's UTF-8 byte length.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewlineError {
    InputBytes,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for NewlineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "default text newline normalization: {self:?}")
    }
}
impl std::error::Error for NewlineError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryAffinity {
    Before,
    After,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedScalar {
    pub source: Range<usize>,
    pub normalized: Range<usize>,
}
#[derive(Debug)]
pub struct NormalizedText<'a> {
    source: &'a str,
    text: String,
    scalars: Vec<NormalizedScalar>,
}
impl<'a> NormalizedText<'a> {
    pub fn source(&self) -> &'a str {
        self.source
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn scalars(&self) -> &[NormalizedScalar] {
        &self.scalars
    }
    /// Map a valid original scalar boundary. Only the boundary inside CRLF
    /// needs affinity; the returned position is always a normalized boundary.
    pub fn to_normalized(&self, offset: usize, affinity: BoundaryAffinity) -> Option<usize> {
        if !self.source.is_char_boundary(offset) {
            return None;
        }
        if offset == self.source.len() {
            return Some(self.text.len());
        }
        let index = self
            .scalars
            .partition_point(|span| span.source.end <= offset);
        let span = self.scalars.get(index)?;
        Some(
            if offset == span.source.start || affinity == BoundaryAffinity::Before {
                span.normalized.start
            } else {
                span.normalized.end
            },
        )
    }
    /// Map a normalized scalar boundary back to the original source boundary.
    pub fn to_source(&self, offset: usize) -> Option<usize> {
        if !self.text.is_char_boundary(offset) {
            return None;
        }
        if offset == self.text.len() {
            return Some(self.source.len());
        }
        let index = self
            .scalars
            .partition_point(|span| span.normalized.end <= offset);
        self.scalars.get(index).map(|span| span.source.start)
    }
}
fn scan(source: &str) -> impl Iterator<Item = (Range<usize>, char)> + '_ {
    let mut chars = source.char_indices().peekable();
    std::iter::from_fn(move || {
        let (start, scalar) = chars.next()?;
        let mut end = start + scalar.len_utf8();
        let output = if scalar == '\r' {
            if chars.peek().is_some_and(|(_, c)| *c == '\n') {
                chars.next();
                end += 1;
            }
            '\n'
        } else {
            scalar
        };
        Some((start..end, output))
    })
}
/// Replace each CR or CRLF with one LF, preserving every other scalar.
/// Use only where the CSS default applies; source-language normalization and
/// optional additional newline recognition are distinct caller policies.
///
/// # Errors
/// Rejects input/work/storage limits and allocation failures without a partial result.
pub fn normalize_default_newlines(
    source: &str,
    limits: NewlineLimits,
) -> Result<NormalizedText<'_>, NewlineError> {
    if source.len() > limits.input_bytes.get() {
        return Err(NewlineError::InputBytes);
    }
    if source
        .len()
        .checked_mul(2)
        .is_none_or(|n| n > limits.work.get())
    {
        return Err(NewlineError::Work);
    }
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(NewlineError::Bytes);
    }
    // Normalization only shortens the source. Both counters are bounded by len.
    let (mut output_bytes, mut scalar_count) = (0usize, 0usize);
    for (_, scalar) in scan(source) {
        output_bytes += scalar.len_utf8();
        scalar_count += 1;
    }
    let map_bytes = scalar_count
        .checked_mul(size_of::<NormalizedScalar>())
        .ok_or(NewlineError::Bytes)?;
    if map_bytes
        .checked_add(output_bytes)
        .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(NewlineError::Bytes);
    }
    let mut text = String::new();
    text.try_reserve_exact(output_bytes)
        .map_err(|_| NewlineError::Allocation)?;
    if text
        .capacity()
        .checked_add(map_bytes)
        .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(NewlineError::Bytes);
    }
    let mut scalars = Vec::new();
    scalars
        .try_reserve_exact(scalar_count)
        .map_err(|_| NewlineError::Allocation)?;
    if scalars
        .capacity()
        .checked_mul(size_of::<NormalizedScalar>())
        .and_then(|n| n.checked_add(text.capacity()))
        .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(NewlineError::Bytes);
    }
    for (original, scalar) in scan(source) {
        let start = text.len();
        text.push(scalar);
        scalars.push(NormalizedScalar {
            source: original,
            normalized: start..text.len(),
        });
    }
    Ok(NormalizedText {
        source,
        text,
        scalars,
    })
}
