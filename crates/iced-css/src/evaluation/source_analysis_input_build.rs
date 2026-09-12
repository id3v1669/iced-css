use super::super::source_structural_events::SourceStructuralEventKind;
use super::*;
use SourceAnalysisInputError as Error;
use SourceAnalysisInputInvariantReason as Reason;
use SourceAnalysisParticipation as Participation;
use SourceStructureKind as Kind;
use std::mem::size_of;

fn invariant(record: Option<usize>, reason: Reason) -> Error {
    Error::Invariant { record, reason }
}
fn checked_limit(value: usize, extra: usize, limit: usize, error: Error) -> Result<usize, Error> {
    value
        .checked_add(extra)
        .filter(|&n| n <= limit)
        .ok_or(error)
}
struct Ledger {
    used: usize,
    allowance: usize,
}
impl Ledger {
    fn charge(&mut self, amount: usize) -> Result<(), Error> {
        self.used = checked_limit(self.used, amount, self.allowance, Error::Work)?;
        #[cfg(test)]
        trace_work(self.used);
        Ok(())
    }
    fn work(&self) -> usize {
        self.used
    }
}
fn count_presentation_members<F>(
    ledger: &mut Ledger,
    _original: usize,
    presentation: SourceCasingPresentation,
    mut visitor: F,
) -> Result<usize, Error>
where
    F: FnMut(usize, char) -> Result<(), Error>,
{
    let characters = presentation.characters();
    let members = characters.len();
    #[cfg(test)]
    trace_phase("members-prepaid", members);
    ledger.charge(members)?;
    for character in characters {
        visitor(members, character)?;
    }
    Ok(members)
}
fn bytes_for_capacity<T>(capacity: usize) -> Result<usize, Error> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or(Error::Bytes)
}
fn admit_retained_bytes(
    scope_bytes: usize,
    scalar_bytes: usize,
    transport_bytes: usize,
    allowance: usize,
) -> Result<usize, Error> {
    scope_bytes
        .checked_add(scalar_bytes)
        .and_then(|n| n.checked_add(transport_bytes))
        .filter(|&n| n <= allowance)
        .ok_or(Error::Bytes)
}
fn reserve_vec_with<T, F>(values: &mut Vec<T>, requested: usize, reserve: F) -> Result<(), Error>
where
    F: FnOnce(&mut Vec<T>, usize) -> Result<(), ()>,
{
    reserve(values, requested).map_err(|()| Error::Allocation)
}
fn reserve_string_with<F>(value: &mut String, requested: usize, reserve: F) -> Result<(), Error>
where
    F: FnOnce(&mut String, usize) -> Result<(), ()>,
{
    reserve(value, requested).map_err(|()| Error::Allocation)
}

