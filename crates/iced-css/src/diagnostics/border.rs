//! Bounded, explicit delivery of authored border conversion events.
use super::{DiagnosticHost, Shared};
use crate::host::source_publication::InitialSourceHost;
use iced_css_core::{
    diagnostics::{
        Admission, CauseError, CauseKey, CauseLimits, CauseTracker, Diagnostic, DiagnosticQueue,
        OmissionReason, Origin, Provenance, SourceSpan,
    },
    identity::InstanceId,
    stylesheet::owned_graph::OwnedStylesheetGraph,
    syntax::imports::{OccurrenceId, ResourceId},
    values::{NonnegativeF32, RangeEvent, RelativeUnit, registry::PropertyId},
};
use std::{
    mem::size_of,
    num::{NonZeroU64, NonZeroUsize},
    sync::{Arc, Weak},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderSide {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Copy)]
pub struct BorderCaptureLimits {
    pub observations: NonZeroUsize,
    pub bindings: NonZeroUsize,
    pub capture_bytes: NonZeroUsize,
    pub prepared_bytes: NonZeroUsize,
    pub capture_work: NonZeroUsize,
    pub provenance_work: NonZeroUsize,
    pub preparation_work: NonZeroUsize,
}

#[derive(Debug, Clone, Copy)]
pub struct BorderHostLimits {
    pub live_causes: NonZeroUsize,
    pub live_bytes: NonZeroUsize,
    pub transaction_bytes: NonZeroUsize,
    pub publication_work: NonZeroUsize,
}

pub struct BorderDeclarationBinding<'g> {
    pub graph: &'g OwnedStylesheetGraph,
    pub resource: ResourceId,
    pub occurrence: OccurrenceId,
    pub declaration: NonZeroU64,
    pub logical_key: NonZeroU64,
    pub provenance: Arc<Provenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderLimit {
    Observations,
    Bindings,
    CaptureBytes,
    PreparedBytes,
    CaptureWork,
    ProvenanceWork,
    PreparationWork,
    LiveCauses,
    LiveBytes,
    TransactionBytes,
    PublicationWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderReportingError {
    UnpublishedSource,
    AlreadyBound,
    WrongSource,
    Closed,
    HostFaulted,
    StaleBatch,
    InvalidLimits,
    Limit(BorderLimit),
    Allocation,
    MissingDeclaration,
    ConflictingMapping,
    InvalidProvenance,
    InvalidWinner,
    InvalidObservation,
    IdentityExhausted,
    GenerationExhausted,
}
impl std::fmt::Display for BorderReportingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "border diagnostic reporting: {self:?}")
    }
}
impl std::error::Error for BorderReportingError {}

pub struct ReportingFacts<F> {
    pub facts: F,
    pub reporting: Result<BorderObservationBatch, BorderReportingError>,
}

