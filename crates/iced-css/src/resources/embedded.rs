//! Explicit custom-scheme application assets. Resolver callbacks run on bounded
//! background workers; routing registration and publication belong to the host.
use super::worker::{FileReader, SubmitError, WorkerFailure};
use iced_css_core::resources::{
    body::{BodyCollector, BodyEnd, BodyError, BodyLimits, CompletedBody},
    uri::{UriError, UriLimits, UriReference, resolve},
};
use std::{future::Future, num::NonZeroUsize, sync::Arc};
#[derive(Debug, Clone, Copy)]
pub struct EmbeddedLimits {
    pub uri: UriLimits,
    pub metadata_bytes: NonZeroUsize,
    pub body: BodyLimits,
}
#[derive(Debug, Clone, Copy)]
pub struct EmbeddedRequest<'a> {
    pub namespace: &'a str,
    pub base: &'a str,
    pub reference_uri: &'a str,
    pub acquisition_uri: &'a str,
}
/// Application-owned input to admission. Its allocations and callback runtime
/// are controlled by the application, not by the admitted candidate's byte cap.
#[derive(Debug)]
pub struct EmbeddedSource {
    pub bytes: Vec<u8>,
    pub version: Vec<u8>,
    pub dependency_base: String,
}
pub trait EmbeddedResolver: Send + Sync + 'static {
    type Error: std::error::Error + Send + 'static;
    fn resolve(&self, request: EmbeddedRequest<'_>) -> Result<EmbeddedSource, Self::Error>;
}
#[derive(Debug)]
pub struct EmbeddedAsset {
    body: CompletedBody,
    version: Vec<u8>,
    dependency_base: String,
}
impl EmbeddedAsset {
    pub fn body(&self) -> &CompletedBody {
        &self.body
    }
    pub fn version(&self) -> &[u8] {
        &self.version
    }
    pub fn dependency_base(&self) -> &str {
        &self.dependency_base
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedConfigError {
    Namespace,
    ReservedNamespace,
    Bytes,
    Work,
}
impl std::fmt::Display for EmbeddedConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedded resolver configuration: {self:?}")
    }
}
impl std::error::Error for EmbeddedConfigError {}
#[derive(Debug)]
pub enum EmbeddedSubmitError {
    Metadata,
    Worker(SubmitError),
}
impl std::fmt::Display for EmbeddedSubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedded submission: {self:?}")
    }
}
impl std::error::Error for EmbeddedSubmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Worker(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub enum EmbeddedError<E> {
    Worker(WorkerFailure),
    Resolver(E),
    Uri(UriError),
    Namespace,
    Metadata,
    DependencyBase,
    BodyBytes,
    Body(BodyError),
}
impl<E: std::error::Error> std::fmt::Display for EmbeddedError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedded resource: {self:?}")
    }
}
impl<E: std::error::Error + 'static> std::error::Error for EmbeddedError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Worker(e) => Some(e),
            Self::Resolver(e) => Some(e),
            Self::Uri(e) => Some(e),
            Self::Body(e) => Some(e),
            _ => None,
        }
    }
}
pub struct EmbeddedReader<R: EmbeddedResolver> {
    namespace: Arc<String>,
    resolver: Arc<R>,
    worker: FileReader,
}
impl<R: EmbeddedResolver> EmbeddedReader<R> {
    /// Custom schemes only. This explicit adapter cannot override built-in file/HTTP.
    pub fn new(
        namespace: String,
        resolver: R,
        maximum: NonZeroUsize,
        limits: UriLimits,
    ) -> Result<Self, EmbeddedConfigError> {
        if namespace.capacity() > limits.bytes.get() {
            return Err(EmbeddedConfigError::Bytes);
        }
        let work = namespace
            .len()
            .checked_mul(4)
            .and_then(|n| n.checked_add(1))
            .ok_or(EmbeddedConfigError::Work)?;
        if work > limits.work.get() {
            return Err(EmbeddedConfigError::Work);
        }
        let mut bytes = namespace.bytes();
        if !bytes.next().is_some_and(|c| c.is_ascii_alphabetic())
            || !bytes.all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
        {
            return Err(EmbeddedConfigError::Namespace);
        }
        if ["file", "http", "https"]
            .iter()
            .any(|s| namespace.eq_ignore_ascii_case(s))
        {
            return Err(EmbeddedConfigError::ReservedNamespace);
        }
        Ok(Self {
            namespace: Arc::new(namespace),
            resolver: Arc::new(resolver),
            worker: FileReader::new(maximum),
        })
    }
    pub fn in_flight(&self) -> usize {
        self.worker.in_flight()
    }
    /// Resolve only an already-authorized request. Namespace mismatches are rejected
    /// before calling application code. URI routines have individual nested limits.
    pub fn resolve(
        &self,
        base: String,
        reference: String,
        limits: EmbeddedLimits,
    ) -> Result<
        impl Future<Output = Result<EmbeddedAsset, EmbeddedError<R::Error>>> + Send + 'static + use<R>,
        EmbeddedSubmitError,
    > {
        admit_input(base.capacity(), reference.capacity(), limits)?;
        let namespace = self.namespace.clone();
        let resolver = self.resolver.clone();
        let work = self
            .worker
            .submit(move || resolve_asset(&namespace, resolver.as_ref(), &base, &reference, limits))
            .map_err(EmbeddedSubmitError::Worker)?;
        Ok(async move { work.await.map_err(EmbeddedError::Worker)? })
    }
}

