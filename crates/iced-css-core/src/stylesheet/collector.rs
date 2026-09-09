//! Serial import collection with explicit provider identities and acquisition.
use super::{
    Records, RuleAssemblyError, UnicodeStylesheetCandidate,
    imports::{ImportBinding, ImportCycleId, ImportCycleTable, ImportResolution},
    located_graph::StylesheetBase,
    owned_graph::OwnedLoadedSheet,
    requests::{
        ImportRequestError, ImportRequestLimits, ResolvedImportRequest, ResolvedUnicodeImports,
        resolve_unicode_import_requests,
    },
};
use crate::syntax::{
    encoding_selection::EncodingMetadata,
    imports::{
        ImportEntry, ImportLimits, ImportTrackError, ImportTracker, OccurrenceId, ResourceId,
    },
};
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportTarget {
    /// Provider identity for the parsed context, including decoder/base semantics.
    pub source: ResourceId,
    pub cycle: ImportCycleId,
}
pub struct AcquiredStylesheet {
    pub candidate: UnicodeStylesheetCandidate,
    pub effective_base: String,
}
pub struct CollectorRoot {
    pub target: ImportTarget,
    pub input: AcquiredStylesheet,
}
#[derive(Debug, Clone, Copy)]
pub struct ImportProbe<'a> {
    pub parent: ResourceId,
    /// Actual admitted parent base, including resolver/redirect base changes.
    pub effective_base: &'a str,
    pub request: &'a ResolvedImportRequest,
    /// Only the parent's selected referring charset is supplied here. The
    /// provider adds actual child transport/linking metadata before decoding.
    pub encoding: EncodingMetadata<'static>,
}
/// Identify before parsed-candidate acquisition, with stable contextual identities.
/// Identity resolution may stage transport metadata/bytes (e.g. redirects/charset);
/// providers must budget and share that work independently of parsed contexts.
/// Provider operations own their I/O, intake, cache and authorization limits.
/// None means explicit unavailability; Err aborts the complete collection.
/// Do not log stale worker results before publication admission. Generic
/// unavailable cuts do not preserve richer provider-specific error details.
pub trait ImportProvider {
    type Error;
    fn identify(&mut self, probe: ImportProbe<'_>) -> Result<ImportTarget, Self::Error>;
    fn acquire(
        &mut self,
        target: ImportTarget,
        probe: ImportProbe<'_>,
    ) -> Result<Option<AcquiredStylesheet>, Self::Error>;
}
#[derive(Debug, Clone, Copy)]
pub struct CollectorLimits {
    /// Separately bounds known targets, loaded sources, total applicable import
    /// entries and active stack depth. Repeated visits use tracking.occurrences.
    pub entries: NonZeroUsize,
    /// Collector/result records, bases and resolved request capacities. Charges
    /// for discarded temporary storage may remain conservative. Nested parsed
    /// candidates use provider intake budgets; vector growth overlap is excluded.
    pub record_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
    pub requests: ImportRequestLimits,
    pub tracking: ImportLimits,
    pub cycle_bytes: NonZeroUsize,
}
pub struct CollectedStylesheets {
    /// Root is at index zero. These are inputs, not an expanded/published graph.
    pub resources: Vec<OwnedLoadedSheet>,
    pub bases: Vec<StylesheetBase>,
    pub cycles: ImportCycleTable,
}
#[derive(Debug)]
pub enum CollectorBuildError {
    Budget(RuleAssemblyError),
    Requests {
        source: ResourceId,
        error: ImportRequestError,
    },
    Tracking(ImportTrackError),
    CycleIdentity {
        source: ResourceId,
    },
    Traversal,
}
impl From<RuleAssemblyError> for CollectorBuildError {
    fn from(error: RuleAssemblyError) -> Self {
        Self::Budget(error)
    }
}
impl From<ImportTrackError> for CollectorBuildError {
    fn from(error: ImportTrackError) -> Self {
        Self::Tracking(error)
    }
}
impl std::fmt::Display for CollectorBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "import collection: {self:?}")
    }
}
impl std::error::Error for CollectorBuildError {}
#[derive(Debug)]
pub enum CollectionError<E> {
    Build(CollectorBuildError),
    Provider {
        source: ResourceId,
        entry: usize,
        error: E,
    },
}
impl<E> From<CollectorBuildError> for CollectionError<E> {
    fn from(error: CollectorBuildError) -> Self {
        Self::Build(error)
    }
}
impl<E: std::fmt::Display> std::fmt::Display for CollectionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Build(error) => error.fmt(f),
            Self::Provider {
                source,
                entry,
                error,
            } => write!(f, "import {source:?} entry {entry}: {error}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for CollectionError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Build(error) => error,
            Self::Provider { error, .. } => error,
        })
    }
}
#[derive(Clone, Copy)]
enum TargetState {
    Pending,
    Loaded(usize),
    Unavailable,
}
#[derive(Clone, Copy)]
struct KnownTarget {
    target: ImportTarget,
    state: TargetState,
}
#[derive(Clone, Copy)]
struct Binding {
    entry: usize,
    known: usize,
}
struct PendingSheet {
    target: ImportTarget,
    input: AcquiredStylesheet,
    requests: ResolvedUnicodeImports,
    bindings: Vec<Binding>,
}
#[derive(Clone, Copy)]
struct Frame {
    sheet: usize,
    next: usize,
    occurrence: OccurrenceId,
}
struct State {
    limits: CollectorLimits,
    records: Records,
    tracker: ImportTracker,
    known: Vec<KnownTarget>,
    sheets: Vec<PendingSheet>,
    frames: Vec<Frame>,
    request_entries: usize,
}
impl State {
    fn new(limits: CollectorLimits) -> Result<Self, CollectorBuildError> {
        if limits.record_bytes.get() > isize::MAX as usize
            || limits.cycle_bytes.get() > isize::MAX as usize
            || limits.cycle_bytes.get() < size_of::<ImportCycleId>()
        {
            return Err(RuleAssemblyError::Bytes.into());
        }
        Ok(Self {
            limits,
            records: Records {
                bytes: 0,
                limit: limits.record_bytes.get(),
                work: limits.work.get(),
            },
            tracker: ImportTracker::new(limits.tracking),
            known: Vec::new(),
            sheets: Vec::new(),
            frames: Vec::new(),
            request_entries: 0,
        })
    }
    fn known_target(&mut self, target: ImportTarget) -> Result<usize, CollectorBuildError> {
        for (index, known) in self.known.iter().enumerate() {
            self.records.spend(1)?;
            if known.target.source == target.source {
                if known.target.cycle != target.cycle {
                    return Err(CollectorBuildError::CycleIdentity {
                        source: target.source,
                    });
                }
                return Ok(index);
            }
        }
        if self.known.len() == self.limits.entries.get() {
            return Err(RuleAssemblyError::Entries.into());
        }
        self.records.reserve(&mut self.known)?;
        let index = self.known.len();
        self.known.push(KnownTarget {
            target,
            state: TargetState::Pending,
        });
        Ok(index)
    }
    fn add_sheet(
        &mut self,
        target: ImportTarget,
        input: AcquiredStylesheet,
    ) -> Result<usize, CollectorBuildError> {
        if self.sheets.len() == self.limits.entries.get() {
            return Err(RuleAssemblyError::Entries.into());
        }
        self.sheets
            .len()
            .checked_add(1)
            .and_then(|count| count.checked_mul(size_of::<ImportCycleId>()))
            .filter(|bytes| *bytes <= self.limits.cycle_bytes.get())
            .ok_or(RuleAssemblyError::Bytes)?;
        self.records.reserve(&mut self.sheets)?;
        self.records.retain(input.effective_base.capacity())?;
        let requests = resolve_unicode_import_requests(
            &input.candidate,
            &input.effective_base,
            self.limits.requests,
        )
        .map_err(|error| CollectorBuildError::Requests {
            source: target.source,
            error,
        })?;
        self.request_entries = self
            .request_entries
            .checked_add(requests.requests().len())
            .filter(|entries| *entries <= self.limits.entries.get())
            .ok_or(RuleAssemblyError::Entries)?;
        requests.charge_retained(&mut self.records)?;
        let index = self.sheets.len();
        self.sheets.push(PendingSheet {
            target,
            input,
            requests,
            bindings: Vec::new(),
        });
        Ok(index)
    }
    fn probe(&self, sheet: usize, request: usize) -> ImportProbe<'_> {
        let sheet = &self.sheets[sheet];
        ImportProbe {
            parent: sheet.target.source,
            effective_base: &sheet.input.effective_base,
            request: &sheet.requests.requests()[request],
            encoding: sheet.requests.encoding_metadata(None, None),
        }
    }
    fn binding(
        &mut self,
        sheet: usize,
        entry: usize,
    ) -> Result<Option<usize>, CollectorBuildError> {
        for binding in &self.sheets[sheet].bindings {
            self.records.spend(1)?;
            if binding.entry == entry {
                return Ok(Some(binding.known));
            }
        }
        Ok(None)
    }
    fn bind(
        &mut self,
        sheet: usize,
        entry: usize,
        known: usize,
    ) -> Result<(), CollectorBuildError> {
        self.records.reserve(&mut self.sheets[sheet].bindings)?;
        self.sheets[sheet].bindings.push(Binding { entry, known });
        Ok(())
    }
    fn enter(&mut self, sheet: usize) -> Result<(), CollectorBuildError> {
        if self.frames.len() == self.limits.entries.get() {
            return Err(RuleAssemblyError::Entries.into());
        }
        self.records.reserve(&mut self.frames)?;
        let ImportEntry::Visit(visit) = self
            .tracker
            .enter(self.sheets[sheet].target.cycle.tracker_key(), true)?
        else {
            return Err(CollectorBuildError::Traversal);
        };
        self.frames.push(Frame {
            sheet,
            next: 0,
            occurrence: visit.occurrence,
        });
        Ok(())
    }
    fn next(&mut self) -> Result<Option<(usize, usize)>, CollectorBuildError> {
        while let Some(frame) = self.frames.last_mut() {
            self.records.spend(1)?;
            let current = *frame;
            if current.next == self.sheets[current.sheet].requests.requests().len() {
                self.tracker.finish(current.occurrence)?;
                self.frames.pop();
            } else {
                frame.next += 1;
                return Ok(Some((current.sheet, current.next)));
            }
        }
        Ok(None)
    }
    fn finish(self) -> Result<CollectedStylesheets, CollectorBuildError> {
        let Self {
            limits,
            mut records,
            sheets,
            known,
            ..
        } = self;
        let mut resources = Vec::new();
        let mut bases = Vec::new();
        let mut cycles = Vec::new();
        for sheet in sheets {
            records.spend(1)?;
            records.reserve(&mut resources)?;
            records.reserve(&mut bases)?;
            records.reserve_with_capacity_limit(&mut cycles, limits.cycle_bytes.get())?;
            let mut imports = Vec::new();
            for binding in sheet.bindings {
                records.spend(1)?;
                records.reserve(&mut imports)?;
                let target = known[binding.known];
                let resolution = match target.state {
                    TargetState::Pending => ImportResolution::DeferredCycle(target.target.cycle),
                    TargetState::Loaded(index) => ImportResolution::Sheet(index),
                    TargetState::Unavailable => {
                        ImportResolution::UnavailableWithCycle(target.target.cycle)
                    }
                };
                imports.push(ImportBinding {
                    entry: binding.entry,
                    resolution,
                });
            }
            resources.push(OwnedLoadedSheet {
                resource: sheet.target.source,
                candidate: sheet.input.candidate,
                imports,
            });
            bases.push(StylesheetBase {
                resource: sheet.target.source,
                effective_base: sheet.input.effective_base,
            });
            cycles.push(sheet.target.cycle);
        }
        Ok(CollectedStylesheets {
            resources,
            bases,
            cycles: ImportCycleTable::new(cycles, limits.cycle_bytes)?,
        })
    }
}