pub struct BorderObservationBatch {
    owner: Weak<Shared>,
    capture_generation: u64,
    observations: Vec<BorderObservation>,
    receipt: BorderCaptureReceipt,
}
pub struct BorderReporter<'source> {
    owner: Weak<Shared>,
    source: &'source InitialSourceHost,
}
pub(crate) struct BorderCaptureStamp {
    owner: Weak<Shared>,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderCaptureReceipt {
    pub observations: usize,
    pub events: usize,
    pub capture_work: usize,
    pub provenance_work: usize,
    pub preparation_work: usize,
    pub capture_bytes: usize,
    pub prepared_bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderPublishReceipt {
    pub observations: usize,
    pub new_causes: usize,
    pub suppressed: usize,
    pub cleared: usize,
    pub queued: usize,
    pub omitted_count: usize,
    pub omitted_bytes: usize,
    pub omitted_allocation: usize,
    pub work: usize,
    pub live_bytes: usize,
    pub transaction_bytes: usize,
}

pub(crate) struct BorderObservation {
    pub(crate) instance: InstanceId,
    pub(crate) property: PropertyId,
    pub(crate) event: Option<BorderPendingEvent>,
}
pub(crate) struct BorderPendingEvent {
    pub(crate) logical_key: NonZeroU64,
    pub(crate) source: Arc<Provenance>,
    pub(crate) span: SourceSpan,
    pub(crate) value: OwnedBorderValue,
    pub(crate) result_bits: u32,
    pub(crate) event: RangeEvent,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OwnedBorderValue {
    Absolute {
        negative: bool,
        limbs: Vec<u32>,
        denominator: [usize; 4],
    },
    Relative {
        negative: bool,
        limbs: Vec<u32>,
        scale: usize,
        unit: RelativeUnit,
        computed_font_bits: u32,
        conversion_basis_bits: u32,
    },
}
struct BorderSemantic {
    id: NonZeroU64,
    logical_key: NonZeroU64,
    source: Arc<Provenance>,
    value: OwnedBorderValue,
}
pub(super) struct BorderState {
    limits: BorderHostLimits,
    generation: u64,
    next_semantic: u64,
    causes: CauseTracker,
    semantics: Vec<BorderSemantic>,
}

impl BorderObservationBatch {
    pub fn receipt(&self) -> BorderCaptureReceipt {
        self.receipt
    }
    pub(crate) fn from_prepared(
        stamp: BorderCaptureStamp,
        observations: Vec<BorderObservation>,
        receipt: BorderCaptureReceipt,
    ) -> Self {
        Self {
            owner: stamp.owner,
            capture_generation: stamp.generation,
            observations,
            receipt,
        }
    }
}

impl DiagnosticHost {
    /// Bind once to an already published source owner. Dropping a reporter does
    /// not erase the binding or its ongoing causes.
    pub fn bind_border_reporting<'source>(
        &mut self,
        source: &'source InitialSourceHost,
        limits: BorderHostLimits,
    ) -> Result<BorderReporter<'source>, BorderReportingError> {
        if source.snapshot().is_none() {
            return Err(BorderReportingError::UnpublishedSource);
        }
        limits.validate()?;
        let mut state = self
            .shared
            .state
            .lock()
            .map_err(|_| BorderReportingError::HostFaulted)?;
        if !state.alive {
            return Err(BorderReportingError::Closed);
        }
        if state.border.is_some() {
            return Err(BorderReportingError::AlreadyBound);
        }
        state.border = Some(BorderState::new(limits)?);
        Ok(BorderReporter {
            owner: Arc::downgrade(&self.shared),
            source,
        })
    }
}

impl BorderReporter<'_> {
    pub(crate) fn source_host(&self) -> &InitialSourceHost {
        self.source
    }
    pub(crate) fn capture_stamp(&self) -> Result<BorderCaptureStamp, BorderReportingError> {
        let shared = self.owner.upgrade().ok_or(BorderReportingError::Closed)?;
        let state = shared
            .state
            .lock()
            .map_err(|_| BorderReportingError::HostFaulted)?;
        if !state.alive {
            return Err(BorderReportingError::Closed);
        }
        let border = state
            .border
            .as_ref()
            .ok_or(BorderReportingError::WrongSource)?;
        Ok(BorderCaptureStamp {
            owner: self.owner.clone(),
            generation: border.generation,
        })
    }
    /// Consume a complete observation. Reporting errors leave causes and queue
    /// unchanged; ordinary queue omissions are successful cause publication.
    pub fn publish(
        &mut self,
        batch: BorderObservationBatch,
    ) -> Result<BorderPublishReceipt, BorderReportingError> {
        if !Weak::ptr_eq(&self.owner, &batch.owner) {
            return Err(BorderReportingError::WrongSource);
        }
        let shared = self.owner.upgrade().ok_or(BorderReportingError::Closed)?;
        let (receipt, waker) = {
            let mut state = shared
                .state
                .lock()
                .map_err(|_| BorderReportingError::HostFaulted)?;
            if !state.alive {
                return Err(BorderReportingError::Closed);
            }
            let border = state
                .border
                .as_ref()
                .ok_or(BorderReportingError::WrongSource)?;
            let prepared = border.prepare_with(&batch, &state.queue, &mut ExactReserve)?;
            let PreparedPublication {
                replacement,
                reports,
                mut receipt,
            } = prepared;
            // All fallible allocation, arithmetic, identity and work checks have
            // completed. No user callback runs inside this lock.
            state.border = Some(replacement);
            let notify = !reports.is_empty();
            for report in reports {
                let counter = match state.queue.admit(report) {
                    Admission::Queued => &mut receipt.queued,
                    Admission::Omitted(OmissionReason::Count) => &mut receipt.omitted_count,
                    Admission::Omitted(OmissionReason::Bytes) => &mut receipt.omitted_bytes,
                    Admission::Omitted(OmissionReason::Allocation) => {
                        &mut receipt.omitted_allocation
                    }
                    Admission::Closed => {
                        unreachable!("live host owns an open queue under this lock")
                    }
                };
                *counter = counter
                    .checked_add(1)
                    .expect("count bounded by prepared report vector");
            }
            let waker = if notify { state.waker.take() } else { None };
            (receipt, waker)
        };
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(receipt)
    }
}

