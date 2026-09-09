//! Selected Unicode body decoding and CSS scalar processing. Encoding selection,
//! signature removal and whole-source publication belong to stylesheet intake.
use std::{num::NonZeroUsize, ops::Range};
#[path = "unicode_body.rs"]
mod unicode;
pub use unicode::{UnicodeBodyEncoding, UnicodeFault, decode_unicode_source};

#[derive(Clone, Copy)]
struct Scalar {
    byte: usize,
    end: usize,
    value: char,
}

#[derive(Debug, Clone, Copy)]
pub struct DecodeLimits {
    pub entries: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub byte: usize,
    pub line: usize,
    pub column: usize,
}
#[derive(Clone, Copy)]
struct Point {
    output_byte: usize,
    source: SourcePosition,
}
pub struct DecodedText {
    text: String,
    points: Vec<Point>,
    nul_repairs: Vec<Range<usize>>,
    retained_bytes: usize,
}
impl DecodedText {
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn nul_repairs(&self) -> &[Range<usize>] {
        &self.nul_repairs
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Query an output scalar boundary or EOF using bounded-size numeric binary
    /// search. Returned offsets are relative to the supplied body bytes.
    pub fn position(&self, output_byte: usize) -> Option<SourcePosition> {
        self.points
            .binary_search_by_key(&output_byte, |point| point.output_byte)
            .ok()
            .map(|index| self.points[index].source)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    Utf8 {
        valid_up_to: usize,
        error_len: Option<usize>,
    },
    Unicode {
        encoding: UnicodeBodyEncoding,
        offset: usize,
        fault: UnicodeFault,
    },
    Entries,
    Bytes,
    Work,
    Allocation,
    Coordinates,
}
impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unicode source decoding: {self:?}")
    }
}
impl std::error::Error for DecodeError {}
fn charge(work: &mut usize, amount: usize) -> Result<(), DecodeError> {
    *work = work.checked_sub(amount).ok_or(DecodeError::Work)?;
    Ok(())
}
fn reserve<T>(count: usize, remaining_bytes: &mut usize) -> Result<Vec<T>, DecodeError> {
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(DecodeError::Bytes)?;
    if requested > *remaining_bytes {
        return Err(DecodeError::Bytes);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| DecodeError::Allocation)?;
    let actual = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(DecodeError::Bytes)?;
    *remaining_bytes = remaining_bytes
        .checked_sub(actual)
        .ok_or(DecodeError::Bytes)?;
    Ok(values)
}

/// Validate before processing any characters. Invalid UTF-8 returns the standard
/// decoder's original error coordinates without replacement or recovery. Valid
/// literal NUL becomes U+FFFD with provenance under Q279; all other scalars remain.
/// CRLF completes one line break after LF; its two source positions remain on the
/// preceding line. Standalone CR/LF/FF each complete one CSS newline.
/// Work charges validation bytes, scalar preflight and prepaid output construction.
pub fn decode_utf8_source(input: &[u8], limits: DecodeLimits) -> Result<DecodedText, DecodeError> {
    let work = body_work(input.len(), limits)?;
    let source = std::str::from_utf8(input).map_err(|error| DecodeError::Utf8 {
        valid_up_to: error.valid_up_to(),
        error_len: error.error_len(),
    })?;
    let scalars = source.char_indices().map(|(byte, value)| {
        Ok(Scalar {
            byte,
            end: byte + value.len_utf8(),
            value,
        })
    });
    build_source(input.len(), scalars, limits, work)
}

fn body_work(input_len: usize, limits: DecodeLimits) -> Result<usize, DecodeError> {
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(DecodeError::Bytes);
    }
    let mut work = limits.work.get();
    charge(
        &mut work,
        input_len.checked_add(1).ok_or(DecodeError::Work)?,
    )?;
    Ok(work)
}

fn build_source<I>(
    input_len: usize,
    scalars: I,
    limits: DecodeLimits,
    mut work: usize,
) -> Result<DecodedText, DecodeError>
where
    I: Iterator<Item = Result<Scalar, DecodeError>> + Clone,
{
    let mut output_bytes = 0usize;
    let mut point_count = 1usize;
    let mut repair_count = 0usize;
    for scalar in scalars.clone() {
        let scalar = scalar?;
        let value = scalar.value;
        charge(&mut work, scalar.end - scalar.byte + 1)?;
        point_count = point_count.checked_add(1).ok_or(DecodeError::Entries)?;
        if value == '\0' {
            repair_count = repair_count.checked_add(1).ok_or(DecodeError::Entries)?;
        }
        if point_count
            .checked_add(repair_count)
            .is_none_or(|count| count > limits.entries.get())
        {
            return Err(DecodeError::Entries);
        }
        output_bytes = output_bytes
            .checked_add(if value == '\0' { 3 } else { value.len_utf8() })
            .ok_or(DecodeError::Bytes)?;
    }
    charge(
        &mut work,
        output_bytes
            .checked_add(point_count)
            .and_then(|amount| amount.checked_add(input_len))
            .ok_or(DecodeError::Work)?,
    )?;
    let mut remaining_bytes = limits.retained_bytes.get();
    if output_bytes > remaining_bytes {
        return Err(DecodeError::Bytes);
    }
    let mut text = String::new();
    text.try_reserve_exact(output_bytes)
        .map_err(|_| DecodeError::Allocation)?;
    remaining_bytes = remaining_bytes
        .checked_sub(text.capacity())
        .ok_or(DecodeError::Bytes)?;
    let mut points = reserve(point_count, &mut remaining_bytes)?;
    let mut nul_repairs = reserve(repair_count, &mut remaining_bytes)?;
    let mut line = 1usize;
    let mut column = 1usize;
    let mut scalars = scalars.peekable();
    while let Some(scalar) = scalars.next() {
        let Scalar { byte, end, value } = scalar?;
        points.push(Point {
            output_byte: text.len(),
            source: SourcePosition { byte, line, column },
        });
        if value == '\0' {
            text.push('\u{fffd}');
            nul_repairs.push(byte..end);
        } else {
            text.push(value);
        }
        let newline = matches!(value, '\n' | '\u{c}')
            || (value == '\r' && !matches!(scalars.peek(), Some(Ok(next)) if next.value == '\n'));
        if newline {
            line = line.checked_add(1).ok_or(DecodeError::Coordinates)?;
            column = 1;
        } else {
            column = column.checked_add(1).ok_or(DecodeError::Coordinates)?;
        }
    }
    points.push(Point {
        output_byte: text.len(),
        source: SourcePosition {
            byte: input_len,
            line,
            column,
        },
    });
    Ok(DecodedText {
        text,
        points,
        nul_repairs,
        retained_bytes: limits.retained_bytes.get() - remaining_bytes,
    })
}
