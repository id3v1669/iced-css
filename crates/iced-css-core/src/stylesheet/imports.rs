//! Expand already-loaded typed sheets. URI equivalence, resolution, acquisition,
//! byte caches and publication are external. No raw source data is cloned.
use super::{
    ParsedStyleRule, ParsedStylesheet, Records, RuleAssemblyError, RuleContext, SheetEntryKind,
};
use crate::{
    cascade::rule::{PositionedRuleOffer, RuleOrigin},
    syntax::imports::{
        ImportEntry, ImportLimits, ImportTrackError, ImportTracker, OccurrenceId, ResourceId,
    },
};
use std::num::{NonZeroU64, NonZeroUsize};

/// Resolver-supplied active-URI identity, scoped to one loaded graph.
/// Distinct decoded source contexts may share a token. It is not a provenance
/// identity or a claim that URI spellings/filesystem paths are equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportCycleId(NonZeroU64);
impl ImportCycleId {
    pub(super) fn tracker_key(self) -> ResourceId {
        ResourceId::new(self.0)
    }
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Immutable cycle metadata with an explicit bound on owned vector capacity.
/// Logical table length is validated against the graph during assembly.
#[derive(Debug)]
pub struct ImportCycleTable {
    values: Vec<ImportCycleId>,
}
impl ImportCycleTable {
    /// Admit existing storage without cloning or allocating. The supplied byte
    /// allowance covers this vector only, not graph records or worker metadata.
    pub fn new(
        values: Vec<ImportCycleId>,
        retained_bytes: NonZeroUsize,
    ) -> Result<Self, RuleAssemblyError> {
        if retained_bytes.get() > isize::MAX as usize
            || values
                .capacity()
                .checked_mul(size_of::<ImportCycleId>())
                .is_none_or(|bytes| bytes > retained_bytes.get())
        {
            return Err(RuleAssemblyError::Bytes);
        }
        Ok(Self { values })
    }
    pub fn as_slice(&self) -> &[ImportCycleId] {
        &self.values
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ImportResolution {
    Sheet(usize),
    /// Target need not be loaded when its explicit URI identity is active.
    /// A noncyclic occurrence remains unresolved and aborts assembly. Requires
    /// an explicit cycle table, never implicit conversion from source identity.
    DeferredCycle(ImportCycleId),
    /// Unavailable acquisition with explicit URI identity. Active occurrences
    /// still cut as cycles; nonactive occurrences use the unavailable policy.
    UnavailableWithCycle(ImportCycleId),
    /// The resolver explicitly supplied an unavailable acquisition result.
    Unavailable,
}
#[derive(Debug, Clone, Copy)]
pub struct ImportBinding {
    /// Top-level entry index in this sheet; must identify an import entry.
    pub entry: usize,
    pub resolution: ImportResolution,
}
#[derive(Debug)]
pub struct LoadedSheet<'a> {
    /// Supplied resource identity, unique in this table. URI equivalence is not
    /// inferred from the spelling of the import prelude.
    pub resource: ResourceId,
    pub sheet: &'a ParsedStylesheet,
    pub imports: &'a [ImportBinding],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnavailableImportPolicy {
    /// Runtime evaluation can retain a located unavailable-import observation.
    Record,
    /// Required by Compile: an unavailable applicable import aborts the candidate.
    Fail,
}
#[derive(Debug, Clone, Copy)]
pub struct LoadedGraphLimits {
    pub unavailable: UnavailableImportPolicy,
    /// Bounds loaded inputs, total bindings, retained rules/cuts, visited sheets, and stack depth
    /// separately. Borrowed input storage is not included in record_bytes.
    pub entries: NonZeroUsize,
    pub record_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
    pub tracking: ImportLimits,
}
#[derive(Debug, Clone, Copy)]
pub struct OccurrenceRule<'a> {
    pub source: ResourceId,
    pub occurrence: OccurrenceId,
    pub rule: &'a ParsedStyleRule,
    pub context: RuleContext,
    pub location: RuleLocation,
}
/// Structural address within the immutable loaded table used for expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleLocation {
    pub sheet: usize,
    pub entry: usize,
    pub media_rule: Option<usize>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportCutReason {
    Cycle,
    MediaExcluded,
    Unavailable,
}
#[derive(Debug)]
pub struct ImportCut {
    pub source: ResourceId,
    pub occurrence: OccurrenceId,
    pub entry: usize,
    pub target: Option<ResourceId>,
    pub reason: ImportCutReason,
}
#[derive(Debug)]
pub struct LoadedImportPlan<'a> {
    pub rules: Vec<OccurrenceRule<'a>>,
    pub cuts: Vec<ImportCut>,
    /// Unique loaded-table indices, including empty sheets, in first-visit order.
    pub visited_sheets: Vec<usize>,
    pub occurrences: u64,
    pub next_context: Option<RuleContext>,
}
#[derive(Debug)]
pub struct OccurrenceDeclaration<'a> {
    pub source: ResourceId,
    pub occurrence: OccurrenceId,
    pub order: u64,
    pub identity: NonZeroU64,
    pub parsed: &'a crate::values::property::ParsedProperty,
    pub location: &'a super::DeclarationLocation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceError {
    Work,
    InvalidLocations,
    Identity,
}
impl std::fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "declaration provenance: {self:?}")
    }
}
impl std::error::Error for ProvenanceError {}
impl<'a> LoadedImportPlan<'a> {
    /// Resolve a winning declaration identity to source provenance, including
    /// the original shorthand declaration. Does not infer its resource base URI.
    /// A lookup visits at most work rule records and performs no allocation.
    pub fn find_declaration(
        &self,
        identity: NonZeroU64,
        work: NonZeroUsize,
    ) -> Result<Option<OccurrenceDeclaration<'a>>, ProvenanceError> {
        find_declaration_in(self.rules.iter().copied(), identity, work)
    }
}
pub(super) fn find_declaration_in<'a>(
    rules: impl IntoIterator<Item = OccurrenceRule<'a>>,
    identity: NonZeroU64,
    work: NonZeroUsize,
) -> Result<Option<OccurrenceDeclaration<'a>>, ProvenanceError> {
    let mut work = work.get();
    for item in rules {
        work = work.checked_sub(1).ok_or(ProvenanceError::Work)?;
        let Some(ordinal) = identity
            .get()
            .checked_sub(item.context.first_declaration.get())
            .and_then(|delta| usize::try_from(delta).ok())
        else {
            continue;
        };
        let rule = item.rule;
        let Some(declaration) = rule.declarations.get(ordinal) else {
            continue;
        };
        let context = item
            .context
            .offset(ordinal)
            .ok_or(ProvenanceError::Identity)?;
        let location = rule
            .locations
            .get(ordinal)
            .ok_or(ProvenanceError::InvalidLocations)?;
        return Ok(Some(OccurrenceDeclaration {
            source: item.source,
            occurrence: item.occurrence,
            order: context.first_order,
            identity,
            parsed: &declaration.parsed,
            location,
        }));
    }
    Ok(None)
}
impl LoadedImportPlan<'_> {
    /// Imported rules inherit the root's origin and author-sheet identity.
    pub fn offers(&self, origin: RuleOrigin) -> impl Iterator<Item = PositionedRuleOffer<'_>> {
        self.rules.iter().map(move |item| PositionedRuleOffer {
            rule: item.rule.offer(origin),
            context: item.context,
        })
    }
}
#[derive(Debug)]
pub enum LoadedGraphError {
    /// Missing required explicit cycle table, or table length differs from sources.
    CycleIdentities,
    Root,
    Entries,
    InvalidBinding,
    DuplicateBinding,
    DuplicateResource,
    UnresolvedImport {
        sheet: usize,
        entry: usize,
    },
    UnavailableImport {
        sheet: usize,
        entry: usize,
    },
    Budget(RuleAssemblyError),
    Tracking(ImportTrackError),
}
impl From<RuleAssemblyError> for LoadedGraphError {
    fn from(error: RuleAssemblyError) -> Self {
        Self::Budget(error)
    }
}
impl From<ImportTrackError> for LoadedGraphError {
    fn from(error: ImportTrackError) -> Self {
        Self::Tracking(error)
    }
}
impl std::fmt::Display for LoadedGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "loaded import graph: {self:?}")
    }
}
impl std::error::Error for LoadedGraphError {}
#[derive(Clone, Copy)]
struct Frame {
    sheet: usize,
    next: usize,
    occurrence: OccurrenceId,
}
struct Builder<'a> {
    limits: LoadedGraphLimits,
    records: Records,
    frames: Vec<Frame>,
    rules: Vec<OccurrenceRule<'a>>,
    cuts: Vec<ImportCut>,
    visited_sheets: Vec<usize>,
    context: Option<RuleContext>,
    tracker: ImportTracker,
}
/// Expand the root with depth-first import ordering and active-path cycle cuts.
/// Every applicable encountered import needs an explicit binding. Inactive media
/// imports require no resolution. Declaration positions are assigned before target
/// matching; empty rules and page context emit no screen declaration offers.
///
/// Bounds are hierarchical: the planner has cumulative validation/traversal work
/// and record capacities; the tracker owns separate bounded per-operation work and
/// storage. Temporary old/new vector growth overlap is excluded. Any failure drops
/// the whole local candidate, leaving borrowed source sheets unchanged.
pub fn expand_loaded_imports<'a>(
    loaded: &[LoadedSheet<'a>],
    root: usize,
    context: RuleContext,
    limits: LoadedGraphLimits,
) -> Result<LoadedImportPlan<'a>, LoadedGraphError> {
    expand_with_cycles(loaded, None, root, context, limits)
}

