//! CF-34 private completed-observation capture. Nothing here publishes a report.
use super::{
    clearance_box_facts::{BoxFactOrigin, BoxRowObserver, ClearanceBoxFact, fallback_x_height},
    sibling_source_projection::SiblingSourceProjection,
    source_display::SourceDisplay,
};
use crate::diagnostics::border::{
    BorderCaptureLimits, BorderCaptureReceipt, BorderCaptureStamp, BorderDeclarationBinding,
    BorderLimit, BorderObservation, BorderObservationBatch, BorderPendingEvent, BorderReporter,
    BorderReportingError, OwnedBorderValue,
};
use iced_css_core::{
    cascade::winner_table::PropertyWinners,
    diagnostics::{Provenance, SourceSpan},
    identity::InstanceId,
    stylesheet::owned_graph::OwnedStylesheetGraph,
    syntax::imports::{OccurrenceId, ResourceId},
    values::{
        BorderLengthView, LengthTarget, RangeEvent, RelativeUnit,
        property::{ParsedProperty, SpecifiedValue},
        registry::PropertyId,
    },
};
use std::{
    collections::TryReserveError,
    mem::size_of,
    num::{NonZeroU64, NonZeroUsize},
    sync::{Arc, atomic::AtomicUsize},
};

type Error = BorderReportingError;
type Result<T> = std::result::Result<T, Error>;

// Same qualified Arc payload layout as diagnostics::queue. This is an owned
// payload account, not allocator bookkeeping or a total process-memory bound.
#[repr(C, align(2))]
struct ChargedArcProvenance {
    _strong: AtomicUsize,
    _weak: AtomicUsize,
    _data: Provenance,
}

pub(super) trait Reservation {
    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        count: usize,
    ) -> std::result::Result<(), TryReserveError>;
}
pub(super) struct StandardReservation;
impl Reservation for StandardReservation {
    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        count: usize,
    ) -> std::result::Result<(), TryReserveError> {
        values.try_reserve_exact(count)
    }
}

struct Work {
    used: usize,
    limit: usize,
    kind: BorderLimit,
}
impl Work {
    fn new(limit: NonZeroUsize, kind: BorderLimit) -> Self {
        Self {
            used: 0,
            limit: limit.get(),
            kind,
        }
    }
    fn charge(&mut self, amount: usize) -> Result<()> {
        self.used = self
            .used
            .checked_add(amount)
            .filter(|&value| value <= self.limit)
            .ok_or(Error::Limit(self.kind))?;
        Ok(())
    }
}
fn plus(a: usize, b: usize, kind: BorderLimit) -> Result<usize> {
    a.checked_add(b).ok_or(Error::Limit(kind))
}
fn times(a: usize, b: usize, kind: BorderLimit) -> Result<usize> {
    a.checked_mul(b).ok_or(Error::Limit(kind))
}
fn within(value: usize, limit: usize, kind: BorderLimit) -> Result<usize> {
    if value > isize::MAX as usize || value > limit {
        Err(Error::Limit(kind))
    } else {
        Ok(value)
    }
}
fn pair_count(count: usize, kind: BorderLimit) -> Result<usize> {
    let previous = count.saturating_sub(1);
    if count.is_multiple_of(2) {
        times(count / 2, previous, kind)
    } else {
        times(count, previous / 2, kind)
    }
}

#[derive(Clone, Copy)]
enum RawSide<'r> {
    Silent {
        property: PropertyId,
    },
    Event {
        property: PropertyId,
        declaration: NonZeroU64,
        value: &'r LengthTarget,
        font: Option<(u32, u32)>,
        result_bits: u32,
        event: RangeEvent,
        binding: Option<usize>,
    },
}
#[derive(Clone, Copy)]
struct RawRow<'r> {
    sides: [RawSide<'r>; 4],
    instance: Option<InstanceId>,
}

