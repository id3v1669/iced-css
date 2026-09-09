//! HTTP Content-Type field-value syntax and actual charset presence. This does not
//! admit a representation as CSS or apply a default character encoding.
use super::http_syntax::{FieldError, Reader};
use std::{borrow::Cow, num::NonZeroUsize, ops::Range};

#[derive(Debug, Clone, Copy)]
pub struct ContentTypeLimits {
    /// Semicolon-delimited slots, including empty slots permitted by HTTP grammar.
    pub parameters: NonZeroUsize,
    /// Copied charset bytes; unescaped values and type tokens borrow their input.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug)]
struct Charset<'a> {
    value: Cow<'a, [u8]>,
    span: Range<usize>,
}
#[derive(Debug)]
pub struct HttpContentType<'a> {
    pub type_name: &'a [u8],
    pub subtype: &'a [u8],
    charset: Option<Charset<'a>>,
}
impl HttpContentType<'_> {
    pub fn charset(&self) -> Option<&[u8]> {
        self.charset.as_ref().map(|value| value.value.as_ref())
    }
    pub fn charset_span(&self) -> Option<Range<usize>> {
        self.charset.as_ref().map(|value| value.span.clone())
    }
    pub fn is_css(&self) -> bool {
        self.type_name.eq_ignore_ascii_case(b"text") && self.subtype.eq_ignore_ascii_case(b"css")
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum ContentTypeError {
    Invalid {
        offset: usize,
    },
    MultipleFields {
        count: usize,
    },
    DuplicateCharset {
        first: Range<usize>,
        second: Range<usize>,
    },
    Parameters,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for ContentTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP Content-Type: {self:?}")
    }
}
impl std::error::Error for ContentTypeError {}

/// Parse field values after HTTP framing. Envelope OWS is removed as specified
/// for field-line extraction. Multiple fields and duplicate charset parameters
/// are rejected by the selected project ambiguity policy, even if identical.
/// Unknown parameters are validated; HTTP quoted-pairs are not CSS escapes.
pub fn parse_content_type_fields<'a>(
    fields: &[&'a [u8]],
    limits: ContentTypeLimits,
) -> Result<Option<HttpContentType<'a>>, ContentTypeError> {
    if fields.is_empty() {
        return Ok(None);
    }
    if fields.len() != 1 {
        return Err(ContentTypeError::MultipleFields {
            count: fields.len(),
        });
    }
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(ContentTypeError::Bytes);
    }
    let input = fields[0];
    let mut reader = Reader {
        input,
        offset: 0,
        work: limits.work.get(),
    };
    reader.ows()?;
    let type_span = reader.token()?;
    reader.expect(b'/')?;
    let subtype_span = reader.token()?;
    let mut charset: Option<Charset<'a>> = None;
    let mut parameters = 0;
    loop {
        reader.ows()?;
        if reader.peek().is_none() {
            break;
        }
        if parameters == limits.parameters.get() {
            return Err(ContentTypeError::Parameters);
        }
        parameters += 1;
        reader.expect(b';')?;
        reader.ows()?;
        if matches!(reader.peek(), None | Some(b';')) {
            continue;
        }
        let name = reader.token()?;
        reader.expect(b'=')?;
        let value = reader.value()?;
        if input[name].eq_ignore_ascii_case(b"charset") {
            if let Some(first) = &charset {
                return Err(ContentTypeError::DuplicateCharset {
                    first: first.span.clone(),
                    second: value.span,
                });
            }
            let bytes = value.decode(&mut reader, limits.retained_bytes.get())?;
            charset = Some(Charset {
                value: bytes,
                span: value.span,
            });
        }
    }
    Ok(Some(HttpContentType {
        type_name: &input[type_span],
        subtype: &input[subtype_span],
        charset,
    }))
}
impl From<FieldError> for ContentTypeError {
    fn from(error: FieldError) -> Self {
        match error {
            FieldError::Invalid { offset } => Self::Invalid { offset },
            FieldError::Bytes => Self::Bytes,
            FieldError::Work => Self::Work,
            FieldError::Allocation => Self::Allocation,
        }
    }
}
