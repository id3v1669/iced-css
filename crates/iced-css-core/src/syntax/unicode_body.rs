use super::{DecodeError, DecodeLimits, DecodedText, Scalar, body_work, build_source};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicodeBodyEncoding {
    Utf16Be,
    Utf16Le,
    Utf32Be,
    Utf32Le,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicodeFault {
    TruncatedUnit,
    UnpairedHighSurrogate,
    UnpairedLowSurrogate,
    InvalidScalar(u32),
}

#[derive(Clone)]
struct Scalars<'a> {
    input: &'a [u8],
    offset: usize,
    encoding: UnicodeBodyEncoding,
    failed: bool,
}
impl Scalars<'_> {
    fn fail(&mut self, fault: UnicodeFault) -> Option<Result<Scalar, DecodeError>> {
        self.failed = true;
        Some(Err(DecodeError::Unicode {
            encoding: self.encoding,
            offset: self.offset,
            fault,
        }))
    }
    fn utf16_word(&self, input: [u8; 2]) -> u16 {
        if self.encoding == UnicodeBodyEncoding::Utf16Le {
            u16::from_le_bytes(input)
        } else {
            u16::from_be_bytes(input)
        }
    }
}
impl Iterator for Scalars<'_> {
    type Item = Result<Scalar, DecodeError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.offset == self.input.len() {
            return None;
        }
        let tail = &self.input[self.offset..];
        let (value, size) = match self.encoding {
            UnicodeBodyEncoding::Utf16Be | UnicodeBodyEncoding::Utf16Le => {
                let Some(first) = tail.get(..2) else {
                    return self.fail(UnicodeFault::TruncatedUnit);
                };
                let high = self.utf16_word([first[0], first[1]]);
                match high {
                    0xdc00..=0xdfff => return self.fail(UnicodeFault::UnpairedLowSurrogate),
                    0xd800..=0xdbff => {
                        let Some(second) = tail.get(2..4) else {
                            return self.fail(UnicodeFault::UnpairedHighSurrogate);
                        };
                        let low = self.utf16_word([second[0], second[1]]);
                        if !(0xdc00..=0xdfff).contains(&low) {
                            return self.fail(UnicodeFault::UnpairedHighSurrogate);
                        }
                        let scalar = 0x10000
                            + ((u32::from(high) - 0xd800) << 10)
                            + (u32::from(low) - 0xdc00);
                        (
                            char::from_u32(scalar).expect("validated UTF-16 pair yields a scalar"),
                            4,
                        )
                    }
                    _ => (
                        char::from_u32(u32::from(high)).expect("non-surrogate BMP unit"),
                        2,
                    ),
                }
            }
            UnicodeBodyEncoding::Utf32Be | UnicodeBodyEncoding::Utf32Le => {
                let Some(bytes) = tail.get(..4) else {
                    return self.fail(UnicodeFault::TruncatedUnit);
                };
                let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
                let value = if self.encoding == UnicodeBodyEncoding::Utf32Le {
                    u32::from_le_bytes(bytes)
                } else {
                    u32::from_be_bytes(bytes)
                };
                let Some(value) = char::from_u32(value) else {
                    return self.fail(UnicodeFault::InvalidScalar(value));
                };
                (value, 4)
            }
        };
        let byte = self.offset;
        self.offset += size;
        Some(Ok(Scalar {
            byte,
            end: self.offset,
            value,
        }))
    }
}

/// Decode an explicitly ordered Unicode body. Initial U+FEFF is body content here;
/// intake must handle any selected signature before calling. Malformed units
/// report their original start, without recovery or surrogate reinterpretation.
pub fn decode_unicode_source(
    input: &[u8],
    encoding: UnicodeBodyEncoding,
    limits: DecodeLimits,
) -> Result<DecodedText, DecodeError> {
    let work = body_work(input.len(), limits)?;
    let scalars = Scalars {
        input,
        offset: 0,
        encoding,
        failed: false,
    };
    build_source(input.len(), scalars, limits, work)
}