pub(super) struct Collector<'r, R = StandardReservation> {
    raw: Vec<Option<RawRow<'r>>>,
    stamp: Option<BorderCaptureStamp>,
    error: Option<Error>,
    limits: BorderCaptureLimits,
    capture_bytes: usize,
    capture_work: usize,
    events: usize,
    /// The first recorded row's font size: the root's, which `rem` measures.
    root_font_size: Option<f32>,
    reservation: R,
}
impl<'r> Collector<'r> {
    pub(super) fn new(
        count: usize,
        reporter: &BorderReporter<'_>,
        limits: BorderCaptureLimits,
    ) -> Self {
        Self::with_reservation(count, reporter, limits, StandardReservation)
    }
}
impl<'r, R: Reservation> Collector<'r, R> {
    fn with_reservation(
        count: usize,
        reporter: &BorderReporter<'_>,
        limits: BorderCaptureLimits,
        reservation: R,
    ) -> Self {
        let mut this = Self {
            raw: Vec::new(),
            stamp: None,
            error: None,
            limits,
            capture_bytes: 0,
            capture_work: 0,
            events: 0,
            root_font_size: None,
            reservation,
        };
        if let Err(error) = this.initialize(count, reporter) {
            this.error = Some(error);
        }
        this
    }
    fn initialize(&mut self, count: usize, reporter: &BorderReporter<'_>) -> Result<()> {
        let limits = self.limits;
        if [
            limits.observations,
            limits.bindings,
            limits.capture_bytes,
            limits.prepared_bytes,
            limits.capture_work,
            limits.provenance_work,
            limits.preparation_work,
        ]
        .iter()
        .any(|value| value.get() > isize::MAX as usize)
        {
            return Err(Error::InvalidLimits);
        }
        self.stamp = Some(reporter.capture_stamp()?);
        let observations = times(count, 4, BorderLimit::Observations)?;
        within(
            observations,
            limits.observations.get(),
            BorderLimit::Observations,
        )?;
        self.capture_work = plus(
            1,
            times(count, 5, BorderLimit::CaptureWork)?,
            BorderLimit::CaptureWork,
        )?;
        within(
            self.capture_work,
            limits.capture_work.get(),
            BorderLimit::CaptureWork,
        )?;
        let requested = times(
            count,
            size_of::<Option<RawRow<'r>>>(),
            BorderLimit::CaptureBytes,
        )?;
        within(
            requested,
            limits.capture_bytes.get(),
            BorderLimit::CaptureBytes,
        )?;
        self.reservation
            .reserve(&mut self.raw, count)
            .map_err(|_| Error::Allocation)?;
        self.capture_bytes = within(
            times(
                self.raw.capacity(),
                size_of::<Option<RawRow<'r>>>(),
                BorderLimit::CaptureBytes,
            )?,
            limits.capture_bytes.get(),
            BorderLimit::CaptureBytes,
        )?;
        self.raw.resize(count, None);
        Ok(())
    }

