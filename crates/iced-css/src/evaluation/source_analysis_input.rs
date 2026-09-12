//! Immutable domain-local presentation with captured-source provenance.
//!
//! This normal-only adapter retains every block scope and every original scalar.
//! Raw bidi consumers borrow individual domains afterward; no CSS line is inferred.
use super::{
    source_casing::{SourceCasing, SourceCasingPresentation, SourceCasingScalar},
    source_participants::SourceParticipant,
    source_structure::{SourceStructure, SourceStructureKind, SourceStructureRecord},
    source_whitespace::{SourceWhitespace, SourceWhitespaceScalar},
};
use iced_css_core::{
    text::bidi_initial::BidiParagraphMode,
    values::bidi::{ComputedDirection, ComputedUnicodeBidi},
};
use std::{fmt, iter::FusedIterator, num::NonZeroUsize, ops::Range};
#[path = "source_analysis_input_build.rs"]
mod build;
pub use build::build_source_analysis_input;

/// Explicit limits; retained bytes bound owned payload, excluding predecessors.
#[derive(Debug, Clone, Copy)]
pub struct SourceAnalysisInputLimits {
    pub sources: NonZeroUsize,
    pub scopes: NonZeroUsize,
    pub output_scalars: NonZeroUsize,
    pub output_bytes: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAnalysisInputInvariantReason {
    OwnerCounts,
    SourceJoin,
    ScopeState,
    RecordParent,
    PayloadCoverage,
    ScalarCoverage,
    EmissionCounts,
    ScopeFinalization,
    Completion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAnalysisInputError {
    OwnerMismatch,
    Sources,
    Scopes,
    OutputScalars,
    OutputBytes,
    Work,
    Bytes,
    Allocation,
    Nonnormal {
        source: usize,
        value: ComputedUnicodeBidi,
    },
    Invariant {
        record: Option<usize>,
        reason: SourceAnalysisInputInvariantReason,
    },
}
impl fmt::Display for SourceAnalysisInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source analysis input: {self:?}")
    }
}
impl std::error::Error for SourceAnalysisInputError {}
/// Structural participation, independent of the number of emitted characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAnalysisParticipation {
    Empty,
    BlockChildren,
    InlineSequence,
}
/// Successful exact counts and actual simultaneous allocation payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceAnalysisInputReceipt {
    pub sources: usize,
    pub records: usize,
    pub payloads: usize,
    pub original_scalars: usize,
    pub scopes: usize,
    pub domains: usize,
    pub output_scalars: usize,
    pub output_bytes: usize,
    pub scope_capacity: usize,
    pub scalar_capacity: usize,
    pub transport_capacity: usize,
    pub scope_element_bytes: usize,
    pub scalar_element_bytes: usize,
    pub work: usize,
    pub retained_bytes: usize,
    pub peak_retained_bytes: usize,
}
#[derive(Debug)]
struct ScopeRecord {
    record_index: usize,
    participation: SourceAnalysisParticipation,
    original_end: usize,
    byte_end: usize,
    analysis_scalar_end: usize,
}
#[derive(Debug)]
struct ScalarRecord {
    payload_record_index: usize,
    byte_end: usize,
    analysis_scalar_end: usize,
}
/// Immutable owner borrowing the exact independently-lived predecessor pair.
#[must_use]
pub struct SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    structure: &'t SourceStructure<'wt, 'e, 'd, 's, 'b, 'r>,
    casing: &'c SourceCasing<'wc, 'e, 'd, 's, 'b, 'r>,
    transport: String,
    scopes: Vec<ScopeRecord>,
    scalars: Vec<ScalarRecord>,
    receipt: SourceAnalysisInputReceipt,
}
impl<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> fmt::Debug
    for SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceAnalysisInput")
            .field("receipt", &self.receipt)
            .finish_non_exhaustive()
    }
}
impl<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn structure(&self) -> &'t SourceStructure<'wt, 'e, 'd, 's, 'b, 'r> {
        self.structure
    }
    pub fn casing(&self) -> &'c SourceCasing<'wc, 'e, 'd, 's, 'b, 'r> {
        self.casing
    }
    pub fn whitespace(&self) -> &'wt SourceWhitespace<'e, 'd, 's, 'b, 'r> {
        self.structure.whitespace()
    }
    pub fn receipt(&self) -> SourceAnalysisInputReceipt {
        self.receipt
    }
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }
    pub fn domain_count(&self) -> usize {
        self.receipt.domains
    }
    pub fn contribution_count(&self) -> usize {
        self.scalars.len()
    }
    pub fn work(&self) -> usize {
        self.receipt.work
    }
    pub fn retained_bytes(&self) -> usize {
        self.receipt.retained_bytes
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.receipt.peak_retained_bytes
    }
    pub fn scope(
        &self,
        index: usize,
    ) -> Option<SourceAnalysisScope<'_, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        self.scopes
            .get(index)
            .map(|_| SourceAnalysisScope { owner: self, index })
    }
    pub fn scopes(&self) -> SourceAnalysisScopes<'_, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceAnalysisScopes {
            owner: self,
            remaining: 0..self.scope_count(),
        }
    }
    pub fn contribution(
        &self,
        original: usize,
    ) -> Option<SourceAnalysisContribution<'_, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        self.scalars
            .get(original)
            .map(|_| SourceAnalysisContribution {
                owner: self,
                original,
            })
    }
    pub fn contributions(
        &self,
    ) -> SourceAnalysisContributions<'_, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceAnalysisContributions {
            owner: self,
            remaining: 0..self.contribution_count(),
        }
    }
    /// Receiver-local Payload lookup; scope selection uses record position, including empty Content.
    pub fn payload_projection(
        &self,
        record: usize,
    ) -> Option<SourceAnalysisPayloadProjection<'_, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        if self.structure.record(record)?.kind() != SourceStructureKind::Payload {
            return None;
        }
        let scope = self.scope_index(record)?;
        Some(SourceAnalysisPayloadProjection {
            owner: self,
            record,
            scope,
        })
    }
    fn prefix(&self, original: usize) -> (usize, usize) {
        original.checked_sub(1).map_or((0, 0), |i| {
            let row = &self.scalars[i];
            (row.byte_end, row.analysis_scalar_end)
        })
    }
    fn scope_index(&self, record: usize) -> Option<usize> {
        #[cfg(test)]
        build::query_scope_size(self.scopes.len());
        self.scopes
            .partition_point(|row| {
                #[cfg(test)]
                build::query_scope_comparison();
                row.record_index <= record
            })
            .checked_sub(1)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SourceAnalysisScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceAnalysisDomain<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceAnalysisContribution<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    original: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceAnalysisMemberOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    original: usize,
    member: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceAnalysisPayloadProjection<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    record: usize,
    scope: usize,
}

impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn owner(&self) -> &'o SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.owner
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn record(&self) -> SourceStructureRecord<'t, 'wt, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .structure
            .record(self.row().record_index)
            .expect("completed scope marker")
    }
    fn row(&self) -> &ScopeRecord {
        &self.owner.scopes[self.index]
    }
    pub fn participation(&self) -> SourceAnalysisParticipation {
        self.row().participation
    }
    pub fn record_range(&self) -> Range<usize> {
        self.row().record_index
            ..self
                .owner
                .scopes
                .get(self.index + 1)
                .map_or(self.owner.receipt.records, |r| r.record_index)
    }
    pub fn original_range(&self) -> Range<usize> {
        self.index
            .checked_sub(1)
            .map_or(0, |i| self.owner.scopes[i].original_end)..self.row().original_end
    }
    pub fn transport_range(&self) -> Range<usize> {
        self.index
            .checked_sub(1)
            .map_or(0, |i| self.owner.scopes[i].byte_end)..self.row().byte_end
    }
    pub fn analysis_range(&self) -> Range<usize> {
        self.index
            .checked_sub(1)
            .map_or(0, |i| self.owner.scopes[i].analysis_scalar_end)
            ..self.row().analysis_scalar_end
    }
    pub fn base_origin(
        &self,
    ) -> SourceAnalysisBaseOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        if self.record().kind() == SourceStructureKind::SourceBlock {
            SourceAnalysisBaseOrigin::OwnBlock(SourceAnalysisOwnBlockBase { scope: *self })
        } else {
            SourceAnalysisBaseOrigin::AnonymousInherited(SourceAnalysisAnonymousBase {
                scope: *self,
            })
        }
    }
    pub fn mode(&self) -> BidiParagraphMode {
        match self.base_origin().direction() {
            ComputedDirection::LeftToRight => BidiParagraphMode::LeftToRight,
            ComputedDirection::RightToLeft => BidiParagraphMode::RightToLeft,
        }
    }
    pub fn inline_sequence(
        &self,
    ) -> Option<SourceAnalysisDomain<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        (self.participation() == SourceAnalysisParticipation::InlineSequence).then_some(
            SourceAnalysisDomain {
                owner: self.owner,
                index: self.index,
            },
        )
    }
    pub fn contributions(
        &self,
    ) -> SourceAnalysisContributions<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceAnalysisContributions {
            owner: self.owner,
            remaining: self.original_range(),
        }
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisDomain<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn scope(&self) -> SourceAnalysisScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceAnalysisScope {
            owner: self.owner,
            index: self.index,
        }
    }
    pub fn text(&self) -> &'o str {
        &self.owner.transport[self.scope().transport_range()]
    }
    pub fn scalar_count(&self) -> usize {
        self.scope().analysis_range().len()
    }
    /// O(log S) lookup of a receiver-local emitted scalar; skips zero-output originals.
    pub fn origin(
        &self,
        local_analysis_scalar: usize,
    ) -> Option<SourceAnalysisMemberOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        let scope = self.scope();
        let range = scope.analysis_range();
        if local_analysis_scalar >= range.len() {
            return None;
        }
        let global = range.start + local_analysis_scalar;
        #[cfg(test)]
        build::query_scalar_size(self.owner.scalars.len());
        let original = self.owner.scalars.partition_point(|row| {
            #[cfg(test)]
            build::query_scalar_comparison();
            row.analysis_scalar_end <= global
        });
        if !scope.original_range().contains(&original) {
            return None;
        }
        Some(SourceAnalysisMemberOrigin {
            owner: self.owner,
            original,
            member: global - self.owner.prefix(original).1,
        })
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisContribution<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn owner(&self) -> &'o SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.owner
    }
    pub fn original_index(&self) -> usize {
        self.original
    }
    pub fn transport_range(&self) -> Range<usize> {
        self.owner.prefix(self.original).0..self.owner.scalars[self.original].byte_end
    }
    pub fn analysis_range(&self) -> Range<usize> {
        self.owner.prefix(self.original).1..self.owner.scalars[self.original].analysis_scalar_end
    }
    pub fn casing_scalar(&self) -> SourceCasingScalar<'c, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .casing
            .scalar(self.original)
            .expect("completed original")
    }
    pub fn whitespace_scalar(&self) -> SourceWhitespaceScalar<'wc, 'e, 'd, 's, 'b, 'r> {
        self.casing_scalar().whitespace_scalar()
    }
    pub fn presentation(&self) -> SourceCasingPresentation {
        self.casing_scalar().presentation()
    }
    pub fn payload(&self) -> SourceStructureRecord<'t, 'wt, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .structure
            .record(self.owner.scalars[self.original].payload_record_index)
            .expect("completed Payload")
    }
    pub fn scope(&self) -> SourceAnalysisScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceAnalysisScope {
            owner: self.owner,
            index: self
                .owner
                .scope_index(self.owner.scalars[self.original].payload_record_index)
                .expect("Payload scope"),
        }
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisMemberOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn contribution(
        &self,
    ) -> SourceAnalysisContribution<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceAnalysisContribution {
            owner: self.owner,
            original: self.original,
        }
    }
    pub fn original_index(&self) -> usize {
        self.original
    }
    pub fn member_index(&self) -> usize {
        self.member
    }
    pub fn analysis_index(&self) -> usize {
        self.owner.prefix(self.original).1 + self.member
    }
    pub fn character(&self) -> char {
        match self.contribution().presentation() {
            SourceCasingPresentation::Scalar(character) => character,
            SourceCasingPresentation::Replacement(characters) => characters[self.member],
            SourceCasingPresentation::SuppressedByWhitespace => {
                unreachable!("checked emitted member")
            }
        }
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisPayloadProjection<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn payload(&self) -> SourceStructureRecord<'t, 'wt, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .structure
            .record(self.record)
            .expect("checked Payload")
    }
    pub fn scope(&self) -> SourceAnalysisScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceAnalysisScope {
            owner: self.owner,
            index: self.scope,
        }
    }
    pub fn domain(&self) -> SourceAnalysisDomain<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceAnalysisDomain {
            owner: self.owner,
            index: self.scope,
        }
    }
    pub fn original_range(&self) -> Range<usize> {
        self.payload()
            .content_event()
            .expect("Payload Content")
            .scalar_range()
    }
    pub fn transport_range(&self) -> Range<usize> {
        let r = self.original_range();
        self.owner.prefix(r.start).0..self.owner.prefix(r.end).0
    }
    pub fn analysis_range(&self) -> Range<usize> {
        let r = self.original_range();
        self.owner.prefix(r.start).1..self.owner.prefix(r.end).1
    }
    pub fn domain_byte_range(&self) -> Range<usize> {
        let r = self.transport_range();
        let start = self.scope().transport_range().start;
        r.start - start..r.end - start
    }
    pub fn domain_scalar_range(&self) -> Range<usize> {
        let r = self.analysis_range();
        let start = self.scope().analysis_range().start;
        r.start - start..r.end - start
    }
}
#[derive(Debug, Clone, Copy)]
pub struct SourceAnalysisOwnBlockBase<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    scope: SourceAnalysisScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
}
#[derive(Debug, Clone, Copy)]
pub struct SourceAnalysisAnonymousBase<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    scope: SourceAnalysisScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
}
#[derive(Debug, Clone, Copy)]
pub enum SourceAnalysisBaseOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    OwnBlock(SourceAnalysisOwnBlockBase<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>),
    AnonymousInherited(SourceAnalysisAnonymousBase<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>),
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisBaseOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn scope(&self) -> SourceAnalysisScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        match self {
            Self::OwnBlock(b) => b.scope,
            Self::AnonymousInherited(b) => b.scope,
        }
    }
    pub fn record(&self) -> SourceStructureRecord<'t, 'wt, 'e, 'd, 's, 'b, 'r> {
        self.scope().record()
    }
    pub fn participant(&self) -> SourceParticipant<'e, 'd, 's, 'b, 'r> {
        match self {
            Self::OwnBlock(_) => self
                .record()
                .participant()
                .expect("source block participant"),
            Self::AnonymousInherited(_) => self
                .record()
                .anonymous_origin()
                .expect("anonymous marker")
                .inheritance_source(),
        }
    }
    pub fn direction(&self) -> ComputedDirection {
        self.participant().bidi_style().direction()
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn is_same_scope(
        &self,
        other: &SourceAnalysisScope<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisDomain<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn is_same_domain(
        &self,
        other: &SourceAnalysisDomain<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisContribution<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn is_same_contribution(
        &self,
        other: &SourceAnalysisContribution<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.original == other.original
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisMemberOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn is_same_origin(
        &self,
        other: &SourceAnalysisMemberOrigin<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.original == other.original
            && self.member == other.member
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceAnalysisPayloadProjection<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn is_same_projection(
        &self,
        other: &SourceAnalysisPayloadProjection<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.record == other.record
    }
}
#[derive(Debug, Clone)]
pub struct SourceAnalysisScopes<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    remaining: Range<usize>,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> Iterator
    for SourceAnalysisScopes<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    type Item = SourceAnalysisScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>;
    fn next(&mut self) -> Option<Self::Item> {
        self.remaining.next().map(|index| SourceAnalysisScope {
            owner: self.owner,
            index,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.remaining.size_hint()
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> ExactSizeIterator
    for SourceAnalysisScopes<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> FusedIterator
    for SourceAnalysisScopes<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
}
#[derive(Debug, Clone)]
pub struct SourceAnalysisContributions<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    remaining: Range<usize>,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> Iterator
    for SourceAnalysisContributions<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    type Item = SourceAnalysisContribution<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>;
    fn next(&mut self) -> Option<Self::Item> {
        self.remaining
            .next()
            .map(|original| SourceAnalysisContribution {
                owner: self.owner,
                original,
            })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.remaining.size_hint()
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> ExactSizeIterator
    for SourceAnalysisContributions<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> FusedIterator
    for SourceAnalysisContributions<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
}
