//! Runtime whole-graph publication with request-bound import failure policy.
use super::{graph_recovery::report_graph_recovery, import_diagnostics::report_import_cuts};
use iced_css_core::{
    resources::publication::{
        PublicationError, PublicationLimits, PublicationStatus, ResourceRevision,
    },
    stylesheet::{
        RuleContext,
        imports::{ImportCycleId, ImportCycleTable, UnavailableImportPolicy},
        located_graph::{BaseLimits, LocatedGraphError, LocatedStylesheetGraph, StylesheetBase},
        owned_graph::{OwnedGraphError, OwnedGraphLimits, OwnedLoadedSheet, OwnedStylesheetGraph},
        publication::{CandidatePublication, SheetCompletion, StylesheetTicket},
    },
};
use std::num::NonZeroUsize;

#[derive(Debug)]
pub enum GraphLoadError<E> {
    Acquisition(E),
    Assembly(OwnedGraphError),
    Bases(LocatedGraphError),
}
impl<E: std::fmt::Display> std::fmt::Display for GraphLoadError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Acquisition(error) => write!(f, "graph acquisition failed: {error}"),
            Self::Assembly(error) => error.fmt(f),
            Self::Bases(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for GraphLoadError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Acquisition(error) => error,
            Self::Assembly(error) => error,
            Self::Bases(error) => error,
        })
    }
}
/// Immutable worker input. No old resource contents are supplied for fallback.
#[derive(Debug, Clone)]
pub struct GraphRequest {
    ticket: StylesheetTicket,
    root: usize,
    context: RuleContext,
    limits: OwnedGraphLimits,
}
impl GraphRequest {
    /// Synchronous assembly, intended for a worker. Supply resources acquired for
    /// this request or their acquisition failure; do not label retained old inputs
    /// as a successful new acquisition. This performs no I/O or publication.
    pub fn assemble<E>(self, resources: Result<Vec<OwnedLoadedSheet>, E>) -> GraphCompletion<E> {
        self.assemble_sources(resources, None)
    }
    /// Assemble using explicit active-URI identities. The table is borrowed only
    /// during assembly; the caller bounds its storage separately.
    pub fn assemble_with_cycles<E>(
        self,
        resources: Result<Vec<OwnedLoadedSheet>, E>,
        cycles: &[ImportCycleId],
    ) -> GraphCompletion<E> {
        self.assemble_sources(resources, Some(cycles))
    }
    fn assemble_sources<E>(
        self,
        resources: Result<Vec<OwnedLoadedSheet>, E>,
        cycles: Option<&[ImportCycleId]>,
    ) -> GraphCompletion<E> {
        let candidate = resources
            .map_err(GraphLoadError::Acquisition)
            .and_then(|resources| {
                match cycles {
                    Some(cycles) => OwnedStylesheetGraph::new_with_cycles(
                        resources,
                        cycles,
                        self.root,
                        self.context,
                        self.limits,
                    ),
                    None => {
                        OwnedStylesheetGraph::new(resources, self.root, self.context, self.limits)
                    }
                }
                .map_err(GraphLoadError::Assembly)
            });
        GraphCompletion {
            ticket: self.ticket,
            candidate,
        }
    }
}
/// Fresh acquisition inputs with explicit effective retrieval bases.
pub struct LocatedGraphInputs {
    pub resources: Vec<OwnedLoadedSheet>,
    pub bases: Vec<StylesheetBase>,
}
/// Fresh located inputs with separately capacity-admitted cycle metadata.
/// Graph assembly validates one cycle token per loaded source.
pub struct CycleLocatedGraphInputs {
    pub graph: LocatedGraphInputs,
    pub cycles: ImportCycleTable,
}
impl From<iced_css_core::stylesheet::collector::CollectedStylesheets> for CycleLocatedGraphInputs {
    fn from(inputs: iced_css_core::stylesheet::collector::CollectedStylesheets) -> Self {
        Self {
            graph: LocatedGraphInputs {
                resources: inputs.resources,
                bases: inputs.bases,
            },
            cycles: inputs.cycles,
        }
    }
}
impl GraphRequest {
    /// Assemble graph and base metadata before producing a publishable completion.
    /// All work is synchronous and belongs on the acquisition worker.
    pub fn assemble_located<E>(
        self,
        inputs: Result<LocatedGraphInputs, E>,
        limits: BaseLimits,
    ) -> GraphCompletion<E, LocatedStylesheetGraph> {
        self.assemble_located_sources(inputs, None, limits)
    }
    /// Assemble source contexts, effective bases and active-URI cycle identities
    /// under this request's immutable failure policy. No previous graph is used.
    /// The borrowed cycle table is caller-owned storage and is not retained.
    pub fn assemble_located_with_cycles<E>(
        self,
        inputs: Result<LocatedGraphInputs, E>,
        cycles: &[ImportCycleId],
        limits: BaseLimits,
    ) -> GraphCompletion<E, LocatedStylesheetGraph> {
        self.assemble_located_sources(inputs, Some(cycles), limits)
    }
    fn assemble_located_sources<E>(
        self,
        inputs: Result<LocatedGraphInputs, E>,
        cycles: Option<&[ImportCycleId]>,
        limits: BaseLimits,
    ) -> GraphCompletion<E, LocatedStylesheetGraph> {
        match inputs {
            Err(error) => GraphCompletion {
                ticket: self.ticket,
                candidate: Err(GraphLoadError::Acquisition(error)),
            },
            Ok(inputs) => {
                let GraphCompletion { ticket, candidate } =
                    self.assemble_sources(Ok::<_, E>(inputs.resources), cycles);
                let candidate = candidate.and_then(|graph| {
                    LocatedStylesheetGraph::new(graph, inputs.bases, limits)
                        .map_err(GraphLoadError::Bases)
                });
                GraphCompletion { ticket, candidate }
            }
        }
    }
}