/// Expand using explicit active-URI identities independently of parsed-source
/// provenance. The borrowed identity table must match the loaded table exactly.
/// Repeated identities cut only while active; later occurrences remain distinct.
/// Identity storage belongs to the caller; traversal uses the existing budgets.
pub fn expand_loaded_imports_with_cycles<'a>(
    loaded: &[LoadedSheet<'a>],
    cycles: &[ImportCycleId],
    root: usize,
    context: RuleContext,
    limits: LoadedGraphLimits,
) -> Result<LoadedImportPlan<'a>, LoadedGraphError> {
    expand_with_cycles(loaded, Some(cycles), root, context, limits)
}

fn expand_with_cycles<'a>(
    loaded: &[LoadedSheet<'a>],
    cycles: Option<&[ImportCycleId]>,
    root: usize,
    context: RuleContext,
    limits: LoadedGraphLimits,
) -> Result<LoadedImportPlan<'a>, LoadedGraphError> {
    if cycles.is_some_and(|cycles| cycles.len() != loaded.len()) {
        return Err(LoadedGraphError::CycleIdentities);
    }
    let cycle_key = |index: usize| {
        cycles.map_or_else(
            || loaded[index].resource,
            |cycles| ResourceId::new(cycles[index].0),
        )
    };
    if root >= loaded.len() {
        return Err(LoadedGraphError::Root);
    }
    if loaded.len() > limits.entries.get() {
        return Err(LoadedGraphError::Entries);
    }
    if limits.record_bytes.get() > isize::MAX as usize {
        return Err(RuleAssemblyError::Bytes.into());
    }
    let mut builder = Builder {
        limits,
        records: Records {
            bytes: 0,
            limit: limits.record_bytes.get(),
            work: limits.work.get(),
        },
        frames: Vec::new(),
        rules: Vec::new(),
        cuts: Vec::new(),
        visited_sheets: Vec::new(),
        context: Some(context),
        tracker: ImportTracker::new(limits.tracking),
    };
    builder.validate(loaded, cycles.is_some())?;
    let ImportEntry::Visit(root_visit) = builder.tracker.enter(cycle_key(root), true)? else {
        unreachable!("fresh tracker root")
    };
    builder.push(Frame {
        sheet: root,
        next: 0,
        occurrence: root_visit.occurrence,
    })?;
    while let Some(frame) = builder.frames.last_mut() {
        builder.records.spend(1)?;
        let current = *frame;
        let source = &loaded[current.sheet];
        let Some(entry) = source.sheet.entries.get(current.next) else {
            builder.tracker.finish(current.occurrence)?;
            builder.frames.pop();
            continue;
        };
        // The selected index is below Vec::len(), so this cannot wrap usize.
        frame.next += 1;
        match &entry.kind {
            SheetEntryKind::Style(rule) => builder.rule(
                source.resource,
                current.occurrence,
                rule,
                RuleLocation {
                    sheet: current.sheet,
                    entry: current.next,
                    media_rule: None,
                },
            )?,
            SheetEntryKind::Media(media) if media.applies_to_screen => {
                for (index, rule) in media.rules.iter().enumerate() {
                    builder.rule(
                        source.resource,
                        current.occurrence,
                        rule,
                        RuleLocation {
                            sheet: current.sheet,
                            entry: current.next,
                            media_rule: Some(index),
                        },
                    )?;
                }
            }
            SheetEntryKind::Import(import) => {
                if !import.applies_to_screen {
                    builder.cut(
                        source.resource,
                        current,
                        None,
                        ImportCutReason::MediaExcluded,
                    )?;
                    continue;
                }
                let mut resolution = None;
                for binding in source.imports {
                    builder.records.spend(1)?;
                    if binding.entry == current.next {
                        resolution = Some(binding.resolution);
                        break;
                    }
                }
                let resolution = resolution.ok_or(LoadedGraphError::UnresolvedImport {
                    sheet: current.sheet,
                    entry: current.next,
                })?;
                match resolution {
                    ImportResolution::DeferredCycle(key) => {
                        if builder.tracker.is_active(ResourceId::new(key.0))? {
                            builder.cut(source.resource, current, None, ImportCutReason::Cycle)?;
                        } else {
                            return Err(LoadedGraphError::UnresolvedImport {
                                sheet: current.sheet,
                                entry: current.next,
                            });
                        }
                    }
                    ImportResolution::UnavailableWithCycle(key) => {
                        if builder.tracker.is_active(ResourceId::new(key.0))? {
                            builder.cut(source.resource, current, None, ImportCutReason::Cycle)?;
                        } else {
                            builder.unavailable(source.resource, current)?;
                        }
                    }
                    ImportResolution::Unavailable => {
                        builder.unavailable(source.resource, current)?
                    }
                    ImportResolution::Sheet(index) => {
                        let target = loaded[index].resource;
                        match builder.tracker.enter(cycle_key(index), true)? {
                            ImportEntry::Cycle => builder.cut(
                                source.resource,
                                current,
                                Some(target),
                                ImportCutReason::Cycle,
                            )?,
                            ImportEntry::Visit(visit) => builder.push(Frame {
                                sheet: index,
                                next: 0,
                                occurrence: visit.occurrence,
                            })?,
                            ImportEntry::MediaExcluded => unreachable!("explicit active import"),
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(LoadedImportPlan {
        rules: builder.rules,
        cuts: builder.cuts,
        visited_sheets: builder.visited_sheets,
        occurrences: builder.tracker.admitted_occurrences(),
        next_context: builder.context,
    })
}
impl<'a> Builder<'a> {
    fn unavailable(&mut self, source: ResourceId, current: Frame) -> Result<(), LoadedGraphError> {
        if self.limits.unavailable == UnavailableImportPolicy::Fail {
            return Err(LoadedGraphError::UnavailableImport {
                sheet: current.sheet,
                entry: current.next,
            });
        }
        self.cut(source, current, None, ImportCutReason::Unavailable)
    }

    fn validate(
        &mut self,
        loaded: &[LoadedSheet<'_>],
        explicit_cycles: bool,
    ) -> Result<(), LoadedGraphError> {
        let mut bindings = 0usize;
        for (index, source) in loaded.iter().enumerate() {
            self.records.spend(1)?;
            for prior in &loaded[..index] {
                self.records.spend(1)?;
                if source.resource == prior.resource {
                    return Err(LoadedGraphError::DuplicateResource);
                }
            }
            bindings = bindings
                .checked_add(source.imports.len())
                .filter(|count| *count <= self.limits.entries.get())
                .ok_or(LoadedGraphError::Entries)?;
            for (index, binding) in source.imports.iter().enumerate() {
                self.records.spend(1)?;
                if !explicit_cycles
                    && matches!(
                        binding.resolution,
                        ImportResolution::DeferredCycle(_)
                            | ImportResolution::UnavailableWithCycle(_)
                    )
                {
                    return Err(LoadedGraphError::CycleIdentities);
                }
                if !matches!(
                    source
                        .sheet
                        .entries
                        .get(binding.entry)
                        .map(|entry| &entry.kind),
                    Some(SheetEntryKind::Import(_))
                ) || matches!(binding.resolution, ImportResolution::Sheet(index) if index >= loaded.len())
                {
                    return Err(LoadedGraphError::InvalidBinding);
                }
                for prior in &source.imports[..index] {
                    self.records.spend(1)?;
                    if prior.entry == binding.entry {
                        return Err(LoadedGraphError::DuplicateBinding);
                    }
                }
            }
        }
        Ok(())
    }
    fn push(&mut self, frame: Frame) -> Result<(), LoadedGraphError> {
        if self.frames.len() == self.limits.entries.get() {
            return Err(LoadedGraphError::Entries);
        }
        let mut visited = false;
        for prior in &self.visited_sheets {
            self.records.spend(1)?;
            if *prior == frame.sheet {
                visited = true;
                break;
            }
        }
        if !visited {
            if self.visited_sheets.len() == self.limits.entries.get() {
                return Err(LoadedGraphError::Entries);
            }
            self.records.reserve(&mut self.visited_sheets)?;
            self.visited_sheets.push(frame.sheet);
        }
        self.records.reserve(&mut self.frames)?;
        self.frames.push(frame);
        Ok(())
    }
    fn output_room(&self) -> Result<(), LoadedGraphError> {
        if self
            .rules
            .len()
            .checked_add(self.cuts.len())
            .is_none_or(|count| count >= self.limits.entries.get())
        {
            return Err(LoadedGraphError::Entries);
        }
        Ok(())
    }
    fn cut(
        &mut self,
        source: ResourceId,
        frame: Frame,
        target: Option<ResourceId>,
        reason: ImportCutReason,
    ) -> Result<(), LoadedGraphError> {
        self.output_room()?;
        self.records.reserve(&mut self.cuts)?;
        self.cuts.push(ImportCut {
            source,
            occurrence: frame.occurrence,
            entry: frame.next,
            target,
            reason,
        });
        Ok(())
    }
    fn rule(
        &mut self,
        source: ResourceId,
        occurrence: OccurrenceId,
        rule: &'a ParsedStyleRule,
        location: RuleLocation,
    ) -> Result<(), LoadedGraphError> {
        self.records.spend(1)?;
        if rule.declarations.is_empty() {
            return Ok(());
        }
        self.output_room()?;
        let context = self.context.ok_or(RuleAssemblyError::Identity)?;
        context
            .offset(rule.declarations.len() - 1)
            .ok_or(RuleAssemblyError::Identity)?;
        self.records.reserve(&mut self.rules)?;
        self.rules.push(OccurrenceRule {
            source,
            occurrence,
            rule,
            context,
            location,
        });
        self.context = context.offset(rule.declarations.len());
        Ok(())
    }
}