impl BorderHostLimits {
    fn validate(self) -> Result<(), BorderReportingError> {
        if [
            self.live_causes,
            self.live_bytes,
            self.transaction_bytes,
            self.publication_work,
        ]
        .iter()
        .any(|value| value.get() > isize::MAX as usize)
        {
            return Err(BorderReportingError::InvalidLimits);
        }
        Ok(())
    }
}

type Result<T, E = BorderReportingError> = std::result::Result<T, E>;

// Private reservation policy: production always uses exact fallible Vec reserve.
// Child-module tests can supply deterministic failure or spare capacity policies.
trait ReservePolicy {
    fn reserve<T>(&mut self, value: &mut Vec<T>, additional: usize) -> Result<()>;
}
struct ExactReserve;
impl ReservePolicy for ExactReserve {
    fn reserve<T>(&mut self, value: &mut Vec<T>, additional: usize) -> Result<()> {
        value
            .try_reserve_exact(additional)
            .map_err(|_| BorderReportingError::Allocation)
    }
}

struct Work {
    used: usize,
    limit: usize,
}
impl Work {
    fn charge(&mut self, amount: usize) -> Result<()> {
        let next = add(self.used, amount, BorderLimit::PublicationWork)?;
        if next > self.limit {
            return Err(BorderReportingError::Limit(BorderLimit::PublicationWork));
        }
        self.used = next;
        Ok(())
    }
}
fn add(a: usize, b: usize, limit: BorderLimit) -> Result<usize> {
    a.checked_add(b)
        .filter(|value| *value <= isize::MAX as usize)
        .ok_or(BorderReportingError::Limit(limit))
}
fn mul(a: usize, b: usize, limit: BorderLimit) -> Result<usize> {
    a.checked_mul(b)
        .filter(|value| *value <= isize::MAX as usize)
        .ok_or(BorderReportingError::Limit(limit))
}
fn pairs(n: usize) -> Result<usize> {
    let m = n.saturating_sub(1);
    if n.is_multiple_of(2) {
        mul(n / 2, m, BorderLimit::PublicationWork)
    } else {
        mul(n, m / 2, BorderLimit::PublicationWork)
    }
}
fn vector_bytes<T>(capacity: usize, limit: BorderLimit) -> Result<usize> {
    mul(capacity, size_of::<T>(), limit)
}

// Same qualified requested Arc layout as the existing diagnostic queue. This
// must be requalified on toolchain changes; allocator bookkeeping is excluded.
#[repr(C, align(2))]
struct ChargedArcProvenance {
    _strong: std::sync::atomic::AtomicUsize,
    _weak: std::sync::atomic::AtomicUsize,
    _data: Provenance,
}
fn provenance_bytes(source: &Provenance, limit: BorderLimit) -> Result<usize> {
    add(
        size_of::<ChargedArcProvenance>(),
        source.logical_name.capacity(),
        limit,
    )
}
impl OwnedBorderValue {
    fn limbs(&self) -> &Vec<u32> {
        match self {
            Self::Absolute { limbs, .. } | Self::Relative { limbs, .. } => limbs,
        }
    }
    fn with_limbs(&self, limbs: Vec<u32>) -> Self {
        match self {
            Self::Absolute {
                negative,
                denominator,
                ..
            } => Self::Absolute {
                negative: *negative,
                limbs,
                denominator: *denominator,
            },
            Self::Relative {
                negative,
                scale,
                unit,
                computed_font_bits,
                conversion_basis_bits,
                ..
            } => Self::Relative {
                negative: *negative,
                limbs,
                scale: *scale,
                unit: *unit,
                computed_font_bits: *computed_font_bits,
                conversion_basis_bits: *conversion_basis_bits,
            },
        }
    }
}
fn numeric_bytes(limbs: &Vec<u32>, limit: BorderLimit) -> Result<usize> {
    vector_bytes::<u32>(limbs.capacity(), limit)
}