struct CompletedGeneration {
    scopes: Vec<ScopeRecord>,
    scalars: Vec<ScalarRecord>,
    transport: String,
}
/// Reserve the actual final containers; each acquired capacity is admitted while
/// all earlier allocations remain live and before acquiring the next container.
fn reserve_three(
    q: usize,
    s: usize,
    t: usize,
    allowance: usize,
) -> Result<CompletedGeneration, Error> {
    let scope_floor = bytes_for_capacity::<ScopeRecord>(q)?;
    let scalar_floor = bytes_for_capacity::<ScalarRecord>(s)?;
    let text_floor = bytes_for_capacity::<u8>(t)?;
    admit_retained_bytes(scope_floor, scalar_floor, text_floor, allowance)?;
    let mut scopes = Vec::new();
    let mut scalars = Vec::new();
    let mut transport = String::new();
    if q != 0 {
        #[cfg(test)]
        trace_reservation(0);
        reserve_vec_with(&mut scopes, q, |v, n| {
            v.try_reserve_exact(n).map_err(|_| ())
        })?;
    }
    let scope_bytes = bytes_for_capacity::<ScopeRecord>(scopes.capacity())?;
    admit_retained_bytes(scope_bytes, scalar_floor, text_floor, allowance)?;
    if s != 0 {
        #[cfg(test)]
        trace_reservation(1);
        reserve_vec_with(&mut scalars, s, |v, n| {
            v.try_reserve_exact(n).map_err(|_| ())
        })?;
    }
    let scalar_bytes = bytes_for_capacity::<ScalarRecord>(scalars.capacity())?;
    admit_retained_bytes(scope_bytes, scalar_bytes, text_floor, allowance)?;
    if t != 0 {
        #[cfg(test)]
        trace_reservation(2);
        reserve_string_with(&mut transport, t, |v, n| {
            v.try_reserve_exact(n).map_err(|_| ())
        })?;
    }
    admit_retained_bytes(
        scope_bytes,
        scalar_bytes,
        bytes_for_capacity::<u8>(transport.capacity())?,
        allowance,
    )?;
    Ok(CompletedGeneration {
        scopes,
        scalars,
        transport,
    })
}
struct ScopeGeneration {
    output: CompletedGeneration,
    scopes: usize,
    originals: usize,
    characters: usize,
    bytes: usize,
    capacities: (usize, usize, usize),
    original_cursor: usize,
    character_cursor: usize,
    pending_scope: Option<usize>,
    finalized_scopes: usize,
}
impl ScopeGeneration {
    fn new(output: CompletedGeneration, q: usize, s: usize, u: usize, t: usize) -> Self {
        let capacities = (
            output.scopes.capacity(),
            output.scalars.capacity(),
            output.transport.capacity(),
        );
        Self {
            output,
            scopes: q,
            originals: s,
            characters: u,
            bytes: t,
            capacities,
            original_cursor: 0,
            character_cursor: 0,
            pending_scope: None,
            finalized_scopes: 0,
        }
    }
    /// Only append creates a pending row; a predecessor must already be finalized.
    fn append_scope(
        &mut self,
        record_index: usize,
        participation: Participation,
        original_end: usize,
        byte_end: usize,
        analysis_scalar_end: usize,
    ) -> Result<(), Reason> {
        if self.pending_scope.is_some() || self.finalized_scopes != self.output.scopes.len() {
            return Err(Reason::ScopeFinalization);
        }
        if self.output.scopes.len() >= self.scopes
            || self.output.scopes.len() >= self.capacities.0
            || self
                .output
                .scopes
                .last()
                .is_some_and(|r| r.record_index >= record_index)
            || (original_end, byte_end, analysis_scalar_end)
                != (
                    self.original_cursor,
                    self.output.transport.len(),
                    self.character_cursor,
                )
        {
            return Err(Reason::EmissionCounts);
        }
        let index = self.output.scopes.len();
        self.output.scopes.push(ScopeRecord {
            record_index,
            participation,
            original_end,
            byte_end,
            analysis_scalar_end,
        });
        self.pending_scope = Some(index);
        Ok(())
    }
    /// The sole production transition that writes completed ends and clears pending.
    fn finalize_scope(
        &mut self,
        index: usize,
        original_end: usize,
        byte_end: usize,
        analysis_scalar_end: usize,
    ) -> Result<(), Reason> {
        if self.pending_scope != Some(index)
            || self.output.scopes.len().checked_sub(1) != Some(index)
            || self.finalized_scopes != index
        {
            return Err(Reason::ScopeFinalization);
        }
        let row = &mut self.output.scopes[index];
        if original_end < row.original_end
            || byte_end < row.byte_end
            || analysis_scalar_end < row.analysis_scalar_end
            || original_end > self.originals
            || byte_end > self.bytes
            || analysis_scalar_end > self.characters
            || (original_end, byte_end, analysis_scalar_end)
                != (
                    self.original_cursor,
                    self.output.transport.len(),
                    self.character_cursor,
                )
        {
            return Err(Reason::ScopeFinalization);
        }
        row.original_end = original_end;
        row.byte_end = byte_end;
        row.analysis_scalar_end = analysis_scalar_end;
        self.finalized_scopes += 1;
        self.pending_scope = None;
        Ok(())
    }
    fn append_character(&mut self, character: char) -> Result<(), Reason> {
        let next_bytes = self
            .output
            .transport
            .len()
            .checked_add(character.len_utf8())
            .ok_or(Reason::EmissionCounts)?;
        if self.pending_scope.is_none()
            || self.character_cursor >= self.characters
            || next_bytes > self.bytes
            || next_bytes > self.capacities.2
        {
            return Err(Reason::EmissionCounts);
        }
        self.output.transport.push(character);
        self.character_cursor += 1;
        Ok(())
    }
    fn append_original(
        &mut self,
        payload_record_index: usize,
        original_index: usize,
    ) -> Result<(), Reason> {
        if self.pending_scope.is_none()
            || original_index != self.original_cursor
            || self.original_cursor >= self.originals
            || self.output.scalars.len() >= self.capacities.1
        {
            return Err(Reason::EmissionCounts);
        }
        self.output.scalars.push(ScalarRecord {
            payload_record_index,
            byte_end: self.output.transport.len(),
            analysis_scalar_end: self.character_cursor,
        });
        self.original_cursor += 1;
        Ok(())
    }
    fn validate_completion(&self) -> Result<(), Reason> {
        if self.pending_scope.is_some()
            || self.finalized_scopes != self.scopes
            || self.finalized_scopes != self.output.scopes.len()
        {
            return Err(Reason::ScopeFinalization);
        }
        if self.output.scopes.len() != self.scopes
            || self.output.scalars.len() != self.originals
            || self.original_cursor != self.originals
            || self.character_cursor != self.characters
            || self.output.transport.len() != self.bytes
            || (
                self.output.scopes.capacity(),
                self.output.scalars.capacity(),
                self.output.transport.capacity(),
            ) != self.capacities
            || self.output.scopes.last().map_or((0, 0, 0), |r| {
                (r.original_end, r.byte_end, r.analysis_scalar_end)
            }) != (self.originals, self.bytes, self.characters)
            || self
                .output
                .scalars
                .last()
                .map_or((0, 0), |r| (r.byte_end, r.analysis_scalar_end))
                != (self.bytes, self.characters)
        {
            return Err(Reason::Completion);
        }
        Ok(())
    }
    fn finish(self) -> Result<CompletedGeneration, Reason> {
        self.validate_completion()?;
        Ok(self.output)
    }
}
#[derive(Clone, Copy)]
struct Counts {
    sources: usize,
    records: usize,
    payloads: usize,
    originals: usize,
    scopes: usize,
    domains: usize,
    characters: usize,
    bytes: usize,
}
fn marker_participation(
    structure: &SourceStructure<'_, '_, '_, '_, '_, '_>,
    index: usize,
) -> Result<Participation, Error> {
    let marker = structure
        .record(index)
        .ok_or_else(|| invariant(Some(index), Reason::ScopeState))?;
    let participation = match structure.record(index + 1) {
        Some(next) if next.formatting_parent().is_some_and(|p| p.index() == index) => {
            match next.kind() {
                Kind::SourceInline => Participation::InlineSequence,
                Kind::SourceBlock | Kind::AnonymousBlock => Participation::BlockChildren,
                Kind::Payload => return Err(invariant(Some(index), Reason::ScopeState)),
            }
        }
        _ => Participation::Empty,
    };
    if marker.kind() == Kind::AnonymousBlock && participation != Participation::InlineSequence {
        return Err(invariant(Some(index), Reason::ScopeState));
    }
    Ok(participation)
}

