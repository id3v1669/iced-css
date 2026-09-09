//! One runtime file graph owner with typed request/completion transport on Unix.
use super::graph::{GraphCompletion, GraphLoadError, GraphRequest, RuntimeLocatedStylesheetGraph};
use crate::resources::{
    file_dependencies::FileDependencyReport,
    file_imports::{FileImportError, FileImportLimits, UnixFileImportProvider},
    graph::GraphWorkerError,
    stylesheet::StylesheetEncoding,
    worker::FileReader,
};
use iced_css_core::{
    resources::{
        publication::{PublicationError, PublicationLimits, PublicationStatus, ResourceRevision},
        triggers::{RequestError, RequestReason, ResourceKind, RuntimeMode, request_allowed},
    },
    stylesheet::{
        RuleContext,
        collector::{CollectionError, CollectorLimits, collect_imports},
        identities::{IdentityError, IdentityLimits, ImportIdentityRegistry},
        located_graph::{BaseLimits, LocatedStylesheetGraph},
        owned_graph::OwnedGraphLimits,
        publication::SheetCompletion,
    },
};
use std::{
    future::Future,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};
#[derive(Debug)]
pub struct FileGraphSource {
    pub uri: String,
    pub encoding: StylesheetEncoding,
}
#[derive(Debug, Clone, Copy)]
pub struct FileGraphLimits {
    pub file: FileImportLimits,
    pub collector: CollectorLimits,
    pub graph: OwnedGraphLimits,
    pub bases: BaseLimits,
    /// Combined owned transport/linking/referring label capacities in queued input.
    pub metadata_bytes: NonZeroUsize,
}
/// Last capacity-admitted requested input, independent of active CSS. Shared
/// allocations avoid cloning URI/encoding buffers for retry and queued workers.
#[derive(Debug)]
pub struct FileGraphConfiguration {
    source: FileGraphSource,
    limits: FileGraphLimits,
}
impl FileGraphConfiguration {
    pub fn source(&self) -> &FileGraphSource {
        &self.source
    }
    pub fn limits(&self) -> FileGraphLimits {
        self.limits
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileGraphRetryError {
    MissingConfiguration,
    Request(RequestError),
}
impl std::fmt::Display for FileGraphRetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file graph retry: {self:?}")
    }
}
impl std::error::Error for FileGraphRetryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Request(error) => Some(error),
            Self::MissingConfiguration => None,
        }
    }
}
#[derive(Debug)]
pub enum FileGraphAcquisitionError {
    UriBytes,
    MetadataBytes,
    IdentityPoisoned,
    Watch(super::file_watch::FileGraphWatchError),
    Root(FileImportError),
    Imports(CollectionError<FileImportError>),
}
impl std::fmt::Display for FileGraphAcquisitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file graph acquisition: {self:?}")
    }
}
impl std::error::Error for FileGraphAcquisitionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Root(e) => Some(e),
            Self::Imports(e) => Some(e),
            Self::Watch(e) => Some(e),
            _ => None,
        }
    }
}
/// Admitted input before resource acquisition. This retains the same graph
/// ticket and immutable source/limits; construction starts no resource worker.
#[derive(Debug)]
pub struct FileGraphRequest {
    request: GraphRequest,
    prepared: Result<Arc<FileGraphConfiguration>, FileGraphAcquisitionError>,
    limits: FileGraphLimits,
}
impl FileGraphRequest {
    pub fn configuration(&self) -> Option<&FileGraphConfiguration> {
        self.prepared.as_ref().ok().map(Arc::as_ref)
    }
}
/// Opaque graph/report pairing for Message delivery. Reports become visible on
/// the owner only after the corresponding completion passes admission.
#[derive(Debug)]
pub struct FileGraphCompletion {
    inner: GraphCompletion<GraphWorkerError<FileGraphAcquisitionError>, LocatedStylesheetGraph>,
    dependencies: Option<FileDependencyReport>,
}
pub type FileGraphLoadError = GraphLoadError<GraphWorkerError<FileGraphAcquisitionError>>;
/// Reuses the located graph's single publication gate. One persistent registry
/// supplies stable source/cycle IDs across requests, including failed requests.
/// Its lock is acquired only on resource workers, never by the request/update
/// path. Initial local collection is serialized under that lock; a blocked file
/// read can delay a later worker. No timing or cancellation guarantee is implied.
pub struct RuntimeFileStylesheetGraph {
    graph: RuntimeLocatedStylesheetGraph,
    identities: Arc<Mutex<ImportIdentityRegistry>>,
    mode: RuntimeMode,
    context: RuleContext,
    active_dependencies: Option<FileDependencyReport>,
    candidate_dependencies: Option<FileDependencyReport>,
    configuration: Option<Arc<FileGraphConfiguration>>,
}
impl RuntimeFileStylesheetGraph {
    pub fn new(
        mode: RuntimeMode,
        context: RuleContext,
        publication: PublicationLimits,
        diagnostic_work: NonZeroUsize,
        identities: IdentityLimits,
    ) -> Result<Self, IdentityError> {
        let identities = ImportIdentityRegistry::new(identities)?;
        Ok(Self {
            graph: RuntimeLocatedStylesheetGraph::new(publication, diagnostic_work),
            identities: Arc::new(Mutex::new(identities)),
            mode,
            context,
            active_dependencies: None,
            candidate_dependencies: None,
            configuration: None,
        })
    }
    /// Requested source and limits, not proof of acquisition success. A later
    /// worker failure retains this retry target while old active CSS stays intact.
    pub fn configuration(&self) -> Option<&FileGraphConfiguration> {
        self.configuration.as_deref()
    }
    /// Pending owner/generation check, not a reservation or publication guarantee.
    pub fn is_current(&self, completion: &FileGraphCompletion) -> bool {
        self.graph.is_current(&completion.inner)
    }
    /// Eligible completion evidence for asynchronous preparation. Recheck the
    /// completion after awaiting; a missing report is not proof of obsolescence.
    pub fn pending_dependencies<'a>(
        &self,
        completion: &'a FileGraphCompletion,
    ) -> Option<&'a FileDependencyReport> {
        if self.is_current(completion) {
            completion.dependencies.as_ref()
        } else {
            None
        }
    }
    pub fn active_dependencies(&self) -> Option<&FileDependencyReport> {
        self.active_dependencies.as_ref()
    }
    /// Paths seen by the latest admitted failed candidate. Check report rejection;
    /// neither presence nor absence certifies complete native watch coverage.
    pub fn candidate_dependencies(&self) -> Option<&FileDependencyReport> {
        self.candidate_dependencies.as_ref()
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
    /// Route an actual authorized reason, allocate the graph ticket, then submit
    /// one bounded root/import collection. The future fits iced::Task::perform
    /// without borrowing the owner or reader. Receiving it does not publish CSS.
    /// Input-capacity and submission errors become paired failed completions;
    /// reason/counter rejection submits no work. This API starts no watcher.
    pub fn request(
        &mut self,
        reader: &FileReader,
        reason: RequestReason,
        source: FileGraphSource,
        limits: FileGraphLimits,
    ) -> Result<impl Future<Output = FileGraphCompletion> + Send + 'static + use<>, RequestError>
    {
        let request = self.begin(reason, source, limits)?;
        Ok(self.collect(reader, request))
    }
    /// Admit the request and retain its validated storage without file I/O or a
    /// worker lease. This does not establish root coverage or validate CSS bytes.
    pub fn begin(
        &mut self,
        reason: RequestReason,
        source: FileGraphSource,
        limits: FileGraphLimits,
    ) -> Result<FileGraphRequest, RequestError> {
        let request = self.admit(reason, limits)?;
        let prepared = validate_source(&source, limits).map(|()| {
            let configuration = Arc::new(FileGraphConfiguration { source, limits });
            self.configuration = Some(Arc::clone(&configuration));
            configuration
        });
        Ok(FileGraphRequest {
            request,
            prepared,
            limits,
        })
    }
    /// Retry the most recent capacity-admitted request without duplicating its
    /// source buffers. Uses the same reason/counter gate and retained limits.
    /// Missing configuration and rejected reasons allocate no request ticket.
    /// This schedules one attempt, not an automatic loop or watch subscription.
    pub fn retry(
        &mut self,
        reader: &FileReader,
        reason: RequestReason,
    ) -> Result<
        impl Future<Output = FileGraphCompletion> + Send + 'static + use<>,
        FileGraphRetryError,
    > {
        let request = self.begin_retry(reason)?;
        Ok(self.collect(reader, request))
    }
    /// Stage one retry using retained source/limits, without submitting a worker.
    pub fn begin_retry(
        &mut self,
        reason: RequestReason,
    ) -> Result<FileGraphRequest, FileGraphRetryError> {
        let Some(configuration) = self.configuration.as_ref().map(Arc::clone) else {
            log::error!(target:"iced_css::stylesheet", "file graph retry rejected: no admitted source configuration");
            return Err(FileGraphRetryError::MissingConfiguration);
        };
        let limits = configuration.limits;
        let request = self
            .admit(reason, limits)
            .map_err(FileGraphRetryError::Request)?;
        Ok(FileGraphRequest {
            request,
            prepared: Ok(configuration),
            limits,
        })
    }
    pub fn is_request_current(&self, request: &FileGraphRequest) -> bool {
        self.graph.is_request_current(&request.request)
    }
    /// Submit only a current staged request. Obsolete/foreign requests produce no
    /// worker and no fabricated completion. Recheck completion on final delivery.
    pub fn acquire_current(
        &self,
        reader: &FileReader,
        request: FileGraphRequest,
    ) -> Option<impl Future<Output = FileGraphCompletion> + Send + 'static + use<>> {
        self.is_request_current(&request)
            .then(|| self.collect(reader, request))
    }
    /// Terminate staged preparation through the original graph gate. An earlier
    /// source-capacity failure takes precedence over the supplied setup error.
    pub(super) fn fail_request(
        &mut self,
        staged: FileGraphRequest,
        error: FileGraphAcquisitionError,
        dependencies: Option<FileDependencyReport>,
    ) -> Result<SheetCompletion<FileGraphLoadError>, PublicationError> {
        if !self.is_request_current(&staged) {
            return Ok(SheetCompletion::Ignored);
        }
        let error = staged.prepared.err().unwrap_or(error);
        self.finish(FileGraphCompletion {
            inner: staged.request.assemble_located(
                Err(GraphWorkerError::Acquisition(error)),
                staged.limits.bases,
            ),
            dependencies,
        })
    }
    fn admit(
        &mut self,
        reason: RequestReason,
        limits: FileGraphLimits,
    ) -> Result<GraphRequest, RequestError> {
        if !request_allowed(
            self.mode,
            ResourceKind::LocalFile,
            reason,
            self.graph.status() == PublicationStatus::Unrequested,
        ) {
            log::error!(target:"iced_css::stylesheet","file graph request rejected: unauthorized {reason:?}");
            return Err(RequestError::Unauthorized);
        }
        self.graph
            .request(0, self.context, limits.graph)
            .map_err(RequestError::Exhausted)
    }
    pub(super) fn collect(
        &self,
        reader: &FileReader,
        staged: FileGraphRequest,
    ) -> impl Future<Output = FileGraphCompletion> + Send + 'static + use<> {
        let FileGraphRequest {
            request,
            prepared,
            limits,
        } = staged;
        let work = match prepared {
            Err(error) => Err((
                request.assemble_located(Err(GraphWorkerError::Acquisition(error)), limits.bases),
                None,
            )),
            Ok(configuration) => {
                let identities = Arc::clone(&self.identities);
                let context = self.context;
                Ok(reader.load_located_stylesheet_graph_with_report(
                    request,
                    limits.bases,
                    move || {
                        let mut identities = match identities.lock() {
                            Ok(identities) => identities,
                            Err(_) => {
                                return (
                                    Err(FileGraphAcquisitionError::IdentityPoisoned),
                                    FileDependencyReport::new(limits.file.dependencies),
                                );
                            }
                        };
                        let mut provider =
                            UnixFileImportProvider::new(&mut identities, context, limits.file);
                        let result = provider
                            .read_root(
                                &configuration.source.uri,
                                configuration.source.encoding.metadata(),
                            )
                            .map_err(FileGraphAcquisitionError::Root)
                            .and_then(|root| {
                                collect_imports(root, &mut provider, limits.collector)
                                    .map(Into::into)
                                    .map_err(FileGraphAcquisitionError::Imports)
                            });
                        (result, provider.into_dependencies())
                    },
                ))
            }
        };
        async move {
            let (inner, dependencies) = match work {
                Ok(work) => work.await,
                Err(completion) => completion,
            };
            FileGraphCompletion {
                inner,
                dependencies,
            }
        }
    }

    /// Terminate a current coverage failure without publishing a successful CSS
    /// candidate. Existing acquisition/assembly errors are preserved; an additional
    /// coverage error is logged after ordinary admission. Obsolete results stay
    /// silent. This does not mutate watch ownership or automatically retry.
    pub fn fail_watch(
        &mut self,
        rejected: super::file_watch::RejectedFileGraphWatch,
    ) -> Result<SheetCompletion<FileGraphLoadError>, PublicationError> {
        let super::file_watch::RejectedFileGraphWatch {
            reason,
            mut completion,
        } = rejected;
        if !self.is_current(&completion) {
            return Ok(SheetCompletion::Ignored);
        }
        let additional = completion
            .inner
            .reject_success(GraphWorkerError::Acquisition(
                FileGraphAcquisitionError::Watch(reason),
            ));
        let result = self.finish(completion);
        if let Some(error) = additional {
            log::error!(target:"iced_css::stylesheet", "file graph coverage also failed: {error}");
        }
        result
    }
    /// Deliver through the owner's Message/update path. Foreign/obsolete results
    /// remain ignored; a current failed replacement retains the whole old graph.
    pub fn finish(
        &mut self,
        completion: FileGraphCompletion,
    ) -> Result<SheetCompletion<FileGraphLoadError>, PublicationError> {
        let FileGraphCompletion {
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
fn validate_source(
    source: &FileGraphSource,
    limits: FileGraphLimits,
) -> Result<(), FileGraphAcquisitionError> {
    if limits.file.file.uri.bytes.get() > isize::MAX as usize
        || source.uri.capacity() > limits.file.file.uri.bytes.get()
    {
        return Err(FileGraphAcquisitionError::UriBytes);
    }
    if source
        .encoding
        .retained_bytes()
        .is_none_or(|bytes| bytes > limits.metadata_bytes.get())
    {
        return Err(FileGraphAcquisitionError::MetadataBytes);
    }
    Ok(())
}