struct PreparedPublication {
    replacement: BorderState,
    reports: Vec<Diagnostic>,
    receipt: BorderPublishReceipt,
}

struct Storage {
    old: usize,
    input: usize,
    keys: usize,
    semantic_slots: usize,
    reports: usize,
    tracker: usize,
    limbs: usize,
    provenance: usize,
    peak: usize,
}
impl Storage {
    fn live(&self) -> Result<usize> {
        let l = BorderLimit::LiveBytes;
        add(
            add(self.tracker, self.semantic_slots, l)?,
            add(self.limbs, self.provenance, l)?,
            l,
        )
    }
    fn check(&mut self, limits: BorderHostLimits) -> Result<()> {
        let live = self.live()?;
        if live > limits.live_bytes.get() {
            return Err(BorderReportingError::Limit(BorderLimit::LiveBytes));
        }
        let t = BorderLimit::TransactionBytes;
        let total = add(
            add(add(self.old, self.input, t)?, live, t)?,
            add(self.keys, self.reports, t)?,
            t,
        )?;
        if total > limits.transaction_bytes.get() {
            return Err(BorderReportingError::Limit(t));
        }
        self.peak = self.peak.max(total);
        Ok(())
    }
}

fn batch_bytes(batch: &BorderObservationBatch, work: &mut Work) -> Result<usize> {
    let l = BorderLimit::TransactionBytes;
    let mut bytes = vector_bytes::<BorderObservation>(batch.observations.capacity(), l)?;
    let mut events = 0usize;
    for observation in &batch.observations {
        work.charge(1)?;
        if observation.event.is_some() {
            events = add(events, 1, BorderLimit::PublicationWork)?;
        }
    }
    work.charge(pairs(events)?)?;
    for (index, observation) in batch.observations.iter().enumerate() {
        work.charge(1)?;
        let Some(event) = &observation.event else {
            continue;
        };
        bytes = add(bytes, numeric_bytes(event.value.limbs(), l)?, l)?;
        let mut seen = false;
        for prior in &batch.observations[..index] {
            work.charge(1)?;
            if let Some(old) = &prior.event {
                seen |= Arc::ptr_eq(&event.source, &old.source);
            }
        }
        if !seen {
            bytes = add(bytes, provenance_bytes(&event.source, l)?, l)?;
        }
    }
    Ok(bytes)
}

fn semantic_bytes(semantics: &Vec<BorderSemantic>, work: &mut Work) -> Result<usize> {
    let l = BorderLimit::LiveBytes;
    let mut bytes = vector_bytes::<BorderSemantic>(semantics.capacity(), l)?;
    work.charge(pairs(semantics.len())?)?;
    for (index, semantic) in semantics.iter().enumerate() {
        work.charge(1)?;
        bytes = add(bytes, numeric_bytes(semantic.value.limbs(), l)?, l)?;
        let mut seen = false;
        for prior in &semantics[..index] {
            work.charge(1)?;
            seen |= Arc::ptr_eq(&semantic.source, &prior.source);
        }
        if !seen {
            bytes = add(bytes, provenance_bytes(&semantic.source, l)?, l)?;
        }
    }
    Ok(bytes)
}

fn semantic_matches(
    known: &BorderSemantic,
    event: &BorderPendingEvent,
    work: &mut Work,
) -> Result<bool> {
    let l = BorderLimit::PublicationWork;
    let mut cost = add(12, known.source.logical_name.len(), l)?;
    cost = add(cost, event.source.logical_name.len(), l)?;
    cost = add(cost, known.value.limbs().len(), l)?;
    cost = add(cost, event.value.limbs().len(), l)?;
    work.charge(cost)?;
    Ok(known.logical_key == event.logical_key
        && known.source.logical_name == event.source.logical_name
        && known.value == event.value)
}

fn observed(key: CauseKey, observations: &[BorderObservation], work: &mut Work) -> Result<bool> {
    for row in observations {
        work.charge(1)?;
        if row.instance == key.instance && row.property == key.property {
            return Ok(true);
        }
    }
    Ok(false)
}

