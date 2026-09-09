//! Fresh HTTP-only import collection. Caller authorization and publication are separate.
use super::{
    graph::GraphWorkerError,
    http::{DirectHttpTransport, HttpError},
    http_stylesheet::{
        HttpStylesheetError, HttpStylesheetReadError, HttpStylesheetReadLimits,
        HttpStylesheetSource, HttpStylesheetSubmitError, acquire_http_representation,
    },
    local::ReadBodyError,
    worker::FileReader,
};
use crate::host::stylesheet::graph::{GraphCompletion, GraphRequest};
use iced_css_core::{
    resources::{
        body::BodyError,
        uri::{UriError, UriLimits, UriReference, normalize_syntax},
    },
    stylesheet::{
        RuleContext,
        collector::{
            AcquiredStylesheet, CollectionError, CollectorLimits, CollectorRoot, ImportProbe,
            ImportProvider, ImportTarget, collect_imports,
        },
        identities::{IdentityError, ImportIdentityRegistry},
        located_graph::{BaseLimits, LocatedStylesheetGraph},
        uri_aliases::{ImportUriAliasError, ImportUriAliasLimits, ImportUriAliases},
    },
    syntax::encoding_selection::EncodingMetadata,
};
use std::{
    future::Future,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};
#[derive(Debug, Clone, Copy)]
pub struct HttpAttemptLimits {
    pub entries: NonZeroUsize,
    /// Conservative acquisition charges plus actual table/string capacities.
    pub retained_bytes: NonZeroUsize,
    /// Attempt table scans, copies, moves and content comparisons, across this graph.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpAttemptError {
    Entries,
    Bytes,
    Work,
    Allocation,
    Target,
    ConflictingRepresentation,
}
impl std::fmt::Display for HttpAttemptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP graph attempt: {self:?}")
    }
}
impl std::error::Error for HttpAttemptError {}
#[path = "http_attempts.rs"]
mod attempts;
pub use attempts::{HttpDependency, HttpDependencyReport};
#[derive(Debug, Clone, Copy)]
pub struct HttpGraphLimits {
    pub acquisition: HttpStylesheetReadLimits,
    pub attempts: HttpAttemptLimits,
    pub aliases: ImportUriAliasLimits,
    pub collector: CollectorLimits,
    pub bases: BaseLimits,
}
#[derive(Debug)]
pub enum HttpImportError {
    Uri(UriError),
    Namespace,
    Identity(IdentityError),
    Aliases(ImportUriAliasError),
    Attempt(HttpAttemptError),
    Acquisition(Arc<HttpStylesheetReadError>),
    Intake(HttpStylesheetError),
    Target,
}
impl From<UriError> for HttpImportError {
    fn from(e: UriError) -> Self {
        Self::Uri(e)
    }
}
impl From<IdentityError> for HttpImportError {
    fn from(e: IdentityError) -> Self {
        Self::Identity(e)
    }
}
impl From<ImportUriAliasError> for HttpImportError {
    fn from(e: ImportUriAliasError) -> Self {
        Self::Aliases(e)
    }
}
impl From<HttpAttemptError> for HttpImportError {
    fn from(e: HttpAttemptError) -> Self {
        Self::Attempt(e)
    }
}
impl std::fmt::Display for HttpImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP import: {self:?}")
    }
}
impl std::error::Error for HttpImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(e) => Some(e),
            Self::Identity(e) => Some(e),
            Self::Aliases(e) => Some(e),
            Self::Attempt(e) => Some(e),
            Self::Acquisition(e) => Some(e.as_ref()),
            Self::Intake(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub enum HttpGraphError {
    Input(HttpStylesheetSubmitError),
    IdentityPoisoned,
    Root(HttpImportError),
    Imports(CollectionError<HttpImportError>),
}
impl std::fmt::Display for HttpGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP stylesheet graph: {self:?}")
    }
}
impl std::error::Error for HttpGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(e) => Some(e),
            Self::Root(e) => Some(e),
            Self::Imports(e) => Some(e),
            Self::IdentityPoisoned => None,
        }
    }
}
struct Provider<'a> {
    transport: &'a DirectHttpTransport,
    identities: &'a mut ImportIdentityRegistry,
    context: RuleContext,
    limits: HttpGraphLimits,
    aliases: ImportUriAliases,
    report: HttpDependencyReport,
}
impl Provider<'_> {
    fn ensure(&mut self, reference: &str) -> Result<usize, HttpImportError> {
        let key = normalized_key(reference, self.limits.acquisition.http.uri)?;
        if let Some(final_key) = self.aliases.lookup(&key)? {
            return self
                .report
                .find_final(final_key)?
                .ok_or(HttpAttemptError::Target.into());
        }
        if let Some(index) = self.report.find_requested(&key)? {
            return Ok(index);
        }
        let index = self
            .report
            .record(reference, key, packet_charge(self.limits.acquisition)?)?;
        let request = self
            .report
            .request_copy(index, self.limits.acquisition.http.uri.bytes)?;
        let result = acquire_http_representation(self.transport, request, self.limits.acquisition);
        self.report.finish(index, result)?;
        let Ok(representation) = self.report.result(index)? else {
            return Ok(index);
        };
        let final_key = normalized_key(
            &representation.metadata().effective_uri,
            self.limits.acquisition.http.uri,
        )?;
        let canonical = self.report.set_final(index, final_key)?;
        let target = self.report.key(canonical)?;
        bind_alias(
            &mut self.aliases,
            target,
            reference,
            self.limits.acquisition.http.uri,
        )?;
        for hop in &representation.metadata().redirect_chain {
            bind_alias(
                &mut self.aliases,
                target,
                hop.source_uri(),
                self.limits.acquisition.http.uri,
            )?;
            bind_alias(
                &mut self.aliases,
                target,
                hop.target_reference_uri(),
                self.limits.acquisition.http.uri,
            )?;
        }
        bind_alias(
            &mut self.aliases,
            target,
            &representation.metadata().effective_uri,
            self.limits.acquisition.http.uri,
        )?;
        Ok(canonical)
    }
    fn target(
        &mut self,
        index: usize,
        linking: Option<&[u8]>,
        referring: Option<&[u8]>,
    ) -> Result<ImportTarget, HttpImportError> {
        let result = self.report.result(index)?;
        let transport = result
            .as_ref()
            .ok()
            .and_then(|value| value.transport_charset());
        Ok(self.identities.identify(
            self.report.key(index)?,
            EncodingMetadata {
                transport,
                linking,
                referring,
            },
            self.context,
        )?)
    }
    fn read(
        &mut self,
        index: usize,
        linking: Option<&[u8]>,
        referring: Option<&[u8]>,
    ) -> Result<AcquiredStylesheet, HttpImportError> {
        let representation = self
            .report
            .result(index)?
            .map_err(HttpImportError::Acquisition)?;
        let candidate = representation
            .parse_context(
                linking,
                referring,
                self.context,
                self.limits.acquisition.stylesheet.intake,
            )
            .map_err(HttpImportError::Intake)?;
        let effective_base = self.report.text_copy(
            &representation.metadata().effective_uri,
            self.limits.acquisition.http.uri.bytes,
        )?;
        Ok(AcquiredStylesheet {
            candidate,
            effective_base,
        })
    }
    fn root(&mut self, source: &HttpStylesheetSource) -> Result<CollectorRoot, HttpImportError> {
        let index = self.ensure(&source.uri)?;
        let target = self.target(
            index,
            source.encoding.linking.as_deref(),
            source.encoding.referring.as_deref(),
        )?;
        let input = self.read(
            index,
            source.encoding.linking.as_deref(),
            source.encoding.referring.as_deref(),
        )?;
        Ok(CollectorRoot { target, input })
    }
}
impl ImportProvider for Provider<'_> {
    type Error = HttpImportError;
    fn identify(&mut self, probe: ImportProbe<'_>) -> Result<ImportTarget, Self::Error> {
        let index = self.ensure(probe.request.reference_uri())?;
        self.target(index, probe.encoding.linking, probe.encoding.referring)
    }
    fn acquire(
        &mut self,
        target: ImportTarget,
        probe: ImportProbe<'_>,
    ) -> Result<Option<AcquiredStylesheet>, Self::Error> {
        let index = self.ensure(probe.request.reference_uri())?;
        if self.target(index, probe.encoding.linking, probe.encoding.referring)? != target {
            return Err(HttpImportError::Target);
        }
        if let Err(error) = self.report.result(index)? {
            return if unavailable(&error) {
                Ok(None)
            } else {
                Err(HttpImportError::Acquisition(error))
            };
        }
        self.read(index, probe.encoding.linking, probe.encoding.referring)
            .map(Some)
    }
}
fn unavailable(error: &HttpStylesheetReadError) -> bool {
    matches!(
        error,
        HttpStylesheetReadError::Transport(
            HttpError::Request(_)
                | HttpError::Read(
                    ReadBodyError::Io(_)
                        | ReadBodyError::Body(BodyError::Incomplete | BodyError::Transport)
                )
        ) | HttpStylesheetReadError::Stylesheet(
            HttpStylesheetError::Status { .. } | HttpStylesheetError::Media(_)
        )
    )
}
fn normalized_key(reference: &str, limits: UriLimits) -> Result<String, HttpImportError> {
    let parsed = UriReference::parse(reference, limits)?;
    if !parsed.scheme().is_some_and(|scheme| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    }) {
        return Err(HttpImportError::Namespace);
    }
    Ok(normalize_syntax(parsed.without_fragment(), limits)?)
}
fn bind_alias(
    aliases: &mut ImportUriAliases,
    target: &str,
    reference: &str,
    limits: UriLimits,
) -> Result<(), HttpImportError> {
    let alias = normalized_key(reference, limits)?;
    if alias == target {
        aliases.bind(target, &[])?;
    } else {
        aliases.bind(target, &[&alias])?;
    }
    Ok(())
}
fn packet_charge(limits: HttpStylesheetReadLimits) -> Result<usize, HttpAttemptError> {
    [
        limits.http.read.body.retained_bytes.get(),
        limits.stylesheet.content.output.retained_bytes.get(),
        limits.http.header_bytes.get(),
        limits.http.uri.bytes.get(),
        limits.redirects.retained_bytes.get(),
        limits.stylesheet.transport_charset_bytes.get(),
        size_of::<super::http_stylesheet::HttpStylesheetRepresentation>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| sum.checked_add(value))
    .ok_or(HttpAttemptError::Bytes)
}
pub type HttpGraphCompletion =
    GraphCompletion<GraphWorkerError<HttpGraphError>, LocatedStylesheetGraph>;
