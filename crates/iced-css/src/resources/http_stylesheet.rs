//! Parsing and optional fresh GET/media admission of an acquired response.
use super::{
    content_decoding::{ContentDecodeError, ContentDecodeLimits, decode_content_codings},
    http::{HttpResponse, HttpResponseMetadata},
    worker::{FileReader, SubmitError, WorkerFailure},
};
use iced_css_core::{
    resources::{
        body::CompletedBody,
        content_codings::{ContentCodingError, ContentCodingLimits, parse_content_coding_fields},
        http_admission::{
            GetResponseKind, StylesheetMediaError, StylesheetMediaPolicy, admit_stylesheet_media,
            classify_get_response,
        },
        http_metadata::{ContentTypeError, ContentTypeLimits, parse_content_type_fields},
    },
    stylesheet::{
        RuleContext, StylesheetIntakeError, StylesheetIntakeLimits, UnicodeStylesheetCandidate,
        parse_separate_unicode_stylesheet,
    },
    syntax::encoding_selection::EncodingMetadata,
};
use reqwest::header::{CONTENT_ENCODING, CONTENT_TYPE, HeaderMap};
use std::{future::Future, num::NonZeroUsize};
/// Transport charset is supplied only by received Content-Type fields.
#[derive(Debug, Default)]
pub struct HttpStylesheetEncoding {
    pub linking: Option<Vec<u8>>,
    pub referring: Option<Vec<u8>>,
}
#[derive(Debug, Clone, Copy)]
pub struct HttpStylesheetLimits {
    /// Actual capacity of the retained, extracted transport charset label.
    pub transport_charset_bytes: NonZeroUsize,
    /// Received field values; transport header/URI storage retains its own limits.
    pub header_fields: NonZeroUsize,
    /// Actual capacity of temporary borrowed Content-Encoding field references.
    pub header_reference_bytes: NonZeroUsize,
    /// Combined owned linking/referring label capacities.
    pub metadata_bytes: NonZeroUsize,
    pub content_type: ContentTypeLimits,
    pub codings: ContentCodingLimits,
    pub content: ContentDecodeLimits,
    pub intake: StylesheetIntakeLimits,
}
#[derive(Debug)]
pub enum HttpStylesheetSubmitError {
    UriBytes,
    Headers,
    Metadata,
    InputBytes,
    Worker(SubmitError),
}
impl std::fmt::Display for HttpStylesheetSubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP stylesheet submission: {self:?}")
    }
}
impl std::error::Error for HttpStylesheetSubmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Worker(error) => Some(error),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub enum HttpStylesheetError {
    TransportCharsetBytes,
    Status {
        received: u16,
        kind: GetResponseKind,
    },
    Media(StylesheetMediaError),
    Worker(WorkerFailure),
    ContentType(ContentTypeError),
    Codings(ContentCodingError),
    Content(ContentDecodeError),
    Intake(StylesheetIntakeError),
    HeaderReferences,
    Allocation,
}
impl std::fmt::Display for HttpStylesheetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP stylesheet parsing: {self:?}")
    }
}
impl std::error::Error for HttpStylesheetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Media(e) => Some(e),
            Self::Worker(e) => Some(e),
            Self::ContentType(e) => Some(e),
            Self::Codings(e) => Some(e),
            Self::Content(e) => Some(e),
            Self::Intake(e) => Some(e),
            _ => None,
        }
    }
}
/// Parsing success does not admit response status, MIME or cache semantics.
/// Source byte positions refer to content-decoded representation bytes.
pub struct ParsedHttpStylesheet {
    stylesheet: UnicodeStylesheetCandidate,
    metadata: HttpResponseMetadata,
}
impl ParsedHttpStylesheet {
    pub fn stylesheet(&self) -> &UnicodeStylesheetCandidate {
        &self.stylesheet
    }
    pub fn metadata(&self) -> &HttpResponseMetadata {
        &self.metadata
    }
    pub fn into_parts(self) -> (UnicodeStylesheetCandidate, HttpResponseMetadata) {
        (self.stylesheet, self.metadata)
    }
}
/// A fresh GET representation admitted under an explicit media policy and parsed
/// completely. This is not a cache entry, refresh authorization or published graph.
pub struct AdmittedHttpStylesheet(ParsedHttpStylesheet);
impl AdmittedHttpStylesheet {
    pub fn stylesheet(&self) -> &UnicodeStylesheetCandidate {
        self.0.stylesheet()
    }
    pub fn metadata(&self) -> &HttpResponseMetadata {
        self.0.metadata()
    }
    pub fn into_parts(self) -> (UnicodeStylesheetCandidate, HttpResponseMetadata) {
        self.0.into_parts()
    }
}
impl FileReader {
    /// Admit fresh GET status and media before content/character parsing, on one
    /// worker. No 304 merge, partial-body reconstruction, network I/O or retry.
    pub fn admit_http_stylesheet_response(
        &self,
        response: HttpResponse,
        encoding: HttpStylesheetEncoding,
        context: RuleContext,
        policy: StylesheetMediaPolicy,
        limits: HttpStylesheetLimits,
    ) -> Result<
        impl Future<Output = Result<AdmittedHttpStylesheet, HttpStylesheetError>>
        + Send
        + 'static
        + use<>,
        HttpStylesheetSubmitError,
    > {
        let work =
            self.submit_http_stylesheet(response, encoding, context, limits, Some(policy))?;
        Ok(async move { work.await.map(AdmittedHttpStylesheet) })
    }
    /// Parse one acquired response on a resource worker. The caller retains
    /// responsibility for HTTP/MIME/cache admission before using it as CSS.
    /// This consumes input even on submission failure and never reacquires it.
    pub fn parse_http_stylesheet_response(
        &self,
        response: HttpResponse,
        encoding: HttpStylesheetEncoding,
        context: RuleContext,
        limits: HttpStylesheetLimits,
    ) -> Result<
        impl Future<Output = Result<ParsedHttpStylesheet, HttpStylesheetError>> + Send + 'static + use<>,
        HttpStylesheetSubmitError,
    > {
        self.submit_http_stylesheet(response, encoding, context, limits, None)
    }
    fn submit_http_stylesheet(
        &self,
        response: HttpResponse,
        encoding: HttpStylesheetEncoding,
        context: RuleContext,
        limits: HttpStylesheetLimits,
        policy: Option<StylesheetMediaPolicy>,
    ) -> Result<
        impl Future<Output = Result<ParsedHttpStylesheet, HttpStylesheetError>> + Send + 'static + use<>,
        HttpStylesheetSubmitError,
    > {
        validate_encoding(&encoding, limits.metadata_bytes)?;
        validate_response(&response, limits)?;
        let work = self
            .submit(move || parse_response(response, encoding, context, limits, policy))
            .map_err(HttpStylesheetSubmitError::Worker)?;
        Ok(async move { work.await.map_err(HttpStylesheetError::Worker)? })
    }
}
pub(super) fn validate_encoding(
    encoding: &HttpStylesheetEncoding,
    maximum: NonZeroUsize,
) -> Result<(), HttpStylesheetSubmitError> {
    let metadata = [&encoding.linking, &encoding.referring]
        .into_iter()
        .try_fold(0usize, |sum, label| {
            sum.checked_add(label.as_ref().map_or(0, Vec::capacity))
        });
    if metadata.is_none_or(|bytes| bytes > maximum.get()) {
        return Err(HttpStylesheetSubmitError::Metadata);
    }
    Ok(())
}
fn validate_response(
    response: &HttpResponse,
    limits: HttpStylesheetLimits,
) -> Result<(), HttpStylesheetSubmitError> {
    let mut fields = 0usize;
    for _ in response.headers().values() {
        fields = fields
            .checked_add(1)
            .filter(|count| *count <= limits.header_fields.get())
            .ok_or(HttpStylesheetSubmitError::Headers)?;
    }
    if response.body().retained_bytes() > limits.content.input_bytes.get() {
        return Err(HttpStylesheetSubmitError::InputBytes);
    }
    Ok(())
}
struct PreparedHttpRepresentation {
    metadata: HttpResponseMetadata,
    body: CompletedBody,
    transport_charset: Option<Vec<u8>>,
}
impl PreparedHttpRepresentation {
    fn parse_context(
        &self,
        linking: Option<&[u8]>,
        referring: Option<&[u8]>,
        context: RuleContext,
        limits: StylesheetIntakeLimits,
    ) -> Result<UnicodeStylesheetCandidate, HttpStylesheetError> {
        parse_separate_unicode_stylesheet(
            self.body.as_slice(),
            EncodingMetadata {
                transport: self.transport_charset.as_deref(),
                linking,
                referring,
            },
            context,
            limits,
        )
        .map_err(HttpStylesheetError::Intake)
    }
}
fn copy_transport_charset(
    label: Option<&[u8]>,
    maximum: NonZeroUsize,
) -> Result<Option<Vec<u8>>, HttpStylesheetError> {
    if maximum.get() > isize::MAX as usize {
        return Err(HttpStylesheetError::TransportCharsetBytes);
    }
    let Some(label) = label else {
        return Ok(None);
    };
    if label.len() > maximum.get() {
        return Err(HttpStylesheetError::TransportCharsetBytes);
    }
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(label.len())
        .map_err(|_| HttpStylesheetError::Allocation)?;
    if owned.capacity() > maximum.get() {
        return Err(HttpStylesheetError::TransportCharsetBytes);
    }
    owned.extend_from_slice(label);
    Ok(Some(owned))
}
fn prepare_response(
    response: HttpResponse,
    limits: HttpStylesheetLimits,
    policy: Option<StylesheetMediaPolicy>,
) -> Result<PreparedHttpRepresentation, HttpStylesheetError> {
    if policy.is_some() {
        let kind = classify_get_response(response.status());
        if !kind.has_complete_representation() {
            return Err(HttpStylesheetError::Status {
                received: response.status(),
                kind,
            });
        }
    }
    let (metadata, body) = response.into_parts();
    let (body, transport_charset) = {
        let count = metadata.headers.get_all(CONTENT_TYPE).iter().count();
        if count > 1 {
            return Err(HttpStylesheetError::ContentType(
                ContentTypeError::MultipleFields { count },
            ));
        }
        let single = metadata
            .headers
            .get(CONTENT_TYPE)
            .map(|value| [value.as_bytes()]);
        let content_type = parse_content_type_fields(
            single.as_ref().map_or(&[][..], |fields| fields.as_slice()),
            limits.content_type,
        )
        .map_err(HttpStylesheetError::ContentType)?;
        if let Some(policy) = policy {
            admit_stylesheet_media(content_type.as_ref(), policy)
                .map_err(HttpStylesheetError::Media)?;
        }
        let transport_charset = copy_transport_charset(
            content_type.as_ref().and_then(|value| value.charset()),
            limits.transport_charset_bytes,
        )?;
        let fields = coding_fields(&metadata.headers, limits)?;
        let codings = parse_content_coding_fields(&fields, limits.codings)
            .map_err(HttpStylesheetError::Codings)?;
        let decoded = decode_content_codings(body, &codings, limits.content)
            .map_err(HttpStylesheetError::Content)?;
        (decoded, transport_charset)
    };
    Ok(PreparedHttpRepresentation {
        metadata,
        body,
        transport_charset,
    })
}
fn parse_response(
    response: HttpResponse,
    encoding: HttpStylesheetEncoding,
    context: RuleContext,
    limits: HttpStylesheetLimits,
    policy: Option<StylesheetMediaPolicy>,
) -> Result<ParsedHttpStylesheet, HttpStylesheetError> {
    let prepared = prepare_response(response, limits, policy)?;
    let stylesheet = prepared.parse_context(
        encoding.linking.as_deref(),
        encoding.referring.as_deref(),
        context,
        limits.intake,
    )?;
    Ok(ParsedHttpStylesheet {
        stylesheet,
        metadata: prepared.metadata,
    })
}
fn coding_fields(
    headers: &HeaderMap,
    limits: HttpStylesheetLimits,
) -> Result<Vec<&[u8]>, HttpStylesheetError> {
    let count = headers.get_all(CONTENT_ENCODING).iter().count();
    if count > limits.codings.fields.get() {
        return Err(HttpStylesheetError::Codings(ContentCodingError::Fields));
    }
    let maximum = limits.header_reference_bytes.get();
    if maximum > isize::MAX as usize
        || count
            .checked_mul(size_of::<&[u8]>())
            .is_none_or(|bytes| bytes > maximum)
    {
        return Err(HttpStylesheetError::HeaderReferences);
    }
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(count)
        .map_err(|_| HttpStylesheetError::Allocation)?;
    if fields
        .capacity()
        .checked_mul(size_of::<&[u8]>())
        .is_none_or(|bytes| bytes > maximum)
    {
        return Err(HttpStylesheetError::HeaderReferences);
    }
    fields.extend(
        headers
            .get_all(CONTENT_ENCODING)
            .iter()
            .map(|value| value.as_bytes()),
    );
    Ok(fields)
}

#[path = "http_stylesheet_acquisition.rs"]
mod acquisition;
pub use acquisition::{HttpStylesheetReadError, HttpStylesheetReadLimits, HttpStylesheetSource};

#[path = "http_stylesheet_representation.rs"]
mod representation;
pub use representation::{HttpStylesheetContextLimits, HttpStylesheetRepresentation};

pub(super) use acquisition::acquire_http_representation;