fn old_cause(
    keys: &[CauseKey],
    observation: &BorderObservation,
    work: &mut Work,
) -> Result<Option<CauseKey>> {
    for key in keys {
        work.charge(1)?;
        if key.instance == observation.instance && key.property == observation.property {
            return Ok(Some(*key));
        }
    }
    Ok(None)
}

fn validate_observations(batch: &BorderObservationBatch, work: &mut Work) -> Result<usize> {
    if batch.receipt.observations != batch.observations.len() {
        return Err(BorderReportingError::InvalidObservation);
    }
    let mut events = 0usize;
    for (index, observation) in batch.observations.iter().enumerate() {
        work.charge(1)?;
        if !matches!(
            observation.property.descriptor().name,
            "border-top-width" | "border-right-width" | "border-bottom-width" | "border-left-width"
        ) {
            return Err(BorderReportingError::InvalidObservation);
        }
        for prior in &batch.observations[..index] {
            work.charge(1)?;
            if observation.instance == prior.instance && observation.property == prior.property {
                return Err(BorderReportingError::InvalidObservation);
            }
        }
        if let Some(event) = &observation.event {
            events = add(events, 1, BorderLimit::PublicationWork)?;
            let valid_result = match event.event {
                RangeEvent::UpperSaturation => event.result_bits == f32::MAX.to_bits(),
                RangeEvent::PositiveRoundedToZero => event.result_bits == 0,
            };
            let (negative, valid_context) = match &event.value {
                OwnedBorderValue::Absolute { negative, .. } => (*negative, true),
                OwnedBorderValue::Relative {
                    negative,
                    computed_font_bits,
                    conversion_basis_bits,
                    ..
                } => {
                    let font = f32::from_bits(*computed_font_bits);
                    let basis = f32::from_bits(*conversion_basis_bits);
                    (
                        *negative,
                        font.is_finite() && font > 0.0 && basis.is_finite() && basis >= 0.0,
                    )
                }
            };
            if !valid_result
                || negative
                || !valid_context
                || event.source.logical_name.is_empty()
                || event.span.bytes.start >= event.span.bytes.end
                || event.value.limbs().last().is_none_or(|last| *last == 0)
            {
                return Err(BorderReportingError::InvalidObservation);
            }
            for limb in event.value.limbs() {
                work.charge(1)?;
                if *limb >= 1_000_000_000 {
                    return Err(BorderReportingError::InvalidObservation);
                }
            }
        }
    }
    if events != batch.receipt.events {
        return Err(BorderReportingError::InvalidObservation);
    }
    Ok(events)
}

struct Staging<'a, P> {
    semantics: Vec<BorderSemantic>,
    storage: Storage,
    limits: BorderHostLimits,
    policy: &'a mut P,
}
impl<P: ReservePolicy> Staging<'_, P> {
    fn insert(
        &mut self,
        id: NonZeroU64,
        logical_key: NonZeroU64,
        source: &Arc<Provenance>,
        value: &OwnedBorderValue,
        work: &mut Work,
    ) -> Result<()> {
        let mut present = false;
        let mut same_source = false;
        for known in &self.semantics {
            // One ID reachability check and one allocation-identity comparison.
            work.charge(2)?;
            present |= known.id == id;
            same_source |= Arc::ptr_eq(&known.source, source);
        }
        if present {
            return Ok(());
        }
        if self.semantics.len() == self.semantics.capacity() {
            return Err(BorderReportingError::InvalidObservation);
        }
        let l = BorderLimit::LiveBytes;
        let requested = vector_bytes::<u32>(value.limbs().len(), l)?;
        let old_limbs = self.storage.limbs;
        self.storage.limbs = add(old_limbs, requested, l)?;
        if !same_source {
            self.storage.provenance =
                add(self.storage.provenance, provenance_bytes(source, l)?, l)?;
        }
        self.storage.check(self.limits)?;
        let mut limbs = Vec::new();
        work.charge(1)?;
        self.policy.reserve(&mut limbs, value.limbs().len())?;
        self.storage.limbs = add(old_limbs, numeric_bytes(&limbs, l)?, l)?;
        self.storage.check(self.limits)?;
        for limb in value.limbs() {
            work.charge(1)?;
            limbs.push(*limb);
        }
        work.charge(2)?; // Arc clone and semantic push.
        self.semantics.push(BorderSemantic {
            id,
            logical_key,
            source: Arc::clone(source),
            value: value.with_limbs(limbs),
        });
        Ok(())
    }
}

