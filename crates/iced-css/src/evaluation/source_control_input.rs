//! Immutable CSS-generated directional controls with exact captured-source origins.
//!
//! This intermediate producer admits uninterrupted inline controls, strict-interior
//! paragraph separators, and a bounded interruption profile. The latter requires one
//! affected inline cut by any number of intervening blocks, none of them rejected for
//! its own bidi value, with exactly one nonempty, B-free payload in every fragment and
//! in every intervening block. Other interruption shapes remain unsupported. It does not replace the broader Normal-only adapter.
use super::{
    source_analysis_input::SourceAnalysisParticipation,
    source_casing::{SourceCasing, SourceCasingPresentation, SourceCasingScalar},
    source_participants::SourceParticipant,
    source_structure::{
        SourceStructure, SourceStructureBoundarySide, SourceStructureEdge, SourceStructureEdgeKind,
        SourceStructureKind, SourceStructureRecord,
    },
    source_whitespace::{SourceWhitespace, SourceWhitespaceScalar},
};
use iced_css_core::{
    text::bidi_initial::BidiParagraphMode,
    values::bidi::{ComputedDirection, ComputedUnicodeBidi},
};
use std::{collections::TryReserveError, fmt, num::NonZeroUsize, ops::Range};
#[path = "source_control_input_build.rs"]
mod build;
pub use build::build_source_control_input;

#[derive(Debug)]
struct ScopeRow {
    record: usize,
    participation: SourceAnalysisParticipation,
    originals: Range<usize>,
    payloads: Range<usize>,
    tokens: Range<usize>,
    bytes: Range<usize>,
    record_end: usize,
}
#[derive(Debug)]
struct PayloadRow {
    record: usize,
    scope: usize,
    originals: Range<usize>,
    tokens: Range<usize>,
}
#[derive(Debug)]
struct OriginalRow {
    payload: usize,
    members: usize,
    tokens: Range<usize>,
}
#[derive(Debug)]
struct TokenRow {
    character: char,
    byte_end: usize,
    scope: usize,
    origin: OriginRow,
}
#[derive(Debug, Clone, Copy)]
enum OriginRow {
    Literal {
        original: usize,
        member: usize,
    },
    Synthetic {
        occurrence: usize,
        cause: usize,
        placement: PlacementRow,
    },
}
#[derive(Debug, Clone, Copy)]
enum PlacementRow {
    Edge(SourceStructureBoundarySide),
    Break {
        original: usize,
        member: usize,
        side: SourceControlBreakSide,
    },
}

