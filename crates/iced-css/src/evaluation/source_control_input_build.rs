use super::super::{
    source_presentation_join::{self as join, JoinReason},
    source_structural_events::SourceStructuralEventKind,
    source_structure::SourceStructureKind as Kind,
};
use super::*;
use SourceAnalysisParticipation as Participation;
use SourceControlInputErrorKind as ErrorKind;
use SourceControlInvariantReason as Reason;
use SourceControlLimit as Limit;
use SourceControlStorage as Slot;
use SourceControlUnsupported as Unsupported;
use iced_css_core::text::bidi_data::{BidiClass, bidi_class};
use std::mem::size_of;

#[derive(Clone, Copy, Default, Debug)]
struct Context {
    source: Option<usize>,
    record: Option<usize>,
    original: Option<usize>,
    member: Option<usize>,
}
impl Context {
    fn error(self, kind: ErrorKind) -> SourceControlInputError {
        SourceControlInputError {
            source: self.source,
            record: self.record,
            original: self.original,
            member: self.member,
            kind,
        }
    }
    fn invariant(self, reason: Reason) -> SourceControlInputError {
        self.error(ErrorKind::Invariant(reason))
    }
    fn join(self, reason: JoinReason) -> SourceControlInputError {
        self.invariant(match reason {
            JoinReason::OwnerCounts => Reason::OwnerCounts,
            JoinReason::SourceJoin => Reason::SourceJoin,
            JoinReason::ScopeState => Reason::ScopeState,
            JoinReason::RecordParent => Reason::RecordParent,
            JoinReason::PayloadCoverage => Reason::PayloadCoverage,
            JoinReason::ScalarCoverage => Reason::ScalarCoverage,
        })
    }
}
fn global() -> Context {
    Context::default()
}
fn admit(
    value: usize,
    limit: usize,
    which: Limit,
    context: Context,
) -> Result<usize, SourceControlInputError> {
    if value > limit {
        Err(context.error(ErrorKind::Limit(which)))
    } else {
        Ok(value)
    }
}
fn add(a: usize, b: usize, context: Context) -> Result<usize, SourceControlInputError> {
    a.checked_add(b)
        .ok_or_else(|| context.error(ErrorKind::Addressability))
}
fn bytes<T>(n: usize) -> Result<usize, SourceControlInputError> {
    n.checked_mul(size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| global().error(ErrorKind::Addressability))
}
#[derive(Debug)]
struct Ledger {
    used: usize,
    limit: usize,
}
impl Ledger {
    fn charge(&mut self, n: usize, c: Context) -> Result<(), SourceControlInputError> {
        self.used = self
            .used
            .checked_add(n)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| c.error(ErrorKind::Limit(Limit::Work)))?;
        Ok(())
    }
}
#[derive(Debug, Clone, Copy)]
struct Frame {
    record: usize,
    source: Option<usize>,
    opening: Option<char>,
}
impl Frame {
    fn context(self) -> Context {
        Context {
            source: self.source,
            record: Some(self.record),
            ..global()
        }
    }
}
#[derive(Default, Debug, PartialEq, Eq)]
struct Counts {
    records: usize,
    payloads: usize,
    originals: usize,
    literals: usize,
    synthetic: usize,
    tokens: usize,
    scopes: usize,
    domains: usize,
    bytes: usize,
    frames: usize,
    depth: usize,
    visits: usize,
    units: usize,
}
#[derive(Debug)]
struct PendingScope {
    record: usize,
    context: Context,
    participation: Participation,
    originals: usize,
    payloads: usize,
    tokens: usize,
    bytes: usize,
}
#[derive(Default, Debug)]
struct Storage {
    scopes: Vec<ScopeRow>,
    payloads: Vec<PayloadRow>,
    originals: Vec<OriginalRow>,
    tokens: Vec<TokenRow>,
    transport: String,
}
impl Storage {
    fn capacities(&self) -> [usize; 5] {
        [
            self.scopes.capacity(),
            self.payloads.capacity(),
            self.originals.capacity(),
            self.tokens.capacity(),
            self.transport.capacity(),
        ]
    }
}
fn push<T>(v: &mut Vec<T>, value: T, c: Context) -> Result<(), SourceControlInputError> {
    if v.len() == v.capacity() {
        return Err(c.invariant(Reason::EmissionCounts));
    }
    v.push(value);
    Ok(())
}
struct Walk<'a> {
    counts: Counts,
    ledger: &'a mut Ledger,
    debit: bool,
    limits: SourceControlInputLimits,
    output: Option<&'a mut Storage>,
    scope: Option<PendingScope>,
    occurrence: usize,
    /// The one live block frame carrying an opening, if any. An overridden block has
    /// `InlineSequence` participation, so no block can nest inside it and at most one
    /// exists at a time. Kept here so the forced-break guard costs no traversal.
    block_opening: Option<Frame>,
}
impl Walk<'_> {
    fn tick(&mut self, c: Context) -> Result<(), SourceControlInputError> {
        if self.debit {
            self.ledger.charge(1, c)?;
        }
        self.counts.units = add(self.counts.units, 1, c)?;
        Ok(())
    }
    fn token(
        &mut self,
        ch: char,
        origin: OriginRow,
        c: Context,
    ) -> Result<(), SourceControlInputError> {
        let n = add(self.counts.tokens, 1, c)?;
        admit(n, self.limits.output_scalars.get(), Limit::OutputScalars, c)?;
        let end = add(self.counts.bytes, ch.len_utf8(), c)?;
        admit(end, self.limits.output_bytes.get(), Limit::OutputBytes, c)?;
        if self
            .scope
            .as_ref()
            .is_none_or(|s| s.participation != Participation::InlineSequence)
        {
            return Err(c.invariant(Reason::ScopeState));
        }
        if let Some(out) = self.output.as_deref_mut() {
            if end > out.transport.capacity() {
                return Err(c.invariant(Reason::EmissionCounts));
            }
            push(
                &mut out.tokens,
                TokenRow {
                    character: ch,
                    byte_end: end,
                    scope: self.counts.scopes - 1,
                    origin,
                },
                c,
            )?;
            out.transport.push(ch);
        }
        self.counts.tokens = n;
        self.counts.bytes = end;
        Ok(())
    }
    fn synthetic(
        &mut self,
        frame: Frame,
        ch: char,
        placement: PlacementRow,
    ) -> Result<(), SourceControlInputError> {
        let mut c = frame.context();
        if let PlacementRow::Break {
            original, member, ..
        } = placement
        {
            c.original = Some(original);
            c.member = Some(member);
        }
        self.tick(c)?;
        self.token(
            ch,
            OriginRow::Synthetic {
                occurrence: self.occurrence,
                cause: frame.record,
                placement,
            },
            c,
        )?;
        self.occurrence = add(self.occurrence, 1, c)?;
        self.counts.synthetic = add(self.counts.synthetic, 1, c)?;
        Ok(())
    }
    fn pop(&mut self, frame: Frame) -> Result<(), SourceControlInputError> {
        if self
            .block_opening
            .is_some_and(|live| live.record == frame.record)
        {
            self.block_opening = None;
        }
        self.tick(frame.context())?;
        if frame.opening.is_some() {
            self.synthetic(
                frame,
                '\u{202c}',
                PlacementRow::Edge(SourceStructureBoundarySide::End),
            )?;
        }
        Ok(())
    }
    fn finalize(&mut self, record_end: usize) -> Result<(), SourceControlInputError> {
        if let Some(s) = self.scope.take() {
            if (s.participation != Participation::InlineSequence
                && (s.tokens != self.counts.tokens
                    || s.originals != self.counts.originals
                    || s.payloads != self.counts.payloads))
                || record_end <= s.record
            {
                return Err(s.context.invariant(Reason::ScopeFinalization));
            }
            if let Some(out) = self.output.as_deref_mut() {
                push(
                    &mut out.scopes,
                    ScopeRow {
                        record: s.record,
                        record_end,
                        participation: s.participation,
                        originals: s.originals..self.counts.originals,
                        payloads: s.payloads..self.counts.payloads,
                        tokens: s.tokens..self.counts.tokens,
                        bytes: s.bytes..self.counts.bytes,
                    },
                    s.context,
                )?;
            }
        }
        Ok(())
    }
}
fn opening(direction: ComputedDirection, bidi: ComputedUnicodeBidi) -> Option<char> {
    match (direction, bidi) {
        (_, ComputedUnicodeBidi::Normal) => None,
        (ComputedDirection::LeftToRight, ComputedUnicodeBidi::Embed) => Some('\u{202a}'),
        (ComputedDirection::RightToLeft, ComputedUnicodeBidi::Embed) => Some('\u{202b}'),
        (ComputedDirection::LeftToRight, ComputedUnicodeBidi::BidiOverride) => Some('\u{202d}'),
        (ComputedDirection::RightToLeft, ComputedUnicodeBidi::BidiOverride) => Some('\u{202e}'),
    }
}
#[derive(Debug, Clone, Copy)]
struct InterruptionInventory {
    source: usize,
    /// Number of interrupting blocks. The validated shape fixes the whole layout, so
    /// every admitted record index is arithmetic in its seven-record period and no
    /// per-record list is stored.
    interruptions: usize,
    trigger: Context,
    kind: SourceStructureEdgeKind,
}
impl InterruptionInventory {
    fn unsupported(self) -> SourceControlInputError {
        self.trigger
            .error(ErrorKind::Unsupported(Unsupported::InterruptedInline(
                self.kind,
            )))
    }
    /// Whether `record` is one of the graph's payloads. Defensive: a validated shape
    /// admits every payload it contains, so no public input reaches the rejecting arm.
    /// Both the affected element's
    /// payloads (offset 4 of each period) and the interrupting blocks' payloads
    /// (offset 7) belong to the admitted graph.
    fn authenticates_payload(self, record: usize) -> bool {
        (record >= 4 && (record - 4).is_multiple_of(7) && (record - 4) / 7 <= self.interruptions)
            || (record >= 7 && record.is_multiple_of(7) && record / 7 <= self.interruptions)
    }
    fn accepts(
        self,
        record: usize,
        source: Option<usize>,
        side: SourceStructureBoundarySide,
        kind: SourceStructureEdgeKind,
    ) -> bool {
        if source != Some(self.source) || record < 2 || !(record - 2).is_multiple_of(7) {
            return false;
        }
        let fragment = (record - 2) / 7;
        if fragment > self.interruptions {
            return false;
        }
        match (side, kind) {
            (SourceStructureBoundarySide::End, SourceStructureEdgeKind::BeforeInterruption) => {
                fragment < self.interruptions
            }
            (SourceStructureBoundarySide::Start, SourceStructureEdgeKind::AfterInterruption) => {
                fragment >= 1
            }
            _ => false,
        }
    }
}
#[derive(Clone, Copy)]
struct EdgeAnchor {
    kind: SourceStructureEdgeKind,
    event: usize,
    event_kind: SourceStructuralEventKind,
    source: usize,
}
fn anchor(
    record: &SourceStructureRecord<'_, '_, '_, '_, '_, '_, '_>,
    side: SourceStructureBoundarySide,
) -> Option<EdgeAnchor> {
    let edge = match side {
        SourceStructureBoundarySide::Start => record.start_edge(),
        SourceStructureBoundarySide::End => record.end_edge(),
    }?;
    if !edge.record().is_same_record(record) || edge.side() != side {
        return None;
    }
    let event = edge.event();
    Some(EdgeAnchor {
        kind: edge.kind(),
        event: event.index(),
        event_kind: event.kind(),
        source: event.participant().source_index(),
    })
}
/// One prepaid record pass. Roles describe the tree; captured source/record IDs
/// are never constants in the admission predicate. No presentation is inspected.
fn inventory_shape(
    structure: &SourceStructure<'_, '_, '_, '_, '_, '_>,
    trigger: Context,
    kind: SourceStructureEdgeKind,
) -> Result<InterruptionInventory, SourceControlInputError> {
    let failure = || trigger.error(ErrorKind::Unsupported(Unsupported::InterruptedInline(kind)));
    // The admitted graph is an outer block followed by N periods of seven records and a
    // final four-record fragment group: records = 7N + 5 and sources = 3N + 3.
    let count = structure.record_count();
    if count < 12 || !(count - 5).is_multiple_of(7) {
        return Err(failure());
    }
    let interruptions = (count - 5) / 7;
    if structure.source_count() != 3 * interruptions + 3 {
        return Err(failure());
    }
    let last_fragment = 7 * interruptions + 2;
    let mut outer = None;
    let mut affected = None;
    let mut previous_block_end = None;
    let mut pending_fragment_end = None;
    for (role, record) in structure.records().enumerate() {
        #[cfg(test)]
        qualification::inventory_record();
        // Role classification is pure arithmetic on the period, so it never consults a
        // captured identity: source indices are observed, never assumed.
        let offset = if role == 0 { 0 } else { (role - 1) % 7 + 1 };
        let expected_kind = match offset {
            0 | 5 => Kind::SourceBlock,
            1 => Kind::AnonymousBlock,
            2 | 3 | 6 => Kind::SourceInline,
            4 | 7 => Kind::Payload,
            _ => return Err(failure()),
        };
        let expected_parent = match offset {
            0 => None,
            1 | 5 => Some(0),
            _ => Some(role - 1),
        };
        // `records()` yields indices in order, so the index check restates the API's
        // guarantee; it is kept because the congruence tests below depend on it.
        if record.kind() != expected_kind
            || record.formatting_parent().map(|p| p.index()) != expected_parent
            || record.index() != role
        {
            return Err(failure());
        }
        let source = record.source_index();
        match record.kind() {
            Kind::SourceBlock | Kind::SourceInline => {
                let participant = record.participant().ok_or_else(failure)?;
                // Only an inline carries an embedding effect (CSS 2.2 §9.10); a block may
                // compute Embed or Override without one. An overridden outer block is
                // already rejected at its first anonymous record before this runs, and an
                // overridden interrupting block is deliberately admitted. The affected
                // inline is exactly the fragment at each period's offset 2.
                let effective = record.kind() == Kind::SourceInline
                    && participant.bidi_style().unicode_bidi() != ComputedUnicodeBidi::Normal;
                if effective != (offset == 2) {
                    return Err(failure());
                }
                let start =
                    anchor(&record, SourceStructureBoundarySide::Start).ok_or_else(failure)?;
                let end = anchor(&record, SourceStructureBoundarySide::End).ok_or_else(failure)?;
                let source = source.ok_or_else(failure)?;
                if offset == 2 {
                    // Every fragment belongs to one affected element; the first opens with
                    // its original begin and the last closes with its original end.
                    if *affected.get_or_insert(source) != source {
                        return Err(failure());
                    }
                    let opens_original = role == 2;
                    let closes_original = role == last_fragment;
                    if start.kind
                        != if opens_original {
                            SourceStructureEdgeKind::OriginalBegin
                        } else {
                            SourceStructureEdgeKind::AfterInterruption
                        }
                        || end.kind
                            != if closes_original {
                                SourceStructureEdgeKind::OriginalEnd
                            } else {
                                SourceStructureEdgeKind::BeforeInterruption
                            }
                        || start.event_kind
                            != if opens_original {
                                SourceStructuralEventKind::Begin
                            } else {
                                SourceStructuralEventKind::End
                            }
                        || end.event_kind
                            != if closes_original {
                                SourceStructuralEventKind::End
                            } else {
                                SourceStructuralEventKind::Begin
                            }
                        || (opens_original && start.source != source)
                        || (closes_original && end.source != source)
                    {
                        return Err(failure());
                    }
                    // An interruption edge names the neighbouring block and shares its
                    // event; cuts intentionally differ by side and are not compared.
                    if !opens_original
                        && previous_block_end.take() != Some((start.event, start.source))
                    {
                        return Err(failure());
                    }
                    if !closes_original {
                        pending_fragment_end = Some((end.event, end.source));
                    }
                } else {
                    // Every other element is unfragmented: both of its edges are original
                    // and name itself. An element with both original edges owns exactly one
                    // record. What this establishes *locally* is only that no such role
                    // carries the affected source: an inline sharing it would share its
                    // nonnormal value and fail the `effective` test above, and a block
                    // cannot share a source with an inline because the structure builder
                    // picks a record's kind from its participant's display. Mutual
                    // distinctness of these roles rests on the builder emitting one Begin
                    // and one End per source, which is not re-proven here; nothing in this
                    // producer depends on it.
                    if start.kind != SourceStructureEdgeKind::OriginalBegin
                        || end.kind != SourceStructureEdgeKind::OriginalEnd
                        || start.event_kind != SourceStructuralEventKind::Begin
                        || end.event_kind != SourceStructuralEventKind::End
                        || start.source != source
                        || end.source != source
                        || affected.is_some_and(|a| a == source)
                    {
                        return Err(failure());
                    }
                    if role == 0 {
                        outer = Some(source);
                    }
                    if offset == 5 {
                        // The interrupting block must answer the fragment that just closed.
                        if pending_fragment_end.take() != Some((start.event, source)) {
                            return Err(failure());
                        }
                        previous_block_end = Some((end.event, source));
                    }
                }
            }
            Kind::AnonymousBlock => {
                let origin = record.anonymous_origin().ok_or_else(failure)?;
                if source.is_some()
                    || origin.enclosing_block().index() != 0
                    || Some(origin.inheritance_source().source_index()) != outer
                {
                    return Err(failure());
                }
            }
            Kind::Payload => {
                if source
                    != structure
                        .record(role - 1)
                        .ok_or_else(failure)?
                        .source_index()
                {
                    return Err(failure());
                }
            }
        }
    }
    if previous_block_end.is_some() || pending_fragment_end.is_some() {
        return Err(failure());
    }
    Ok(InterruptionInventory {
        source: affected.ok_or_else(failure)?,
        interruptions,
        trigger,
        kind,
    })
}
fn inline_open(
    record: &SourceStructureRecord<'_, '_, '_, '_, '_, '_, '_>,
    c: Context,
    ledger: &mut Ledger,
    debit: bool,
    inventory: &mut Option<InterruptionInventory>,
) -> Result<Option<char>, SourceControlInputError> {
    let participant = record
        .participant()
        .ok_or_else(|| c.invariant(Reason::EdgeIdentity))?;
    let style = participant.bidi_style();
    let value = opening(style.direction(), style.unicode_bidi());
    for side in [
        SourceStructureBoundarySide::Start,
        SourceStructureBoundarySide::End,
    ] {
        let edge = match side {
            SourceStructureBoundarySide::Start => record.start_edge(),
            SourceStructureBoundarySide::End => record.end_edge(),
        }
        .ok_or_else(|| c.invariant(Reason::EdgeIdentity))?;
        if edge.record().index() != record.index() || edge.side() != side {
            return Err(c.invariant(Reason::EdgeIdentity));
        }
        let kind = edge.kind();
        let event = edge.event();
        let expected = match kind {
            SourceStructureEdgeKind::OriginalBegin
            | SourceStructureEdgeKind::BeforeInterruption => SourceStructuralEventKind::Begin,
            SourceStructureEdgeKind::OriginalEnd | SourceStructureEdgeKind::AfterInterruption => {
                SourceStructuralEventKind::End
            }
        };
        if event.kind() != expected
            || (matches!(
                kind,
                SourceStructureEdgeKind::OriginalBegin | SourceStructureEdgeKind::OriginalEnd
            ) && Some(event.participant().source_index()) != record.source_index())
        {
            return Err(c.invariant(Reason::EdgeIdentity));
        }
        if value.is_some()
            && !matches!(
                (side, kind),
                (
                    SourceStructureBoundarySide::Start,
                    SourceStructureEdgeKind::OriginalBegin
                ) | (
                    SourceStructureBoundarySide::End,
                    SourceStructureEdgeKind::OriginalEnd
                )
            )
        {
            if inventory.is_none() {
                if !debit {
                    return Err(c.invariant(Reason::EmissionCounts));
                }
                #[cfg(test)]
                qualification::inventory_before(ledger.used);
                ledger.charge(record.owner().record_count(), global())?;
                #[cfg(test)]
                qualification::inventory_after(ledger.used);
                *inventory = Some(inventory_shape(record.owner(), c, kind)?);
            }
            let admitted = inventory.ok_or_else(|| c.invariant(Reason::EmissionCounts))?;
            if !admitted.accepts(record.index(), record.source_index(), side, kind) {
                return Err(c.invariant(Reason::EmissionCounts));
            }
            #[cfg(test)]
            qualification::inventory_use(debit);
        }
    }
    Ok(value)
}
fn walk(
    structure: &SourceStructure<'_, '_, '_, '_, '_, '_>,
    casing: &SourceCasing<'_, '_, '_, '_, '_, '_>,
    frames: &mut Vec<Frame>,
    ledger: &mut Ledger,
    limits: SourceControlInputLimits,
    output: Option<&mut Storage>,
    inventory: &mut Option<InterruptionInventory>,
) -> Result<Counts, SourceControlInputError> {
    if !frames.is_empty() {
        return Err(global().invariant(Reason::TraversalCompletion));
    }
    let debit = output.is_none();
    let mut w = Walk {
        counts: Counts::default(),
        ledger,
        debit,
        limits,
        output,
        scope: None,
        occurrence: 0,
        block_opening: None,
    };
    let mut previous_content = None;
    for index in 0..structure.record_count() {
        let record = structure
            .record(index)
            .ok_or_else(|| global().invariant(Reason::RecordParent))?;
        let c = Context {
            source: record.source_index(),
            record: Some(index),
            ..global()
        };
        w.tick(c)?;
        w.counts.records = add(w.counts.records, 1, c)?;
        let parent = record.formatting_parent();
        let target = parent.as_ref().map(|p| p.index());
        while frames.last().map(|f| f.record) != target {
            let frame = frames
                .pop()
                .ok_or_else(|| c.invariant(Reason::RecordParent))?;
            w.pop(frame)?;
        }
        join::record_parent(
            index,
            record.kind(),
            parent.as_ref().map(|p| (p.index(), p.kind())),
        )
        .map_err(|r| c.join(r))?;
        let mut open = None;
        match record.kind() {
            Kind::SourceBlock | Kind::AnonymousBlock => {
                // CSS 2.2 §9.10 places a block override opening at the start of each
                // anonymous child block box but names only one PDF, at the element
                // end. That per-anonymous-child closing schedule is unresolved.
                if let Some(origin) = record.anonymous_origin()
                    && origin.inheritance_source().bidi_style().unicode_bidi()
                        == ComputedUnicodeBidi::BidiOverride
                {
                    let enclosing = origin.enclosing_block();
                    let bc = Context {
                        source: enclosing.source_index(),
                        record: Some(index),
                        ..global()
                    };
                    return Err(bc.error(ErrorKind::Unsupported(Unsupported::BlockBidi(
                        ComputedUnicodeBidi::BidiOverride,
                    ))));
                }
                let scopes = add(w.counts.scopes, 1, c)?;
                admit(scopes, limits.scopes.get(), Limit::Scopes, c)?;
                join::scope_parent(parent.as_ref().map(|p| p.kind())).map_err(|r| c.join(r))?;
                let participation =
                    join::marker_participation(structure, index).map_err(|r| c.join(r))?;
                w.finalize(index)?;
                w.counts.scopes = scopes;
                if participation == Participation::InlineSequence {
                    w.counts.domains = add(w.counts.domains, 1, c)?;
                    // A block container overrides its inline-level descendants that are
                    // not within another block container. `InlineSequence` is exactly
                    // "no anonymous child block box": source_structure_build's run_parent
                    // wraps *every* inline run of a block that has any block-display
                    // descendant begin, so a block's children are all inline or all
                    // block. The opening therefore goes at the element start and its PDF
                    // at the element end, wrapping this one domain.
                    if let Some(participant) = record.participant() {
                        let style = participant.bidi_style();
                        if style.unicode_bidi() == ComputedUnicodeBidi::BidiOverride {
                            open = opening(style.direction(), style.unicode_bidi());
                        }
                    }
                }
                w.scope = Some(PendingScope {
                    record: index,
                    context: c,
                    participation,
                    originals: w.counts.originals,
                    payloads: w.counts.payloads,
                    tokens: w.counts.tokens,
                    bytes: w.counts.bytes,
                });
                w.occurrence = 0;
            }
            Kind::SourceInline | Kind::Payload => {
                join::inline_parent(
                    record.kind(),
                    w.scope.as_ref().map(|s| (s.record, s.participation)),
                    parent.as_ref().map(|p| (p.index(), p.kind())),
                )
                .map_err(|r| c.join(r))?;
                if record.kind() == Kind::SourceInline {
                    open = inline_open(&record, c, w.ledger, w.debit, inventory)?;
                } else {
                    let content = record
                        .content_event()
                        .ok_or_else(|| c.invariant(Reason::PayloadCoverage))?;
                    let range = join::payload_join(
                        &record,
                        parent.as_ref().and_then(|p| p.source_index()),
                        &content,
                        previous_content,
                        w.counts.originals,
                        casing.scalar_count(),
                    )
                    .map_err(|r| c.join(r))?;
                    let payload = w.counts.payloads;
                    let payload_start = w.counts.tokens;
                    let literal_start = w.counts.literals;
                    if inventory.is_some_and(|i| !i.authenticates_payload(index)) {
                        return Err(c.invariant(Reason::EmissionCounts));
                    }
                    let mut non_b = false;
                    let mut pending: Option<Context> = None;
                    for original in range.clone() {
                        let oc = Context {
                            original: Some(original),
                            ..c
                        };
                        w.tick(oc)?;
                        let scalar = casing
                            .scalar(original)
                            .ok_or_else(|| oc.invariant(Reason::ScalarCoverage))?;
                        join::original_join(
                            original,
                            &scalar,
                            content.participant().source_index(),
                        )
                        .map_err(|r| oc.join(r))?;
                        let original_start = w.counts.tokens;
                        let mut members = 0;
                        for (member, ch) in scalar.presentation().characters().enumerate() {
                            let mc = Context {
                                member: Some(member),
                                ..oc
                            };
                            w.tick(mc)?;
                            let is_b = bidi_class(ch) == BidiClass::B;
                            if is_b && let Some(admitted) = *inventory {
                                return Err(admitted.unsupported());
                            }
                            if is_b {
                                if let Some(earlier) = pending {
                                    return Err(earlier.error(ErrorKind::Unsupported(
                                        Unsupported::BreakBoundary(SourceControlBreakSide::After),
                                    )));
                                }
                                if !non_b {
                                    return Err(mc.error(ErrorKind::Unsupported(
                                        Unsupported::BreakBoundary(SourceControlBreakSide::Before),
                                    )));
                                }
                                // Writing Modes §2.4.4's close/reopen rule is scoped to
                                // an *inline* box broken around a paragraph boundary. A
                                // block container is not broken here, and §9.10 names no
                                // reopening, so renewing a block override at this break
                                // would invent a schedule no authority states.
                                if let Some(block_frame) = w.block_opening {
                                    let bc = Context {
                                        source: block_frame.source,
                                        record: Some(block_frame.record),
                                        original: Some(original),
                                        member: Some(member),
                                    };
                                    return Err(bc.error(ErrorKind::Unsupported(
                                        Unsupported::BlockBidi(ComputedUnicodeBidi::BidiOverride),
                                    )));
                                }
                                for frame in frames.iter().rev().copied() {
                                    w.tick(mc)?;
                                    w.counts.visits = add(w.counts.visits, 1, mc)?;
                                    if frame.opening.is_some() {
                                        w.synthetic(
                                            frame,
                                            '\u{202c}',
                                            PlacementRow::Break {
                                                original,
                                                member,
                                                side: SourceControlBreakSide::Before,
                                            },
                                        )?;
                                    }
                                }
                            }
                            w.token(ch, OriginRow::Literal { original, member }, mc)?;
                            w.counts.literals = add(w.counts.literals, 1, mc)?;
                            members = add(members, 1, mc)?;
                            if is_b {
                                for frame in frames.iter().copied() {
                                    w.tick(mc)?;
                                    w.counts.visits = add(w.counts.visits, 1, mc)?;
                                    if let Some(ch) = frame.opening {
                                        w.synthetic(
                                            frame,
                                            ch,
                                            PlacementRow::Break {
                                                original,
                                                member,
                                                side: SourceControlBreakSide::After,
                                            },
                                        )?;
                                    }
                                }
                                non_b = false;
                                pending = Some(mc);
                            } else {
                                non_b = true;
                                pending = None;
                            }
                        }
                        if let Some(out) = w.output.as_deref_mut() {
                            push(
                                &mut out.originals,
                                OriginalRow {
                                    payload,
                                    members,
                                    tokens: original_start..w.counts.tokens,
                                },
                                oc,
                            )?;
                        }
                        w.counts.originals = add(w.counts.originals, 1, oc)?;
                    }
                    if let Some(earlier) = pending {
                        return Err(earlier.error(ErrorKind::Unsupported(
                            Unsupported::BreakBoundary(SourceControlBreakSide::After),
                        )));
                    }
                    if w.counts.literals == literal_start
                        && let Some(admitted) = *inventory
                    {
                        return Err(admitted.unsupported());
                    }
                    if let Some(out) = w.output.as_deref_mut() {
                        push(
                            &mut out.payloads,
                            PayloadRow {
                                record: index,
                                scope: w.counts.scopes - 1,
                                originals: range,
                                tokens: payload_start..w.counts.tokens,
                            },
                            c,
                        )?;
                    }
                    w.counts.payloads = add(w.counts.payloads, 1, c)?;
                    previous_content = Some(content.index());
                }
            }
        }
        if record.kind() != Kind::Payload {
            let depth = add(frames.len(), 1, c)?;
            admit(depth, limits.depth.get(), Limit::Depth, c)?;
            let frame = Frame {
                record: index,
                source: record.source_index(),
                opening: open,
            };
            push(frames, frame, c)?;
            if open.is_some() && record.kind() == Kind::SourceBlock {
                w.block_opening = Some(frame);
            }
            w.counts.frames = add(w.counts.frames, 1, c)?;
            w.counts.depth = w.counts.depth.max(depth);
            if let Some(ch) = open {
                w.synthetic(
                    frame,
                    ch,
                    PlacementRow::Edge(SourceStructureBoundarySide::Start),
                )?;
            }
        }
    }
    while let Some(frame) = frames.pop() {
        w.pop(frame)?;
    }
    w.tick(global())?;
    if w.counts.records != structure.record_count()
        || w.counts.payloads != structure.receipt().contents
        || w.counts.originals != casing.scalar_count()
        || w.scope.is_none()
    {
        return Err(global().invariant(Reason::Completion));
    }
    w.finalize(structure.record_count())?;
    Ok(w.counts)
}
fn allocation(slot: Slot, source: TryReserveError) -> SourceControlInputError {
    global().error(ErrorKind::Allocation {
        storage: slot,
        source,
    })
}
fn reserve_result(
    slot: Slot,
    result: Result<(), TryReserveError>,
) -> Result<(), SourceControlInputError> {
    result.map_err(|source| allocation(slot, source))
}
fn reserve_vec<T>(v: &mut Vec<T>, n: usize, slot: Slot) -> Result<(), SourceControlInputError> {
    if n != 0 {
        #[cfg(test)]
        let request = qualification::reserve_request(slot, n);
        #[cfg(not(test))]
        let request: Result<usize, TryReserveError> = Ok(n);
        reserve_result(
            slot,
            request.and_then(|requested| v.try_reserve_exact(requested)),
        )?;
    }
    Ok(())
}
fn reserve_string(v: &mut String, n: usize) -> Result<(), SourceControlInputError> {
    if n != 0 {
        #[cfg(test)]
        let request = qualification::reserve_request(Slot::Transport, n);
        #[cfg(not(test))]
        let request: Result<usize, TryReserveError> = Ok(n);
        reserve_result(
            Slot::Transport,
            request.and_then(|requested| v.try_reserve_exact(requested)),
        )?;
    }
    Ok(())
}
fn retained_for(capacities: [usize; 5]) -> Result<usize, SourceControlInputError> {
    let values = [
        bytes::<ScopeRow>(capacities[0])?,
        bytes::<PayloadRow>(capacities[1])?,
        bytes::<OriginalRow>(capacities[2])?,
        bytes::<TokenRow>(capacities[3])?,
        bytes::<u8>(capacities[4])?,
    ];
    values.into_iter().try_fold(0, |a, b| add(a, b, global()))
}
fn admit_scratch(value: usize, l: SourceControlInputLimits) -> Result<(), SourceControlInputError> {
    admit(value, l.scratch_bytes.get(), Limit::ScratchBytes, global())?;
    admit(value, l.peak_bytes.get(), Limit::PeakBytes, global())?;
    Ok(())
}
fn admit_storage(
    retained: usize,
    scratch: usize,
    l: SourceControlInputLimits,
) -> Result<usize, SourceControlInputError> {
    admit(
        retained,
        l.retained_bytes.get(),
        Limit::RetainedBytes,
        global(),
    )?;
    let peak = add(retained, scratch, global())?;
    admit(peak, l.peak_bytes.get(), Limit::PeakBytes, global())?;
    Ok(peak)
}
fn reserve_storage(
    counts: &Counts,
    scratch: usize,
    l: SourceControlInputLimits,
) -> Result<Storage, SourceControlInputError> {
    let mut out = Storage::default();
    let mut pending = [
        counts.scopes,
        counts.payloads,
        counts.originals,
        counts.tokens,
        counts.bytes,
    ];
    admit_storage(retained_for(pending)?, scratch, l)?;
    reserve_vec(&mut out.scopes, pending[0], Slot::Scopes)?;
    pending[0] = out.scopes.capacity();
    admit_storage(retained_for(pending)?, scratch, l)?;
    reserve_vec(&mut out.payloads, pending[1], Slot::Payloads)?;
    pending[1] = out.payloads.capacity();
    admit_storage(retained_for(pending)?, scratch, l)?;
    reserve_vec(&mut out.originals, pending[2], Slot::Originals)?;
    pending[2] = out.originals.capacity();
    admit_storage(retained_for(pending)?, scratch, l)?;
    reserve_vec(&mut out.tokens, pending[3], Slot::Tokens)?;
    pending[3] = out.tokens.capacity();
    admit_storage(retained_for(pending)?, scratch, l)?;
    reserve_string(&mut out.transport, pending[4])?;
    pending[4] = out.transport.capacity();
    admit_storage(retained_for(pending)?, scratch, l)?;
    Ok(out)
}
fn verify_emission(
    out: &Storage,
    count: &Counts,
    replay: &Counts,
    capacities: [usize; 5],
    frame_capacity: usize,
    frames: &Vec<Frame>,
) -> Result<(), SourceControlInputError> {
    if count != replay
        || out.capacities() != capacities
        || frames.capacity() != frame_capacity
        || !frames.is_empty()
        || out.scopes.len() != count.scopes
        || out.payloads.len() != count.payloads
        || out.originals.len() != count.originals
        || out.tokens.len() != count.tokens
        || out.transport.len() != count.bytes
    {
        return Err(global().invariant(Reason::EmissionCounts));
    }
    Ok(())
}
/// Join completed predecessors, count admitted controls, then emit into fixed storage.
pub fn build_source_control_input<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>(
    structure: &'t SourceStructure<'wt, 'e, 'd, 's, 'b, 'r>,
    casing: &'c SourceCasing<'wc, 'e, 'd, 's, 'b, 'r>,
    limits: SourceControlInputLimits,
) -> Result<SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>, SourceControlInputError> {
    if !join::same_whitespace_owner(structure, casing) {
        return Err(global().error(ErrorKind::OwnerMismatch));
    }
    let mut ledger = Ledger {
        used: 0,
        limit: limits.work.get(),
    };
    ledger.charge(1, global())?;
    let sources = structure.source_count();
    let records = structure.record_count();
    let originals = casing.scalar_count();
    admit(sources, limits.sources.get(), Limit::Sources, global())?;
    admit(records, limits.records.get(), Limit::Records, global())?;
    admit(
        originals,
        limits.originals.get(),
        Limit::Originals,
        global(),
    )?;
    join::owner_counts(structure, casing).map_err(|r| global().join(r))?;
    ledger.charge(sources, global())?;
    for (ordinal, source) in casing.sources().enumerate() {
        let c = Context {
            source: Some(source.source_index()),
            ..global()
        };
        join::source_join(ordinal, &source).map_err(|r| c.join(r))?;
        // Block values are no longer decided here: Embed is inert, and Override needs
        // structural facts (anonymous children, forced breaks) that only the walk has.
    }
    let mut frames = Vec::new();
    admit_scratch(bytes::<Frame>(records)?, limits)?;
    reserve_vec(&mut frames, records, Slot::Frames)?;
    let frame_capacity = frames.capacity();
    #[cfg(test)]
    qualification::observe_frames(frame_capacity, false);
    let scratch = bytes::<Frame>(frame_capacity)?;
    admit_scratch(scratch, limits)?;
    let mut inventory = None;
    let counts = if records == 0 {
        Counts::default()
    } else {
        walk(
            structure,
            casing,
            &mut frames,
            &mut ledger,
            limits,
            None,
            &mut inventory,
        )?
    };
    ledger.charge(counts.units, global())?;
    let mut output = reserve_storage(&counts, scratch, limits)?;
    let capacities = output.capacities();
    let replay = if records == 0 {
        Counts::default()
    } else {
        walk(
            structure,
            casing,
            &mut frames,
            &mut ledger,
            limits,
            Some(&mut output),
            &mut inventory,
        )?
    };
    verify_emission(
        &output,
        &counts,
        &replay,
        capacities,
        frame_capacity,
        &frames,
    )?;
    let retained = retained_for(capacities)?;
    let peak = admit_storage(retained, scratch, limits)?;
    let receipt = SourceControlInputReceipt {
        sources,
        records: counts.records,
        payloads: counts.payloads,
        original_scalars: counts.originals,
        literal_scalars: counts.literals,
        synthetic_scalars: counts.synthetic,
        output_scalars: counts.tokens,
        scopes: counts.scopes,
        domains: counts.domains,
        output_bytes: counts.bytes,
        frames_pushed: counts.frames,
        max_depth: counts.depth,
        frame_visits_at_breaks: counts.visits,
        work: ledger.used,
        scope_capacity: capacities[0],
        payload_capacity: capacities[1],
        original_capacity: capacities[2],
        token_capacity: capacities[3],
        transport_capacity: capacities[4],
        frame_capacity,
        scope_element_bytes: size_of::<ScopeRow>(),
        payload_element_bytes: size_of::<PayloadRow>(),
        original_element_bytes: size_of::<OriginalRow>(),
        token_element_bytes: size_of::<TokenRow>(),
        frame_element_bytes: size_of::<Frame>(),
        scratch_bytes: scratch,
        retained_bytes: retained,
        peak_retained_bytes: peak,
    };
    #[cfg(test)]
    qualification::observe_frames(frames.capacity(), true);
    drop(frames);
    Ok(SourceControlInput {
        structure,
        casing,
        transport: output.transport,
        scopes: output.scopes,
        payloads: output.payloads,
        originals: output.originals,
        tokens: output.tokens,
        receipt,
    })
}
#[cfg(test)]
#[path = "source_control_input_tests.rs"]
mod qualification;
