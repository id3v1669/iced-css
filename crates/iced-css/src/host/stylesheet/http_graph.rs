//! Runtime HTTP graph ownership through the existing publication gate.
use super::graph::{GraphLoadError, GraphRequest, RuntimeLocatedStylesheetGraph};
use crate::resources::{
    graph::GraphWorkerError,
    http::DirectHttpTransport,
    http_graph::{
        HttpDependencyReport, HttpGraphCompletion, HttpGraphError, HttpGraphLimits,
        validate_http_graph_source,
    },
    http_stylesheet::HttpStylesheetSource,
    worker::FileReader,
};
use iced_css_core::{
    resources::{
        publication::{PublicationError, PublicationLimits, PublicationStatus, ResourceRevision},
        triggers::{RequestError, RequestReason, ResourceKind, RuntimeMode, request_allowed},
    },
    stylesheet::{
        RuleContext,
        identities::{IdentityError, IdentityLimits, ImportIdentityRegistry},
        located_graph::LocatedStylesheetGraph,
        owned_graph::OwnedGraphLimits,
        publication::SheetCompletion,
    },
};
use std::{
    future::Future,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};
#[derive(Debug, Clone, Copy)]
pub struct HttpRuntimeLimits {
    pub acquisition: HttpGraphLimits,
    pub graph: OwnedGraphLimits,
}
#[derive(Debug)]
pub struct HttpGraphConfiguration {
    source: Arc<HttpStylesheetSource>,
    limits: HttpRuntimeLimits,
}
impl HttpGraphConfiguration {
    pub fn source(&self) -> &HttpStylesheetSource {
        &self.source
    }
    pub fn limits(&self) -> HttpRuntimeLimits {
        self.limits
    }
}
pub struct HttpRuntimeCompletion {
    inner: HttpGraphCompletion,
    dependencies: Option<HttpDependencyReport>,
}
pub type HttpGraphLoadError = GraphLoadError<GraphWorkerError<HttpGraphError>>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpGraphRetryError {
    MissingConfiguration,
    Request(RequestError),
}
impl std::fmt::Display for HttpGraphRetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "http graph retry: {self:?}")
    }
}
impl std::error::Error for HttpGraphRetryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Request(error) => Some(error),
            Self::MissingConfiguration => None,
        }
    }
}
/// Owns a fixed HTTP transport and persistent source identities. Registry
/// locking, HTTP acquisition and CSS parsing happen only on resource workers.
/// The application still owns delivery/queueing of these non-Clone completions.
pub struct RuntimeHttpStylesheetGraph {
    reader: FileReader,
    transport: Arc<DirectHttpTransport>,
    graph: RuntimeLocatedStylesheetGraph,
    identities: Arc<Mutex<ImportIdentityRegistry>>,
    mode: RuntimeMode,
    context: RuleContext,
    configuration: Option<HttpGraphConfiguration>,
    active_dependencies: Option<HttpDependencyReport>,
    candidate_dependencies: Option<HttpDependencyReport>,
}
impl RuntimeHttpStylesheetGraph {
    pub fn new(
        reader: FileReader,
        transport: Arc<DirectHttpTransport>,
        mode: RuntimeMode,
        context: RuleContext,
        publication: PublicationLimits,
        diagnostic_work: NonZeroUsize,
        identities: IdentityLimits,
    ) -> Result<Self, IdentityError> {
        Ok(Self {
            reader,
            transport,
            graph: RuntimeLocatedStylesheetGraph::new(publication, diagnostic_work),
            identities: Arc::new(Mutex::new(ImportIdentityRegistry::new(identities)?)),
            mode,
            context,
            configuration: None,
            active_dependencies: None,
            candidate_dependencies: None,
        })
    }
    pub fn active(&self) -> Option<&LocatedStylesheetGraph> {
        self.graph.active()
    }
    pub fn active_revision(&self) -> Option<ResourceRevision> {
        self.graph.active_revision()
    }
    pub fn status(&self) -> PublicationStatus {
        self.graph.status()
    }
    pub fn configuration(&self) -> Option<&HttpGraphConfiguration> {
        self.configuration.as_ref()
    }
    pub fn active_dependencies(&self) -> Option<&HttpDependencyReport> {
        self.active_dependencies.as_ref()
    }
    pub fn candidate_dependencies(&self) -> Option<&HttpDependencyReport> {
        self.candidate_dependencies.as_ref()
    }
    pub fn is_current(&self, completion: &HttpRuntimeCompletion) -> bool {
        self.graph.is_current(&completion.inner)
    }
    pub fn pending_dependencies<'a>(
        &self,
        completion: &'a HttpRuntimeCompletion,
    ) -> Option<&'a HttpDependencyReport> {
        if self.is_current(completion) {
            completion.dependencies.as_ref()
        } else {
            None
        }
    }
    /// Admit an explicit reason before allocating/submitting any HTTP work.
    /// Invalid input storage yields a paired failure and preserves retry configuration.
    pub fn request(
        &mut self,
        reason: RequestReason,
        source: HttpStylesheetSource,
        limits: HttpRuntimeLimits,
    ) -> Result<impl Future<Output = HttpRuntimeCompletion> + Send + 'static + use<>, RequestError>
    {
        let request = self.admit(reason, limits)?;
        let valid = validate_http_graph_source(&source, limits.acquisition).is_ok();
        let source = Arc::new(source);
        if valid {
            self.configuration = Some(HttpGraphConfiguration {
                source: Arc::clone(&source),
                limits,
            });
        }
        Ok(self.collect(request, source, limits))
    }
    /// Retry retained source buffers only on an admitted explicit reason. This
    /// starts no watcher, notification subscription or passive refresh loop.
    pub fn retry(
        &mut self,
        reason: RequestReason,
    ) -> Result<
        impl Future<Output = HttpRuntimeCompletion> + Send + 'static + use<>,
        HttpGraphRetryError,
    > {
        let Some(configuration) = self.configuration.as_ref() else {
            log::error!(target:"iced_css::stylesheet", "http graph retry rejected: no admitted source configuration");
            return Err(HttpGraphRetryError::MissingConfiguration);
        };
        let source = Arc::clone(&configuration.source);
        let limits = configuration.limits;
        let request = self
            .admit(reason, limits)
            .map_err(HttpGraphRetryError::Request)?;
        Ok(self.collect(request, source, limits))
    }
    fn admit(
        &mut self,
        reason: RequestReason,
        limits: HttpRuntimeLimits,
    ) -> Result<GraphRequest, RequestError> {
        if !request_allowed(
            self.mode,
            ResourceKind::Web,
            reason,
            self.graph.status() == PublicationStatus::Unrequested,
        ) {
            log::error!(target:"iced_css::stylesheet", "http graph request rejected: unauthorized {reason:?}");
            return Err(RequestError::Unauthorized);
        }
        self.graph
            .request(0, self.context, limits.graph)
            .map_err(RequestError::Exhausted)
    }
    fn collect(
        &self,
        request: GraphRequest,
        source: Arc<HttpStylesheetSource>,
        limits: HttpRuntimeLimits,
    ) -> impl Future<Output = HttpRuntimeCompletion> + Send + 'static + use<> {
        let work = self.reader.load_shared_http_stylesheet_graph(
            request,
            Arc::clone(&self.transport),
            Arc::clone(&self.identities),
            self.context,
            source,
            limits.acquisition,
        );
        async move {
            let (inner, dependencies) = work.await;
            HttpRuntimeCompletion {
                inner,
                dependencies,
            }
        }
    }
    /// Move evidence after graph admission and before application logging. A
    /// current failure preserves the complete old graph and its active report.
    pub fn finish(
        &mut self,
        completion: HttpRuntimeCompletion,
    ) -> Result<SheetCompletion<HttpGraphLoadError>, PublicationError> {
        let HttpRuntimeCompletion {
            inner,
            dependencies,
        } = completion;
        let active = &mut self.active_dependencies;
        let candidate = &mut self.candidate_dependencies;
        self.graph
            .finish_after_admission(inner, move |outcome| match outcome {
                SheetCompletion::Published(_) => {
                    *active = dependencies;
                    *candidate = None;
                }
                SheetCompletion::Failed(_) => {
                    *candidate = dependencies;
                }
                SheetCompletion::Ignored => {}
            })
    }
}

impl std::fmt::Debug for HttpRuntimeCompletion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpRuntimeCompletion")
            .field("inner", &self.inner)
            .field(
                "dependency_entries",
                &self
                    .dependencies
                    .as_ref()
                    .map(|report| report.entries().len()),
            )
            .finish()
    }
}
