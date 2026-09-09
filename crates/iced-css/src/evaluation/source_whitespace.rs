//! Mapped source-only whitespace under the admitted structural source domain.
use super::{
    source_eligibility::SourceEligibility,
    source_participants::{SourceParticipant, SourceParticipants},
    source_structural_events::{
        SourceStructuralEventError, SourceStructuralEventKind, SourceStructuralEventLimits,
        source_structural_events,
    },
};
use iced_css_core::{
    formatting::display::Display,
    text::{
        lf_conversion::{LfConversionError, LfConversionLimits, LfDecision, convert_line_feeds},
        lf_whitespace::LfWhitespaceAction,
        line_edges::WhiteSpace,
        newlines::{NewlineError, NewlineLimits, normalize_default_newlines},
        preserved_spaces::{
            PreservedSpace, PreservedSpaceError, PreservedSpaceLimits, preserved_space_sequences,
        },
        space_collapse::{
            SpaceAction, SpaceCollapseError, SpaceCollapseLimits, SpaceItem, SpaceUnit,
            collapse_adjacent_spaces,
        },
        whitespace_preprocessing::WhitespaceRendering,
    },
};
use std::{mem::size_of, num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct SourceWhitespaceLimits {
    pub sources: NonZeroUsize,
    pub events: NonZeroUsize,
    pub input_bytes: NonZeroUsize,
    pub input_scalars: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceWhitespaceError {
    Structural(SourceStructuralEventError),
    InputBytes {
        source: usize,
    },
    InputScalars {
        source: usize,
    },
    Work,
    Bytes,
    Allocation,
    WhiteSpace {
        source: usize,
    },
    Newlines {
        source: usize,
        error: NewlineError,
    },
    Lf {
        source: usize,
        error: LfConversionError,
    },
    Preserved {
        source: usize,
        error: PreservedSpaceError,
    },
    Collapse {
        start: usize,
        error: SpaceCollapseError,
    },
}
impl std::fmt::Display for SourceWhitespaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source whitespace: {self:?}")
    }
}
impl std::error::Error for SourceWhitespaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Structural(e) => Some(e),
            Self::Newlines { error, .. } => Some(error),
            Self::Lf { error, .. } => Some(error),
            Self::Preserved { error, .. } => Some(error),
            Self::Collapse { error, .. } => Some(error),
            _ => None,
        }
    }
}
#[derive(Debug)]
struct EventRecord {
    source: usize,
    kind: SourceStructuralEventKind,
    scalars: Range<usize>,
}
#[derive(Debug)]
struct ScalarRecord {
    source: usize,
    source_bytes: Range<usize>,
    normalized: char,
    white_space: WhiteSpace,
    lf: LfDecision,
    preserved: PreservedSpace,
    adjacent: SpaceAction,
}
/// Immutable processed scalar origins and source boundary occurrences.
/// Borrows eligibility directly; temporary participant wrappers may be dropped.
/// Queries allocate nothing. Their iteration work belongs to the caller.
/// Source/content counts refer to eligible occurrences, not the entire snapshot.
/// Every normalized scalar remains mapped to its original source-byte range,
/// including removed entries and CRLF contractions spanning rich runs. Empty
/// Content and Begin/End occurrences retain distinct event anchors.
///
/// Scalar storage reserves the original scalar count r once, although normalized
/// length n can be smaller after CRLF contraction (n <= r <= 2n). Reported bytes
/// use actual capacities, not lengths or requested capacity; this can reject a
/// budget that a more elaborate exact-n allocation scheme could satisfy.
///
/// ```compile_fail,E0505
/// use iced_css::evaluation::{source_eligibility::SourceEligibility,
///     source_participants::SourceParticipants, source_whitespace::*};
/// fn cannot_drop(e: &SourceEligibility<'_, '_, '_, '_>, limits: SourceWhitespaceLimits) {
///     let output = preprocess_source_whitespace(&SourceParticipants::new(e), limits).unwrap();
///     let scalar = output.scalar(0).unwrap();
///     drop(output);
///     let _ = scalar.source_range();
/// }
/// ```
/// ```
/// use iced_css::evaluation::{source_eligibility::SourceEligibility,
///     source_participants::SourceParticipants, source_whitespace::*};
/// fn wrappers(e: &SourceEligibility<'_, '_, '_, '_>, limits: SourceWhitespaceLimits) {
///     let participants = SourceParticipants::new(e);
///     let output = preprocess_source_whitespace(&participants, limits).unwrap();
///     drop(participants);
///     if let Some(event) = output.event(0) {
///         let participant = event.participant();
///         drop(output);
///         let _ = participant.token();
///     }
/// }
/// ```
#[must_use]
pub struct SourceWhitespace<'e, 'd, 's, 'b, 'r> {
    eligibility: &'e SourceEligibility<'d, 's, 'b, 'r>,
    events: Vec<EventRecord>,
    scalars: Vec<ScalarRecord>,
    sources: usize,
    contents: usize,
    input_bytes: usize,
    input_scalars: usize,
    work: usize,
    retained_bytes: usize,
    peak_retained_bytes: usize,
}
impl<'e, 'd, 's, 'b, 'r> SourceWhitespace<'e, 'd, 's, 'b, 'r> {
    pub fn eligibility(&self) -> &'e SourceEligibility<'d, 's, 'b, 'r> {
        self.eligibility
    }
    pub fn source_count(&self) -> usize {
        self.sources
    }
    pub fn content_count(&self) -> usize {
        self.contents
    }
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
    pub fn scalar_count(&self) -> usize {
        self.scalars.len()
    }
    pub fn input_bytes(&self) -> usize {
        self.input_bytes
    }
    pub fn input_scalar_count(&self) -> usize {
        self.input_scalars
    }
    /// Exact successful reservation: 2+k+4e+r+2B+t+12n.
    pub fn work(&self) -> usize {
        self.work
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Maximum admitted simultaneous owned payload, including helper scratch.
    /// Excludes borrowed owners, headers, allocator internals and failed-operation
    /// physical allocation peaks; capacities are checked after acquisition.
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
    pub fn event(&self, index: usize) -> Option<SourceWhitespaceEvent<'_, 'e, 'd, 's, 'b, 'r>> {
        self.events.get(index)?;
        Some(SourceWhitespaceEvent { owner: self, index })
    }
    pub fn scalar(&self, index: usize) -> Option<SourceWhitespaceScalar<'_, 'e, 'd, 's, 'b, 'r>> {
        self.scalars.get(index)?;
        Some(SourceWhitespaceScalar { owner: self, index })
    }
    pub fn events(&self) -> SourceWhitespaceEvents<'_, 'e, 'd, 's, 'b, 'r> {
        SourceWhitespaceEvents {
            owner: self,
            range: 0..self.events.len(),
        }
    }
    pub fn scalars(&self) -> SourceWhitespaceScalars<'_, 'e, 'd, 's, 'b, 'r> {
        SourceWhitespaceScalars {
            owner: self,
            range: 0..self.scalars.len(),
        }
    }
    pub fn between_events(
        &self,
        left: usize,
        right: usize,
    ) -> Option<SourceWhitespaceInterval<'_, 'e, 'd, 's, 'b, 'r>> {
        if left >= right {
            return None;
        }
        let a = self.events.get(left)?;
        let b = self.events.get(right)?;
        Some(SourceWhitespaceInterval {
            owner: self,
            events: left + 1..right,
            scalars: a.scalars.end..b.scalars.start,
        })
    }
    fn participant(&self, source: usize) -> SourceParticipant<'e, 'd, 's, 'b, 'r> {
        SourceParticipants::new(self.eligibility)
            .get(source)
            .expect("admitted original source")
    }
}
pub struct SourceWhitespaceEvent<'o, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'e, 'd, 's, 'b, 'r> SourceWhitespaceEvent<'_, 'e, 'd, 's, 'b, 'r> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn kind(&self) -> SourceStructuralEventKind {
        self.owner.events[self.index].kind
    }
    pub fn scalar_range(&self) -> Range<usize> {
        self.owner.events[self.index].scalars.clone()
    }
    pub fn participant(&self) -> SourceParticipant<'e, 'd, 's, 'b, 'r> {
        self.owner.participant(self.owner.events[self.index].source)
    }
}
pub struct SourceWhitespaceScalar<'o, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'e, 'd, 's, 'b, 'r> SourceWhitespaceScalar<'_, 'e, 'd, 's, 'b, 'r> {
    fn record(&self) -> &ScalarRecord {
        &self.owner.scalars[self.index]
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn source_range(&self) -> Range<usize> {
        self.record().source_bytes.clone()
    }
    pub fn normalized(&self) -> char {
        self.record().normalized
    }
    pub fn white_space(&self) -> WhiteSpace {
        self.record().white_space
    }
    pub fn lf_decision(&self) -> LfDecision {
        self.record().lf
    }
    pub fn preserved_space(&self) -> PreservedSpace {
        self.record().preserved
    }
    pub fn adjacent_action(&self) -> SpaceAction {
        self.record().adjacent
    }
    pub fn participant(&self) -> SourceParticipant<'e, 'd, 's, 'b, 'r> {
        self.owner.participant(self.record().source)
    }
    pub fn rendering(&self) -> WhitespaceRendering {
        match (self.lf_decision(), self.adjacent_action()) {
            (LfDecision::RemoveWhitespace, _) => WhitespaceRendering::RemoveWhitespace,
            (LfDecision::RemoveLf, _) => WhitespaceRendering::RemoveLf,
            (_, SpaceAction::RemovedAdjacentSpace) => WhitespaceRendering::RemoveAdjacentSpace,
            (LfDecision::Space, _) | (_, SpaceAction::TabToSpace) => WhitespaceRendering::Space,
            (LfDecision::Keep, SpaceAction::Unchanged) => WhitespaceRendering::Keep,
        }
    }
    pub fn rendered_scalar(&self) -> Option<char> {
        match self.rendering() {
            WhitespaceRendering::Keep => Some(self.normalized()),
            WhitespaceRendering::Space => Some(' '),
            _ => None,
        }
    }
}
pub struct SourceWhitespaceInterval<'o, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    events: Range<usize>,
    scalars: Range<usize>,
}
impl<'o, 'e, 'd, 's, 'b, 'r> SourceWhitespaceInterval<'o, 'e, 'd, 's, 'b, 'r> {
    pub fn scalar_range(&self) -> Range<usize> {
        self.scalars.clone()
    }
    pub fn event_range(&self) -> Range<usize> {
        self.events.clone()
    }
    pub fn events(&self) -> SourceWhitespaceEvents<'o, 'e, 'd, 's, 'b, 'r> {
        SourceWhitespaceEvents {
            owner: self.owner,
            range: self.events.clone(),
        }
    }
    pub fn scalars(&self) -> SourceWhitespaceScalars<'o, 'e, 'd, 's, 'b, 'r> {
        SourceWhitespaceScalars {
            owner: self.owner,
            range: self.scalars.clone(),
        }
    }
}

