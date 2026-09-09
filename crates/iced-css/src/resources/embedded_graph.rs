//! Fresh custom-namespace import collection inside one embedded reader worker.
use super::{
    EmbeddedDependencyError, EmbeddedDependencyLimits, EmbeddedDependencyReport, EmbeddedError,
    EmbeddedReader, EmbeddedResolver, EmbeddedStylesheetLimits, EmbeddedSubmitError, admit_input,
    resolve_asset, stylesheet::parse_asset,
};
use crate::{
    host::stylesheet::graph::{GraphCompletion, GraphRequest},
    resources::{graph::GraphWorkerError, stylesheet::StylesheetEncoding},
};
use iced_css_core::{
    resources::uri::{UriReference, resolve},
    stylesheet::{
        RuleContext, StylesheetIntakeError,
        collector::{
            AcquiredStylesheet, CollectionError, CollectorLimits, CollectorRoot, ImportProbe,
            ImportProvider, ImportTarget, collect_imports,
        },
        identities::{IdentityError, ImportIdentityRegistry},
        located_graph::{BaseLimits, LocatedStylesheetGraph},
    },
    syntax::encoding_selection::EncodingMetadata,
};
use std::{
    future::Future,
    sync::{Arc, Mutex},
};
#[derive(Debug)]
pub struct EmbeddedGraphSource {
    pub base: String,
    pub reference: String,
    pub encoding: StylesheetEncoding,
}
#[derive(Debug, Clone, Copy)]
pub struct EmbeddedGraphLimits {
    pub stylesheet: EmbeddedStylesheetLimits,
    pub collector: CollectorLimits,
    pub bases: BaseLimits,
    pub dependencies: EmbeddedDependencyLimits,
}
#[derive(Debug)]
pub enum EmbeddedImportError<E> {
    Asset(EmbeddedError<E>),
    Identity(IdentityError),
    Dependencies(EmbeddedDependencyError),
    Intake(StylesheetIntakeError),
    Target,
}
impl<E: std::error::Error> std::fmt::Display for EmbeddedImportError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedded import: {self:?}")
    }
}
impl<E: std::error::Error + 'static> std::error::Error for EmbeddedImportError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Asset(e) => Some(e),
            Self::Identity(e) => Some(e),
            Self::Dependencies(e) => Some(e),
            Self::Intake(e) => Some(e),
            Self::Target => None,
        }
    }
}
#[derive(Debug)]
pub enum EmbeddedGraphError<E> {
    Input(EmbeddedSubmitError),
    IdentityPoisoned,
    Root(EmbeddedImportError<E>),
    Imports(CollectionError<EmbeddedImportError<E>>),
}
impl<E: std::error::Error> std::fmt::Display for EmbeddedGraphError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedded graph: {self:?}")
    }
}
impl<E: std::error::Error + 'static> std::error::Error for EmbeddedGraphError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(e) => Some(e),
            Self::Root(e) => Some(e),
            Self::Imports(e) => Some(e),
            Self::IdentityPoisoned => None,
        }
    }
}
struct Provider<'a, R> {
    namespace: &'a str,
    resolver: &'a R,
    identities: &'a mut ImportIdentityRegistry,
    context: RuleContext,
    limits: EmbeddedStylesheetLimits,
    report: EmbeddedDependencyReport,
}
impl<R: EmbeddedResolver> Provider<'_, R> {
    fn identify_reference(
        &mut self,
        base: &str,
        reference: &str,
        encoding: EncodingMetadata<'_>,
    ) -> Result<(ImportTarget, usize), EmbeddedImportError<R::Error>> {
        let uri = resolve(base, reference, self.limits.asset.uri)
            .map_err(|e| EmbeddedImportError::Asset(EmbeddedError::Uri(e)))?;
        let parsed = UriReference::parse(&uri, self.limits.asset.uri)
            .map_err(|e| EmbeddedImportError::Asset(EmbeddedError::Uri(e)))?;
        if !parsed
            .scheme()
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case(self.namespace))
        {
            return Err(EmbeddedImportError::Asset(EmbeddedError::Namespace));
        }
        let target = self
            .identities
            .identify(parsed.without_fragment(), encoding, self.context)
            .map_err(EmbeddedImportError::Identity)?;
        let index = self
            .report
            .record(target.source, parsed.without_fragment())
            .map_err(EmbeddedImportError::Dependencies)?;
        Ok((target, index))
    }
    fn read(
        &mut self,
        index: usize,
        base: &str,
        reference: &str,
        encoding: EncodingMetadata<'_>,
    ) -> Result<AcquiredStylesheet, EmbeddedImportError<R::Error>> {
        let asset = resolve_asset(
            self.namespace,
            self.resolver,
            base,
            reference,
            self.limits.asset,
        )
        .map_err(EmbeddedImportError::Asset)?;
        let parsed = parse_asset(asset, encoding, self.context, self.limits.intake)
            .map_err(EmbeddedImportError::Intake)?;
        let (candidate, version, effective_base) = parsed.into_parts();
        self.report
            .set_version(index, version)
            .map_err(EmbeddedImportError::Dependencies)?;
        Ok(AcquiredStylesheet {
            candidate,
            effective_base,
        })
    }
    fn root(
        &mut self,
        source: &EmbeddedGraphSource,
    ) -> Result<CollectorRoot, EmbeddedImportError<R::Error>> {
        let (target, index) =
            self.identify_reference(&source.base, &source.reference, source.encoding.metadata())?;
        let input = self.read(
            index,
            &source.base,
            &source.reference,
            source.encoding.metadata(),
        )?;
        Ok(CollectorRoot { target, input })
    }
}
impl<R: EmbeddedResolver> ImportProvider for Provider<'_, R> {
    type Error = EmbeddedImportError<R::Error>;
    fn identify(&mut self, probe: ImportProbe<'_>) -> Result<ImportTarget, Self::Error> {
        self.identify_reference(
            probe.effective_base,
            probe.request.reference_uri(),
            probe.encoding,
        )
        .map(|(target, _)| target)
    }
    fn acquire(
        &mut self,
        target: ImportTarget,
        probe: ImportProbe<'_>,
    ) -> Result<Option<AcquiredStylesheet>, Self::Error> {
        let (expected, index) = self.identify_reference(
            probe.effective_base,
            probe.request.reference_uri(),
            probe.encoding,
        )?;
        if expected != target {
            return Err(EmbeddedImportError::Target);
        }
        match self.read(
            index,
            probe.effective_base,
            probe.request.reference_uri(),
            probe.encoding,
        ) {
            Ok(input) => Ok(Some(input)),
            Err(EmbeddedImportError::Asset(EmbeddedError::Resolver(_))) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
/// Completion paired to the graph caller's original publication ticket.
pub type EmbeddedGraphCompletion<E> =
    GraphCompletion<GraphWorkerError<EmbeddedGraphError<E>>, LocatedStylesheetGraph>;
impl<R: EmbeddedResolver> EmbeddedReader<R> {
    /// Collect an already-authorized fresh graph under this reader's one worker
    /// lease. The supplied graph ticket retains its original publication owner.
    /// Existing resolver versions are evidence, never fallback acquisition bodies.
    pub fn load_stylesheet_graph(
        &self,
        request: GraphRequest,
        identities: Arc<Mutex<ImportIdentityRegistry>>,
        context: RuleContext,
        source: EmbeddedGraphSource,
        limits: EmbeddedGraphLimits,
    ) -> impl Future<
        Output = (
            EmbeddedGraphCompletion<R::Error>,
            Option<EmbeddedDependencyReport>,
        ),
    >
    + Send
    + 'static
    + use<R> {
        self.load_shared_stylesheet_graph(request, identities, context, Arc::new(source), limits)
    }
    pub(crate) fn load_shared_stylesheet_graph(
        &self,
        request: GraphRequest,
        identities: Arc<Mutex<ImportIdentityRegistry>>,
        context: RuleContext,
        source: Arc<EmbeddedGraphSource>,
        limits: EmbeddedGraphLimits,
    ) -> impl Future<
        Output = (
            EmbeddedGraphCompletion<R::Error>,
            Option<EmbeddedDependencyReport>,
        ),
    >
    + Send
    + 'static
    + use<R> {
        let input = validate_graph_source(&source, limits);
        let work = match input {
            Err(error) => Err((
                request.assemble_located(
                    Err(GraphWorkerError::Acquisition(EmbeddedGraphError::Input(
                        error,
                    ))),
                    limits.bases,
                ),
                None,
            )),
            Ok(()) => {
                let namespace = self.namespace.clone();
                let resolver = self.resolver.clone();
                Ok(self.worker.load_located_stylesheet_graph_with_report(
                    request,
                    limits.bases,
                    move || {
                        let report = EmbeddedDependencyReport::new(limits.dependencies);
                        let mut identities = match identities.lock() {
                            Ok(ids) => ids,
                            Err(_) => return (Err(EmbeddedGraphError::IdentityPoisoned), report),
                        };
                        let mut provider = Provider {
                            namespace: &namespace,
                            resolver: resolver.as_ref(),
                            identities: &mut identities,
                            context,
                            limits: limits.stylesheet,
                            report,
                        };
                        let result = provider
                            .root(&source)
                            .map_err(EmbeddedGraphError::Root)
                            .and_then(|root| {
                                collect_imports(root, &mut provider, limits.collector)
                                    .map(Into::into)
                                    .map_err(EmbeddedGraphError::Imports)
                            });
                        (result, provider.report)
                    },
                ))
            }
        };
        async move {
            match work {
                Ok(work) => work.await,
                Err(completion) => completion,
            }
        }
    }
}

pub(crate) fn validate_graph_source(
    source: &EmbeddedGraphSource,
    limits: EmbeddedGraphLimits,
) -> Result<(), EmbeddedSubmitError> {
    admit_input(
        source.base.capacity(),
        source.reference.capacity(),
        limits.stylesheet.asset,
    )
    .and_then(|()| {
        if source
            .encoding
            .retained_bytes()
            .is_none_or(|bytes| bytes > limits.stylesheet.metadata_bytes.get())
        {
            Err(EmbeddedSubmitError::Metadata)
        } else {
            Ok(())
        }
    })
}