/// Count, admit, and directly generate a normal-only mapped input.
///
/// # Errors
/// Returns the first error in the documented pointer, work, style, count, and
/// allocation schedule. No partially initialized input is published.
pub fn build_source_analysis_input<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>(
    structure: &'t SourceStructure<'wt, 'e, 'd, 's, 'b, 'r>,
    casing: &'c SourceCasing<'wc, 'e, 'd, 's, 'b, 'r>,
    limits: SourceAnalysisInputLimits,
) -> Result<SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>, Error> {
    if !std::ptr::eq(structure.whitespace(), casing.whitespace()) {
        return Err(Error::OwnerMismatch);
    }
    let mut ledger = Ledger {
        used: 0,
        allowance: limits.work.get(),
    };
    #[cfg(test)]
    trace_phase("setup", 1);
    ledger.charge(1)?;
    let mut counts = Counts {
        sources: structure.source_count(),
        records: structure.record_count(),
        payloads: structure.receipt().contents,
        originals: casing.scalar_count(),
        scopes: 0,
        domains: 0,
        characters: 0,
        bytes: 0,
    };
    if counts.sources > limits.sources.get() {
        return Err(Error::Sources);
    }
    let whitespace = structure.whitespace();
    let sr = structure.receipt();
    if casing.source_count() != counts.sources
        || whitespace.source_count() != counts.sources
        || whitespace.content_count() != counts.payloads
        || whitespace.scalar_count() != counts.originals
        || sr.records != counts.records
        || sr.sources != counts.sources
        || (counts.sources == 0) != (counts.records == 0)
        || (counts.records == 0 && (counts.payloads != 0 || counts.originals != 0))
    {
        return Err(invariant(None, Reason::OwnerCounts));
    }
    if counts.records != 0 {
        let prepaid = counts
            .sources
            .checked_add(counts.records)
            .and_then(|n| n.checked_add(counts.originals))
            .ok_or(Error::Work)?;
        #[cfg(test)]
        {
            trace_phase("eligible-normal-prepaid", counts.sources);
            trace_phase("records-prepaid", counts.records);
            trace_phase("original-visit", counts.originals);
        }
        ledger.charge(prepaid)?;
        for (ordinal, source) in casing.sources().enumerate() {
            if source.ordinal() != ordinal
                || source.source_index() != source.participant().source_index()
            {
                return Err(invariant(None, Reason::SourceJoin));
            }
            let value = source.participant().bidi_style().unicode_bidi();
            if value != ComputedUnicodeBidi::Normal {
                return Err(Error::Nonnormal {
                    source: source.source_index(),
                    value,
                });
            }
        }
        let mut marker = None;
        let mut previous_content = None;
        let mut originals = 0;
        let mut payloads = 0;
        for index in 0..counts.records {
            let record = structure
                .record(index)
                .ok_or_else(|| invariant(Some(index), Reason::RecordParent))?;
            let parent = record.formatting_parent();
            if let Some(p) = &parent {
                if p.index() >= index {
                    return Err(invariant(Some(index), Reason::RecordParent));
                }
            } else if index != 0 || record.kind() != Kind::SourceBlock {
                return Err(invariant(Some(index), Reason::RecordParent));
            }
            match record.kind() {
                Kind::SourceBlock | Kind::AnonymousBlock => {
                    counts.scopes =
                        checked_limit(counts.scopes, 1, limits.scopes.get(), Error::Scopes)?;
                    if parent.is_some_and(|p| p.kind() != Kind::SourceBlock) {
                        return Err(invariant(Some(index), Reason::RecordParent));
                    }
                    let participation = marker_participation(structure, index)?;
                    counts.domains += usize::from(participation == Participation::InlineSequence);
                    marker = Some((index, participation));
                }
                Kind::SourceInline | Kind::Payload => {
                    let (marker_index, participation) =
                        marker.ok_or_else(|| invariant(Some(index), Reason::ScopeState))?;
                    if participation != Participation::InlineSequence {
                        return Err(invariant(Some(index), Reason::ScopeState));
                    }
                    let parent =
                        parent.ok_or_else(|| invariant(Some(index), Reason::RecordParent))?;
                    if parent.index() < marker_index
                        || !(parent.kind() == Kind::SourceInline
                            || (record.kind() == Kind::SourceInline
                                && parent.index() == marker_index))
                    {
                        return Err(invariant(Some(index), Reason::RecordParent));
                    }
                    if record.kind() == Kind::Payload {
                        let content = record
                            .content_event()
                            .ok_or_else(|| invariant(Some(index), Reason::PayloadCoverage))?;
                        if content.kind() != SourceStructuralEventKind::Content
                            || previous_content.is_some_and(|p| p >= content.index())
                            || parent.source_index() != Some(content.participant().source_index())
                            || record.source_index() != parent.source_index()
                        {
                            return Err(invariant(Some(index), Reason::PayloadCoverage));
                        }
                        let range = content.scalar_range();
                        if range.start != originals
                            || range.end < range.start
                            || range.end > counts.originals
                        {
                            return Err(invariant(Some(index), Reason::ScalarCoverage));
                        }
                        payloads += 1;
                        previous_content = Some(content.index());
                        for original in range {
                            let scalar = casing
                                .scalar(original)
                                .ok_or_else(|| invariant(Some(index), Reason::ScalarCoverage))?;
                            if scalar.index() != original
                                || scalar.source().source_index()
                                    != content.participant().source_index()
                                || scalar.whitespace_scalar().participant().source_index()
                                    != content.participant().source_index()
                            {
                                return Err(invariant(Some(index), Reason::ScalarCoverage));
                            }
                            let presentation = scalar.presentation();
                            counts.characters = checked_limit(
                                counts.characters,
                                presentation.characters().len(),
                                limits.output_scalars.get(),
                                Error::OutputScalars,
                            )?;
                            count_presentation_members(
                                &mut ledger,
                                original,
                                presentation,
                                |_, character| {
                                    counts.bytes = checked_limit(
                                        counts.bytes,
                                        character.len_utf8(),
                                        limits.output_bytes.get(),
                                        Error::OutputBytes,
                                    )?;
                                    Ok(())
                                },
                            )?;
                            originals += 1;
                        }
                    }
                }
            }
        }
        if marker.is_none() || originals != counts.originals || payloads != counts.payloads {
            return Err(invariant(None, Reason::Completion));
        }
        #[cfg(test)]
        trace_complete();
        let generation = 1usize
            .checked_add(counts.records)
            .and_then(|n| n.checked_add(counts.originals))
            .and_then(|n| n.checked_add(counts.characters))
            .ok_or(Error::Work)?;
        #[cfg(test)]
        trace_phase("generation-prepaid", generation);
        ledger.charge(generation)?;
    }
    let output = reserve_three(
        counts.scopes,
        counts.originals,
        counts.bytes,
        limits.retained_bytes.get(),
    )?;
    let mut generation = ScopeGeneration::new(
        output,
        counts.scopes,
        counts.originals,
        counts.characters,
        counts.bytes,
    );
    let mut records = 0;
    let mut payloads = 0;
    let mut domains = 0;
    for index in 0..counts.records {
        let record = structure
            .record(index)
            .ok_or_else(|| invariant(Some(index), Reason::RecordParent))?;
        let result = match record.kind() {
            Kind::SourceBlock | Kind::AnonymousBlock => {
                if let Some(pending) = generation.pending_scope {
                    generation
                        .finalize_scope(
                            pending,
                            generation.original_cursor,
                            generation.output.transport.len(),
                            generation.character_cursor,
                        )
                        .map_err(|r| invariant(Some(index), r))?;
                }
                let participation = marker_participation(structure, index)?;
                domains += usize::from(participation == Participation::InlineSequence);
                generation.append_scope(
                    index,
                    participation,
                    generation.original_cursor,
                    generation.output.transport.len(),
                    generation.character_cursor,
                )
            }
            Kind::Payload => {
                let content = record
                    .content_event()
                    .ok_or_else(|| invariant(Some(index), Reason::PayloadCoverage))?;
                for original in content.scalar_range() {
                    let scalar = casing
                        .scalar(original)
                        .ok_or_else(|| invariant(Some(index), Reason::ScalarCoverage))?;
                    for character in scalar.presentation().characters() {
                        generation
                            .append_character(character)
                            .map_err(|r| invariant(Some(index), r))?;
                    }
                    generation
                        .append_original(index, original)
                        .map_err(|r| invariant(Some(index), r))?;
                }
                payloads += 1;
                Ok(())
            }
            Kind::SourceInline => Ok(()),
        };
        result.map_err(|r| invariant(Some(index), r))?;
        records += 1;
    }
    if let Some(pending) = generation.pending_scope {
        generation
            .finalize_scope(
                pending,
                generation.original_cursor,
                generation.output.transport.len(),
                generation.character_cursor,
            )
            .map_err(|r| invariant(None, r))?;
    }
    if records != counts.records || payloads != counts.payloads || domains != counts.domains {
        return Err(invariant(None, Reason::Completion));
    }
    let completed = generation.finish().map_err(|r| invariant(None, r))?;
    let scope_capacity = completed.scopes.capacity();
    let scalar_capacity = completed.scalars.capacity();
    let transport_capacity = completed.transport.capacity();
    let retained_bytes = admit_retained_bytes(
        bytes_for_capacity::<ScopeRecord>(scope_capacity)?,
        bytes_for_capacity::<ScalarRecord>(scalar_capacity)?,
        bytes_for_capacity::<u8>(transport_capacity)?,
        limits.retained_bytes.get(),
    )?;
    let receipt = SourceAnalysisInputReceipt {
        sources: counts.sources,
        records: counts.records,
        payloads: counts.payloads,
        original_scalars: counts.originals,
        scopes: counts.scopes,
        domains: counts.domains,
        output_scalars: counts.characters,
        output_bytes: counts.bytes,
        scope_capacity,
        scalar_capacity,
        transport_capacity,
        scope_element_bytes: size_of::<ScopeRecord>(),
        scalar_element_bytes: size_of::<ScalarRecord>(),
        work: ledger.work(),
        retained_bytes,
        peak_retained_bytes: retained_bytes,
    };
    Ok(SourceAnalysisInput {
        structure,
        casing,
        transport: completed.transport,
        scopes: completed.scopes,
        scalars: completed.scalars,
        receipt,
    })
}

#[cfg(test)]
include!("source_analysis_input_tests.rs");
