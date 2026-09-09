//! Already-authorized GET chain and CSS admission under one resource-worker lease.
use super::{
    AdmittedHttpStylesheet, HttpStylesheetEncoding, HttpStylesheetError, HttpStylesheetLimits,
    HttpStylesheetRepresentation, HttpStylesheetSubmitError, representation::prepare_admitted,
    validate_encoding, validate_response,
};
use crate::resources::{
    http::{DirectHttpTransport, HttpError, HttpLimits, RedirectLimits},
    worker::{FileReader, WorkerFailure},
};
use iced::futures::executor::block_on;
use iced_css_core::{resources::http_admission::StylesheetMediaPolicy, stylesheet::RuleContext};
use std::{future::Future, sync::Arc};
#[derive(Debug)]
pub struct HttpStylesheetSource {
    /// Absolute HTTP(S) reference, including authored fragment provenance.
    pub uri: String,
    pub encoding: HttpStylesheetEncoding,
}
#[derive(Debug, Clone, Copy)]
pub struct HttpStylesheetReadLimits {
    pub http: HttpLimits,
    pub redirects: RedirectLimits,
    pub stylesheet: HttpStylesheetLimits,
    /// Required choice; no MIME or charset default is inferred by acquisition.
    pub media: StylesheetMediaPolicy,
}
#[derive(Debug)]
pub enum HttpStylesheetReadError {
    Worker(WorkerFailure),
    Transport(HttpError),
    Input(HttpStylesheetSubmitError),
    Stylesheet(HttpStylesheetError),
}
impl std::fmt::Display for HttpStylesheetReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP stylesheet acquisition: {self:?}")
    }
}
impl std::error::Error for HttpStylesheetReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Worker(e) => e,
            Self::Transport(e) => e,
            Self::Input(e) => e,
            Self::Stylesheet(e) => e,
        })
    }
}
impl FileReader {
    /// Acquire and content-decode one already-authorized HTTP stylesheet, leaving
    /// linking/referring interpretation to a later bounded context operation.
    pub fn read_http_stylesheet_representation(
        &self,
        transport: Arc<DirectHttpTransport>,
        uri: String,
        limits: HttpStylesheetReadLimits,
    ) -> Result<
        impl Future<Output = Result<HttpStylesheetRepresentation, HttpStylesheetReadError>>
        + Send
        + 'static
        + use<>,
        HttpStylesheetSubmitError,
    > {
        validate_uri_capacity(uri.capacity(), limits)?;
        let work = self
            .submit(move || acquire_http_representation(&transport, uri, limits))
            .map_err(HttpStylesheetSubmitError::Worker)?;
        Ok(async move { work.await.map_err(HttpStylesheetReadError::Worker)? })
    }

    /// Acquire a fresh, already-authorized GET representation and parse it under
    /// one resource lease. That worker waits on the separate HTTP actor; callers
    /// need no Tokio runtime. No nested resource job, retry, cache or publication.
    /// Dropping a started operation does not forcibly interrupt its HTTP exchange.
    pub fn read_http_stylesheet(
        &self,
        transport: Arc<DirectHttpTransport>,
        source: HttpStylesheetSource,
        context: RuleContext,
        limits: HttpStylesheetReadLimits,
    ) -> Result<
        impl Future<Output = Result<AdmittedHttpStylesheet, HttpStylesheetReadError>>
        + Send
        + 'static
        + use<>,
        HttpStylesheetSubmitError,
    > {
        validate_uri_capacity(source.uri.capacity(), limits)?;
        validate_encoding(&source.encoding, limits.stylesheet.metadata_bytes)?;
        let work = self
            .submit(move || {
                acquire_http_representation(&transport, source.uri, limits)?
                    .into_stylesheet(source.encoding, context, limits.stylesheet.intake)
                    .map_err(HttpStylesheetReadError::Stylesheet)
            })
            .map_err(HttpStylesheetSubmitError::Worker)?;
        Ok(async move { work.await.map_err(HttpStylesheetReadError::Worker)? })
    }
}

fn validate_uri_capacity(
    capacity: usize,
    limits: HttpStylesheetReadLimits,
) -> Result<(), HttpStylesheetSubmitError> {
    if capacity > limits.http.uri.bytes.get() || capacity > limits.redirects.retained_bytes.get() {
        return Err(HttpStylesheetSubmitError::UriBytes);
    }
    Ok(())
}
/// Only call within an already-admitted resource job; this waits for the HTTP actor.
pub(in crate::resources) fn acquire_http_representation(
    transport: &DirectHttpTransport,
    uri: String,
    limits: HttpStylesheetReadLimits,
) -> Result<HttpStylesheetRepresentation, HttpStylesheetReadError> {
    validate_uri_capacity(uri.capacity(), limits).map_err(HttpStylesheetReadError::Input)?;
    let exchange = transport
        .get_following_redirects(uri, limits.http, limits.redirects)
        .map_err(HttpStylesheetReadError::Transport)?;
    let response = block_on(exchange).map_err(HttpStylesheetReadError::Transport)?;
    validate_response(&response, limits.stylesheet).map_err(HttpStylesheetReadError::Input)?;
    prepare_admitted(response, limits.media, limits.stylesheet)
        .map_err(HttpStylesheetReadError::Stylesheet)
}