#[must_use]
pub struct SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    structure: &'t SourceStructure<'wt, 'e, 'd, 's, 'b, 'r>,
    casing: &'c SourceCasing<'wc, 'e, 'd, 's, 'b, 'r>,
    transport: String,
    scopes: Vec<ScopeRow>,
    payloads: Vec<PayloadRow>,
    originals: Vec<OriginalRow>,
    tokens: Vec<TokenRow>,
    receipt: SourceControlInputReceipt,
}
impl<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
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
    pub fn receipt(&self) -> SourceControlInputReceipt {
        self.receipt
    }
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }
    pub fn domain_count(&self) -> usize {
        self.receipt.domains
    }
    pub fn payload_count(&self) -> usize {
        self.payloads.len()
    }
    pub fn contribution_count(&self) -> usize {
        self.originals.len()
    }
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
    pub fn scope<'o>(
        &'o self,
        index: usize,
    ) -> Option<SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        self.scopes.get(index)?;
        Some(SourceControlScope { owner: self, index })
    }
    pub fn payload<'o>(
        &'o self,
        index: usize,
    ) -> Option<SourceControlPayload<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        self.payloads.get(index)?;
        Some(SourceControlPayload { owner: self, index })
    }
    pub fn contribution<'o>(
        &'o self,
        original: usize,
    ) -> Option<SourceControlContribution<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        self.originals.get(original)?;
        Some(SourceControlContribution {
            owner: self,
            index: original,
        })
    }
}
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn owner(&self) -> &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.owner
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn record(&self) -> SourceStructureRecord<'t, 'wt, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .structure
            .record(self.row().record)
            .expect("completed scope marker")
    }
    pub fn participation(&self) -> SourceAnalysisParticipation {
        self.row().participation
    }
    pub fn record_range(&self) -> Range<usize> {
        self.row().record..self.row().record_end
    }
    pub fn original_range(&self) -> Range<usize> {
        self.row().originals.clone()
    }
    pub fn payload_range(&self) -> Range<usize> {
        self.row().payloads.clone()
    }
    pub fn inline_sequence(
        &self,
    ) -> Option<SourceControlDomain<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        (self.participation() == SourceAnalysisParticipation::InlineSequence).then_some(
            SourceControlDomain {
                owner: self.owner,
                index: self.index,
            },
        )
    }
    pub fn base_origin(&self) -> SourceControlBaseOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        if self.record().kind() == SourceStructureKind::SourceBlock {
            SourceControlBaseOrigin::OwnBlock(SourceControlOwnBlockBase {
                owner: self.owner,
                index: self.index,
            })
        } else {
            SourceControlBaseOrigin::AnonymousInherited(SourceControlAnonymousBase {
                owner: self.owner,
                index: self.index,
            })
        }
    }
    pub fn mode(&self) -> BidiParagraphMode {
        match self.base_origin().direction() {
            ComputedDirection::LeftToRight => BidiParagraphMode::LeftToRight,
            ComputedDirection::RightToLeft => BidiParagraphMode::RightToLeft,
        }
    }
    pub fn is_same_scope(
        &self,
        other: &SourceControlScope<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct SourceControlDomain<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlDomain<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn scope(&self) -> SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceControlScope {
            owner: self.owner,
            index: self.index,
        }
    }
    pub fn text(&self) -> &'o str {
        &self.owner.transport[self.owner.scopes[self.index].bytes.clone()]
    }
    pub fn scalar_count(&self) -> usize {
        self.owner.scopes[self.index].tokens.len()
    }
    pub fn token(
        &self,
        index: usize,
    ) -> Option<SourceControlToken<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        let range = &self.owner.scopes[self.index].tokens;
        (index < range.len()).then(|| SourceControlToken {
            owner: self.owner,
            index: range.start + index,
        })
    }
    pub fn is_same_domain(
        &self,
        other: &SourceControlDomain<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct SourceControlPayload<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlPayload<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn owner(&self) -> &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.owner
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn record(&self) -> SourceStructureRecord<'t, 'wt, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .structure
            .record(self.row().record)
            .expect("completed Payload record")
    }
    pub fn scope(&self) -> SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceControlScope {
            owner: self.owner,
            index: self.row().scope,
        }
    }
    pub fn original_range(&self) -> Range<usize> {
        self.row().originals.clone()
    }
    pub fn token_range(&self) -> Range<usize> {
        let start = self.owner.scopes[self.row().scope].tokens.start;
        self.row().tokens.start - start..self.row().tokens.end - start
    }
    pub fn is_same_payload(
        &self,
        other: &SourceControlPayload<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct SourceControlContribution<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlContribution<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn owner(&self) -> &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.owner
    }
    pub fn original_index(&self) -> usize {
        self.index
    }
    pub fn payload(&self) -> SourceControlPayload<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceControlPayload {
            owner: self.owner,
            index: self.row().payload,
        }
    }
    pub fn scope(&self) -> SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.payload().scope()
    }
    pub fn casing_scalar(&self) -> SourceCasingScalar<'c, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .casing
            .scalar(self.index)
            .expect("completed original scalar")
    }
    pub fn whitespace_scalar(&self) -> SourceWhitespaceScalar<'wc, 'e, 'd, 's, 'b, 'r> {
        self.casing_scalar().whitespace_scalar()
    }
    pub fn presentation(&self) -> SourceCasingPresentation {
        self.casing_scalar().presentation()
    }
    pub fn member_count(&self) -> usize {
        self.row().members
    }
    pub fn token_range(&self) -> Range<usize> {
        let start = self.owner.scopes[self.payload().row().scope].tokens.start;
        self.row().tokens.start - start..self.row().tokens.end - start
    }
    pub fn member(
        &self,
        index: usize,
    ) -> Option<SourceControlLiteralOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        (index < self.member_count()).then_some(SourceControlLiteralOrigin {
            owner: self.owner,
            original: self.index,
            member: index,
        })
    }
    /// Scans only this original contribution's token interval; allocates nothing.
    pub fn literal_token(
        &self,
        member: usize,
    ) -> Option<SourceControlToken<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>> {
        if member >= self.member_count() {
            return None;
        }
        self.row()
            .tokens
            .clone()
            .find_map(|index| match self.owner.tokens[index].origin {
                OriginRow::Literal {
                    original,
                    member: found,
                } if original == self.index && found == member => Some(SourceControlToken {
                    owner: self.owner,
                    index,
                }),
                _ => None,
            })
    }
    pub fn is_same_contribution(
        &self,
        other: &SourceControlContribution<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct SourceControlToken<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlToken<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn owner(&self) -> &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.owner
    }
    pub fn domain(&self) -> SourceControlDomain<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceControlDomain {
            owner: self.owner,
            index: self.row().scope,
        }
    }
    pub fn ordinal(&self) -> usize {
        self.index - self.owner.scopes[self.row().scope].tokens.start
    }
    pub fn character(&self) -> char {
        self.row().character
    }
    pub fn byte_range(&self) -> Range<usize> {
        let end = self.row().byte_end - self.owner.scopes[self.row().scope].bytes.start;
        end - self.character().len_utf8()..end
    }
    pub fn origin(&self) -> SourceControlOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        match self.row().origin {
            OriginRow::Literal { original, member } => {
                SourceControlOrigin::Literal(SourceControlLiteralOrigin {
                    owner: self.owner,
                    original,
                    member,
                })
            }
            OriginRow::Synthetic { .. } => {
                SourceControlOrigin::Synthetic(SourceControlSyntheticOrigin {
                    owner: self.owner,
                    index: self.index,
                })
            }
        }
    }
    pub fn is_same_token(
        &self,
        other: &SourceControlToken<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct SourceControlLiteralOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    original: usize,
    member: usize,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlLiteralOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn owner(&self) -> &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.owner
    }
    pub fn contribution(
        &self,
    ) -> SourceControlContribution<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceControlContribution {
            owner: self.owner,
            index: self.original,
        }
    }
    pub fn original_index(&self) -> usize {
        self.original
    }
    pub fn member_index(&self) -> usize {
        self.member
    }
    pub fn payload(&self) -> SourceControlPayload<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.contribution().payload()
    }
    pub fn scope(&self) -> SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.contribution().scope()
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
    /// Scans only this original contribution's token interval; allocates nothing.
    pub fn token(&self) -> SourceControlToken<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.contribution()
            .literal_token(self.member)
            .expect("completed literal token")
    }
    pub fn is_same_origin(
        &self,
        other: &SourceControlLiteralOrigin<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.original == other.original
            && self.member == other.member
    }
}
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct SourceControlSyntheticOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlSyntheticOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn owner(&self) -> &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.owner
    }
    pub fn domain(&self) -> SourceControlDomain<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        self.token().domain()
    }
    /// Synthetic-only ordinal within this domain, distinct from token ordinal.
    pub fn occurrence_index(&self) -> usize {
        self.parts().0
    }
    pub fn token(&self) -> SourceControlToken<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceControlToken {
            owner: self.owner,
            index: self.index,
        }
    }
    pub fn cause_fragment(&self) -> SourceStructureRecord<'t, 'wt, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .structure
            .record(self.parts().1)
            .expect("completed CSS cause fragment")
    }
    pub fn cause(&self) -> SourceParticipant<'e, 'd, 's, 'b, 'r> {
        self.cause_fragment()
            .participant()
            .expect("completed CSS cause participant")
    }
    pub fn placement(&self) -> SourceControlPlacement<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        match self.parts().2 {
            PlacementRow::Edge(side) => SourceControlPlacement::FragmentEdge(match side {
                SourceStructureBoundarySide::Start => self
                    .cause_fragment()
                    .start_edge()
                    .expect("completed start edge"),
                SourceStructureBoundarySide::End => self
                    .cause_fragment()
                    .end_edge()
                    .expect("completed end edge"),
            }),
            PlacementRow::Break {
                original,
                member,
                side,
            } => SourceControlPlacement::Break {
                anchor: SourceControlLiteralOrigin {
                    owner: self.owner,
                    original,
                    member,
                },
                side,
            },
        }
    }
    pub fn is_same_origin(
        &self,
        other: &SourceControlSyntheticOrigin<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct SourceControlOwnBlockBase<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlOwnBlockBase<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn scope(&self) -> SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceControlScope {
            owner: self.owner,
            index: self.index,
        }
    }
    pub fn record(&self) -> SourceStructureRecord<'t, 'wt, 'e, 'd, 's, 'b, 'r> {
        self.scope().record()
    }
    pub fn participant(&self) -> SourceParticipant<'e, 'd, 's, 'b, 'r> {
        self.record().participant().expect("completed own block")
    }
    pub fn direction(&self) -> ComputedDirection {
        self.participant().bidi_style().direction()
    }
    pub fn is_same_base(
        &self,
        other: &SourceControlOwnBlockBase<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct SourceControlAnonymousBase<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlAnonymousBase<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn scope(&self) -> SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        SourceControlScope {
            owner: self.owner,
            index: self.index,
        }
    }
    pub fn record(&self) -> SourceStructureRecord<'t, 'wt, 'e, 'd, 's, 'b, 'r> {
        self.scope().record()
    }
    pub fn participant(&self) -> SourceParticipant<'e, 'd, 's, 'b, 'r> {
        self.record()
            .anonymous_origin()
            .expect("completed anonymous marker")
            .inheritance_source()
    }
    pub fn direction(&self) -> ComputedDirection {
        self.participant().bidi_style().direction()
    }
    pub fn is_same_base(
        &self,
        other: &SourceControlAnonymousBase<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlBaseOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    pub fn scope(&self) -> SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
        match self {
            Self::OwnBlock(base) => base.scope(),
            Self::AnonymousInherited(base) => base.scope(),
        }
    }
    pub fn record(&self) -> SourceStructureRecord<'t, 'wt, 'e, 'd, 's, 'b, 'r> {
        self.scope().record()
    }
    pub fn participant(&self) -> SourceParticipant<'e, 'd, 's, 'b, 'r> {
        match self {
            Self::OwnBlock(base) => base.participant(),
            Self::AnonymousInherited(base) => base.participant(),
        }
    }
    pub fn direction(&self) -> ComputedDirection {
        self.participant().bidi_style().direction()
    }
    pub fn is_same_base(
        &self,
        other: &SourceControlBaseOrigin<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        match (self, other) {
            (Self::OwnBlock(a), SourceControlBaseOrigin::OwnBlock(b)) => a.is_same_base(b),
            (Self::AnonymousInherited(a), SourceControlBaseOrigin::AnonymousInherited(b)) => {
                a.is_same_base(b)
            }
            _ => false,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum SourceControlOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    Literal(SourceControlLiteralOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>),
    Synthetic(SourceControlSyntheticOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>),
}
#[derive(Debug, Clone, Copy)]
pub enum SourceControlBaseOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    OwnBlock(SourceControlOwnBlockBase<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>),
    AnonymousInherited(SourceControlAnonymousBase<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>),
}
#[derive(Debug)]
pub enum SourceControlPlacement<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> {
    FragmentEdge(SourceStructureEdge<'t, 'wt, 'e, 'd, 's, 'b, 'r>),
    Break {
        anchor: SourceControlLiteralOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>,
        side: SourceControlBreakSide,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceControlBreakSide {
    Before,
    After,
}
#[derive(Debug, Clone, Copy)]
pub struct SourceControlInputLimits {
    pub sources: NonZeroUsize,
    pub records: NonZeroUsize,
    pub originals: NonZeroUsize,
    pub scopes: NonZeroUsize,
    pub depth: NonZeroUsize,
    pub output_scalars: NonZeroUsize,
    pub output_bytes: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub scratch_bytes: NonZeroUsize,
    pub peak_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceControlInputReceipt {
    pub sources: usize,
    pub records: usize,
    pub payloads: usize,
    pub original_scalars: usize,
    pub literal_scalars: usize,
    pub synthetic_scalars: usize,
    pub output_scalars: usize,
    pub scopes: usize,
    pub domains: usize,
    pub output_bytes: usize,
    pub frames_pushed: usize,
    pub max_depth: usize,
    pub frame_visits_at_breaks: usize,
    pub work: usize,
    pub scope_capacity: usize,
    pub payload_capacity: usize,
    pub original_capacity: usize,
    pub token_capacity: usize,
    pub transport_capacity: usize,
    pub frame_capacity: usize,
    pub scope_element_bytes: usize,
    pub payload_element_bytes: usize,
    pub original_element_bytes: usize,
    pub token_element_bytes: usize,
    pub frame_element_bytes: usize,
    pub scratch_bytes: usize,
    pub retained_bytes: usize,
    pub peak_retained_bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceControlLimit {
    Sources,
    Records,
    Originals,
    Scopes,
    Depth,
    OutputScalars,
    OutputBytes,
    RetainedBytes,
    ScratchBytes,
    PeakBytes,
    Work,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceControlStorage {
    Frames,
    Scopes,
    Payloads,
    Originals,
    Tokens,
    Transport,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceControlUnsupported {
    /// A block-level `unicode-bidi` value in a configuration this profile does not
    /// resolve. Block `embed` is inert and always admitted; block `bidi-override` is
    /// admitted only without an anonymous child block box and without a forced break
    /// inside its own domain, because CSS 2.2 §9.10 enumerates no closing schedule for
    /// anonymous children and Writing Modes §2.4.4's restart rule covers inline boxes.
    BlockBidi(ComputedUnicodeBidi),
    InterruptedInline(SourceStructureEdgeKind),
    BreakBoundary(SourceControlBreakSide),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceControlInvariantReason {
    OwnerCounts,
    SourceJoin,
    ScopeState,
    RecordParent,
    PayloadCoverage,
    ScalarCoverage,
    EdgeIdentity,
    TraversalCompletion,
    EmissionCounts,
    ScopeFinalization,
    Completion,
}
#[derive(Debug)]
pub enum SourceControlInputErrorKind {
    OwnerMismatch,
    Limit(SourceControlLimit),
    Addressability,
    Allocation {
        storage: SourceControlStorage,
        source: TryReserveError,
    },
    Unsupported(SourceControlUnsupported),
    Invariant(SourceControlInvariantReason),
}
#[derive(Debug)]
pub struct SourceControlInputError {
    pub source: Option<usize>,
    pub record: Option<usize>,
    pub original: Option<usize>,
    pub member: Option<usize>,
    pub kind: SourceControlInputErrorKind,
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlScope<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    fn row(&self) -> &ScopeRow {
        &self.owner.scopes[self.index]
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlPayload<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    fn row(&self) -> &PayloadRow {
        &self.owner.payloads[self.index]
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlContribution<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    fn row(&self) -> &OriginalRow {
        &self.owner.originals[self.index]
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlToken<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    fn row(&self) -> &TokenRow {
        &self.owner.tokens[self.index]
    }
}
impl<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
    SourceControlSyntheticOrigin<'o, 't, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    fn parts(&self) -> (usize, usize, PlacementRow) {
        match self.owner.tokens[self.index].origin {
            OriginRow::Synthetic {
                occurrence,
                cause,
                placement,
            } => (occurrence, cause, placement),
            OriginRow::Literal { .. } => unreachable!("checked synthetic token"),
        }
    }
}
impl<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r> fmt::Debug
    for SourceControlInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceControlInput")
            .field("receipt", &self.receipt)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for SourceControlInputErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SourceControlInputErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation { source, .. } => Some(source),
            _ => None,
        }
    }
}
impl fmt::Display for SourceControlInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "source control input: {} [source={:?}, record={:?}, original={:?}, member={:?}]",
            self.kind, self.source, self.record, self.original, self.member
        )
    }
}
impl std::error::Error for SourceControlInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(&self.kind)
    }
}