/// Opaque result/ticket pairing, suitable for transport in an application Message.
pub struct GraphCompletion<E, Candidate = OwnedStylesheetGraph> {
    ticket: StylesheetTicket,
    candidate: Result<Candidate, GraphLoadError<E>>,
}
#[cfg(unix)]
impl<E, Candidate> GraphCompletion<E, Candidate> {
    /// Internal preparation admission: preserve an existing graph error, returning
    /// the additional error for reporting after final admission.
    pub(super) fn reject_success(&mut self, error: E) -> Option<E> {
        if self.candidate.is_ok() {
            self.candidate = Err(GraphLoadError::Acquisition(error));
            None
        } else {
            Some(error)
        }
    }
}
impl<E: std::fmt::Debug, Candidate> std::fmt::Debug for GraphCompletion<E, Candidate> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphCompletion")
            .field("ticket", &self.ticket)
            .field("error", &self.candidate.as_ref().err())
            .finish_non_exhaustive()
    }
}
mod sealed {
    pub trait Sealed {}
    impl Sealed for iced_css_core::stylesheet::owned_graph::OwnedStylesheetGraph {}
    impl Sealed for iced_css_core::stylesheet::located_graph::LocatedStylesheetGraph {}
}
/// Candidate forms supported by the shared graph lifecycle. Sealed so callers
/// cannot substitute a graph whose content changes during publication/reporting.
pub trait GraphCandidate: sealed::Sealed {
    fn graph(&self) -> &OwnedStylesheetGraph;
}
impl GraphCandidate for OwnedStylesheetGraph {
    fn graph(&self) -> &OwnedStylesheetGraph {
        self
    }
}
impl GraphCandidate for LocatedStylesheetGraph {
    fn graph(&self) -> &OwnedStylesheetGraph {
        self.graph()
    }
}
pub type RuntimeStylesheetGraph = RuntimeGraph<OwnedStylesheetGraph>;
pub type RuntimeLocatedStylesheetGraph = RuntimeGraph<LocatedStylesheetGraph>;
pub struct RuntimeGraph<Candidate> {
    publication: CandidatePublication<Candidate>,
    /// Positive budget per diagnostic pass: parser recovery and import cuts.
    recovery_work: NonZeroUsize,
}
impl<Candidate: GraphCandidate> RuntimeGraph<Candidate> {
    pub fn new(limits: PublicationLimits, recovery_work: NonZeroUsize) -> Self {
        Self {
            publication: CandidatePublication::new(limits),
            recovery_work,
        }
    }
    /// Begin an authorized request. Initial load permits explicitly unavailable
    /// imports; a replacement requires all applicable imports to be available.
    /// This policy overrides the supplied expansion unavailable-import setting.
    pub fn request(
        &mut self,
        root: usize,
        context: RuleContext,
        mut limits: OwnedGraphLimits,
    ) -> Result<GraphRequest, PublicationError> {
        limits.expansion.unavailable = if self.publication.active().is_some() {
            UnavailableImportPolicy::Fail
        } else {
            UnavailableImportPolicy::Record
        };
        match self.publication.request() {
            Ok(ticket) => Ok(GraphRequest {
                ticket,
                root,
                context,
                limits,
            }),
            Err(error) => {
                log::error!(target: "iced_css::stylesheet", "stylesheet graph request rejected: {error}");
                Err(error)
            }
        }
    }
    pub fn active(&self) -> Option<&Candidate> {
        self.publication.active()
    }
    pub fn active_revision(&self) -> Option<ResourceRevision> {
        self.publication.active_revision()
    }
    pub fn status(&self) -> PublicationStatus {
        self.publication.status()
    }
    /// Read-only staged-request admission; no reservation or content validation.
    pub fn is_request_current(&self, request: &GraphRequest) -> bool {
        self.publication.is_current(&request.ticket)
    }
    /// Read-only preflight; no reservation, validation or revision guarantee.
    /// Asynchronous preparation must recheck this and still use finish.
    pub fn is_current<E>(&self, completion: &GraphCompletion<E, Candidate>) -> bool {
        self.publication.is_current(&completion.ticket)
    }
    /// Publish only a current owned completion. Failed reloads retain the complete
    /// previous graph, including its root. Reporting never rolls back accepted CSS.
    pub fn finish<E: std::fmt::Display>(
        &mut self,
        completion: GraphCompletion<E, Candidate>,
    ) -> Result<SheetCompletion<GraphLoadError<E>>, PublicationError> {
        self.finish_after_admission(completion, |_| {})
    }
    // Internal owners attach immutable evidence before application log callbacks
    // can run. The hook is not exposed as a public user callback API.
    pub(super) fn finish_after_admission<
        E: std::fmt::Display,
        F: FnOnce(&SheetCompletion<GraphLoadError<E>>),
    >(
        &mut self,
        completion: GraphCompletion<E, Candidate>,
        accepted: F,
    ) -> Result<SheetCompletion<GraphLoadError<E>>, PublicationError> {
        let generation = completion.ticket.generation();
        let result = self
            .publication
            .finish_with_previous(completion.ticket, completion.candidate);
        if let Ok((outcome, _)) = &result {
            accepted(outcome);
        }
        if let Ok((SheetCompletion::Published(_), previous)) = &result
            && let Some(active) = self.publication.active()
        {
            if let Err(error) = report_graph_recovery(
                active.graph(),
                previous.as_ref().map(GraphCandidate::graph),
                self.recovery_work,
            ) {
                log::error!(target: "iced_css::stylesheet", "stylesheet graph request {generation}: {error}");
            }
            if let Err(error) = report_import_cuts(
                active.graph(),
                previous.as_ref().map(GraphCandidate::graph),
                self.recovery_work,
            ) {
                log::error!(target: "iced_css::stylesheet", "stylesheet graph request {generation}: {error}");
            }
        }
        match &result {
            Ok((SheetCompletion::Failed(error), _)) => {
                log::error!(target: "iced_css::stylesheet", "stylesheet graph request {generation} failed: {error}");
            }
            Err(error) => {
                log::error!(target: "iced_css::stylesheet", "stylesheet graph request {generation} publication failed: {error}");
            }
            _ => {}
        }
        result.map(|(completion, _)| completion)
    }
}
