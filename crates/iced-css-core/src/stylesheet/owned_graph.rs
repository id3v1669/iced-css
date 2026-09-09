//! Owned, immutable backing sheets and compact import-occurrence locations.
use super::{
    Records, RuleAssemblyError, RuleContext, SheetEntryKind, UnicodeStylesheetCandidate,
    imports::{
        ImportBinding, ImportCut, ImportCycleId, LoadedGraphError, LoadedGraphLimits,
        LoadedImportPlan, LoadedSheet, OccurrenceRule, RuleLocation, expand_loaded_imports,
        expand_loaded_imports_with_cycles,
    },
};
use crate::{
    cascade::rule::{PositionedRuleOffer, RuleOrigin},
    syntax::imports::{OccurrenceId, ResourceId},
};
use std::num::NonZeroUsize;

/// Already-acquired input. Identity and binding resolution are supplied explicitly.
pub struct OwnedLoadedSheet {
    pub resource: ResourceId,
    pub candidate: UnicodeStylesheetCandidate,
    pub imports: Vec<ImportBinding>,
}
#[derive(Debug, Clone, Copy)]
pub struct OwnedGraphLimits {
    pub expansion: LoadedGraphLimits,
    /// Input table/binding capacities, temporary borrowed table, compact rules and
    /// moved cuts/visited indices. Parsed candidates keep separate intake budgets.
    /// The expansion planner's temporary records have their own budget.
    pub record_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug)]
