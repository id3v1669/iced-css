//! Unicode-only separate-file intake. Other registered charsets remain explicit
//! unimplemented results, not unknown labels or lossy UTF-8 replacements.
use super::{
    charset::{Charset, lookup_iana_charset_iter},
    decode::{
        DecodeError, DecodeLimits, DecodedText, SourcePosition, UnicodeBodyEncoding,
        decode_unicode_source, decode_utf8_source,
    },
    encoding::{ByteOrder, EncodingHint},
    encoding_selection::{
        EncodingChoice, EncodingMetadata, EncodingOrigin, SelectionError, SelectionLimits,
        select_separate_encoding,
    },
    encoding_transport::TransportEncoding,
};
use std::{num::NonZeroUsize, ops::Range};

#[derive(Debug, Clone, Copy)]
pub struct IntakeLimits {
    pub selection: SelectionLimits,
    pub decode: DecodeLimits,
    /// Shared by selected-name resolution and decoded declaration validation.
    pub charset_work: NonZeroUsize,
}
pub struct UnicodeStylesheet {
    body: DecodedText,
    signature: usize,
    origin: EncodingOrigin,
    referring_label: &'static [u8],
    transport_endian_error: bool,
}
impl UnicodeStylesheet {
    pub fn text(&self) -> &str {
        self.body.text()
    }
    pub fn signature_bytes(&self) -> usize {
        self.signature
    }
    pub fn origin(&self) -> EncodingOrigin {
        self.origin
    }
    pub fn transport_endian_error(&self) -> bool {
        self.transport_endian_error
    }
    /// Canonical selected charset for a child stylesheet’s referring metadata.
    /// Generic Unicode charsets retain their identity independently of byte order.
    pub fn referring_label(&self) -> &'static [u8] {
        self.referring_label
    }
    pub fn retained_bytes(&self) -> usize {
        self.body.retained_bytes()
    }
    pub fn position(&self, output_byte: usize) -> Option<SourcePosition> {
        self.body.position(output_byte).map(|mut position| {
            // The private body spans a suffix of the original slice.
            position.byte += self.signature;
            position
        })
    }
    pub fn nul_repairs(&self) -> impl ExactSizeIterator<Item = Range<usize>> + '_ {
        self.body
            .nul_repairs()
            .iter()
            .map(|range| range.start + self.signature..range.end + self.signature)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntakeError {
    Selection(SelectionError),
    UnknownCharset,
    UnimplementedCharset { mib: u16 },
    UnsupportedOrder,
    CharsetWork,
    InappropriateCharset,
    Decode(DecodeError),
}
impl std::fmt::Display for IntakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unicode stylesheet intake: {self:?}")
    }
}
impl std::error::Error for IntakeError {}
#[derive(Clone, Copy)]
enum Body {
    Utf8,
    Unicode(UnicodeBodyEncoding),
}
fn ordered(hint: EncodingHint) -> Result<Body, IntakeError> {
    Ok(match hint {
        EncodingHint::Utf8 => Body::Utf8,
        EncodingHint::Utf16(ByteOrder::Big) => Body::Unicode(UnicodeBodyEncoding::Utf16Be),
        EncodingHint::Utf16(ByteOrder::Little) => Body::Unicode(UnicodeBodyEncoding::Utf16Le),
        EncodingHint::Utf32(ByteOrder::Big) => Body::Unicode(UnicodeBodyEncoding::Utf32Be),
        EncodingHint::Utf32(ByteOrder::Little) => Body::Unicode(UnicodeBodyEncoding::Utf32Le),
        _ => return Err(IntakeError::UnsupportedOrder),
    })
}
fn family_label(hint: EncodingHint) -> Result<&'static [u8], IntakeError> {
    match hint {
        EncodingHint::Utf8 => Ok(b"UTF-8"),
        EncodingHint::Utf16(_) => Ok(b"UTF-16"),
        EncodingHint::Utf32(_) => Ok(b"UTF-32"),
        EncodingHint::Declared { .. } => Err(IntakeError::UnsupportedOrder),
    }
}
fn registered<I: ExactSizeIterator<Item = u8> + Clone>(
    label: I,
    work: &mut usize,
) -> Result<&'static Charset, IntakeError> {
    lookup_iana_charset_iter(label, work)
        .map_err(|_| IntakeError::CharsetWork)?
        .ok_or(IntakeError::UnknownCharset)
}
fn scheme(
    input: &[u8],
    charset: &Charset,
    default: Option<ByteOrder>,
) -> Result<(Body, usize, &'static [u8]), IntakeError> {
    use TransportEncoding::*;
    let class = charset
        .unicode_transport_class()
        .ok_or(IntakeError::UnimplementedCharset { mib: charset.mib() })?;
    let mut signature = 0;
    let hint = match class {
        Utf8 => {
            if input.starts_with(b"\xef\xbb\xbf") {
                signature = 3;
            }
            EncodingHint::Utf8
        }
        Utf16 | Utf32 => {
            let (big, little): (&[u8], &[u8]) = if class == Utf16 {
                (b"\xfe\xff", b"\xff\xfe")
            } else {
                (b"\0\0\xfe\xff", b"\xff\xfe\0\0")
            };
            let order = if input.starts_with(big) {
                signature = big.len();
                ByteOrder::Big
            } else if input.starts_with(little) {
                signature = little.len();
                ByteOrder::Little
            } else {
                default.unwrap_or(ByteOrder::Big)
            };
            if class == Utf16 {
                EncodingHint::Utf16(order)
            } else {
                EncodingHint::Utf32(order)
            }
        }
        Utf16Be => EncodingHint::Utf16(ByteOrder::Big),
        Utf16Le => EncodingHint::Utf16(ByteOrder::Little),
        Utf32Be => EncodingHint::Utf32(ByteOrder::Big),
        Utf32Le => EncodingHint::Utf32(ByteOrder::Little),
        Other => unreachable!("registry's Unicode classification excludes Other"),
    };
    let label: &'static [u8] = match class {
        Utf8 => b"UTF-8",
        Utf16 => b"UTF-16",
        Utf32 => b"UTF-32",
        Utf16Be => b"UTF-16BE",
        Utf16Le => b"UTF-16LE",
        Utf32Be => b"UTF-32BE",
        Utf32Le => b"UTF-32LE",
        Other => unreachable!("registry's Unicode classification excludes Other"),
    };
    Ok((ordered(hint)?, signature, label))
}
fn appropriate(text: &str, expected: &Charset, work: &mut usize) -> Result<(), IntakeError> {
    let mut length = 0;
    // Inspect only the initial literal declaration, charging every probe. Do not
    // treat a quoted escape or CSS newline as part of a registered name.
    for byte in text.bytes() {
        *work = work.checked_sub(1).ok_or(IntakeError::CharsetWork)?;
        length += 1;
        if byte == b';' {
            break;
        }
    }
    let initial = &text.as_bytes()[..length];
    let name = initial
        .strip_prefix(b"@charset \"")
        .and_then(|rest| rest.strip_suffix(b"\";"))
        .ok_or(IntakeError::InappropriateCharset)?;
    let actual = lookup_iana_charset_iter(name.iter().copied(), work)
        .map_err(|_| IntakeError::CharsetWork)?;
    if actual.is_some_and(|charset| charset.mib() == expected.mib()) {
        Ok(())
    } else {
        Err(IntakeError::InappropriateCharset)
    }
}

