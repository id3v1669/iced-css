//! Private, allocation-free predecessor joins. Callers retain their own policy,
//! state transitions, limits, work charges and public error mappings.
use super::{
    source_analysis_input::SourceAnalysisParticipation as Participation,
    source_casing::{SourceCasing, SourceCasingScalar, SourceCasingSource},
    source_structural_events::SourceStructuralEventKind,
    source_structure::{SourceStructure, SourceStructureKind as Kind, SourceStructureRecord},
    source_whitespace::SourceWhitespaceEvent,
};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JoinReason {
    OwnerCounts,
    SourceJoin,
    ScopeState,
    RecordParent,
    PayloadCoverage,
    ScalarCoverage,
}
pub(super) fn same_whitespace_owner(
    structure: &SourceStructure<'_, '_, '_, '_, '_, '_>,
    casing: &SourceCasing<'_, '_, '_, '_, '_, '_>,
) -> bool {
    std::ptr::eq(structure.whitespace(), casing.whitespace())
}
pub(super) fn owner_counts(
    structure: &SourceStructure<'_, '_, '_, '_, '_, '_>,
    casing: &SourceCasing<'_, '_, '_, '_, '_, '_>,
) -> Result<(), JoinReason> {
    let sources = structure.source_count();
    let records = structure.record_count();
    let payloads = structure.receipt().contents;
    let originals = casing.scalar_count();
    let whitespace = structure.whitespace();
    let sr = structure.receipt();
    if casing.source_count() != sources
        || whitespace.source_count() != sources
        || whitespace.content_count() != payloads
        || whitespace.scalar_count() != originals
        || sr.records != records
        || sr.sources != sources
        || (sources == 0) != (records == 0)
        || (records == 0 && (payloads != 0 || originals != 0))
    {
        return Err(JoinReason::OwnerCounts);
    }
    Ok(())
}
pub(super) fn source_join(
    ordinal: usize,
    source: &SourceCasingSource<'_, '_, '_, '_, '_, '_, '_>,
) -> Result<(), JoinReason> {
    if source.ordinal() != ordinal || source.source_index() != source.participant().source_index() {
        return Err(JoinReason::SourceJoin);
    }
    Ok(())
}
pub(super) fn record_parent(
    index: usize,
    kind: Kind,
    parent: Option<(usize, Kind)>,
) -> Result<(), JoinReason> {
    if let Some((p, _)) = parent {
        if p >= index {
            return Err(JoinReason::RecordParent);
        }
    } else if index != 0 || kind != Kind::SourceBlock {
        return Err(JoinReason::RecordParent);
    }
    Ok(())
}
pub(super) fn scope_parent(parent: Option<Kind>) -> Result<(), JoinReason> {
    if parent.is_some_and(|p| p != Kind::SourceBlock) {
        return Err(JoinReason::RecordParent);
    }
    Ok(())
}
pub(super) fn marker_participation(
    structure: &SourceStructure<'_, '_, '_, '_, '_, '_>,
    index: usize,
) -> Result<Participation, JoinReason> {
    let marker = structure.record(index).ok_or(JoinReason::ScopeState)?;
    let participation = match structure.record(index + 1) {
        Some(next) if next.formatting_parent().is_some_and(|p| p.index() == index) => {
            match next.kind() {
                Kind::SourceInline => Participation::InlineSequence,
                Kind::SourceBlock | Kind::AnonymousBlock => Participation::BlockChildren,
                Kind::Payload => return Err(JoinReason::ScopeState),
            }
        }
        _ => Participation::Empty,
    };
    if marker.kind() == Kind::AnonymousBlock && participation != Participation::InlineSequence {
        return Err(JoinReason::ScopeState);
    }
    Ok(participation)
}
pub(super) fn inline_parent(
    kind: Kind,
    marker: Option<(usize, Participation)>,
    parent: Option<(usize, Kind)>,
) -> Result<(), JoinReason> {
    let (marker_index, participation) = marker.ok_or(JoinReason::ScopeState)?;
    if participation != Participation::InlineSequence {
        return Err(JoinReason::ScopeState);
    }
    let (parent_index, parent_kind) = parent.ok_or(JoinReason::RecordParent)?;
    if parent_index < marker_index
        || !(parent_kind == Kind::SourceInline
            || (kind == Kind::SourceInline && parent_index == marker_index))
    {
        return Err(JoinReason::RecordParent);
    }
    Ok(())
}
pub(super) fn payload_join(
    record: &SourceStructureRecord<'_, '_, '_, '_, '_, '_, '_>,
    parent_source: Option<usize>,
    content: &SourceWhitespaceEvent<'_, '_, '_, '_, '_, '_>,
    previous_content: Option<usize>,
    next_original: usize,
    total_originals: usize,
) -> Result<Range<usize>, JoinReason> {
    if content.kind() != SourceStructuralEventKind::Content
        || previous_content.is_some_and(|p| p >= content.index())
        || parent_source != Some(content.participant().source_index())
        || record.source_index() != parent_source
    {
        return Err(JoinReason::PayloadCoverage);
    }
    let range = content.scalar_range();
    if range.start != next_original || range.end < range.start || range.end > total_originals {
        return Err(JoinReason::ScalarCoverage);
    }
    Ok(range)
}
pub(super) fn original_join(
    original: usize,
    scalar: &SourceCasingScalar<'_, '_, '_, '_, '_, '_, '_>,
    content_source: usize,
) -> Result<(), JoinReason> {
    if scalar.index() != original
        || scalar.source().source_index() != content_source
        || scalar.whitespace_scalar().participant().source_index() != content_source
    {
        return Err(JoinReason::ScalarCoverage);
    }
    Ok(())
}