impl BorderState {
    fn new(limits: BorderHostLimits) -> Result<Self> {
        limits.validate()?;
        let cause_limits = CauseLimits::new(limits.live_causes.get(), limits.live_bytes.get())
            .map_err(|_| BorderReportingError::InvalidLimits)?;
        Ok(Self {
            limits,
            generation: 1,
            next_semantic: 1,
            causes: CauseTracker::new(cause_limits),
            semantics: Vec::new(),
        })
    }

    fn prepare_with(
        &self,
        batch: &BorderObservationBatch,
        queue: &DiagnosticQueue,
        policy: &mut impl ReservePolicy,
    ) -> Result<PreparedPublication> {
        if batch.capture_generation != self.generation {
            return Err(BorderReportingError::StaleBatch);
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(BorderReportingError::GenerationExhausted)?;
        let mut work = Work {
            used: 0,
            limit: self.limits.publication_work.get(),
        };
        let b = batch.observations.len();
        let l = self.causes.live_count();
        let w = BorderLimit::PublicationWork;
        work.charge(add(
            mul(b, add(l, 1, w)?, w)?,
            mul(l, add(b, 1, w)?, w)?,
            w,
        )?)?;
        let events = validate_observations(batch, &mut work)?;
        let input = batch_bytes(batch, &mut work)?;
        let old = add(
            self.causes.retained_bytes(),
            semantic_bytes(&self.semantics, &mut work)?,
            BorderLimit::LiveBytes,
        )?;
        let mut count = events;
        for key in self.causes.keys() {
            work.charge(1)?;
            if !observed(*key, &batch.observations, &mut work)? {
                count = add(count, 1, BorderLimit::LiveCauses)?;
            }
        }
        if count > self.limits.live_causes.get() {
            return Err(BorderReportingError::Limit(BorderLimit::LiveCauses));
        }
        let mut storage = Storage {
            old,
            input,
            keys: vector_bytes::<CauseKey>(count, BorderLimit::TransactionBytes)?,
            semantic_slots: vector_bytes::<BorderSemantic>(count, BorderLimit::LiveBytes)?,
            reports: vector_bytes::<Diagnostic>(events, BorderLimit::TransactionBytes)?,
            tracker: vector_bytes::<CauseKey>(count, BorderLimit::LiveBytes)?,
            limbs: 0,
            provenance: 0,
            peak: 0,
        };
        storage.check(self.limits)?;
        let mut keys = Vec::new();
        let mut semantics = Vec::new();
        let mut reports = Vec::new();
        if count != 0 {
            work.charge(1)?;
            policy.reserve(&mut keys, count)?;
            storage.keys =
                vector_bytes::<CauseKey>(keys.capacity(), BorderLimit::TransactionBytes)?;
            storage.check(self.limits)?;
            work.charge(1)?;
            policy.reserve(&mut semantics, count)?;
            storage.semantic_slots =
                vector_bytes::<BorderSemantic>(semantics.capacity(), BorderLimit::LiveBytes)?;
            storage.check(self.limits)?;
        }
        if events != 0 {
            work.charge(1)?;
            policy.reserve(&mut reports, events)?;
            storage.reports =
                vector_bytes::<Diagnostic>(reports.capacity(), BorderLimit::TransactionBytes)?;
            storage.check(self.limits)?;
        }
        let mut staged = Staging {
            semantics,
            storage,
            limits: self.limits,
            policy,
        };
        for key in self.causes.keys() {
            work.charge(1)?;
            if observed(*key, &batch.observations, &mut work)? {
                continue;
            }
            let mut retained = None;
            for semantic in &self.semantics {
                work.charge(1)?;
                if semantic.id == key.semantic {
                    retained = Some(semantic);
                    break;
                }
            }
            let semantic = retained.ok_or(BorderReportingError::InvalidObservation)?;
            staged.insert(
                semantic.id,
                semantic.logical_key,
                &semantic.source,
                &semantic.value,
                &mut work,
            )?;
            work.charge(1)?;
            keys.push(*key);
        }
        let mut next_semantic = self.next_semantic;
        let mut suppressed = 0usize;
        let mut cleared = 0usize;
        for observation in &batch.observations {
            work.charge(1)?;
            let old_key = old_cause(self.causes.keys(), observation, &mut work)?;
            let Some(event) = &observation.event else {
                if old_key.is_some() {
                    cleared = add(cleared, 1, w)?;
                }
                continue;
            };
            let mut matching = None;
            for known in &self.semantics {
                work.charge(1)?;
                if semantic_matches(known, event, &mut work)? {
                    matching = Some(known);
                    break;
                }
            }
            let id = if let Some(known) = matching {
                staged.insert(
                    known.id,
                    known.logical_key,
                    &known.source,
                    &known.value,
                    &mut work,
                )?;
                known.id
            } else {
                let mut prior = None;
                for known in &staged.semantics {
                    work.charge(1)?;
                    if semantic_matches(known, event, &mut work)? {
                        prior = Some(known.id);
                        break;
                    }
                }
                if let Some(id) = prior {
                    id
                } else {
                    let id = NonZeroU64::new(next_semantic)
                        .ok_or(BorderReportingError::IdentityExhausted)?;
                    next_semantic = next_semantic
                        .checked_add(1)
                        .ok_or(BorderReportingError::IdentityExhausted)?;
                    staged.insert(
                        id,
                        event.logical_key,
                        &event.source,
                        &event.value,
                        &mut work,
                    )?;
                    id
                }
            };
            let key = CauseKey {
                instance: observation.instance,
                property: observation.property,
                event: event.event,
                semantic: id,
            };
            work.charge(1)?;
            keys.push(key);
            if old_key == Some(key) {
                suppressed = add(suppressed, 1, w)?;
            } else {
                if old_key.is_some() {
                    cleared = add(cleared, 1, w)?;
                }
                work.charge(3)?; // provenance clone, diagnostic construction, push.
                let report = Diagnostic::range(
                    NonnegativeF32 {
                        value: f32::from_bits(event.result_bits),
                        event: Some(event.event),
                    },
                    Origin::Authored {
                        source: Arc::clone(&event.source),
                        span: event.span.clone(),
                    },
                    Some(observation.instance),
                    observation.property,
                )
                .ok_or(BorderReportingError::InvalidObservation)?;
                reports.push(report);
            }
        }
        if keys.len() != count {
            return Err(BorderReportingError::InvalidObservation);
        }
        // The core replacement has its own fallible allocator. The host's private
        // reservation seam does not claim to inject failure into that allocator.
        work.charge(pairs(count)?)?;
        if count != 0 {
            work.charge(add(1, count, w)?)?;
        }
        let causes = self
            .causes
            .prepare_replacement(&keys)
            .map_err(|error| match error {
                CauseError::Allocation => BorderReportingError::Allocation,
                _ => BorderReportingError::Limit(BorderLimit::LiveBytes),
            })?;
        staged.storage.tracker = causes.retained_bytes();
        staged.storage.check(self.limits)?;
        let a = reports.len();
        let q = queue.pending_count();
        let o = add(q, queue.retained_bytes() / size_of::<Origin>(), w)?;
        let qa = add(q, a, w)?;
        let oa = add(o, a, w)?;
        let allowance = add(add(16, mul(8, qa, w)?, w)?, mul(8, mul(oa, oa, w)?, w)?, w)?;
        work.charge(mul(a, allowance, w)?)?;
        let receipt = BorderPublishReceipt {
            observations: b,
            new_causes: a,
            suppressed,
            cleared,
            queued: 0,
            omitted_count: 0,
            omitted_bytes: 0,
            omitted_allocation: 0,
            work: work.used,
            live_bytes: staged.storage.live()?,
            transaction_bytes: staged.storage.peak,
        };
        let replacement = Self {
            limits: self.limits,
            generation,
            next_semantic,
            causes,
            semantics: staged.semantics,
        };
        Ok(PreparedPublication {
            replacement,
            reports,
            receipt,
        })
    }
}

#[cfg(test)]
#[path = "border_tests.rs"]
mod tests;