    fn record(
        &mut self,
        original: usize,
        winners: &PropertyWinners<'r>,
        row: &ClearanceBoxFact,
        ids: &[PropertyId; 29],
    ) -> Result<()> {
        if self.raw.get(original).is_none_or(Option::is_some) {
            return Err(Error::InvalidObservation);
        }
        if self.root_font_size.is_none() {
            self.root_font_size = row.font_size.map(|fact| fact.pixels);
        }
        let mut sides = std::array::from_fn(|side| RawSide::Silent {
            property: ids[20 + side],
        });
        let mut events = 0usize;
        for (side, output) in sides.iter_mut().enumerate() {
            let fact = row.border_width[side];
            if fact.origin != BoxFactOrigin::Specified {
                continue;
            }
            let Some(event) = fact.event else {
                continue;
            };
            let property = ids[20 + side];
            let candidate = winners
                .candidate(property)
                .map_err(|_| Error::InvalidWinner)?
                .ok_or(Error::InvalidWinner)?;
            // Copy the graph-owned reference out of the local table. No resolved
            // default-value temporary is ever retained by this observer.
            let parsed: &'r ParsedProperty = candidate.value;
            let SpecifiedValue::Length(value) = &parsed.value else {
                return Err(Error::InvalidWinner);
            };
            let value: &'r LengthTarget = value;
            let font = match value {
                LengthTarget::Relative(_, unit) => {
                    let size = row.font_size.ok_or(Error::InvalidWinner)?.pixels;
                    let basis = match unit {
                        RelativeUnit::Em => size,
                        RelativeUnit::Ex => fallback_x_height(size),
                        // Rows are recorded root first: `rem` measures the first one's.
                        RelativeUnit::Rem | RelativeUnit::Ch => {
                            unit.fallback_basis(size, self.root_font_size.unwrap_or(size))
                        }
                    };
                    Some((size.to_bits(), basis.to_bits()))
                }
                LengthTarget::Absolute(_) => None,
                LengthTarget::Percentage(_) | LengthTarget::Auto => {
                    return Err(Error::InvalidWinner);
                }
            };
            *output = RawSide::Event {
                property,
                declaration: candidate.declaration,
                value,
                font,
                result_bits: fact.pixels.to_bits(),
                event,
                binding: None,
            };
            events += 1;
        }
        self.events = plus(self.events, events, BorderLimit::Observations)?;
        self.raw[original] = Some(RawRow {
            sides,
            instance: None,
        });
        Ok(())
    }