pub enum OwnedGraphError {
    Budget(RuleAssemblyError),
    Expansion(LoadedGraphError),
}
impl From<RuleAssemblyError> for OwnedGraphError {
    fn from(error: RuleAssemblyError) -> Self {
        Self::Budget(error)
    }
}
impl std::fmt::Display for OwnedGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "owned stylesheet graph: {self:?}")
    }
}
impl std::error::Error for OwnedGraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Budget(error) => error,
            Self::Expansion(error) => error,
        })
    }
}
struct OwnedOccurrence {
    source: ResourceId,
    occurrence: OccurrenceId,
    context: RuleContext,
    location: RuleLocation,
}
/// A whole expanded candidate without self-references or cloned parsed properties.
/// No mutable access to backing sheets, bindings or structural addresses escapes.
pub struct OwnedStylesheetGraph {
    resources: Vec<OwnedLoadedSheet>,
    root: usize,
    rules: Vec<OwnedOccurrence>,
    cuts: Vec<ImportCut>,
    visited_sheets: Vec<usize>,
    occurrences: u64,
    next_context: Option<RuleContext>,
}
impl OwnedStylesheetGraph {
    /// Reuse the existing validated expander, then move its provenance into owned
    /// records. Failure drops the entire input candidate. This performs no I/O,
    /// URI normalization, publication or dependency-watch mutation.
    pub fn new(
        resources: Vec<OwnedLoadedSheet>,
        root: usize,
        context: RuleContext,
        limits: OwnedGraphLimits,
    ) -> Result<Self, OwnedGraphError> {
        Self::assemble(resources, None, root, context, limits)
    }
    /// Assemble with resolver-supplied active-URI identities distinct from source
    /// provenance. The borrowed table has exactly one token per input source and
    /// is not retained. Caller-owned table storage is outside graph record_bytes.
    pub fn new_with_cycles(
        resources: Vec<OwnedLoadedSheet>,
        cycles: &[ImportCycleId],
        root: usize,
        context: RuleContext,
        limits: OwnedGraphLimits,
    ) -> Result<Self, OwnedGraphError> {
        Self::assemble(resources, Some(cycles), root, context, limits)
    }
    fn assemble(
        resources: Vec<OwnedLoadedSheet>,
        cycles: Option<&[ImportCycleId]>,
        root: usize,
        context: RuleContext,
        limits: OwnedGraphLimits,
    ) -> Result<Self, OwnedGraphError> {
        if resources.len() > limits.expansion.entries.get() {
            return Err(OwnedGraphError::Expansion(LoadedGraphError::Entries));
        }
        if limits.record_bytes.get() > isize::MAX as usize {
            return Err(RuleAssemblyError::Bytes.into());
        }
        let mut records = Records {
            bytes: 0,
            limit: limits.record_bytes.get(),
            work: limits.work.get(),
        };
        charge_capacity::<OwnedLoadedSheet>(&mut records, resources.capacity())?;
        let mut loaded = Vec::new();
        for resource in &resources {
            records.spend(1)?;
            charge_capacity::<ImportBinding>(&mut records, resource.imports.capacity())?;
            records.reserve(&mut loaded)?;
            loaded.push(LoadedSheet {
                resource: resource.resource,
                sheet: resource.candidate.sheet(),
                imports: &resource.imports,
            });
        }
        let LoadedImportPlan {
            rules: borrowed_rules,
            cuts,
            visited_sheets,
            occurrences,
            next_context,
        } = match cycles {
            Some(cycles) => {
                expand_loaded_imports_with_cycles(&loaded, cycles, root, context, limits.expansion)
            }
            None => expand_loaded_imports(&loaded, root, context, limits.expansion),
        }
        .map_err(OwnedGraphError::Expansion)?;
        charge_capacity::<ImportCut>(&mut records, cuts.capacity())?;
        charge_capacity::<usize>(&mut records, visited_sheets.capacity())?;
        let mut rules = Vec::new();
        for rule in borrowed_rules {
            records.spend(1)?;
            records.reserve(&mut rules)?;
            rules.push(OwnedOccurrence {
                source: rule.source,
                occurrence: rule.occurrence,
                context: rule.context,
                location: rule.location,
            });
        }
        Ok(Self {
            resources,
            root,
            rules,
            cuts,
            visited_sheets,
            occurrences,
            next_context,
        })
    }
    pub fn resources(&self) -> &[OwnedLoadedSheet] {
        &self.resources
    }
    /// Only sheets reached by applicable imports, including the root and empty
    /// resources. Repeated occurrences reference the same resource once here.
    pub fn active_resources(&self) -> impl ExactSizeIterator<Item = &OwnedLoadedSheet> {
        self.visited_sheets
            .iter()
            .map(|index| &self.resources[*index])
    }
    pub fn root(&self) -> ResourceId {
        self.resources[self.root].resource
    }
    pub fn cuts(&self) -> &[ImportCut] {
        &self.cuts
    }
    pub fn occurrences(&self) -> u64 {
        self.occurrences
    }
    pub fn next_context(&self) -> Option<RuleContext> {
        self.next_context
    }
    /// Each address was produced by traversal of these exact backing sheets.
    /// The private immutable representation preserves that invariant after moves.
    pub fn rules(&self) -> impl ExactSizeIterator<Item = OccurrenceRule<'_>> {
        self.rules.iter().map(|record| {
            let location = record.location;
            let entry = &self.resources[location.sheet].candidate.sheet().entries[location.entry];
            let rule = match (&entry.kind, location.media_rule) {
                (SheetEntryKind::Style(rule), None) => rule,
                (SheetEntryKind::Media(media), Some(index)) => &media.rules[index],
                _ => unreachable!("locations are produced by the immutable graph's expander"),
            };
            OccurrenceRule {
                source: record.source,
                occurrence: record.occurrence,
                context: record.context,
                location,
                rule,
            }
        })
    }
    /// Recover a declaration's original source/property and import occurrence.
    /// Returned references borrow this graph. Work is bounded by visited rules;
    /// resource base URI association remains external.
    pub fn find_declaration(
        &self,
        identity: std::num::NonZeroU64,
        work: NonZeroUsize,
    ) -> Result<Option<super::imports::OccurrenceDeclaration<'_>>, super::imports::ProvenanceError>
    {
        super::imports::find_declaration_in(self.rules(), identity, work)
    }
    pub fn offers(&self, origin: RuleOrigin) -> impl Iterator<Item = PositionedRuleOffer<'_>> {
        self.rules().map(move |item| PositionedRuleOffer {
            rule: item.rule.offer(origin),
            context: item.context,
        })
    }
}
fn charge_capacity<T>(records: &mut Records, capacity: usize) -> Result<(), RuleAssemblyError> {
    records.bytes = capacity
        .checked_mul(size_of::<T>())
        .and_then(|bytes| records.bytes.checked_add(bytes))
        .filter(|bytes| *bytes <= records.limit)
        .ok_or(RuleAssemblyError::Bytes)?;
    Ok(())
}