pub(crate) fn validate_http_graph_source(
    source: &HttpStylesheetSource,
    limits: HttpGraphLimits,
) -> Result<(), HttpStylesheetSubmitError> {
    if source.uri.capacity() > limits.acquisition.http.uri.bytes.get()
        || source.uri.capacity() > limits.acquisition.redirects.retained_bytes.get()
    {
        Err(HttpStylesheetSubmitError::UriBytes)
    } else {
        super::http_stylesheet::validate_encoding(
            &source.encoding,
            limits.acquisition.stylesheet.metadata_bytes,
        )
    }
}
impl FileReader {
    /// Collect a fresh already-authorized HTTP graph on one resource worker.
    /// The attempt table starts empty; no previous stylesheet/import is consulted.
    /// Returned evidence is paired to the original request, before owner admission.
    pub fn load_http_stylesheet_graph(
        &self,
        request: GraphRequest,
        transport: Arc<DirectHttpTransport>,
        identities: Arc<Mutex<ImportIdentityRegistry>>,
        context: RuleContext,
        source: HttpStylesheetSource,
        limits: HttpGraphLimits,
    ) -> impl Future<Output = (HttpGraphCompletion, Option<HttpDependencyReport>)> + Send + 'static + use<>
    {
        self.load_shared_http_stylesheet_graph(
            request,
            transport,
            identities,
            context,
            Arc::new(source),
            limits,
        )
    }
    pub(crate) fn load_shared_http_stylesheet_graph(
        &self,
        request: GraphRequest,
        transport: Arc<DirectHttpTransport>,
        identities: Arc<Mutex<ImportIdentityRegistry>>,
        context: RuleContext,
        source: Arc<HttpStylesheetSource>,
        limits: HttpGraphLimits,
    ) -> impl Future<Output = (HttpGraphCompletion, Option<HttpDependencyReport>)> + Send + 'static + use<>
    {
        let input = validate_http_graph_source(&source, limits);
        let work = match input {
            Err(error) => Err((
                request.assemble_located(
                    Err(GraphWorkerError::Acquisition(HttpGraphError::Input(error))),
                    limits.bases,
                ),
                None,
            )),
            Ok(()) => Ok(self.load_located_stylesheet_graph_with_report(
                request,
                limits.bases,
                move || {
                    let report = HttpDependencyReport::new(limits.attempts);
                    let mut identities = match identities.lock() {
                        Ok(value) => value,
                        Err(_) => return (Err(HttpGraphError::IdentityPoisoned), report),
                    };
                    let aliases = match ImportUriAliases::new(limits.aliases) {
                        Ok(value) => value,
                        Err(error) => {
                            return (
                                Err(HttpGraphError::Root(HttpImportError::Aliases(error))),
                                report,
                            );
                        }
                    };
                    let mut provider = Provider {
                        transport: &transport,
                        identities: &mut identities,
                        context,
                        limits,
                        aliases,
                        report,
                    };
                    let result = provider
                        .root(&source)
                        .map_err(HttpGraphError::Root)
                        .and_then(|root| {
                            collect_imports(root, &mut provider, limits.collector)
                                .map_err(HttpGraphError::Imports)
                        })
                        .map(Into::into);
                    (result, provider.report)
                },
            )),
        };
        async move {
            match work {
                Ok(work) => work.await,
                Err(completion) => completion,
            }
        }
    }
}