    pub(super) fn finish(
        mut self,
        projection: &SiblingSourceProjection<'_>,
        display: &SourceDisplay<'_, '_, '_>,
        reporter: &BorderReporter<'_>,
        bindings: &[BorderDeclarationBinding<'_>],
    ) -> Result<BorderObservationBatch> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if !std::ptr::eq(projection.host(), reporter.source_host())
            || reporter
                .source_host()
                .snapshot()
                .is_none_or(|snapshot| !std::ptr::eq(snapshot, display.snapshot()))
            || !std::ptr::eq(projection.snapshot(), display.snapshot())
        {
            return Err(Error::WrongSource);
        }
        // Check current liveness without replacing the original capture generation.
        drop(reporter.capture_stamp()?);
        let stamp = self.stamp.take().ok_or(Error::InvalidObservation)?;
        let count = self.raw.len();
        let observed = times(count, 4, BorderLimit::Observations)?;
        let mut preparation = Work::new(self.limits.preparation_work, BorderLimit::PreparationWork);
        let mut provenance = Work::new(self.limits.provenance_work, BorderLimit::ProvenanceWork);
        preparation.charge(count)?;
        if count != display.snapshot().construction().nodes().len()
            || self.raw.iter().any(Option::is_none)
        {
            return Err(Error::InvalidObservation);
        }
        provenance.charge(times(
            count,
            plus(count, 1, BorderLimit::ProvenanceWork)?,
            BorderLimit::ProvenanceWork,
        )?)?;
        for (original, row) in self.raw.iter_mut().enumerate() {
            let token = display.token(original).ok_or(Error::WrongSource)?;
            let instance = reporter
                .source_host()
                .engine()
                .instance(token)
                .map_err(|_| Error::WrongSource)?;
            row.as_mut().ok_or(Error::InvalidObservation)?.instance = Some(instance);
        }
        within(
            bindings.len(),
            self.limits.bindings.get(),
            BorderLimit::Bindings,
        )?;
        let mut resolved = Vec::new();
        let cache_requested = times(
            bindings.len(),
            size_of::<ResolvedBinding<'_>>(),
            BorderLimit::PreparedBytes,
        )?;
        within(
            plus(
                self.capture_bytes,
                cache_requested,
                BorderLimit::PreparedBytes,
            )?,
            self.limits.prepared_bytes.get(),
            BorderLimit::PreparedBytes,
        )?;
        preparation.charge(1)?;
        self.reservation
            .reserve(&mut resolved, bindings.len())
            .map_err(|_| Error::Allocation)?;
        let cache_bytes = times(
            resolved.capacity(),
            size_of::<ResolvedBinding<'_>>(),
            BorderLimit::PreparedBytes,
        )?;
        let fixed = within(
            plus(self.capture_bytes, cache_bytes, BorderLimit::PreparedBytes)?,
            self.limits.prepared_bytes.get(),
            BorderLimit::PreparedBytes,
        )?;
        for binding in bindings {
            let row = resolve_binding(binding, display, &resolved, &mut provenance)?;
            preparation.charge(1)?;
            resolved.push(row);
        }
        // Complete winner coverage before any owned batch allocation. Store
        // only the cache slot in existing raw storage: no second lookup table.
        preparation.charge(observed)?;
        for row in &mut self.raw {
            for side in &mut row.as_mut().ok_or(Error::InvalidObservation)?.sides {
                if let RawSide::Event {
                    property,
                    declaration,
                    value,
                    binding,
                    ..
                } = side
                {
                    provenance.charge(bindings.len())?;
                    let slot = resolved
                        .iter()
                        .position(|source| source.declaration == *declaration)
                        .ok_or(Error::MissingDeclaration)?;
                    resolved[slot].validate_leaf(*property, value, &mut provenance)?;
                    *binding = Some(slot);
                }
            }
        }
        let mut observations = Vec::new();
        let requested = times(
            observed,
            size_of::<BorderObservation>(),
            BorderLimit::PreparedBytes,
        )?;
        within(
            plus(fixed, requested, BorderLimit::PreparedBytes)?,
            self.limits.prepared_bytes.get(),
            BorderLimit::PreparedBytes,
        )?;
        preparation.charge(1)?;
        self.reservation
            .reserve(&mut observations, observed)
            .map_err(|_| Error::Allocation)?;
        let mut peak = within(
            plus(
                fixed,
                times(
                    observations.capacity(),
                    size_of::<BorderObservation>(),
                    BorderLimit::PreparedBytes,
                )?,
                BorderLimit::PreparedBytes,
            )?,
            self.limits.prepared_bytes.get(),
            BorderLimit::PreparedBytes,
        )?;
        // Earlier-prefix scans need no temporary seen set. Prepay all possible
        // observation visits and Arc pointer comparisons for this owned account.
        preparation.charge(times(self.events, observed, BorderLimit::PreparationWork)?)?;
        preparation.charge(pair_count(self.events, BorderLimit::PreparationWork)?)?;
        for row in self.raw {
            let row = row.ok_or(Error::InvalidObservation)?;
            let instance = row.instance.ok_or(Error::InvalidObservation)?;
            for side in row.sides {
                preparation.charge(1)?;
                let (property, event) = match side {
                    RawSide::Silent { property } => (property, None),
                    RawSide::Event {
                        property,
                        value,
                        font,
                        result_bits,
                        event,
                        binding,
                        ..
                    } => {
                        let source = resolved
                            .get(binding.ok_or(Error::InvalidObservation)?)
                            .ok_or(Error::InvalidObservation)?;
                        let value = copy_value(
                            value,
                            font,
                            &mut self.reservation,
                            &mut preparation,
                            &mut peak,
                            self.limits.prepared_bytes.get(),
                        )?;
                        let seen = observations.iter().any(|old: &BorderObservation| {
                            old.event
                                .as_ref()
                                .is_some_and(|event| Arc::ptr_eq(&event.source, source.provenance))
                        });
                        if !seen {
                            let bytes = plus(
                                size_of::<ChargedArcProvenance>(),
                                source.provenance.logical_name.capacity(),
                                BorderLimit::PreparedBytes,
                            )?;
                            peak = within(
                                plus(peak, bytes, BorderLimit::PreparedBytes)?,
                                self.limits.prepared_bytes.get(),
                                BorderLimit::PreparedBytes,
                            )?;
                        }
                        preparation.charge(1)?;
                        (
                            property,
                            Some(BorderPendingEvent {
                                logical_key: source.logical_key,
                                source: Arc::clone(source.provenance),
                                span: source.span.clone(),
                                value,
                                result_bits,
                                event,
                            }),
                        )
                    }
                };
                preparation.charge(1)?;
                observations.push(BorderObservation {
                    instance,
                    property,
                    event,
                });
            }
        }
        let receipt = BorderCaptureReceipt {
            observations: observed,
            events: self.events,
            capture_work: self.capture_work,
            provenance_work: provenance.used,
            preparation_work: preparation.used,
            capture_bytes: self.capture_bytes,
            prepared_bytes: peak,
        };
        Ok(BorderObservationBatch::from_prepared(
            stamp,
            observations,
            receipt,
        ))
    }
}
impl<'r, R: Reservation> BoxRowObserver<'r> for Collector<'r, R> {
    fn observe(
        &mut self,
        original: usize,
        winners: &PropertyWinners<'r>,
        row: &ClearanceBoxFact,
        ids: &[PropertyId; 29],
    ) {
        if self.error.is_none()
            && let Err(error) = self.record(original, winners, row, ids)
        {
            self.error = Some(error);
        }
    }
}