pub struct SourceWhitespaceEvents<'o, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    range: Range<usize>,
}
impl<'o, 'e, 'd, 's, 'b, 'r> Iterator for SourceWhitespaceEvents<'o, 'e, 'd, 's, 'b, 'r> {
    type Item = SourceWhitespaceEvent<'o, 'e, 'd, 's, 'b, 'r>;
    fn next(&mut self) -> Option<Self::Item> {
        self.owner.event(self.range.next()?)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl ExactSizeIterator for SourceWhitespaceEvents<'_, '_, '_, '_, '_, '_> {}
impl std::iter::FusedIterator for SourceWhitespaceEvents<'_, '_, '_, '_, '_, '_> {}

pub struct SourceWhitespaceScalars<'o, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    range: Range<usize>,
}
impl<'o, 'e, 'd, 's, 'b, 'r> Iterator for SourceWhitespaceScalars<'o, 'e, 'd, 's, 'b, 'r> {
    type Item = SourceWhitespaceScalar<'o, 'e, 'd, 's, 'b, 'r>;
    fn next(&mut self) -> Option<Self::Item> {
        self.owner.scalar(self.range.next()?)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl ExactSizeIterator for SourceWhitespaceScalars<'_, '_, '_, '_, '_, '_> {}
impl std::iter::FusedIterator for SourceWhitespaceScalars<'_, '_, '_, '_, '_, '_> {}

type ResultOf<T> = Result<T, SourceWhitespaceError>;
fn payload<T>(capacity: usize) -> ResultOf<usize> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or(SourceWhitespaceError::Bytes)
}
fn admitted(parts: &[usize], allowance: usize) -> ResultOf<usize> {
    parts
        .iter()
        .try_fold(0usize, |a, b| a.checked_add(*b))
        .filter(|n| *n <= allowance)
        .ok_or(SourceWhitespaceError::Bytes)
}
fn forwarded(remaining: usize, needed: usize) -> ResultOf<NonZeroUsize> {
    if needed > remaining || needed > isize::MAX as usize {
        return Err(SourceWhitespaceError::Bytes);
    }
    Ok(NonZeroUsize::new(remaining.min(isize::MAX as usize).max(1)).expect("positive sentinel"))
}
fn amount(count: usize, multiple: usize, extra: usize) -> ResultOf<usize> {
    count
        .checked_mul(multiple)
        .and_then(|n| n.checked_add(extra))
        .ok_or(SourceWhitespaceError::Work)
}
struct Ledger {
    byte_limit: usize,
    work_limit: usize,
    work: usize,
    live: usize,
    peak: usize,
}
impl Ledger {
    fn new(limits: SourceWhitespaceLimits) -> Self {
        Self {
            byte_limit: limits.retained_bytes.get(),
            work_limit: limits.work.get(),
            work: 0,
            live: 0,
            peak: 0,
        }
    }
    fn debit(&mut self, needed: usize) -> ResultOf<NonZeroUsize> {
        self.work = self
            .work
            .checked_add(needed)
            .filter(|n| *n <= self.work_limit)
            .ok_or(SourceWhitespaceError::Work)?;
        Ok(NonZeroUsize::new(needed.max(1)).expect("positive sentinel"))
    }
    fn remaining_work(&self) -> usize {
        self.work_limit - self.work
    }
    fn remaining_bytes(&self) -> usize {
        self.byte_limit - self.live
    }
    fn set_live(&mut self, parts: &[usize]) -> ResultOf<()> {
        self.live = admitted(parts, self.byte_limit)?;
        self.peak = self.peak.max(self.live);
        Ok(())
    }
    fn reserve<T>(&mut self, vec: &mut Vec<T>, count: usize) -> ResultOf<usize> {
        admitted(&[self.live, payload::<T>(count)?], self.byte_limit)?;
        vec.try_reserve_exact(count)
            .map_err(|_| SourceWhitespaceError::Allocation)?;
        let bytes = payload::<T>(vec.capacity())?;
        self.set_live(&[self.live, bytes])?;
        Ok(bytes)
    }
}
fn newline_error(source: usize, error: NewlineError) -> SourceWhitespaceError {
    SourceWhitespaceError::Newlines { source, error }
}
fn lf_error(source: usize, error: LfConversionError) -> SourceWhitespaceError {
    SourceWhitespaceError::Lf { source, error }
}
fn preserved_error(source: usize, error: PreservedSpaceError) -> SourceWhitespaceError {
    SourceWhitespaceError::Preserved { source, error }
}
fn collapse_error(start: usize, error: SpaceCollapseError) -> SourceWhitespaceError {
    SourceWhitespaceError::Collapse { start, error }
}
fn text_of<'p>(participant: &'p SourceParticipant<'_, '_, '_, '_, '_>) -> &'p str {
    match participant.content() {
        crate::capture::ContentCapability::Text(text)
        | crate::capture::ContentCapability::RichText { text, .. } => text,
        _ => unreachable!("structural content is text"),
    }
}
fn white_space(participant: &SourceParticipant<'_, '_, '_, '_, '_>) -> ResultOf<WhiteSpace> {
    match participant.text_keywords().white_space().keyword().as_str() {
        "normal" => Ok(WhiteSpace::Normal),
        "nowrap" => Ok(WhiteSpace::Nowrap),
        "pre" => Ok(WhiteSpace::Pre),
        "pre-wrap" => Ok(WhiteSpace::PreWrap),
        "pre-line" => Ok(WhiteSpace::PreLine),
        _ => Err(SourceWhitespaceError::WhiteSpace {
            source: participant.source_index(),
        }),
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Events,
    Scalars,
    Normalized,
    LfPeak,
    LfScratchDropped,
    Preserved,
    PayloadDropped,
    Items,
    Collapse,
    ActionsDropped,
    ItemsDropped,
    Published,
}
#[cfg(test)]
#[derive(Debug, Clone, Copy, Default)]
struct Components {
    event_capacity: usize,
    scalar_capacity: usize,
    item_capacity: usize,
    preserved_capacity: usize,
    action_capacity: usize,
    normalized_bytes: usize,
    lf_retained_bytes: usize,
    lf_peak_bytes: usize,
}
#[cfg(test)]
#[derive(Debug, Clone, Copy, Default)]
struct Allowances {
    source: Option<usize>,
    segment: Option<(usize, usize)>,
    true_bytes: usize,
    forwarded_bytes: Option<usize>,
    true_work: usize,
    forwarded_work: Option<usize>,
    required_work: usize,
}
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
struct Receipt {
    phase: Phase,
    components: Components,
    allowances: Allowances,
    reported_live: usize,
    reported_peak: usize,
}
macro_rules! observe {
    ($observer:ident,$ledger:ident,$phase:expr,$components:expr,$allowances:expr) => {
        #[cfg(test)]
        $observer(Receipt {
            phase: $phase,
            components: $components,
            allowances: $allowances,
            reported_live: $ledger.live,
            reported_peak: $ledger.peak,
        });
    };
}

/// Process the complete admitted structural domain with original source identity.
/// Text/RichText aggregate payloads normalize and apply steps1–3 independently;
/// compatible step4 spans inline boundaries, stopping at each Block Begin/End.
/// No line selection, generated content, anonymous boxes or case execution occurs.
///
/// # Errors
/// Whole-domain errors precede payload traversal. Per-payload byte admission
/// precedes scalar decoding; scalar admission precedes its work charge. Other
/// work and payload checks occur before their corresponding operations. Nested
/// helper errors retain source/segment context. No partial output escapes.
pub fn preprocess_source_whitespace<'e, 'd, 's, 'b, 'r>(
    participants: &SourceParticipants<'e, 'd, 's, 'b, 'r>,
    limits: SourceWhitespaceLimits,
) -> ResultOf<SourceWhitespace<'e, 'd, 's, 'b, 'r>> {
    build(
        participants,
        limits,
        #[cfg(test)]
        &mut |_| {},
    )
}
fn build<'e, 'd, 's, 'b, 'r>(
    participants: &SourceParticipants<'e, 'd, 's, 'b, 'r>,
    limits: SourceWhitespaceLimits,
    #[cfg(test)] observer: &mut dyn FnMut(Receipt),
) -> ResultOf<SourceWhitespace<'e, 'd, 's, 'b, 'r>> {
    let mut ledger = Ledger::new(limits);
    ledger.debit(1)?;
    let structural_work =
        NonZeroUsize::new(ledger.remaining_work()).ok_or(SourceWhitespaceError::Work)?;
    let stream = source_structural_events(
        participants,
        SourceStructuralEventLimits {
            sources: limits.sources,
            events: limits.events,
            work: structural_work,
        },
    )
    .map_err(SourceWhitespaceError::Structural)?;
    ledger.debit(stream.work())?;
    let sources = stream.source_count();
    let contents = stream.content_count();
    let event_count = stream.event_count();
    ledger.debit(event_count)?;
    let mut events = Vec::new();
    let event_bytes = ledger.reserve::<EventRecord>(&mut events, event_count)?;
    observe!(
        observer,
        ledger,
        Phase::Events,
        Components {
            event_capacity: events.capacity(),
            ..Default::default()
        },
        Allowances::default()
    );
    let (mut input_bytes, mut raw_count) = (0usize, 0usize);
    for event in stream {
        let participant = event.participant();
        let source = participant.source_index();
        if event.kind() == SourceStructuralEventKind::Content {
            let text = text_of(participant);
            input_bytes = input_bytes
                .checked_add(text.len())
                .filter(|n| *n <= limits.input_bytes.get())
                .ok_or(SourceWhitespaceError::InputBytes { source })?;
            let mut chars = text.chars();
            while !chars.as_str().is_empty() {
                raw_count = raw_count
                    .checked_add(1)
                    .filter(|n| *n <= limits.input_scalars.get())
                    .ok_or(SourceWhitespaceError::InputScalars { source })?;
                ledger.debit(1)?;
                chars.next().expect("nonempty scalar input");
            }
        }
        events.push(EventRecord {
            source,
            kind: event.kind(),
            scalars: 0..0,
        });
    }
    let mut scalars = Vec::new();
    let scalar_bytes = ledger.reserve::<ScalarRecord>(&mut scalars, raw_count)?;
    let global = admitted(&[event_bytes, scalar_bytes], ledger.byte_limit)?;
    #[cfg(test)]
    let base = Components {
        event_capacity: events.capacity(),
        scalar_capacity: scalars.capacity(),
        ..Default::default()
    };
    observe!(
        observer,
        ledger,
        Phase::Scalars,
        base,
        Allowances::default()
    );
    ledger.debit(event_count)?;
    for event in &mut events {
        let start = scalars.len();
        if event.kind == SourceStructuralEventKind::Content {
            let source = event.source;
            let participant = participants.get(source).expect("admitted source");
            let mode = white_space(&participant)?;
            let raw = text_of(&participant);
            let needed = amount(raw.len(), 2, 0)?;
            #[cfg(test)]
            let true_work = ledger.remaining_work();
            let work = ledger.debit(needed)?;
            let remaining = ledger.remaining_bytes();
            let bytes = forwarded(remaining, usize::from(!raw.is_empty()))?;
            let normalized = normalize_default_newlines(
                raw,
                NewlineLimits {
                    input_bytes: limits.input_bytes,
                    retained_bytes: bytes,
                    work,
                },
            )
            .map_err(|e| newline_error(source, e))?;
            let normalized_bytes = normalized.retained_bytes();
            ledger.set_live(&[global, normalized_bytes])?;
            observe!(
                observer,
                ledger,
                Phase::Normalized,
                Components {
                    normalized_bytes,
                    ..base
                },
                Allowances {
                    source: Some(source),
                    true_bytes: remaining,
                    forwarded_bytes: Some(bytes.get()),
                    true_work,
                    forwarded_work: Some(work.get()),
                    required_work: needed,
                    ..Default::default()
                }
            );
            let count = normalized.scalars().len();
            let needed = amount(count, 6, 1)?;
            #[cfg(test)]
            let true_work = ledger.remaining_work();
            let work = ledger.debit(needed)?;
            let remaining = ledger.remaining_bytes();
            let minimum = admitted(
                &[
                    payload::<LfDecision>(count)?,
                    payload::<LfWhitespaceAction>(count)?,
                ],
                usize::MAX,
            )?;
            let bytes = forwarded(remaining, minimum)?;
            let lf = convert_line_feeds(
                &normalized,
                mode,
                LfConversionLimits {
                    scalars: limits.input_scalars,
                    retained_bytes: bytes,
                    work,
                },
            )
            .map_err(|e| lf_error(source, e))?;
            ledger.set_live(&[global, normalized_bytes, lf.peak_retained_bytes()])?;
            observe!(
                observer,
                ledger,
                Phase::LfPeak,
                Components {
                    normalized_bytes,
                    lf_retained_bytes: lf.retained_bytes(),
                    lf_peak_bytes: lf.peak_retained_bytes(),
                    ..base
                },
                Allowances {
                    source: Some(source),
                    true_bytes: remaining,
                    forwarded_bytes: Some(bytes.get()),
                    true_work,
                    forwarded_work: Some(work.get()),
                    required_work: needed,
                    ..Default::default()
                }
            );
            ledger.set_live(&[global, normalized_bytes, lf.retained_bytes()])?;
            observe!(
                observer,
                ledger,
                Phase::LfScratchDropped,
                Components {
                    normalized_bytes,
                    lf_retained_bytes: lf.retained_bytes(),
                    ..base
                },
                Allowances::default()
            );
            let needed = amount(count, 2, 0)?;
            #[cfg(test)]
            let true_work = ledger.remaining_work();
            let work = ledger.debit(needed)?;
            let remaining = ledger.remaining_bytes();
            let bytes = forwarded(remaining, payload::<PreservedSpace>(count)?)?;
            let preserved = preserved_space_sequences(
                &normalized,
                mode,
                PreservedSpaceLimits {
                    scalars: limits.input_scalars,
                    retained_bytes: bytes,
                    work,
                },
            )
            .map_err(|e| preserved_error(source, e))?;
            ledger.set_live(&[
                global,
                normalized_bytes,
                lf.retained_bytes(),
                payload::<PreservedSpace>(preserved.capacity())?,
            ])?;
            observe!(
                observer,
                ledger,
                Phase::Preserved,
                Components {
                    normalized_bytes,
                    lf_retained_bytes: lf.retained_bytes(),
                    preserved_capacity: preserved.capacity(),
                    ..base
                },
                Allowances {
                    source: Some(source),
                    true_bytes: remaining,
                    forwarded_bytes: Some(bytes.get()),
                    true_work,
                    forwarded_work: Some(work.get()),
                    required_work: needed,
                    ..Default::default()
                }
            );
            ledger.debit(count)?;
            for (((normalized, origin), decision), preserved) in normalized
                .text()
                .chars()
                .zip(normalized.scalars())
                .zip(lf.decisions())
                .zip(&preserved)
            {
                scalars.push(ScalarRecord {
                    source,
                    source_bytes: origin.source.clone(),
                    normalized,
                    white_space: mode,
                    lf: *decision,
                    preserved: *preserved,
                    adjacent: SpaceAction::Unchanged,
                });
            }
            drop(preserved);
            drop(lf);
            drop(normalized);
            ledger.set_live(&[global])?;
            observe!(
                observer,
                ledger,
                Phase::PayloadDropped,
                base,
                Allowances::default()
            );
        }
        event.scalars = start..scalars.len();
    }
    let count = scalars.len();
    let mut items = Vec::new();
    let item_bytes = ledger.reserve::<SpaceItem>(&mut items, count)?;
    #[cfg(test)]
    let projected = Components {
        item_capacity: items.capacity(),
        ..base
    };
    observe!(
        observer,
        ledger,
        Phase::Items,
        projected,
        Allowances::default()
    );
    ledger.debit(count)?;
    for scalar in &scalars {
        items.push(SpaceItem {
            unit: match scalar.lf {
                LfDecision::Keep => SpaceUnit::Scalar(scalar.normalized),
                LfDecision::Space => SpaceUnit::Scalar(' '),
                _ => SpaceUnit::Removed,
            },
            white_space: scalar.white_space,
        });
    }
    ledger.debit(event_count)?;
    let mut start = 0;
    for event in &events {
        if event.kind != SourceStructuralEventKind::Content
            && participants
                .get(event.source)
                .expect("admitted source")
                .resolution()
                .display
                == Display::Block
        {
            let end = event.scalars.start;
            collapse_segment(
                start..end,
                &items,
                &mut scalars,
                &mut ledger,
                (global, item_bytes),
                #[cfg(test)]
                projected,
                #[cfg(test)]
                observer,
            )?;
            start = end;
        }
    }
    collapse_segment(
        start..count,
        &items,
        &mut scalars,
        &mut ledger,
        (global, item_bytes),
        #[cfg(test)]
        projected,
        #[cfg(test)]
        observer,
    )?;
    drop(items);
    ledger.set_live(&[global])?;
    observe!(
        observer,
        ledger,
        Phase::ItemsDropped,
        base,
        Allowances::default()
    );
    observe!(
        observer,
        ledger,
        Phase::Published,
        base,
        Allowances::default()
    );
    Ok(SourceWhitespace {
        eligibility: participants.eligibility(),
        events,
        scalars,
        sources,
        contents,
        input_bytes,
        input_scalars: raw_count,
        work: ledger.work,
        retained_bytes: ledger.live,
        peak_retained_bytes: ledger.peak,
    })
}
fn collapse_segment(
    range: Range<usize>,
    items: &[SpaceItem],
    scalars: &mut [ScalarRecord],
    ledger: &mut Ledger,
    occupied: (usize, usize),
    #[cfg(test)] components: Components,
    #[cfg(test)] observer: &mut dyn FnMut(Receipt),
) -> ResultOf<()> {
    let (global, item_bytes) = occupied;
    let count = range.len();
    if count == 0 {
        return Ok(());
    }
    #[cfg(test)]
    let true_work = ledger.remaining_work();
    let work = ledger.debit(count)?;
    let remaining = ledger.remaining_bytes();
    let bytes = forwarded(remaining, payload::<SpaceAction>(count)?)?;
    let actions = collapse_adjacent_spaces(
        &items[range.clone()],
        SpaceCollapseLimits {
            items: NonZeroUsize::new(count).expect("nonempty segment"),
            retained_bytes: bytes,
            work,
        },
    )
    .map_err(|e| collapse_error(range.start, e))?;
    ledger.set_live(&[
        global,
        item_bytes,
        payload::<SpaceAction>(actions.capacity())?,
    ])?;
    observe!(
        observer,
        ledger,
        Phase::Collapse,
        Components {
            action_capacity: actions.capacity(),
            ..components
        },
        Allowances {
            segment: Some((range.start, range.end)),
            true_bytes: remaining,
            forwarded_bytes: Some(bytes.get()),
            true_work,
            forwarded_work: Some(work.get()),
            required_work: count,
            ..Default::default()
        }
    );
    ledger.debit(count)?;
    for (scalar, action) in scalars[range].iter_mut().zip(&actions) {
        scalar.adjacent = *action;
    }
    drop(actions);
    ledger.set_live(&[global, item_bytes])?;
    observe!(
        observer,
        ledger,
        Phase::ActionsDropped,
        components,
        Allowances::default()
    );
    Ok(())
}

#[cfg(test)]
#[path = "source_whitespace_tests.rs"]
mod tests;