/// Collect fresh inputs serially. Identify each source import once, acquire each
/// needed parsed context at most once, and revisit shared sources per ancestry.
/// Cyclic contexts need not be acquired. Known unavailability is not retried.
///
/// No old graph is supplied. Apply the result through owned/located expansion and
/// the appropriate initial/Compile/replacement policy before publication. This
/// collector neither validates URI identity claims nor supplies transport routing.
pub fn collect_imports<P: ImportProvider>(
    root: CollectorRoot,
    provider: &mut P,
    limits: CollectorLimits,
) -> Result<CollectedStylesheets, CollectionError<P::Error>> {
    let mut state = State::new(limits)?;
    let root_sheet = state.add_sheet(root.target, root.input)?;
    let root_known = state.known_target(root.target)?;
    state.known[root_known].state = TargetState::Loaded(root_sheet);
    state.enter(root_sheet)?;
    while let Some((sheet, request)) = state.next()? {
        let source = state.sheets[sheet].target.source;
        let entry = state.sheets[sheet].requests.requests()[request].entry;
        let known = match state.binding(sheet, entry)? {
            Some(index) => index,
            None => {
                let target = provider
                    .identify(state.probe(sheet, request))
                    .map_err(|error| CollectionError::Provider {
                        source,
                        entry,
                        error,
                    })?;
                let index = state.known_target(target)?;
                state.bind(sheet, entry, index)?;
                index
            }
        };
        let target = state.known[known].target;
        if state
            .tracker
            .is_active(target.cycle.tracker_key())
            .map_err(CollectorBuildError::Tracking)?
        {
            continue;
        }
        if matches!(state.known[known].state, TargetState::Pending) {
            let acquired = provider
                .acquire(target, state.probe(sheet, request))
                .map_err(|error| CollectionError::Provider {
                    source,
                    entry,
                    error,
                })?;
            state.known[known].state = match acquired {
                Some(input) => TargetState::Loaded(state.add_sheet(target, input)?),
                None => TargetState::Unavailable,
            };
        }
        if let TargetState::Loaded(index) = state.known[known].state {
            state.enter(index)?;
        }
    }
    Ok(state.finish()?)
}