fn admit_input(
    base_capacity: usize,
    reference_capacity: usize,
    limits: EmbeddedLimits,
) -> Result<(), EmbeddedSubmitError> {
    let metadata = base_capacity
        .checked_add(reference_capacity)
        .ok_or(EmbeddedSubmitError::Metadata)?;
    if metadata > limits.metadata_bytes.get() {
        return Err(EmbeddedSubmitError::Metadata);
    }
    Ok(())
}
fn resolve_asset<R: EmbeddedResolver>(
    namespace: &str,
    resolver: &R,
    base: &str,
    reference: &str,
    limits: EmbeddedLimits,
) -> Result<EmbeddedAsset, EmbeddedError<R::Error>> {
    let uri = resolve(base, reference, limits.uri).map_err(EmbeddedError::Uri)?;
    let parsed = UriReference::parse(&uri, limits.uri).map_err(EmbeddedError::Uri)?;
    if !parsed
        .scheme()
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case(namespace))
    {
        return Err(EmbeddedError::Namespace);
    }
    let source = resolver
        .resolve(EmbeddedRequest {
            namespace,
            base,
            reference_uri: &uri,
            acquisition_uri: parsed.without_fragment(),
        })
        .map_err(EmbeddedError::Resolver)?;
    let metadata = source
        .version
        .capacity()
        .checked_add(source.dependency_base.capacity())
        .ok_or(EmbeddedError::Metadata)?;
    if metadata > limits.metadata_bytes.get() {
        return Err(EmbeddedError::Metadata);
    }
    if source.bytes.capacity() > limits.body.retained_bytes.get() {
        return Err(EmbeddedError::BodyBytes);
    }
    let dependency =
        UriReference::parse(&source.dependency_base, limits.uri).map_err(EmbeddedError::Uri)?;
    if dependency.scheme().is_none() || dependency.fragment().is_some() {
        return Err(EmbeddedError::DependencyBase);
    }
    let length = u64::try_from(source.bytes.len()).map_err(|_| EmbeddedError::BodyBytes)?;
    let mut body = BodyCollector::new(Some(length), limits.body).map_err(EmbeddedError::Body)?;
    body.append(&source.bytes).map_err(EmbeddedError::Body)?;
    let body = body
        .finish(BodyEnd::Complete)
        .map_err(EmbeddedError::Body)?;
    Ok(EmbeddedAsset {
        body,
        version: source.version,
        dependency_base: source.dependency_base,
    })
}
#[path = "embedded_stylesheet.rs"]
mod stylesheet;
pub use stylesheet::{EmbeddedStylesheet, EmbeddedStylesheetError, EmbeddedStylesheetLimits};
#[path = "embedded_dependencies.rs"]
mod dependencies;
pub use dependencies::{
    EmbeddedDependency, EmbeddedDependencyError, EmbeddedDependencyLimits, EmbeddedDependencyReport,
};
#[path = "embedded_graph.rs"]
mod graph;
pub use graph::{
    EmbeddedGraphCompletion, EmbeddedGraphError, EmbeddedGraphLimits, EmbeddedGraphSource,
    EmbeddedImportError,
};

pub(crate) use graph::validate_graph_source;