struct ResolvedBinding<'a> {
    graph: &'a OwnedStylesheetGraph,
    resource: ResourceId,
    occurrence: OccurrenceId,
    declaration: NonZeroU64,
    logical_key: NonZeroU64,
    parsed: &'a ParsedProperty,
    provenance: &'a Arc<Provenance>,
    span: SourceSpan,
}
impl ResolvedBinding<'_> {
    fn validate_leaf(
        &self,
        property: PropertyId,
        value: &LengthTarget,
        work: &mut Work,
    ) -> Result<()> {
        let leaves = match &self.parsed.value {
            SpecifiedValue::Expanded(leaves) => leaves.as_slice(),
            _ => std::slice::from_ref(self.parsed),
        };
        work.charge(leaves.len())?;
        if leaves.iter().any(|leaf| leaf.property == property
            && matches!(&leaf.value, SpecifiedValue::Length(actual) if std::ptr::eq(actual, value)))
        {
            Ok(())
        } else {
            Err(Error::InvalidWinner)
        }
    }
}
fn resolve_binding<'a>(
    binding: &'a BorderDeclarationBinding<'a>,
    display: &SourceDisplay<'_, '_, '_>,
    previous: &[ResolvedBinding<'a>],
    work: &mut Work,
) -> Result<ResolvedBinding<'a>> {
    let graphs = display.bundle().bindings();
    work.charge(graphs.len())?;
    if !graphs
        .iter()
        .any(|admitted| std::ptr::eq(admitted.graph, binding.graph))
    {
        return Err(Error::InvalidProvenance);
    }
    if binding.provenance.logical_name.is_empty() {
        return Err(Error::InvalidProvenance);
    }
    for old in previous {
        work.charge(plus(
            3,
            plus(
                old.provenance.logical_name.len(),
                binding.provenance.logical_name.len(),
                BorderLimit::ProvenanceWork,
            )?,
            BorderLimit::ProvenanceWork,
        )?)?;
        if old.logical_key == binding.logical_key
            || (std::ptr::eq(old.graph, binding.graph)
                && old.declaration == binding.declaration
                && old.occurrence == binding.occurrence)
        {
            return Err(Error::ConflictingMapping);
        }
        if std::ptr::eq(old.graph, binding.graph)
            && old.resource == binding.resource
            && (old.provenance.logical_name != binding.provenance.logical_name
                || old.provenance.revision != binding.provenance.revision)
        {
            return Err(Error::ConflictingMapping);
        }
    }
    let rules = binding.graph.rules().len();
    work.charge(plus(1, rules, BorderLimit::ProvenanceWork)?)?;
    let declaration = binding
        .graph
        .find_declaration(
            binding.declaration,
            NonZeroUsize::new(rules.max(1)).ok_or(Error::InvalidProvenance)?,
        )
        .map_err(|_| Error::InvalidProvenance)?
        .ok_or(Error::MissingDeclaration)?;
    if declaration.source != binding.resource || declaration.occurrence != binding.occurrence {
        return Err(Error::InvalidProvenance);
    }
    let resources = binding.graph.resources();
    work.charge(resources.len())?;
    let mut selected = None;
    for resource in resources {
        if resource.resource == binding.resource && selected.replace(resource).is_some() {
            return Err(Error::InvalidProvenance);
        }
    }
    let resource = selected.ok_or(Error::InvalidProvenance)?;
    work.charge(times(
        2,
        plus(
            resource.candidate.source().text().len(),
            1,
            BorderLimit::ProvenanceWork,
        )?,
        BorderLimit::ProvenanceWork,
    )?)?;
    let original = resource
        .candidate
        .original_span(declaration.location.declaration.clone())
        .ok_or(Error::InvalidProvenance)?;
    if original.start.byte > original.end.byte {
        return Err(Error::InvalidProvenance);
    }
    let span = SourceSpan {
        bytes: original.start.byte..original.end.byte,
        line: NonZeroUsize::new(original.start.line).ok_or(Error::InvalidProvenance)?,
        column: NonZeroUsize::new(original.start.column).ok_or(Error::InvalidProvenance)?,
    };
    Ok(ResolvedBinding {
        graph: binding.graph,
        resource: binding.resource,
        occurrence: binding.occurrence,
        declaration: binding.declaration,
        logical_key: binding.logical_key,
        parsed: declaration.parsed,
        provenance: &binding.provenance,
        span,
    })
}

