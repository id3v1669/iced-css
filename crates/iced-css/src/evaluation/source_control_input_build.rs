use super::super::{
    source_participants::SourceOuterCategory,
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
    fragments: [usize; 2],
    payloads: [usize; 3],
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
    fn accepts(
        self,
        record: usize,
        source: Option<usize>,
        side: SourceStructureBoundarySide,
        kind: SourceStructureEdgeKind,
    ) -> bool {
        source == Some(self.source)
            && ((record == self.fragments[0]
                && side == SourceStructureBoundarySide::End
                && kind == SourceStructureEdgeKind::BeforeInterruption)
                || (record == self.fragments[1]
                    && side == SourceStructureBoundarySide::Start
                    && kind == SourceStructureEdgeKind::AfterInterruption))
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
    if structure.source_count() != 6 || structure.record_count() != 12 {
        return Err(failure());
    }
    let kinds = [
        Kind::SourceBlock,
        Kind::AnonymousBlock,
        Kind::SourceInline,
        Kind::SourceInline,
        Kind::Payload,
        Kind::SourceBlock,
        Kind::SourceInline,
        Kind::Payload,
        Kind::AnonymousBlock,
        Kind::SourceInline,
        Kind::SourceInline,
        Kind::Payload,
    ];
    let parents = [
        None,
        Some(0),
        Some(1),
        Some(2),
        Some(3),
        Some(0),
        Some(5),
        Some(6),
        Some(0),
        Some(8),
        Some(9),
        Some(10),
    ];
    let mut records = [0; 12];
    let mut sources = [None; 12];
    let mut starts = [None; 12];
    let mut ends = [None; 12];
    for (role, record) in structure.records().enumerate() {
        #[cfg(test)]
        qualification::inventory_record();
        if record.kind() != kinds[role]
            || record.formatting_parent().map(|p| p.index()) != parents[role].map(|p| records[p])
        {
            return Err(failure());
        }
        records[role] = record.index();
        sources[role] = record.source_index();
        match record.kind() {
            Kind::SourceBlock | Kind::SourceInline => {
                let participant = record.participant().ok_or_else(failure)?;
                // Only an inline carries an embedding effect (CSS 2.2 §9.10); a block
                // may compute Embed without effect, and block Override was rejected
                // by the source pass. The affected inline roles are exactly 2 and 9.
                let effective = record.kind() == Kind::SourceInline
                    && participant.bidi_style().unicode_bidi() != ComputedUnicodeBidi::Normal;
                if effective != (role == 2 || role == 9) {
                    return Err(failure());
                }
                starts[role] =
                    Some(anchor(&record, SourceStructureBoundarySide::Start).ok_or_else(failure)?);
                ends[role] =
                    Some(anchor(&record, SourceStructureBoundarySide::End).ok_or_else(failure)?);
            }
            Kind::AnonymousBlock => {
                let origin = record.anonymous_origin().ok_or_else(failure)?;
                if sources[role].is_some()
                    || origin.enclosing_block().index() != records[0]
                    || Some(origin.inheritance_source().source_index()) != sources[0]
                {
                    return Err(failure());
                }
            }
            Kind::Payload => {
                if sources[role] != sources[parents[role].ok_or_else(failure)?] {
                    return Err(failure());
                }
            }
        }
    }
    let distinct = [
        sources[0],
        sources[2],
        sources[3],
        sources[5],
        sources[6],
        sources[10],
    ];
    for (index, source) in distinct.iter().enumerate() {
        if source.is_none() || distinct[..index].contains(source) {
            return Err(failure());
        }
    }
    if sources[9] != sources[2] {
        return Err(failure());
    }
    for role in [0, 2, 3, 5, 6, 9, 10] {
        for (edge, expected, event_kind) in [
            (
                starts[role].ok_or_else(failure)?,
                if role == 9 {
                    SourceStructureEdgeKind::AfterInterruption
                } else {
                    SourceStructureEdgeKind::OriginalBegin
                },
                if role == 9 {
                    SourceStructuralEventKind::End
                } else {
                    SourceStructuralEventKind::Begin
                },
            ),
            (
                ends[role].ok_or_else(failure)?,
                if role == 2 {
                    SourceStructureEdgeKind::BeforeInterruption
                } else {
                    SourceStructureEdgeKind::OriginalEnd
                },
                if role == 2 {
                    SourceStructuralEventKind::Begin
                } else {
                    SourceStructuralEventKind::End
                },
            ),
        ] {
            let source_role = if matches!(
                expected,
                SourceStructureEdgeKind::BeforeInterruption
                    | SourceStructureEdgeKind::AfterInterruption
            ) {
                5
            } else {
                role
            };
            if edge.kind != expected
                || edge.event_kind != event_kind
                || Some(edge.source) != sources[source_role]
            {
                return Err(failure());
            }
        }
    }
    // Edge cuts intentionally differ by side; compare their shared events.
    if ends[2].ok_or_else(failure)?.event != starts[5].ok_or_else(failure)?.event
        || starts[9].ok_or_else(failure)?.event != ends[5].ok_or_else(failure)?.event
    {
        return Err(failure());
    }
    Ok(InterruptionInventory {
        source: sources[2].ok_or_else(failure)?,
        fragments: [records[2], records[9]],
        payloads: [records[4], records[7], records[11]],
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
                let scopes = add(w.counts.scopes, 1, c)?;
                admit(scopes, limits.scopes.get(), Limit::Scopes, c)?;
                join::scope_parent(parent.as_ref().map(|p| p.kind())).map_err(|r| c.join(r))?;
                let participation =
                    join::marker_participation(structure, index).map_err(|r| c.join(r))?;
                w.finalize(index)?;
                w.counts.scopes = scopes;
                if participation == Participation::InlineSequence {
                    w.counts.domains = add(w.counts.domains, 1, c)?;
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
                    if inventory.is_some_and(|i| !i.payloads.contains(&index)) {
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
        let participant = source.participant();
        let bidi = participant.bidi_style().unicode_bidi();
        // Block Embed has no inline embedding effect and is admitted as Normal while
        // keeping its computed value; block Override's descendant/anonymous-block
        // rule is not implemented and still rejects here, before frames and inventory.
        if participant.outer_category() == SourceOuterCategory::Block
            && bidi == ComputedUnicodeBidi::BidiOverride
        {
            return Err(c.error(ErrorKind::Unsupported(Unsupported::BlockBidi(bidi))));
        }
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