/// Apply CSS's precedence and Unicode scheme decoding without publishing source
/// state. Decode errors use original byte offsets. Charset rules are retained as
/// text; only an as-specified detection result requires initial-rule validation.
/// Work is bounded by the sum of selection, shared charset and decode allowances.
pub fn decode_separate_unicode(
    input: &[u8],
    metadata: EncodingMetadata<'_>,
    limits: IntakeLimits,
) -> Result<UnicodeStylesheet, IntakeError> {
    let selection = select_separate_encoding(input, metadata, limits.selection)
        .map_err(IntakeError::Selection)?;
    let mut work = limits.charset_work.get();
    let mut expected = None;
    let mut transport_endian_error = false;
    let (body, signature, referring_label) = match selection.choice {
        EncodingChoice::DefaultUtf8 => (Body::Utf8, 0, b"UTF-8" as &'static [u8]),
        EncodingChoice::Label(label) => {
            scheme(input, registered(label.iter().copied(), &mut work)?, None)?
        }
        EncodingChoice::TransportBom(overridden) => {
            transport_endian_error = overridden.explicit_endian_error;
            (
                ordered(overridden.encoding)?,
                overridden.bom_bytes,
                family_label(overridden.encoding)?,
            )
        }
        EncodingChoice::Detected(prefix) => match prefix.hint {
            EncodingHint::Declared { default_order } => {
                let name = prefix.name.expect("declared detection rows carry a name");
                let charset = registered(name.bytes(), &mut work)?;
                expected = Some(charset);
                scheme(input, charset, default_order)?
            }
            hint => (ordered(hint)?, prefix.consumed, family_label(hint)?),
        },
    };
    let bytes = &input[signature..];
    let body = match body {
        Body::Utf8 => decode_utf8_source(bytes, limits.decode),
        Body::Unicode(encoding) => decode_unicode_source(bytes, encoding, limits.decode),
    }
    .map_err(|error| {
        IntakeError::Decode(match error {
            DecodeError::Utf8 {
                valid_up_to,
                error_len,
            } => DecodeError::Utf8 {
                valid_up_to: valid_up_to + signature,
                error_len,
            },
            DecodeError::Unicode {
                encoding,
                offset,
                fault,
            } => DecodeError::Unicode {
                encoding,
                offset: offset + signature,
                fault,
            },
            other => other,
        })
    })?;
    if let Some(charset) = expected {
        appropriate(body.text(), charset, &mut work)?;
    }
    Ok(UnicodeStylesheet {
        body,
        signature,
        origin: selection.origin,
        referring_label,
        transport_endian_error,
    })
}
