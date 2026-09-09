//! Runtime embedded graph ownership through the existing publication gate.
use super::graph::{GraphLoadError, GraphRequest, RuntimeLocatedStylesheetGraph};
use crate::resources::{
    embedded::{
        EmbeddedDependencyReport, EmbeddedGraphCompletion, EmbeddedGraphError, EmbeddedGraphLimits,
        EmbeddedGraphSource, EmbeddedReader, EmbeddedResolver, validate_graph_source,
    },
    graph::GraphWorkerError,
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
pub struct EmbeddedRuntimeLimits {
    pub acquisition: EmbeddedGraphLimits,
    pub graph: OwnedGraphLimits,
}
#[derive(Debug)]
pub struct EmbeddedGraphConfiguration {
    source: Arc<EmbeddedGraphSource>,
    limits: EmbeddedRuntimeLimits,
}
impl EmbeddedGraphConfiguration {
    pub fn source(&self) -> &EmbeddedGraphSource {
        &self.source
    }
    pub fn limits(&self) -> EmbeddedRuntimeLimits {
        self.limits
    }
}
#[derive(Debug)]
pub struct EmbeddedRuntimeCompletion<E> {
    inner: EmbeddedGraphCompletion<E>,
    dependencies: Option<EmbeddedDependencyReport>,
}
pub type EmbeddedGraphLoadError<E> = GraphLoadError<GraphWorkerError<EmbeddedGraphError<E>>>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedGraphRetryError {
    MissingConfiguration,
    Request(RequestError),
}
impl std::fmt::Display for EmbeddedGraphRetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedded graph retry: {self:?}")
    }
}
impl std::error::Error for EmbeddedGraphRetryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Request(error) => Some(error),
            Self::MissingConfiguration => None,
        }
    }
}
/// Owns a fixed resolver registration and persistent source identities. Registry
/// locking, resolver callbacks and CSS parsing happen only on resource workers.
/// The application still owns delivery/queueing of these non-Clone completions.
pub struct RuntimeEmbeddedStylesheetGraph<R: EmbeddedResolver> {
    reader: EmbeddedReader<R>,
    graph: RuntimeLocatedStylesheetGraph,
    identities: Arc<Mutex<ImportIdentityRegistry>>,
    mode: RuntimeMode,
    context: RuleContext,
    configuration: Option<EmbeddedGraphConfiguration>,
    active_dependencies: Option<EmbeddedDependencyReport>,
    candidate_dependencies: Option<EmbeddedDependencyReport>,
}
impl<R: EmbeddedResolver> RuntimeEmbeddedStylesheetGraph<R> {
    pub fn new(
        reader: EmbeddedReader<R>,
        mode: RuntimeMode,
        context: RuleContext,
        publication: PublicationLimits,
        diagnostic_work: NonZeroUsize,
        identities: IdentityLimits,
    ) -> Result<Self, IdentityError> {
        Ok(Self {
            reader,
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
    pub fn configuration(&self) -> Option<&EmbeddedGraphConfiguration> {
        self.configuration.as_ref()
    }
    pub fn active_dependencies(&self) -> Option<&EmbeddedDependencyReport> {
        self.active_dependencies.as_ref()
    }
    pub fn candidate_dependencies(&self) -> Option<&EmbeddedDependencyReport> {
        self.candidate_dependencies.as_ref()
    }
    pub fn is_current(&self, completion: &EmbeddedRuntimeCompletion<R::Error>) -> bool {
        self.graph.is_current(&completion.inner)
    }
    pub fn pending_dependencies<'a>(
        &self,
        completion: &'a EmbeddedRuntimeCompletion<R::Error>,
    ) -> Option<&'a EmbeddedDependencyReport> {
        if self.is_current(completion) {
            completion.dependencies.as_ref()
        } else {
            None
        }
    }
    /// Admit an explicit reason before allocating/submitting any resolver work.
    /// Invalid input storage yields a paired failure and preserves retry configuration.
    pub fn request(
        &mut self,
        reason: RequestReason,
        source: EmbeddedGraphSource,
        limits: EmbeddedRuntimeLimits,
    ) -> Result<
        impl Future<Output = EmbeddedRuntimeCompletion<R::Error>> + Send + 'static + use<R>,
        RequestError,
    > {
        let request = self.admit(reason, limits)?;
        let valid = validate_graph_source(&source, limits.acquisition).is_ok();
        let source = Arc::new(source);
        if valid {
            self.configuration = Some(EmbeddedGraphConfiguration {
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
        impl Future<Output = EmbeddedRuntimeCompletion<R::Error>> + Send + 'static + use<R>,
        EmbeddedGraphRetryError,
    > {
        let Some(configuration) = self.configuration.as_ref() else {
            log::error!(target:"iced_css::stylesheet", "embedded graph retry rejected: no admitted source configuration");
            return Err(EmbeddedGraphRetryError::MissingConfiguration);
        };
        let source = Arc::clone(&configuration.source);
        let limits = configuration.limits;
        let request = self
            .admit(reason, limits)
            .map_err(EmbeddedGraphRetryError::Request)?;
        Ok(self.collect(request, source, limits))
    }
    fn admit(
        &mut self,
        reason: RequestReason,
        limits: EmbeddedRuntimeLimits,
    ) -> Result<GraphRequest, RequestError> {
        if !request_allowed(
            self.mode,
            ResourceKind::Embedded,
            reason,
            self.graph.status() == PublicationStatus::Unrequested,
        ) {
            log::error!(target:"iced_css::stylesheet", "embedded graph request rejected: unauthorized {reason:?}");
            return Err(RequestError::Unauthorized);
        }
        self.graph
            .request(0, self.context, limits.graph)
            .map_err(RequestError::Exhausted)
    }
    fn collect(
        &self,
        request: GraphRequest,
        source: Arc<EmbeddedGraphSource>,
        limits: EmbeddedRuntimeLimits,
    ) -> impl Future<Output = EmbeddedRuntimeCompletion<R::Error>> + Send + 'static + use<R> {
        let work = self.reader.load_shared_stylesheet_graph(
            request,
            Arc::clone(&self.identities),
            self.context,
            source,
            limits.acquisition,
        );
        async move {
            let (inner, dependencies) = work.await;
            EmbeddedRuntimeCompletion {
                inner,
                dependencies,
            }
        }
    }
    /// Move evidence after graph admission and before application logging. A
    /// current failure preserves the complete old graph and its active report.
    pub fn finish(
        &mut self,
        completion: EmbeddedRuntimeCompletion<R::Error>,
    ) -> Result<SheetCompletion<EmbeddedGraphLoadError<R::Error>>, PublicationError> {
        let EmbeddedRuntimeCompletion {
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
