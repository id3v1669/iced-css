use super::*;
use crate::evaluation::source_structural_events::SourceStructuralEventKind as EventKind;
use iced_css_core::{
    formatting::display::Display,
    text::{lf_conversion::LfDecision, line_edges::WhiteSpace, unicode::is_bidi_control},
};
use std::mem::size_of;

#[derive(Debug)]
struct Frame {
    ordinal: EligibleOrdinal,
    state: FrameState,
}
#[derive(Debug, Clone, Copy)]
struct PreviousFrontier {
    source: SourceIndex,
    end: EventIndex,
    indirect: bool,
}
#[derive(Debug)]
enum FrameState {
    CountInline,
    CountBlock {
        enclosing: Option<StackIndex>,
        inline_run_open: bool,
        inline_runs: usize,
        previous_frontier: Option<PreviousFrontier>,
    },
    EmitInline {
        current: Option<RecordIndex>,
    },
    EmitBlock {
        enclosing: Option<StackIndex>,
        record: RecordIndex,
        wrapper: Option<RecordIndex>,
    },
}
fn invariant(event: Option<usize>, reason: SourceStructureInvariantReason) -> SourceStructureError {
    SourceStructureError::Invariant { event, reason }
}
fn state_error(event: usize) -> SourceStructureError {
    invariant(Some(event), SourceStructureInvariantReason::RecordState)
}
fn stack_error(event: usize) -> SourceStructureError {
    invariant(Some(event), SourceStructureInvariantReason::Stack)
}
fn join_error(event: usize) -> SourceStructureError {
    invariant(Some(event), SourceStructureInvariantReason::SourceJoin)
}
fn add(a: usize, b: usize, error: SourceStructureError) -> Result<usize, SourceStructureError> {
    a.checked_add(b).ok_or(error)
}
fn bytes(count: usize, element: usize) -> Result<usize, SourceStructureError> {
    count
        .checked_mul(element)
        .filter(|&n| n <= isize::MAX as usize)
        .ok_or(SourceStructureError::Bytes)
}
fn overlap(prior: usize, payload: usize, limit: usize) -> Result<usize, SourceStructureError> {
    add(prior, payload, SourceStructureError::Bytes).and_then(|n| {
        if n <= limit {
            Ok(n)
        } else {
            Err(SourceStructureError::Bytes)
        }
    })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Facts,
    Frames,
    Output,
}
#[derive(Default)]
struct Observer {
    #[cfg(test)]
    fault: Option<Phase>,
    #[cfg(test)]
    trace: Vec<Observation>,
}
#[cfg(test)]
#[derive(Debug)]
struct Observation {
    phase: &'static str,
    event: Option<usize>,
    requested: usize,
    capacity: usize,
    element: usize,
    overlap: usize,
    work_before: usize,
    work_after: usize,
    records_before: usize,
    records_after: usize,
    split_closes: usize,
    split_reopens: usize,
    counters: Option<Counters>,
}
impl Observer {
    fn phase(&mut self, name: &'static str) {
        #[cfg(test)]
        self.trace.push(Observation {
            phase: name,
            event: None,
            requested: 0,
            capacity: 0,
            element: 0,
            overlap: 0,
            work_before: 0,
            work_after: 0,
            records_before: 0,
            records_after: 0,
            split_closes: 0,
            split_reopens: 0,
            counters: None,
        });
        #[cfg(not(test))]
        let _ = name;
    }
    fn reservation(&mut self, phase: Phase) -> Result<(), SourceStructureError> {
        #[cfg(test)]
        if self.fault == Some(phase) {
            return Err(SourceStructureError::Allocation);
        }
        #[cfg(not(test))]
        let _ = phase;
        Ok(())
    }
}
fn reserve<T>(
    vector: &mut Vec<T>,
    count: usize,
    prior: usize,
    limit: usize,
    phase: Phase,
    observer: &mut Observer,
) -> Result<usize, SourceStructureError> {
    let (requested, acquired) = match phase {
        Phase::Facts => ("facts-requested", "facts-acquired"),
        Phase::Frames => ("frames-requested", "frames-acquired"),
        Phase::Output => ("output-requested", "output-acquired"),
    };
    observer.phase(requested);
    let requested_overlap = overlap(prior, bytes(count, size_of::<T>())?, limit)?;
    #[cfg(test)]
    if let Some(row) = observer.trace.last_mut() {
        row.requested = count;
        row.element = size_of::<T>();
        row.overlap = requested_overlap;
    }
    #[cfg(not(test))]
    let _ = requested_overlap;
    observer.reservation(phase)?;
    vector
        .try_reserve_exact(count)
        .map_err(|_| SourceStructureError::Allocation)?;
    observer.phase(acquired);
    let payload = bytes(vector.capacity(), size_of::<T>())?;
    #[cfg(test)]
    if let Some(row) = observer.trace.last_mut() {
        row.requested = count;
        row.capacity = vector.capacity();
        row.element = size_of::<T>();
        row.overlap = add(prior, payload, SourceStructureError::Bytes)?;
    }
    overlap(prior, payload, limit)
}
fn debit(work: &mut usize, amount: usize, limit: usize) -> Result<(), SourceStructureError> {
    let next = add(*work, amount, SourceStructureError::Work)?;
    if next > limit {
        return Err(SourceStructureError::Work);
    }
    *work = next;
    Ok(())
}
fn increment(value: &mut usize, amount: usize) -> Result<(), SourceStructureError> {
    *value = add(*value, amount, SourceStructureError::Records)?;
    Ok(())
}
fn record_increment(
    receipt: &mut SourceStructureReceipt,
    amount: usize,
    limit: usize,
) -> Result<(), SourceStructureError> {
    increment(&mut receipt.records, amount)?;
    if receipt.records > limit {
        return Err(SourceStructureError::Records);
    }
    Ok(())
}
fn empty_receipt(whitespace: &SourceWhitespace<'_, '_, '_, '_, '_>) -> SourceStructureReceipt {
    SourceStructureReceipt {
        sources: whitespace.source_count(),
        events: whitespace.event_count(),
        scalars: whitespace.scalar_count(),
        contents: whitespace.content_count(),
        blocks: 0,
        inline_sources: 0,
        continuations: 0,
        anonymous_blocks: 0,
        records: 0,
        gap_events: 0,
        gap_scalars: 0,
        work: 0,
        fact_capacity: 0,
        frame_capacity: 0,
        record_capacity: 0,
        frame_element_bytes: size_of::<Frame>(),
        record_element_bytes: size_of::<Record>(),
        retained_bytes: 0,
        peak_retained_bytes: 0,
    }
}
/// Builds the admitted captured-source projection with explicit resource budgets.
///
/// No generated content, anonymous computed styles, paragraphs or geometry are
/// resolved. The borrowed whitespace and its predecessors remain live.
///
/// # Errors
/// Returns the first resource gate or unresolved splitting-gap diagnostic reached
/// by the two-pass schedule. Defensive invariant failures never publish an owner.
pub fn build_source_structure<'w, 'e, 'd, 's, 'b, 'r>(
    whitespace: &'w SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    limits: SourceStructureLimits,
) -> Result<SourceStructure<'w, 'e, 'd, 's, 'b, 'r>, SourceStructureError> {
    build_observed(whitespace, limits, &mut Observer::default())
}
fn build_observed<'w, 'e, 'd, 's, 'b, 'r>(
    whitespace: &'w SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    limits: SourceStructureLimits,
    observer: &mut Observer,
) -> Result<SourceStructure<'w, 'e, 'd, 's, 'b, 'r>, SourceStructureError> {
    let mut receipt = empty_receipt(whitespace);
    let k = receipt.sources;
    if k > limits.sources.get() {
        return Err(SourceStructureError::Sources);
    }
    record_increment(
        &mut receipt,
        add(k, whitespace.content_count(), SourceStructureError::Records)?,
        limits.records.get(),
    )?;
    if k == 0 {
        debit(&mut receipt.work, 1, limits.work.get())?;
        return Ok(SourceStructure {
            whitespace,
            records: Vec::new(),
            receipt,
        });
    }
    debit(
        &mut receipt.work,
        add(
            add(1, k, SourceStructureError::Work)?,
            receipt.events,
            SourceStructureError::Work,
        )?,
        limits.work.get(),
    )?;
    observer.phase("scratch-combined-requested");
    #[cfg(test)]
    if let Some(row) = observer.trace.last_mut() {
        row.work_after = receipt.work;
        row.records_after = receipt.records;
    }
    overlap(
        bytes(k, size_of::<u8>())?,
        bytes(k, size_of::<Frame>())?,
        limits.retained_bytes.get(),
    )?;
    let mut facts = Vec::new();
    let fact_bytes = reserve(
        &mut facts,
        k,
        0,
        limits.retained_bytes.get(),
        Phase::Facts,
        observer,
    )?;
    let mut stack = Vec::new();
    let scratch = reserve(
        &mut stack,
        k,
        fact_bytes,
        limits.retained_bytes.get(),
        Phase::Frames,
        observer,
    )?;
    facts.resize(k, 0u8);
    count(
        whitespace,
        limits,
        &mut receipt,
        &mut facts,
        &mut stack,
        observer,
    )?;
    let emit_work = add(
        add(
            add(1, receipt.events, SourceStructureError::Work)?,
            receipt
                .continuations
                .checked_mul(2)
                .ok_or(SourceStructureError::Work)?,
            SourceStructureError::Work,
        )?,
        receipt.records,
        SourceStructureError::Work,
    )?;
    observer.phase("emit-debit");
    #[cfg(test)]
    let before = receipt.work;
    debit(&mut receipt.work, emit_work, limits.work.get())?;
    #[cfg(test)]
    if let Some(row) = observer.trace.last_mut() {
        row.work_before = before;
        row.work_after = receipt.work;
    }
    let mut records = Vec::new();
    receipt.peak_retained_bytes = reserve(
        &mut records,
        receipt.records,
        scratch,
        limits.retained_bytes.get(),
        Phase::Output,
        observer,
    )?;
    receipt.fact_capacity = facts.capacity();
    receipt.frame_capacity = stack.capacity();
    receipt.record_capacity = records.capacity();
    receipt.retained_bytes = bytes(records.capacity(), size_of::<Record>())?;
    let mut builder = Builder {
        whitespace,
        records,
        pending_ends: 0,
        counters: Counters::default(),
    };
    builder.emit(&facts, &mut stack, observer)?;
    builder.complete(&receipt, &stack)?;
    observer.phase("complete");
    #[cfg(test)]
    if let Some(row) = observer.trace.last_mut() {
        row.split_closes = builder.counters.split_closes;
        row.split_reopens = builder.counters.split_reopens;
        row.counters = Some(builder.counters);
    }
    drop(facts);
    drop(stack);
    Ok(SourceStructure {
        whitespace,
        records: builder.records,
        receipt,
    })
}
fn ordinal(
    whitespace: &SourceWhitespace<'_, '_, '_, '_, '_>,
    source: SourceIndex,
    event: usize,
) -> Result<EligibleOrdinal, SourceStructureError> {
    let ordinal = whitespace
        .eligibility()
        .ordinal(source.0)
        .ok_or_else(|| join_error(event))?;
    if whitespace
        .eligibility()
        .entry(ordinal)
        .is_none_or(|entry| entry.source != source.0)
    {
        return Err(join_error(event));
    }
    Ok(EligibleOrdinal(ordinal))
}
fn source(
    whitespace: &SourceWhitespace<'_, '_, '_, '_, '_>,
    ordinal: EligibleOrdinal,
    event: usize,
) -> Result<SourceIndex, SourceStructureError> {
    Ok(SourceIndex(
        whitespace
            .eligibility()
            .entry(ordinal.0)
            .ok_or_else(|| join_error(event))?
            .source,
    ))
}
fn adjacent(
    whitespace: &SourceWhitespace<'_, '_, '_, '_, '_>,
    a: usize,
    b: usize,
) -> Result<bool, SourceStructureError> {
    let Some(left) = whitespace.event(a) else {
        return Ok(false);
    };
    let Some(right) = whitespace.event(b) else {
        return Ok(false);
    };
    if left.kind() != EventKind::End
        || right.kind() != EventKind::Begin
        || left.participant().resolution().display != Display::Block
        || right.participant().resolution().display != Display::Block
    {
        return Ok(false);
    }
    let lo = ordinal(
        whitespace,
        SourceIndex(left.participant().source_index()),
        a,
    )?;
    let ro = ordinal(
        whitespace,
        SourceIndex(right.participant().source_index()),
        b,
    )?;
    Ok(whitespace
        .eligibility()
        .entry(lo.0)
        .ok_or_else(|| join_error(a))?
        .parent
        == whitespace
            .eligibility()
            .entry(ro.0)
            .ok_or_else(|| join_error(b))?
            .parent)
}
fn begin_join(
    whitespace: &SourceWhitespace<'_, '_, '_, '_, '_>,
    ordinal: EligibleOrdinal,
    stack: &[Frame],
    event: usize,
) -> Result<(), SourceStructureError> {
    let parent = whitespace
        .eligibility()
        .entry(ordinal.0)
        .ok_or_else(|| join_error(event))?
        .parent;
    if parent != stack.last().map(|frame| frame.ordinal.0) {
        return Err(join_error(event));
    }
    Ok(())
}
fn run(frame: &mut Frame, event: usize) -> Result<(), SourceStructureError> {
    let FrameState::CountBlock {
        inline_run_open,
        inline_runs,
        ..
    } = &mut frame.state
    else {
        return Err(stack_error(event));
    };
    if !*inline_run_open {
        increment(inline_runs, 1)?;
        *inline_run_open = true;
    }
    Ok(())
}
fn count(
    whitespace: &SourceWhitespace<'_, '_, '_, '_, '_>,
    limits: SourceStructureLimits,
    receipt: &mut SourceStructureReceipt,
    facts: &mut [u8],
    stack: &mut Vec<Frame>,
    observer: &mut Observer,
) -> Result<(), SourceStructureError> {
    let mut nearest: Option<StackIndex> = None;
    for event in whitespace.events() {
        let e = event.index();
        let src = SourceIndex(event.participant().source_index());
        let ord = ordinal(whitespace, src, e)?;
        match event.kind() {
            EventKind::Begin => {
                begin_join(whitespace, ord, stack, e)?;
                let state = if event.participant().resolution().display == Display::Block {
                    increment(&mut receipt.blocks, 1)?;
                    if let Some(q) = nearest {
                        let frame = stack.get(q.0).ok_or_else(|| stack_error(e))?;
                        let FrameState::CountBlock {
                            previous_frontier, ..
                        } = frame.state
                        else {
                            return Err(stack_error(e));
                        };
                        let depth = stack
                            .len()
                            .checked_sub(q.0 + 1)
                            .ok_or_else(|| stack_error(e))?;
                        if let Some(previous) = previous_frontier
                            && (previous.indirect || depth > 0)
                        {
                            gap(
                                whitespace,
                                previous,
                                src,
                                EventIndex(e),
                                limits,
                                receipt,
                                observer,
                            )?;
                        }
                        let frame = stack.get_mut(q.0).ok_or_else(|| stack_error(e))?;
                        *facts
                            .get_mut(frame.ordinal.0)
                            .ok_or_else(|| join_error(e))? = 1;
                        let FrameState::CountBlock {
                            inline_run_open, ..
                        } = &mut frame.state
                        else {
                            return Err(stack_error(e));
                        };
                        *inline_run_open = false;
                    }
                    let enclosing = nearest;
                    nearest = Some(StackIndex(stack.len()));
                    FrameState::CountBlock {
                        enclosing,
                        inline_run_open: false,
                        inline_runs: 0,
                        previous_frontier: None,
                    }
                } else {
                    increment(&mut receipt.inline_sources, 1)?;
                    run(
                        stack
                            .get_mut(nearest.ok_or_else(|| stack_error(e))?.0)
                            .ok_or_else(|| stack_error(e))?,
                        e,
                    )?;
                    FrameState::CountInline
                };
                stack.push(Frame {
                    ordinal: ord,
                    state,
                });
            }
            EventKind::Content => {
                if stack.last().is_none_or(|frame| {
                    frame.ordinal != ord || !matches!(frame.state, FrameState::CountInline)
                }) {
                    return Err(stack_error(e));
                }
            }
            EventKind::End => {
                let frame = stack.pop().ok_or_else(|| stack_error(e))?;
                if frame.ordinal != ord {
                    return Err(join_error(e));
                }
                match frame.state {
                    FrameState::CountInline => {}
                    FrameState::CountBlock {
                        enclosing,
                        inline_runs,
                        ..
                    } => {
                        if nearest != Some(StackIndex(stack.len())) {
                            return Err(stack_error(e));
                        }
                        if *facts.get(ord.0).ok_or_else(|| join_error(e))? != 0 {
                            increment(&mut receipt.anonymous_blocks, inline_runs)?;
                            record_increment(receipt, inline_runs, limits.records.get())?;
                        }
                        nearest = enclosing;
                        if let Some(q) = nearest {
                            let depth = stack
                                .len()
                                .checked_sub(q.0 + 1)
                                .ok_or_else(|| stack_error(e))?;
                            let parent = stack.get_mut(q.0).ok_or_else(|| stack_error(e))?;
                            let FrameState::CountBlock {
                                previous_frontier, ..
                            } = &mut parent.state
                            else {
                                return Err(stack_error(e));
                            };
                            *previous_frontier = Some(PreviousFrontier {
                                source: src,
                                end: EventIndex(e),
                                indirect: depth > 0,
                            });
                            if depth > 0
                                && !adjacent(
                                    whitespace,
                                    e,
                                    e.checked_add(1).ok_or(SourceStructureError::Work)?,
                                )?
                            {
                                observer.phase("continuation-increment");
                                #[cfg(test)]
                                let before = receipt.records;
                                increment(&mut receipt.continuations, depth)?;
                                record_increment(receipt, depth, limits.records.get())?;
                                #[cfg(test)]
                                if let Some(row) = observer.trace.last_mut() {
                                    row.event = Some(e);
                                    row.records_before = before;
                                    row.records_after = receipt.records;
                                }
                                run(parent, e)?;
                            }
                        }
                    }
                    _ => return Err(stack_error(e)),
                }
            }
        }
    }
    if !stack.is_empty() || nearest.is_some() {
        return Err(invariant(None, SourceStructureInvariantReason::Completion));
    }
    Ok(())
}
fn gap(
    whitespace: &SourceWhitespace<'_, '_, '_, '_, '_>,
    left: PreviousFrontier,
    right: SourceIndex,
    end: EventIndex,
    limits: SourceStructureLimits,
    receipt: &mut SourceStructureReceipt,
    observer: &mut Observer,
) -> Result<(), SourceStructureError> {
    let interval = whitespace
        .between_events(left.end.0, end.0)
        .ok_or_else(|| join_error(end.0))?;
    let events = interval.event_range();
    let scalars = interval.scalar_range();
    let amount = add(events.len(), scalars.len(), SourceStructureError::Work)?;
    observer.phase("gap-debit-requested");
    #[cfg(test)]
    let before = receipt.work;
    debit(&mut receipt.work, amount, limits.work.get())?;
    observer.phase("gap-debit");
    #[cfg(test)]
    if let Some(row) = observer.trace.last_mut() {
        row.event = Some(end.0);
        row.requested = events.len();
        row.capacity = scalars.len();
        row.work_before = before;
        row.work_after = receipt.work;
    }
    receipt.gap_events = add(receipt.gap_events, events.len(), SourceStructureError::Work)?;
    receipt.gap_scalars = add(
        receipt.gap_scalars,
        scalars.len(),
        SourceStructureError::Work,
    )?;
    let mut first_inline = None;
    for event in interval.events() {
        if event.participant().resolution().display == Display::Block {
            return Err(join_error(event.index()));
        }
        if event.kind() != EventKind::Content && first_inline.is_none() {
            first_inline = Some((event.participant().source_index(), event.index()));
        }
    }
    let mut control = None;
    let mut collapsible = true;
    for scalar in interval.scalars() {
        if is_bidi_control(scalar.normalized()) && control.is_none() {
            control = Some((scalar.participant().source_index(), scalar.index()));
        }
        let mode = scalar.white_space();
        let shape = match scalar.normalized() {
            ' ' | '\t' => matches!(
                mode,
                WhiteSpace::Normal | WhiteSpace::Nowrap | WhiteSpace::PreLine
            ),
            '\n' => {
                matches!(mode, WhiteSpace::Normal | WhiteSpace::Nowrap)
                    && matches!(
                        scalar.lf_decision(),
                        LfDecision::Space | LfDecision::RemoveLf
                    )
            }
            _ => false,
        };
        collapsible &= shape && matches!(scalar.rendered_scalar(), None | Some(' '));
    }
    let witness = if let Some((source, scalar)) = control {
        Some(SourceStructureGapWitness::UnresolvedLiteralBidiGap { source, scalar })
    } else if !events.is_empty() && scalars.is_empty() {
        let (source, event) = first_inline.ok_or_else(|| join_error(end.0))?;
        Some(SourceStructureGapWitness::UnresolvedEmptyInlineGap { source, event })
    } else if !scalars.is_empty() && collapsible {
        let (source, event) = first_inline.ok_or_else(|| join_error(end.0))?;
        Some(SourceStructureGapWitness::UnresolvedCollapsibleElementGap { source, event })
    } else {
        None
    };
    if let Some(witness) = witness {
        return Err(SourceStructureError::Gap {
            left: SourceStructureGapEndpoint {
                source: left.source.0,
                event: left.end.0,
            },
            right: SourceStructureGapEndpoint {
                source: right.0,
                event: end.0,
            },
            events,
            scalars,
            witness,
        });
    }
    Ok(())
}
#[derive(Default, Debug, Clone, Copy)]
struct Counters {
    events: usize,
    begins: usize,
    contents: usize,
    blocks: usize,
    block_closes: usize,
    inlines: usize,
    inline_closes: usize,
    payloads: usize,
    anonymous: usize,
    split_closes: usize,
    split_reopens: usize,
}
struct Builder<'w, 'e, 'd, 's, 'b, 'r> {
    whitespace: &'w SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    records: Vec<Record>,
    pending_ends: usize,
    counters: Counters,
}
impl Builder<'_, '_, '_, '_, '_, '_> {
    fn append(
        &mut self,
        parent: Option<RecordIndex>,
        kind: RecordKind,
        event: usize,
    ) -> Result<RecordIndex, SourceStructureError> {
        if self.records.len() == self.records.capacity() {
            return Err(state_error(event));
        }
        if let Some(parent) = parent {
            let parent = self
                .records
                .get(parent.0)
                .ok_or_else(|| state_error(event))?;
            let valid = matches!(
                (&parent.kind, &kind),
                (
                    RecordKind::SourceBlock { end: None, .. },
                    RecordKind::SourceBlock { .. }
                        | RecordKind::SourceInline { .. }
                        | RecordKind::AnonymousBlock
                ) | (
                    RecordKind::SourceInline { end: None, .. },
                    RecordKind::SourceInline { .. } | RecordKind::Payload { .. }
                ) | (RecordKind::AnonymousBlock, RecordKind::SourceInline { .. })
            );
            if !valid {
                return Err(state_error(event));
            }
        } else if !self.records.is_empty() || !matches!(kind, RecordKind::SourceBlock { .. }) {
            return Err(state_error(event));
        }
        match kind {
            RecordKind::SourceBlock {
                begin, end: None, ..
            } => {
                self.check_start(begin, event)?;
                increment(&mut self.pending_ends, 1)?;
                increment(&mut self.counters.blocks, 1)?;
            }
            RecordKind::SourceInline {
                start, end: None, ..
            } => {
                let (InlineStart::OriginalBegin(anchor) | InlineStart::AfterInterruption(anchor)) =
                    start;
                self.check_start(anchor, event)?;
                increment(&mut self.pending_ends, 1)?;
                increment(&mut self.counters.inlines, 1)?;
            }
            RecordKind::AnonymousBlock => increment(&mut self.counters.anonymous, 1)?,
            RecordKind::Payload { content } => {
                let input = self
                    .whitespace
                    .event(content.0)
                    .ok_or_else(|| join_error(event))?;
                if input.kind() != EventKind::Content {
                    return Err(join_error(event));
                }
                let Some(Record {
                    kind: RecordKind::SourceInline { source, .. },
                    ..
                }) = parent.and_then(|p| self.records.get(p.0))
                else {
                    return Err(state_error(event));
                };
                if source.0 != input.participant().source_index() {
                    return Err(join_error(event));
                }
                increment(&mut self.counters.payloads, 1)?;
            }
            _ => return Err(state_error(event)),
        }
        let index = RecordIndex(self.records.len());
        self.records.push(Record { parent, kind });
        Ok(index)
    }
    fn check_start(&self, anchor: EventIndex, event: usize) -> Result<(), SourceStructureError> {
        let cut = anchor.0.checked_add(1).ok_or(SourceStructureError::Work)?;
        if cut > self.whitespace.event_count() {
            return Err(join_error(event));
        }
        Ok(())
    }
    fn close_block(
        &mut self,
        index: RecordIndex,
        source: SourceIndex,
        event: EventIndex,
    ) -> Result<(), SourceStructureError> {
        let anchor = self
            .whitespace
            .event(event.0)
            .ok_or_else(|| join_error(event.0))?;
        if anchor.kind() != EventKind::End
            || anchor.participant().source_index() != source.0
            || anchor.participant().resolution().display != Display::Block
        {
            return Err(join_error(event.0));
        }
        let RecordKind::SourceBlock {
            source: stored,
            end,
            ..
        } = &mut self
            .records
            .get_mut(index.0)
            .ok_or_else(|| state_error(event.0))?
            .kind
        else {
            return Err(state_error(event.0));
        };
        if *stored != source || end.is_some() {
            return Err(state_error(event.0));
        }
        let pending = self
            .pending_ends
            .checked_sub(1)
            .ok_or_else(|| state_error(event.0))?;
        *end = Some(event);
        self.pending_ends = pending;
        increment(&mut self.counters.block_closes, 1)
    }
    fn close_inline(
        &mut self,
        index: RecordIndex,
        source: SourceIndex,
        end_value: InlineEnd,
    ) -> Result<(), SourceStructureError> {
        let (InlineEnd::OriginalEnd(event) | InlineEnd::BeforeInterruption(event)) = end_value;
        let anchor = self
            .whitespace
            .event(event.0)
            .ok_or_else(|| join_error(event.0))?;
        match end_value {
            InlineEnd::OriginalEnd(_)
                if anchor.kind() != EventKind::End
                    || anchor.participant().source_index() != source.0 =>
            {
                return Err(join_error(event.0));
            }
            InlineEnd::BeforeInterruption(_)
                if anchor.kind() != EventKind::Begin
                    || anchor.participant().resolution().display != Display::Block =>
            {
                return Err(join_error(event.0));
            }
            _ => {}
        }
        let RecordKind::SourceInline {
            source: stored,
            end,
            ..
        } = &mut self
            .records
            .get_mut(index.0)
            .ok_or_else(|| state_error(event.0))?
            .kind
        else {
            return Err(state_error(event.0));
        };
        if *stored != source || end.is_some() {
            return Err(state_error(event.0));
        }
        let pending = self
            .pending_ends
            .checked_sub(1)
            .ok_or_else(|| state_error(event.0))?;
        *end = Some(end_value);
        self.pending_ends = pending;
        increment(&mut self.counters.inline_closes, 1)
    }
    fn run_parent(
        &mut self,
        q: StackIndex,
        facts: &[u8],
        stack: &mut [Frame],
        event: usize,
    ) -> Result<RecordIndex, SourceStructureError> {
        let frame = stack.get_mut(q.0).ok_or_else(|| stack_error(event))?;
        let FrameState::EmitBlock {
            record, wrapper, ..
        } = &mut frame.state
        else {
            return Err(stack_error(event));
        };
        if *facts
            .get(frame.ordinal.0)
            .ok_or_else(|| join_error(event))?
            == 0
        {
            return Ok(*record);
        }
        if let Some(wrapper) = wrapper {
            return Ok(*wrapper);
        }
        let index = self.append(Some(*record), RecordKind::AnonymousBlock, event)?;
        *wrapper = Some(index);
        Ok(index)
    }
    fn inline_current(frame: &Frame, event: usize) -> Result<RecordIndex, SourceStructureError> {
        match frame.state {
            FrameState::EmitInline {
                current: Some(index),
            } => Ok(index),
            _ => Err(state_error(event)),
        }
    }
    fn emit(
        &mut self,
        facts: &[u8],
        stack: &mut Vec<Frame>,
        observer: &mut Observer,
    ) -> Result<(), SourceStructureError> {
        let mut nearest: Option<StackIndex> = None;
        for event in self.whitespace.events() {
            let e = event.index();
            let src = SourceIndex(event.participant().source_index());
            let ord = ordinal(self.whitespace, src, e)?;
            increment(&mut self.counters.events, 1)?;
            match event.kind() {
                EventKind::Begin => {
                    begin_join(self.whitespace, ord, stack, e)?;
                    increment(&mut self.counters.begins, 1)?;
                    let state = if event.participant().resolution().display == Display::Block {
                        let parent = if let Some(q) = nearest {
                            let continuing = if let Some(previous) = e.checked_sub(1) {
                                adjacent(self.whitespace, previous, e)?
                            } else {
                                false
                            };
                            if !continuing {
                                for frame in
                                    stack.get_mut(q.0 + 1..).ok_or_else(|| stack_error(e))?
                                {
                                    let current = Self::inline_current(frame, e)?;
                                    let source = source(self.whitespace, frame.ordinal, e)?;
                                    self.close_inline(
                                        current,
                                        source,
                                        InlineEnd::BeforeInterruption(EventIndex(e)),
                                    )?;
                                    frame.state = FrameState::EmitInline { current: None };
                                    increment(&mut self.counters.split_closes, 1)?;
                                }
                            }
                            let FrameState::EmitBlock {
                                record, wrapper, ..
                            } = &mut stack.get_mut(q.0).ok_or_else(|| stack_error(e))?.state
                            else {
                                return Err(stack_error(e));
                            };
                            *wrapper = None;
                            Some(*record)
                        } else {
                            None
                        };
                        let record = self.append(
                            parent,
                            RecordKind::SourceBlock {
                                source: src,
                                begin: EventIndex(e),
                                end: None,
                            },
                            e,
                        )?;
                        let enclosing = nearest;
                        nearest = Some(StackIndex(stack.len()));
                        FrameState::EmitBlock {
                            enclosing,
                            record,
                            wrapper: None,
                        }
                    } else {
                        let q = nearest.ok_or_else(|| stack_error(e))?;
                        let parent = if stack.len() > q.0 + 1 {
                            Self::inline_current(stack.last().ok_or_else(|| stack_error(e))?, e)?
                        } else {
                            self.run_parent(q, facts, stack, e)?
                        };
                        let current = self.append(
                            Some(parent),
                            RecordKind::SourceInline {
                                source: src,
                                start: InlineStart::OriginalBegin(EventIndex(e)),
                                end: None,
                            },
                            e,
                        )?;
                        FrameState::EmitInline {
                            current: Some(current),
                        }
                    };
                    stack.push(Frame {
                        ordinal: ord,
                        state,
                    });
                }
                EventKind::Content => {
                    let frame = stack.last().ok_or_else(|| stack_error(e))?;
                    if frame.ordinal != ord {
                        return Err(join_error(e));
                    }
                    let parent = Self::inline_current(frame, e)?;
                    self.append(
                        Some(parent),
                        RecordKind::Payload {
                            content: EventIndex(e),
                        },
                        e,
                    )?;
                    increment(&mut self.counters.contents, 1)?;
                }
                EventKind::End => {
                    let frame = stack.pop().ok_or_else(|| stack_error(e))?;
                    if frame.ordinal != ord {
                        return Err(join_error(e));
                    }
                    match frame.state {
                        FrameState::EmitInline { current } => self.close_inline(
                            current.ok_or_else(|| state_error(e))?,
                            src,
                            InlineEnd::OriginalEnd(EventIndex(e)),
                        )?,
                        FrameState::EmitBlock {
                            enclosing, record, ..
                        } => {
                            if nearest != Some(StackIndex(stack.len())) {
                                return Err(stack_error(e));
                            }
                            self.close_block(record, src, EventIndex(e))?;
                            nearest = enclosing;
                            if let Some(q) = nearest
                                && stack.len() > q.0 + 1
                                && !adjacent(
                                    self.whitespace,
                                    e,
                                    e.checked_add(1).ok_or(SourceStructureError::Work)?,
                                )?
                            {
                                let mut parent = self.run_parent(q, facts, stack, e)?;
                                for frame in
                                    stack.get_mut(q.0 + 1..).ok_or_else(|| stack_error(e))?
                                {
                                    if !matches!(
                                        frame.state,
                                        FrameState::EmitInline { current: None }
                                    ) {
                                        return Err(state_error(e));
                                    }
                                    let source = source(self.whitespace, frame.ordinal, e)?;
                                    let current = self.append(
                                        Some(parent),
                                        RecordKind::SourceInline {
                                            source,
                                            start: InlineStart::AfterInterruption(EventIndex(e)),
                                            end: None,
                                        },
                                        e,
                                    )?;
                                    frame.state = FrameState::EmitInline {
                                        current: Some(current),
                                    };
                                    parent = current;
                                    increment(&mut self.counters.split_reopens, 1)?;
                                }
                            }
                        }
                        _ => return Err(stack_error(e)),
                    }
                }
            }
        }
        if nearest.is_some() {
            return Err(invariant(None, SourceStructureInvariantReason::Completion));
        }
        let _ = observer;
        Ok(())
    }
    fn complete(
        &self,
        r: &SourceStructureReceipt,
        stack: &[Frame],
    ) -> Result<(), SourceStructureError> {
        let c = &self.counters;
        let inlines = add(
            r.inline_sources,
            r.continuations,
            SourceStructureError::Records,
        )?;
        if c.events != r.events
            || c.begins != r.sources
            || c.contents != r.contents
            || c.blocks != r.blocks
            || c.block_closes != r.blocks
            || c.inlines != inlines
            || c.inline_closes != inlines
            || c.payloads != r.contents
            || c.anonymous != r.anonymous_blocks
            || c.split_closes != r.continuations
            || c.split_reopens != r.continuations
            || self.records.len() != r.records
            || self.pending_ends != 0
            || !stack.is_empty()
            || self.records.first().is_none_or(|record| {
                record.parent.is_some() || !matches!(record.kind, RecordKind::SourceBlock { .. })
            })
        {
            return Err(invariant(None, SourceStructureInvariantReason::Completion));
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "source_structure_tests.rs"]
mod tests;
