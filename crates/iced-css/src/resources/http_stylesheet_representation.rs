//! One admitted acquisition's bytes, reusable across CSS decoding contexts.
use super::{
    AdmittedHttpStylesheet, HttpStylesheetEncoding, HttpStylesheetError, HttpStylesheetLimits,
    HttpStylesheetSubmitError, ParsedHttpStylesheet, PreparedHttpRepresentation, prepare_response,
    validate_encoding, validate_response,
};
use crate::resources::{
    http::{HttpResponse, HttpResponseMetadata},
    worker::FileReader,
};
use iced_css_core::{
    resources::{body::CompletedBody, http_admission::StylesheetMediaPolicy},
    stylesheet::{RuleContext, StylesheetIntakeLimits, UnicodeStylesheetCandidate},
};
use std::{future::Future, num::NonZeroUsize, sync::Arc};
/// Fresh GET status/media and content coding admitted, characters not yet decoded.
/// Sharing this owner is not transport-cache reuse or a refresh authorization.
/// Coded response metadata is preserved beside explicitly content-decoded bytes.
pub struct HttpStylesheetRepresentation(PreparedHttpRepresentation);
impl HttpStylesheetRepresentation {
    /// Internal caller supplies admitted borrowed labels and its own context budget.
    pub(in crate::resources) fn parse_context(
        &self,
        linking: Option<&[u8]>,
        referring: Option<&[u8]>,
        context: RuleContext,
        limits: StylesheetIntakeLimits,
    ) -> Result<UnicodeStylesheetCandidate, HttpStylesheetError> {
        self.0.parse_context(linking, referring, context, limits)
    }
    pub(super) fn into_stylesheet(
        self,
        encoding: HttpStylesheetEncoding,
        context: RuleContext,
        limits: StylesheetIntakeLimits,
    ) -> Result<AdmittedHttpStylesheet, HttpStylesheetError> {
        let stylesheet = self.parse_context(
            encoding.linking.as_deref(),
            encoding.referring.as_deref(),
            context,
            limits,
        )?;
        Ok(AdmittedHttpStylesheet(ParsedHttpStylesheet {
            stylesheet,
            metadata: self.0.metadata,
        }))
    }

    pub fn metadata(&self) -> &HttpResponseMetadata {
        &self.0.metadata
    }
    pub fn content_decoded_body(&self) -> &CompletedBody {
        &self.0.body
    }
    pub fn transport_charset(&self) -> Option<&[u8]> {
        self.0.transport_charset.as_deref()
    }
    pub fn into_parts(self) -> (HttpResponseMetadata, CompletedBody, Option<Vec<u8>>) {
        (self.0.metadata, self.0.body, self.0.transport_charset)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct HttpStylesheetContextLimits {
    /// Capacity of the already content-decoded representation, not its coded body.
    pub input_bytes: NonZeroUsize,
    /// Combined owned external linking/referring capacities for this job.
    pub metadata_bytes: NonZeroUsize,
    pub intake: StylesheetIntakeLimits,
}
impl FileReader {
    /// Prepare one already acquired fresh GET representation on a worker.
    /// Invalid status/media or incomplete content coding returns no representation.
    pub fn decode_http_stylesheet_response(
        &self,
        response: HttpResponse,
        policy: StylesheetMediaPolicy,
        limits: HttpStylesheetLimits,
    ) -> Result<
        impl Future<Output = Result<HttpStylesheetRepresentation, HttpStylesheetError>>
        + Send
        + 'static
        + use<>,
        HttpStylesheetSubmitError,
    > {
        validate_response(&response, limits)?;
        let work = self
            .submit(move || prepare_admitted(response, policy, limits))
            .map_err(HttpStylesheetSubmitError::Worker)?;
        Ok(async move { work.await.map_err(HttpStylesheetError::Worker)? })
    }
    /// Parse a shared representation in an independent CSS context without HTTP
    /// I/O or content re-decoding. Each parse has its own worker/intake allowances;
    /// the representation owner needs separate aggregate retained-storage admission.
    pub fn parse_http_stylesheet_representation(
        &self,
        representation: Arc<HttpStylesheetRepresentation>,
        encoding: HttpStylesheetEncoding,
        context: RuleContext,
        limits: HttpStylesheetContextLimits,
    ) -> Result<
        impl Future<Output = Result<UnicodeStylesheetCandidate, HttpStylesheetError>>
        + Send
        + 'static
        + use<>,
        HttpStylesheetSubmitError,
    > {
        validate_encoding(&encoding, limits.metadata_bytes)?;
        if representation.0.body.retained_bytes() > limits.input_bytes.get() {
            return Err(HttpStylesheetSubmitError::InputBytes);
        }
        let work = self
            .submit(move || {
                representation.parse_context(
                    encoding.linking.as_deref(),
                    encoding.referring.as_deref(),
                    context,
                    limits.intake,
                )
            })
            .map_err(HttpStylesheetSubmitError::Worker)?;
        Ok(async move { work.await.map_err(HttpStylesheetError::Worker)? })
    }
}

// The resource-layer caller must admit response storage before this operation.
pub(super) fn prepare_admitted(
    response: HttpResponse,
    policy: StylesheetMediaPolicy,
    limits: HttpStylesheetLimits,
) -> Result<HttpStylesheetRepresentation, HttpStylesheetError> {
    prepare_response(response, limits, Some(policy)).map(HttpStylesheetRepresentation)
}
