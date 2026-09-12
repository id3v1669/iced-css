//! Captured-source structural projection borrowing one exact whitespace owner.
//!
//! This preserves captured source ancestry, empty continuations and payloads.
//! It rejects unresolved inline-splitting gaps. It does not compute generated
//! content, anonymous styles, containing blocks, paragraphs, lines or geometry.
//! Generated content can change adjacency and require rebuilding this projection.
use super::source_participants::{SourceParticipant, SourceParticipants};
use super::source_whitespace::{SourceWhitespace, SourceWhitespaceEvent};
use std::{fmt, iter::FusedIterator, num::NonZeroUsize, ops::Range};
#[path = "source_structure_build.rs"]
mod build;
pub use build::build_source_structure;

#[derive(Debug, Clone, Copy)]
pub struct SourceStructureLimits {
    pub sources: NonZeroUsize,
    pub records: NonZeroUsize,
    /// Simultaneous owned vector payload, including scratch; excludes predecessors.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStructureKind {
    SourceBlock,
    SourceInline,
    AnonymousBlock,
    Payload,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStructureEdgeKind {
    OriginalBegin,
    OriginalEnd,
    BeforeInterruption,
    AfterInterruption,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStructureBoundarySide {
    Start,
    End,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStructureAnonymousReason {
    EncloseInlineRun,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStructureInvariantReason {
    SourceJoin,
    Stack,
    RecordState,
    Completion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceStructureGapEndpoint {
    pub source: usize,
    pub event: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStructureGapWitness {
    UnresolvedEmptyInlineGap { source: usize, event: usize },
    UnresolvedCollapsibleElementGap { source: usize, event: usize },
    UnresolvedLiteralBidiGap { source: usize, scalar: usize },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStructureError {
    Sources,
    Records,
    Work,
    Bytes,
    Allocation,
    Gap {
        left: SourceStructureGapEndpoint,
        right: SourceStructureGapEndpoint,
        events: Range<usize>,
        scalars: Range<usize>,
        witness: SourceStructureGapWitness,
    },
    Invariant {
        event: Option<usize>,
        reason: SourceStructureInvariantReason,
    },
}
impl fmt::Display for SourceStructureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source structure: {self:?}")
    }
}
impl std::error::Error for SourceStructureError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceStructureReceipt {
    pub sources: usize,
    pub events: usize,
    pub scalars: usize,
    pub contents: usize,
    pub blocks: usize,
    pub inline_sources: usize,
    pub continuations: usize,
    pub anonymous_blocks: usize,
    pub records: usize,
    pub gap_events: usize,
    pub gap_scalars: usize,
    pub work: usize,
    pub fact_capacity: usize,
    pub frame_capacity: usize,
    pub record_capacity: usize,
    pub frame_element_bytes: usize,
    pub record_element_bytes: usize,
    pub retained_bytes: usize,
    pub peak_retained_bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceIndex(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EventIndex(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordIndex(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EligibleOrdinal(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StackIndex(usize);
#[derive(Debug, Clone, Copy)]
enum InlineStart {
    OriginalBegin(EventIndex),
    AfterInterruption(EventIndex),
}
#[derive(Debug, Clone, Copy)]
enum InlineEnd {
    OriginalEnd(EventIndex),
    BeforeInterruption(EventIndex),
}
#[derive(Debug)]
struct Record {
    parent: Option<RecordIndex>,
    kind: RecordKind,
}
#[derive(Debug)]
enum RecordKind {
    SourceBlock {
        source: SourceIndex,
        begin: EventIndex,
        end: Option<EventIndex>,
    },
    SourceInline {
        source: SourceIndex,
        start: InlineStart,
        end: Option<InlineEnd>,
    },
    AnonymousBlock,
    Payload {
        content: EventIndex,
    },
}
#[must_use]
pub struct SourceStructure<'w, 'e, 'd, 's, 'b, 'r> {
    whitespace: &'w SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    records: Vec<Record>,
    receipt: SourceStructureReceipt,
}
impl fmt::Debug for SourceStructure<'_, '_, '_, '_, '_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceStructure")
            .field("receipt", &self.receipt)
            .finish_non_exhaustive()
    }
}
#[derive(Debug)]
#[must_use]
pub struct SourceStructureRecord<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceStructure<'w, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
#[derive(Debug)]
#[must_use]
pub struct SourceStructureEdge<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceStructure<'w, 'e, 'd, 's, 'b, 'r>,
    index: usize,
    side: SourceStructureBoundarySide,
}
#[derive(Debug)]
#[must_use]
pub struct SourceStructureAnonymousOrigin<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceStructure<'w, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
#[derive(Debug)]
#[must_use]
pub struct SourceStructureAnonymousBoundary<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceStructure<'w, 'e, 'd, 's, 'b, 'r>,
    index: usize,
    side: SourceStructureBoundarySide,
}
#[derive(Debug)]
#[must_use]
pub struct SourceStructureRecords<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceStructure<'w, 'e, 'd, 's, 'b, 'r>,
    remaining: Range<usize>,
}
#[derive(Debug)]
#[must_use]
pub struct SourceStructureChildren<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceStructure<'w, 'e, 'd, 's, 'b, 'r>,
    parent: usize,
    remaining: Range<usize>,
}
#[derive(Debug)]
#[must_use]
pub struct SourceStructureFragments<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceStructure<'w, 'e, 'd, 's, 'b, 'r>,
    source: usize,
    remaining: Range<usize>,
}
impl<'w, 'e, 'd, 's, 'b, 'r> SourceStructure<'w, 'e, 'd, 's, 'b, 'r> {
    pub fn whitespace(&self) -> &'w SourceWhitespace<'e, 'd, 's, 'b, 'r> {
        self.whitespace
    }
    pub fn receipt(&self) -> SourceStructureReceipt {
        self.receipt
    }
    pub fn source_count(&self) -> usize {
        self.receipt.sources
    }
    pub fn record_count(&self) -> usize {
        self.records.len()
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
    pub fn record(
        &self,
        index: usize,
    ) -> Option<SourceStructureRecord<'_, 'w, 'e, 'd, 's, 'b, 'r>> {
        self.records.get(index)?;
        Some(SourceStructureRecord { owner: self, index })
    }
    pub fn root(&self) -> Option<SourceStructureRecord<'_, 'w, 'e, 'd, 's, 'b, 'r>> {
        self.record(0)
    }
    pub fn records(&self) -> SourceStructureRecords<'_, 'w, 'e, 'd, 's, 'b, 'r> {
        SourceStructureRecords {
            owner: self,
            remaining: 0..self.records.len(),
        }
    }
    pub fn fragments(
        &self,
        source: usize,
    ) -> Option<SourceStructureFragments<'_, 'w, 'e, 'd, 's, 'b, 'r>> {
        let _ = SourceParticipants::new(self.whitespace.eligibility()).get(source)?;
        let end = if self.whitespace.eligibility().ordinal(source).is_some() {
            self.records.len()
        } else {
            0
        };
        Some(SourceStructureFragments {
            owner: self,
            source,
            remaining: 0..end,
        })
    }
}
impl<'o, 'w, 'e, 'd, 's, 'b, 'r> SourceStructureRecord<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    pub fn owner(&self) -> &'o SourceStructure<'w, 'e, 'd, 's, 'b, 'r> {
        self.owner
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn kind(&self) -> SourceStructureKind {
        match self.owner.records[self.index].kind {
            RecordKind::SourceBlock { .. } => SourceStructureKind::SourceBlock,
            RecordKind::SourceInline { .. } => SourceStructureKind::SourceInline,
            RecordKind::AnonymousBlock => SourceStructureKind::AnonymousBlock,
            RecordKind::Payload { .. } => SourceStructureKind::Payload,
        }
    }
    pub fn source_index(&self) -> Option<usize> {
        match self.owner.records[self.index].kind {
            RecordKind::SourceBlock { source, .. } | RecordKind::SourceInline { source, .. } => {
                Some(source.0)
            }
            RecordKind::Payload { content } => Some(
                self.owner
                    .whitespace
                    .event(content.0)
                    .expect("completed content")
                    .participant()
                    .source_index(),
            ),
            RecordKind::AnonymousBlock => None,
        }
    }
    pub fn participant(&self) -> Option<SourceParticipant<'e, 'd, 's, 'b, 'r>> {
        SourceParticipants::new(self.owner.whitespace.eligibility()).get(self.source_index()?)
    }
    pub fn formatting_parent(&self) -> Option<SourceStructureRecord<'o, 'w, 'e, 'd, 's, 'b, 'r>> {
        self.owner.record(self.owner.records[self.index].parent?.0)
    }
    /// Allocation-free scan; repeated caller scans are not producer work.
    pub fn children(&self) -> SourceStructureChildren<'o, 'w, 'e, 'd, 's, 'b, 'r> {
        SourceStructureChildren {
            owner: self.owner,
            parent: self.index,
            remaining: self.index + 1..self.owner.records.len(),
        }
    }
    pub fn start_edge(&self) -> Option<SourceStructureEdge<'o, 'w, 'e, 'd, 's, 'b, 'r>> {
        matches!(
            self.kind(),
            SourceStructureKind::SourceBlock | SourceStructureKind::SourceInline
        )
        .then_some(SourceStructureEdge {
            owner: self.owner,
            index: self.index,
            side: SourceStructureBoundarySide::Start,
        })
    }
    pub fn end_edge(&self) -> Option<SourceStructureEdge<'o, 'w, 'e, 'd, 's, 'b, 'r>> {
        matches!(
            self.kind(),
            SourceStructureKind::SourceBlock | SourceStructureKind::SourceInline
        )
        .then_some(SourceStructureEdge {
            owner: self.owner,
            index: self.index,
            side: SourceStructureBoundarySide::End,
        })
    }
    pub fn content_event(&self) -> Option<SourceWhitespaceEvent<'w, 'e, 'd, 's, 'b, 'r>> {
        if let RecordKind::Payload { content } = self.owner.records[self.index].kind {
            self.owner.whitespace.event(content.0)
        } else {
            None
        }
    }
    pub fn anonymous_origin(
        &self,
    ) -> Option<SourceStructureAnonymousOrigin<'o, 'w, 'e, 'd, 's, 'b, 'r>> {
        matches!(self.kind(), SourceStructureKind::AnonymousBlock).then_some(
            SourceStructureAnonymousOrigin {
                owner: self.owner,
                index: self.index,
            },
        )
    }
    pub fn is_same_record(
        &self,
        other: &SourceStructureRecord<'_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
    }
}
impl<'o, 'w, 'e, 'd, 's, 'b, 'r> SourceStructureEdge<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    pub fn record(&self) -> SourceStructureRecord<'o, 'w, 'e, 'd, 's, 'b, 'r> {
        self.owner.record(self.index).expect("bound record")
    }
    pub fn side(&self) -> SourceStructureBoundarySide {
        self.side
    }
    fn anchor(&self) -> (SourceStructureEdgeKind, EventIndex) {
        use SourceStructureBoundarySide::{End, Start};
        use SourceStructureEdgeKind as K;
        match (&self.owner.records[self.index].kind, self.side) {
            (RecordKind::SourceBlock { begin, .. }, Start) => (K::OriginalBegin, *begin),
            (RecordKind::SourceBlock { end, .. }, End) => {
                (K::OriginalEnd, end.expect("published block end"))
            }
            (
                RecordKind::SourceInline {
                    start: InlineStart::OriginalBegin(e),
                    ..
                },
                Start,
            ) => (K::OriginalBegin, *e),
            (
                RecordKind::SourceInline {
                    start: InlineStart::AfterInterruption(e),
                    ..
                },
                Start,
            ) => (K::AfterInterruption, *e),
            (RecordKind::SourceInline { end, .. }, End) => match end.expect("published inline end")
            {
                InlineEnd::OriginalEnd(e) => (K::OriginalEnd, e),
                InlineEnd::BeforeInterruption(e) => (K::BeforeInterruption, e),
            },
            _ => unreachable!("bound source edge"),
        }
    }
    pub fn kind(&self) -> SourceStructureEdgeKind {
        self.anchor().0
    }
    pub fn event(&self) -> SourceWhitespaceEvent<'w, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .whitespace
            .event(self.anchor().1.0)
            .expect("bound anchor")
    }
    pub fn cut(&self) -> usize {
        let e = self.anchor().1.0;
        match self.side {
            SourceStructureBoundarySide::Start => e + 1,
            SourceStructureBoundarySide::End => e,
        }
    }
    pub fn is_same_edge(&self, other: &SourceStructureEdge<'_, '_, '_, '_, '_, '_, '_>) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
            && self.side == other.side
    }
}
impl<'o, 'w, 'e, 'd, 's, 'b, 'r> SourceStructureAnonymousOrigin<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    pub fn record(&self) -> SourceStructureRecord<'o, 'w, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .record(self.index)
            .expect("bound anonymous record")
    }
    pub fn enclosing_block(&self) -> SourceStructureRecord<'o, 'w, 'e, 'd, 's, 'b, 'r> {
        self.record()
            .formatting_parent()
            .expect("anonymous block parent")
    }
    pub fn inheritance_source(&self) -> SourceParticipant<'e, 'd, 's, 'b, 'r> {
        self.enclosing_block().participant().expect("source block")
    }
    pub fn reason(&self) -> SourceStructureAnonymousReason {
        SourceStructureAnonymousReason::EncloseInlineRun
    }
    pub fn start_boundary(&self) -> SourceStructureAnonymousBoundary<'o, 'w, 'e, 'd, 's, 'b, 'r> {
        SourceStructureAnonymousBoundary {
            owner: self.owner,
            index: self.index,
            side: SourceStructureBoundarySide::Start,
        }
    }
    pub fn end_boundary(&self) -> SourceStructureAnonymousBoundary<'o, 'w, 'e, 'd, 's, 'b, 'r> {
        SourceStructureAnonymousBoundary {
            owner: self.owner,
            index: self.index,
            side: SourceStructureBoundarySide::End,
        }
    }
}
impl<'o, 'w, 'e, 'd, 's, 'b, 'r> SourceStructureAnonymousBoundary<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    pub fn record(&self) -> SourceStructureRecord<'o, 'w, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .record(self.index)
            .expect("bound anonymous record")
    }
    pub fn side(&self) -> SourceStructureBoundarySide {
        self.side
    }
    /// Scans direct children; support coordinates do not identify this boundary.
    pub fn source_support(&self) -> SourceStructureEdge<'o, 'w, 'e, 'd, 's, 'b, 'r> {
        let mut children = self.record().children();
        match self.side {
            SourceStructureBoundarySide::Start => children
                .next()
                .expect("nonempty wrapper")
                .start_edge()
                .expect("inline child"),
            SourceStructureBoundarySide::End => children
                .last()
                .expect("nonempty wrapper")
                .end_edge()
                .expect("inline child"),
        }
    }
    pub fn is_same_boundary(
        &self,
        other: &SourceStructureAnonymousBoundary<'_, '_, '_, '_, '_, '_, '_>,
    ) -> bool {
        std::ptr::eq(
            self.owner as *const _ as *const (),
            other.owner as *const _ as *const (),
        ) && self.index == other.index
            && self.side == other.side
    }
}
impl<'o, 'w, 'e, 'd, 's, 'b, 'r> Iterator for SourceStructureRecords<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    type Item = SourceStructureRecord<'o, 'w, 'e, 'd, 's, 'b, 'r>;
    fn next(&mut self) -> Option<Self::Item> {
        self.owner.record(self.remaining.next()?)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.remaining.size_hint()
    }
}
impl FusedIterator for SourceStructureRecords<'_, '_, '_, '_, '_, '_, '_> {}
impl<'o, 'w, 'e, 'd, 's, 'b, 'r> Iterator for SourceStructureChildren<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    type Item = SourceStructureRecord<'o, 'w, 'e, 'd, 's, 'b, 'r>;
    fn next(&mut self) -> Option<Self::Item> {
        for index in self.remaining.by_ref() {
            if self.owner.records[index].parent == Some(RecordIndex(self.parent)) {
                return self.owner.record(index);
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.remaining.len()))
    }
}
impl FusedIterator for SourceStructureChildren<'_, '_, '_, '_, '_, '_, '_> {}
impl<'o, 'w, 'e, 'd, 's, 'b, 'r> Iterator for SourceStructureFragments<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    type Item = SourceStructureRecord<'o, 'w, 'e, 'd, 's, 'b, 'r>;
    fn next(&mut self) -> Option<Self::Item> {
        for index in self.remaining.by_ref() {
            if matches!(self.owner.records[index].kind, RecordKind::SourceBlock { source, .. } | RecordKind::SourceInline { source, .. } if source.0 == self.source)
            {
                return self.owner.record(index);
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.remaining.len()))
    }
}
impl FusedIterator for SourceStructureFragments<'_, '_, '_, '_, '_, '_, '_> {}
impl ExactSizeIterator for SourceStructureRecords<'_, '_, '_, '_, '_, '_, '_> {}
