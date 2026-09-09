//! Complete, bounded gzip/zlib content decoding before CSS character intake.
use flate2::{Decompress, DecompressError, FlushDecompress, Status};
use iced_css_core::resources::{
    body::{BodyCollector, BodyEnd, BodyError, BodyLimits, CompletedBody},
    content_codings::ContentCodings,
};
use std::{num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct ContentDecodeLimits {
    /// Each stage's borrowed input length; initial owned input capacity too.
    pub input_bytes: NonZeroUsize,
    /// Per-stage retained output capacity and collector work; no backend-state bound.
    pub output: BodyLimits,
    /// Shared across stages/members. Each call has at most 8 KiB input and output.
    /// This is not a measured CPU instruction or elapsed-time allowance.
    pub calls: NonZeroUsize,
    pub stages: NonZeroUsize,
    /// Total gzip members across all coding stages; zlib streams do not use it.
    pub members: NonZeroUsize,
}
#[derive(Debug)]
pub enum ContentDecodeError {
    InputBytes,
    Body(BodyError),
    Calls,
    Stages,
    Members,
    Unsupported { field: usize, span: Range<usize> },
    Decode(DecompressError),
    Incomplete,
    Trailing,
    Progress,
}
impl std::fmt::Display for ContentDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP content decoding: {self:?}")
    }
}
impl std::error::Error for ContentDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Body(e) => Some(e),
            Self::Decode(e) => Some(e),
            _ => None,
        }
    }
}
impl From<BodyError> for ContentDecodeError {
    fn from(e: BodyError) -> Self {
        Self::Body(e)
    }
}
#[derive(Clone, Copy)]
enum Format {
    Gzip,
    Zlib,
}
/// Decode only explicitly supported codings, in reverse application order. An
/// uncoded body moves without cloning after input/output admission. Old/new bodies,
/// fixed stack chunks and backend state can coexist; output capacity is not a
/// total process-memory limit. No partial body escapes a failed stage.
pub fn decode_content_codings(
    mut body: CompletedBody,
    codings: &ContentCodings<'_>,
    limits: ContentDecodeLimits,
) -> Result<CompletedBody, ContentDecodeError> {
    if body.retained_bytes() > limits.input_bytes.get() {
        return Err(ContentDecodeError::InputBytes);
    }
    if limits.output.retained_bytes.get() > isize::MAX as usize {
        return Err(BodyError::Bytes.into());
    }
    if codings.codings().len() > limits.stages.get() {
        return Err(ContentDecodeError::Stages);
    }
    // Admit all algorithms before processing any body. Unsupported names cannot
    // be interpreted as identity or as a different compression wrapper.
    for coding in codings.codings() {
        if format(coding.name()).is_none() {
            return Err(ContentDecodeError::Unsupported {
                field: coding.field(),
                span: coding.span(),
            });
        }
    }
    let mut calls = limits.calls.get();
    let mut members = limits.members.get();
    for coding in codings.decoding_order() {
        let selected = format(coding.name()).ok_or(ContentDecodeError::Unsupported {
            field: coding.field(),
            span: coding.span(),
        })?;
        body = decode_stage(body.as_slice(), selected, limits, &mut calls, &mut members)?;
    }
    if body.retained_bytes() > limits.output.retained_bytes.get() {
        return Err(BodyError::Bytes.into());
    }
    Ok(body)
}
fn format(name: &[u8]) -> Option<Format> {
    if name.eq_ignore_ascii_case(b"gzip") || name.eq_ignore_ascii_case(b"x-gzip") {
        Some(Format::Gzip)
    } else if name.eq_ignore_ascii_case(b"deflate") {
        Some(Format::Zlib)
    } else {
        None
    }
}
fn decoder(format: Format, members: &mut usize) -> Result<Decompress, ContentDecodeError> {
    match format {
        Format::Zlib => Ok(Decompress::new(true)),
        Format::Gzip => {
            *members = members.checked_sub(1).ok_or(ContentDecodeError::Members)?;
            // Constant 15 satisfies the upstream constructor's documented range.
            Ok(Decompress::new_gzip(15))
        }
    }
}
fn decode_stage(
    input: &[u8],
    format: Format,
    limits: ContentDecodeLimits,
    calls: &mut usize,
    members: &mut usize,
) -> Result<CompletedBody, ContentDecodeError> {
    if input.len() > limits.input_bytes.get() {
        return Err(ContentDecodeError::InputBytes);
    }
    let mut body = BodyCollector::new(None, limits.output)?;
    let mut decoder = decoder(format, members)?;
    let mut at = 0usize;
    let mut output = [0u8; 8192];
    loop {
        *calls = calls.checked_sub(1).ok_or(ContentDecodeError::Calls)?;
        let end = at.saturating_add(8192).min(input.len());
        let before_in = decoder.total_in();
        let before_out = decoder.total_out();
        let status = decoder
            .decompress(&input[at..end], &mut output, FlushDecompress::None)
            .map_err(ContentDecodeError::Decode)?;
        let consumed = decoder
            .total_in()
            .checked_sub(before_in)
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n <= end - at)
            .ok_or(ContentDecodeError::Progress)?;
        let produced = decoder
            .total_out()
            .checked_sub(before_out)
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n <= output.len())
            .ok_or(ContentDecodeError::Progress)?;
        at += consumed;
        body.append(&output[..produced])?;
        if status == Status::StreamEnd {
            if at == input.len() {
                return body.finish(BodyEnd::Complete).map_err(Into::into);
            }
            if matches!(format, Format::Zlib) {
                return Err(ContentDecodeError::Trailing);
            }
            decoder = self::decoder(format, members)?;
        } else if consumed == 0 && produced == 0 {
            return Err(if at == input.len() {
                ContentDecodeError::Incomplete
            } else {
                ContentDecodeError::Progress
            });
        }
    }
}