fn copy_value<R: Reservation>(
    value: &LengthTarget,
    font: Option<(u32, u32)>,
    reservation: &mut R,
    work: &mut Work,
    peak: &mut usize,
    limit: usize,
) -> Result<OwnedBorderValue> {
    let view = value.border_diagnostic_view().ok_or(Error::InvalidWinner)?;
    let source = match view {
        BorderLengthView::Absolute {
            negative: false,
            limbs,
            ..
        }
        | BorderLengthView::Relative {
            negative: false,
            limbs,
            ..
        } => limbs,
        _ => return Err(Error::InvalidWinner),
    };
    within(
        plus(
            *peak,
            times(source.len(), size_of::<u32>(), BorderLimit::PreparedBytes)?,
            BorderLimit::PreparedBytes,
        )?,
        limit,
        BorderLimit::PreparedBytes,
    )?;
    work.charge(1)?;
    let mut limbs = Vec::new();
    reservation
        .reserve(&mut limbs, source.len())
        .map_err(|_| Error::Allocation)?;
    *peak = within(
        plus(
            *peak,
            times(
                limbs.capacity(),
                size_of::<u32>(),
                BorderLimit::PreparedBytes,
            )?,
            BorderLimit::PreparedBytes,
        )?,
        limit,
        BorderLimit::PreparedBytes,
    )?;
    work.charge(source.len())?;
    limbs.extend_from_slice(source);
    Ok(match view {
        BorderLengthView::Absolute {
            negative,
            denominator,
            ..
        } => OwnedBorderValue::Absolute {
            negative,
            limbs,
            denominator,
        },
        BorderLengthView::Relative {
            negative,
            scale,
            unit,
            ..
        } => {
            let (computed_font_bits, conversion_basis_bits) = font.ok_or(Error::InvalidWinner)?;
            OwnedBorderValue::Relative {
                negative,
                limbs,
                scale,
                unit,
                computed_font_bits,
                conversion_basis_bits,
            }
        }
    })
}

#[cfg(test)]
#[path = "border_diagnostics_tests.rs"]
mod tests;
