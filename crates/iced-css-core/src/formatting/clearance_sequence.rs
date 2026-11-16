//! Bounded vertical clearance ownership for one parent containing B1, one float,
//! and one or more following normal siblings.
//!
//! Admission validates the complete source graph and every supplied profile before
//! reserving output storage or publishing geometry. The returned owner borrows only
//! the source slice; copied placement records do not retain the input facts slice.

pub(super) mod chain;

use chain::{ChainState, GroupSpan};
pub use chain::{SequenceMarginGroupView, SequenceMarginRunView, SequenceNormalView};

use super::{
    clearance_owner::{
        BlockVertical, ChildContext, ChildFacts, ClearanceOwnerError, FirstFloatInput, KernelError,
        MarginMember, OuterFacts, ParentFacts, place_first_float, validate_outer,
        validate_sequence_child, validate_sequence_float_extents, validate_sequence_float_width,
        validate_sequence_parent, vertical_decoration,
    },
    floats::{Band, PlacedFloat},
};
use crate::source::SourceNode;
use std::{collections::TryReserveError, mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct ClearanceSequenceInput<'s, 'f> {
    pub outer: OuterFacts<'s>,
    pub parent: ParentFacts<'s>,
    /// Exact projected children B1, F, and tail in source order.
    pub children: &'f [ChildFacts<'s>],
    /// Explicit finite nonnegative content width; the core forms the margin-box
    /// width from it and the float's used horizontal margins. Finite oversized
    /// placement is supported.
    pub float_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearanceSequenceLimits {
    pub sources: NonZeroUsize,
    pub work: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearanceSequenceReceipt {
    pub nodes: usize,
    pub tail: usize,
    pub work: usize,
    pub record_capacity: usize,
    pub member_capacity: usize,
    pub retained_bytes: usize,
}

#[derive(Debug)]
pub enum ClearanceSequenceError {
    Sources,
    Facts,
    Work,
    Bytes,
    Allocation(TryReserveError),
    Profile(ClearanceOwnerError),
    Placement {
        source: usize,
        error: ClearanceOwnerError,
    },
}

impl std::fmt::Display for ClearanceSequenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "clearance sequence: {self:?}")
    }
}

impl std::error::Error for ClearanceSequenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::Profile(error) | Self::Placement { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct SequenceNormalPlacement {
    source_index: usize,
    vertical: BlockVertical,
    content_vertical: BlockVertical,
    padding_vertical: BlockVertical,
    hypothetical_top: f32,
    zero_clearance_top: Option<f32>,
    clearance: Option<f32>,
    hypothetical_group: GroupSpan,
    actual_groups: [GroupSpan; 2],
    actual_len: usize,
}

impl SequenceNormalPlacement {
    pub fn source_index(&self) -> usize {
        self.source_index
    }
    pub fn vertical(&self) -> BlockVertical {
        self.vertical
    }
    /// Copied content span; `vertical()` remains the border span.
    pub fn content_vertical(&self) -> BlockVertical {
        self.content_vertical
    }
    /// Copied padding span, between the border span and the content span: on a side
    /// without border its edge is the border edge, on a side without padding the
    /// content edge.
    pub fn padding_vertical(&self) -> BlockVertical {
        self.padding_vertical
    }
    pub fn hypothetical_top(&self) -> f32 {
        self.hypothetical_top
    }
    pub fn zero_clearance_top(&self) -> Option<f32> {
        self.zero_clearance_top
    }
    pub fn clearance(&self) -> Option<f32> {
        self.clearance
    }
    pub fn barrier_source_index(&self) -> Option<usize> {
        self.clearance.map(|_| self.source_index)
    }
}

#[derive(Debug)]
pub struct ClearanceSequenceOwner<'s> {
    sources: &'s [SourceNode],
    before: BlockVertical,
    before_content: BlockVertical,
    before_padding: BlockVertical,
    float_border: Band,
    float_border_vertical: BlockVertical,
    placed_float: PlacedFloat,
    float_content: Band,
    float_content_vertical: BlockVertical,
    float_padding: Band,
    float_padding_vertical: BlockVertical,
    tail: Vec<SequenceNormalPlacement>,
    members: Vec<MarginMember>,
    terminal: GroupSpan,
    receipt: ClearanceSequenceReceipt,
}

impl<'s> ClearanceSequenceOwner<'s> {
    pub fn sources(&self) -> &'s [SourceNode] {
        self.sources
    }
    pub fn source(&self, index: usize) -> Option<&'s SourceNode> {
        self.sources.get(index)
    }
    pub fn before(&self) -> BlockVertical {
        self.before
    }
    /// Copied B1 content span; `before()` remains the border span.
    pub fn before_content(&self) -> BlockVertical {
        self.before_content
    }
    /// Copied B1 padding span: on a side without border its edge is the border edge,
    /// on a side without padding the content edge.
    pub fn before_padding(&self) -> BlockVertical {
        self.before_padding
    }
    /// Float border band, independent of its placed margin band.
    pub fn float_border(&self) -> Band {
        self.float_border
    }
    /// Float border span, independent of its placed margin span.
    pub fn float_border_vertical(&self) -> BlockVertical {
        self.float_border_vertical
    }
    pub fn placed_float(&self) -> PlacedFloat {
        self.placed_float
    }
    /// Copied float content band, independently published from its margin box.
    /// With zero padding and border on this axis, the predecessor margin-offset
    /// near/far path is retained; if the used margins are also zero, copy the placer
    /// band. Otherwise publish the side-selected near border edge, near content edge
    /// (border and padding summed once), far content edge, far border edge, then the
    /// two padding edges. Each stage rounds once. Signed margins may place
    /// border/content outside the margin box; no span is clamped. Nonnegative
    /// padding and borders keep border around padding around content.
    pub fn float_content(&self) -> Band {
        self.float_content
    }
    /// Copied float content span, independently published from its margin box.
    /// With zero vertical padding and border, retain the predecessor margin-offset
    /// stages; if both used vertical margins are also zero, copy the placer span.
    /// Otherwise publish border top, content top, content bottom, border bottom,
    /// then padding top and padding bottom. Signed margins may move either span
    /// outside the margin box; nonnegative padding and borders keep border around
    /// padding around content. No clamp.
    pub fn float_content_vertical(&self) -> BlockVertical {
        self.float_content_vertical
    }
    /// Copied float padding band: on a side without border its edge is the border
    /// edge, on a side without padding the content edge.
    pub fn float_padding(&self) -> Band {
        self.float_padding
    }
    /// Copied float padding span, the vertical analogue of `float_padding()`.
    pub fn float_padding_vertical(&self) -> BlockVertical {
        self.float_padding_vertical
    }
    pub fn tail(&self) -> &[SequenceNormalPlacement] {
        &self.tail
    }
    /// Borrow a record together with the member arena of this owner only.
    pub fn tail_view(&self, index: usize) -> Option<SequenceNormalView<'_>> {
        self.tail
            .get(index)
            .map(|record| SequenceNormalView::new(record, &self.members))
    }
    /// Visit owner-bound records in source order without allocating.
    pub fn tail_views(
        &self,
    ) -> impl ExactSizeIterator<Item = SequenceNormalView<'_>> + DoubleEndedIterator + '_ {
        self.tail
            .iter()
            .map(|record| SequenceNormalView::new(record, &self.members))
    }
    /// Final adjoining members/extrema. No new numeric edge is published here.
    /// The descriptor is inline owner storage, outside the retained-allocation limit.
    pub fn terminal_run(&self) -> SequenceMarginRunView<'_> {
        SequenceMarginRunView::new(self.terminal, &self.members)
    }
    pub fn receipt(&self) -> ClearanceSequenceReceipt {
        self.receipt
    }
}

/// Validate and solve the complete source-owned sibling sequence.
///
/// Source/count/work checks precede the complete graph and profile scan. All
/// profile checks, including the last child and float-width domain, precede the
/// fallible record/member allocations. Record capacity and its byte floor are checked
/// before reserving members; both actual capacities are checked before geometry.
/// Sequence normals admit finite signed absolute or Auto margins on every edge.
/// Floats admit computed Auto or finite signed
/// absolute margins on all four edges (Auto is used zero) while both exact outer
/// extents are nonnegative; otherwise, after every child's profile and the float-width
/// domain, the profile rejects the float with `NegativeFloatOuterExtent`. The float is
/// placed by its margin box, formed from content extents, borders, padding and used
/// margins. Content, padding and border bands/spans are published separately;
/// finite nonnegative absolute padding and border widths are admitted on all
/// Sequence children (CBR). P needs no top/bottom isolation: it is a new block
/// formatting context root (CSS2.2 8.3.1). B1 and the float admit any computed
/// `clear`. No float precedes either
/// of them in the parent's new block formatting context, so CSS2.2 §9.5.2 and float
/// constraint #10 select nothing; the value is not read after admission. Every
/// Sequence normal (B1 or a tail) may instead carry `ChildContext::OtherNewBfc`, a
/// normal-flow block that establishes a new block formatting context. Right after
/// that normal's placement, `NewBfcFloatContact` rejects intersection of its nonempty
/// published border interval with F's nonempty margin interval, tagged with its
/// source. Contact and empty intervals pass (Q1), except that (G6a, Q-3) a zero-height
/// new-BFC border box strictly inside F's margin interval is `NewBfcFloatContact`; signed
/// margins make rejection reachable. Childless F admits computed Auto height with
/// positive-zero used content height; a normal keeps a positive explicit height, or +0
/// when its own top and bottom margins are not adjoining (G6a, CSS2.2 8.3.1), and Legacy
/// admission is unchanged.
/// CTB/CTT (the user's U1=A; CSS2.2 8.3.1, 9.5.1): an ordinary normal with used height
/// +0 or -0 and no vertical border or padding admits finite signed normal margins (S4).
/// A NaN or negative-infinity normal margin retains the earlier eligibility refusal
/// `NonpositiveBlockHeight`; positive infinity reaches source-ordered validation.
/// F's margins do not enter this eligibility scan. F's hypothetical top is
/// B1's top border edge when B1 collapses through. Consecutive through tails extend
/// the same adjoining member run. Clearance splits the incoming run from the clearing
/// top margin; later through tails use the published cleared border and clearing top
/// as a two-scalar reference, with one final position fold (9.5.2). A positive or
/// separated normal ends the run. Terminal membership has no numeric publication.
/// The facade passes used heights after `min-height`/`max-height` (MMX, CSS2.2 10.7):
/// an F whose constrained Auto height is positive, and a normal whose computed Auto or
/// zero height is raised by a positive `min-height`, arrive as explicit heights.
/// P and every child may be relatively positioned (REL, CSS2.2 9.4.3); the owner
/// publishes their normal-flow geometry, which the offsets do not change, and the
/// outer source stays static.
/// NZS admits an explicit NonAdjoiningZero whose own margins stay separate;
/// S4 does not change that provenance or the Legacy profile.
/// Work `1062 + 12*N + N*(N-1)/2 + 333*(N-4) + 76*(N-2) + 224*(N-3)` is prepaid,
/// including constant-size typed margin validation and used-value service for each
/// child and the float's outer-extent sign check, outer-extent, content-band and
/// vertical content stages, every child's border terms and padding publications, and
/// the widened display test of P and every child. CTT prepays 311 fixed and 262
/// per-tail policy units for the arena, both capacity checks and chain descriptors;
/// all earlier charges remain, without credit. These are not measured instructions.
///
/// # Errors
/// Returns bounded admission/allocation failures, complete-profile failures, or a
/// placement failure tagged with the actual source whose stage failed.
pub fn build_clearance_sequence<'s>(
    sources: &'s [SourceNode],
    input: ClearanceSequenceInput<'s, '_>,
    limits: ClearanceSequenceLimits,
) -> Result<ClearanceSequenceOwner<'s>, ClearanceSequenceError> {
    build_with_reserves(
        sources,
        input,
        limits,
        |records, requested| records.try_reserve_exact(requested),
        |members, requested| members.try_reserve_exact(requested),
    )
}

fn build_with_reserves<'s>(
    sources: &'s [SourceNode],
    input: ClearanceSequenceInput<'s, '_>,
    limits: ClearanceSequenceLimits,
    reserve_records: impl FnOnce(
        &mut Vec<SequenceNormalPlacement>,
        usize,
    ) -> Result<(), TryReserveError>,
    reserve_members: impl FnOnce(&mut Vec<MarginMember>, usize) -> Result<(), TryReserveError>,
) -> Result<ClearanceSequenceOwner<'s>, ClearanceSequenceError> {
    let nodes = sources.len();
    if nodes < 5 || nodes > limits.sources.get() {
        return Err(ClearanceSequenceError::Sources);
    }
    if input.children.len() != nodes - 2 {
        return Err(ClearanceSequenceError::Facts);
    }
    let tail_count = nodes - 4;
    let work = sequence_work(nodes, tail_count).ok_or(ClearanceSequenceError::Work)?;
    if work > limits.work.get() {
        return Err(ClearanceSequenceError::Work);
    }

    for (source, node) in sources.iter().enumerate() {
        let expected = match source {
            0 => 1,
            1 => nodes - 2,
            _ => 0,
        };
        if node.children.len() != expected {
            return Err(profile(ClearanceOwnerError::ChildCount { source }));
        }
    }
    if !same_source(input.outer.source, &sources[0]) {
        return Err(profile(ClearanceOwnerError::SourcePointer { source: 0 }));
    }
    if !same_source(input.parent.source, &sources[1]) {
        return Err(profile(ClearanceOwnerError::SourcePointer { source: 1 }));
    }
    for (offset, facts) in input.children.iter().enumerate() {
        let source = offset + 2;
        if !same_source(facts.source, &sources[source]) {
            return Err(profile(ClearanceOwnerError::SourcePointer { source }));
        }
    }
    for first in 0..nodes {
        for second in first + 1..nodes {
            if sources[first].instance == sources[second].instance {
                return Err(profile(ClearanceOwnerError::DuplicateInstance {
                    first,
                    second,
                }));
            }
        }
    }
    for (source, node) in sources.iter().enumerate() {
        let expected = match source {
            0 => None,
            1 => Some(sources[0].instance),
            _ => Some(sources[1].instance),
        };
        if node.parent != expected {
            return Err(profile(ClearanceOwnerError::ParentMembership { source }));
        }
    }
    if sources[0].children[0] != sources[1].instance {
        return Err(profile(ClearanceOwnerError::ChildMembership {
            source: 0,
            slot: 0,
        }));
    }
    for (slot, child) in sources[1].children.iter().enumerate() {
        if *child != sources[slot + 2].instance {
            return Err(profile(ClearanceOwnerError::ChildMembership {
                source: 1,
                slot,
            }));
        }
    }

    validate_outer(input.outer, 0).map_err(profile)?;
    validate_sequence_parent(input.parent, 1).map_err(profile)?;
    // S4: admit finite signed normals without changing invalid-input precedence.
    // NaN and negative infinity retain the old eligibility refusal; positive
    // infinity reaches source-ordered finite validation. F remains excluded.
    let through_eligible = input.children[..1]
        .iter()
        .chain(&input.children[2..])
        .all(|facts| {
            let top = facts.margin.used_top();
            let bottom = facts.margin.used_bottom();
            (top >= 0. || top.is_finite()) && (bottom >= 0. || bottom.is_finite())
        });
    let mut checked_float_height = None;
    for (offset, facts) in input.children.iter().copied().enumerate() {
        let source = offset + 2;
        let height =
            validate_sequence_child(facts, source, source == 3, source != 3 && through_eligible)
                .map_err(profile)?;
        if source == 3 {
            checked_float_height = Some(height);
        }
    }
    let float_height =
        checked_float_height.expect("validated Sequence shape contains source-3 float");
    validate_sequence_float_width(input.float_width, 3).map_err(profile)?;
    validate_sequence_float_extents(input.children[1], input.float_width, float_height, 3)
        .map_err(profile)?;

    let member_count = member_slots(nodes).ok_or(ClearanceSequenceError::Bytes)?;
    let requested_bytes =
        retained_bytes(tail_count, member_count).ok_or(ClearanceSequenceError::Bytes)?;
    if requested_bytes > limits.retained_bytes.get() {
        return Err(ClearanceSequenceError::Bytes);
    }
    let mut tail = Vec::new();
    reserve_records(&mut tail, tail_count).map_err(ClearanceSequenceError::Allocation)?;
    if tail.capacity() < tail_count {
        return Err(ClearanceSequenceError::Bytes);
    }
    let first_actual =
        retained_bytes(tail.capacity(), member_count).ok_or(ClearanceSequenceError::Bytes)?;
    if first_actual > limits.retained_bytes.get() {
        return Err(ClearanceSequenceError::Bytes);
    }
    let mut members = Vec::new();
    reserve_members(&mut members, member_count).map_err(ClearanceSequenceError::Allocation)?;
    if members.capacity() < member_count {
        return Err(ClearanceSequenceError::Bytes);
    }
    let actual_bytes =
        retained_bytes(tail.capacity(), members.capacity()).ok_or(ClearanceSequenceError::Bytes)?;
    if actual_bytes > limits.retained_bytes.get() {
        return Err(ClearanceSequenceError::Bytes);
    }

    let before_height = explicit_height(input.children[0]);
    // CTB/CTT: only the adjoining height assertion participates in a through run.
    // NZS zeros keep their own top and bottom margins separate.
    let collapses = collapses_through(input.children[0]);
    let first = place_first_float(FirstFloatInput {
        parent: input.parent,
        before: input.children[0],
        before_height,
        before_source: 2,
        float: input.children[1],
        float_height,
        float_width: input.float_width,
        float_source: 3,
        before_collapses_through: collapses,
    })
    .map_err(placement)?;
    // CSS2.2 §9.5, Q1: reject intersection of nonempty published intervals.
    // This bounded core has no normal horizontal geometry for avoidance search.
    // Signed margins may put F above B1; rounded-empty B1 and F remain admitted. G6a: a
    // zero-height border box strictly inside F's margin interval is rejected too;
    // rounded-empty boxes with a positive height or a vertical border or padding stay
    // admitted.
    if input.children[0].context == ChildContext::OtherNewBfc
        && new_bfc_intersects_float(input.children[0], first.before, first.placed_float)
    {
        return Err(ClearanceSequenceError::Placement {
            source: 2,
            error: ClearanceOwnerError::NewBfcFloatContact,
        });
    }
    let mut chain = ChainState::first(
        &mut members,
        input.children[0],
        first.before.bottom,
        input.parent.content_top,
        collapses,
    )
    .map_err(placement)?;
    for (offset, facts) in input.children[2..].iter().copied().enumerate() {
        let source = offset + 4;
        let through = collapses_through(facts);
        let next = chain
            .place_next(
                &mut members,
                facts,
                explicit_height(facts),
                source,
                through,
                first.placed_float,
            )
            .map_err(placement)?;
        // CSS2.2 §9.5, Q1: a new-BFC tail obeys the B1 rule, checked right after its own
        // placement so that the first failing source in source order is reported.
        if facts.context == ChildContext::OtherNewBfc
            && new_bfc_intersects_float(facts, next.vertical, first.placed_float)
        {
            return Err(ClearanceSequenceError::Placement {
                source,
                error: ClearanceOwnerError::NewBfcFloatContact,
            });
        }
        tail.push(next);
    }
    Ok(ClearanceSequenceOwner {
        sources,
        before: first.before,
        before_content: first.before_content,
        before_padding: first.before_padding,
        float_border: first.float_border,
        float_border_vertical: first.float_border_vertical,
        placed_float: first.placed_float,
        float_content: first.float_content,
        float_content_vertical: first.float_content_vertical,
        float_padding: first.float_padding,
        float_padding_vertical: first.float_padding_vertical,
        receipt: ClearanceSequenceReceipt {
            nodes,
            tail: tail_count,
            work,
            record_capacity: tail.capacity(),
            member_capacity: members.capacity(),
            retained_bytes: actual_bytes,
        },
        tail,
        members,
        terminal: chain.terminal(),
    })
}

pub(super) fn same_source(actual: &SourceNode, expected: &SourceNode) -> bool {
    std::ptr::eq(actual, expected) && actual.instance == expected.instance
}

fn profile(error: ClearanceOwnerError) -> ClearanceSequenceError {
    ClearanceSequenceError::Profile(error)
}

fn placement(failure: KernelError) -> ClearanceSequenceError {
    ClearanceSequenceError::Placement {
        source: failure.source,
        error: failure.error,
    }
}

/// Both published intervals are nonempty and strictly intersect (CSS2.2 §9.5, Q1).
/// Or (G6a, Q-3) the normal's border box has zero height and its position lies strictly
/// inside F's margin interval; contact with either edge of that interval passes (Q1).
pub(super) fn new_bfc_intersects_float(
    facts: ChildFacts<'_>,
    border: BlockVertical,
    float: PlacedFloat,
) -> bool {
    (border.top < border.bottom || zero_border_box(facts))
        && float.top < float.bottom
        && float.top < border.bottom
        && border.top < float.bottom
}

/// CTB/CTT adjoining provenance is independent of geometric zero height (NZS).
pub(super) fn collapses_through(facts: ChildFacts<'_>) -> bool {
    facts.context == ChildContext::OrdinaryNonBfc
        && matches!(
            facts.height,
            super::clearance_owner::OwnerHeight::Explicit(_)
        )
        && zero_border_box(facts)
}

/// G6a (Q-3): a zero-height border box (CSS2.2 8.1): zero used height and no top or
/// bottom border or padding. A positive height rounded to an empty interval is not one
/// (PR-5). NZS non-adjoining zeros still qualify for this geometric contact predicate.
fn zero_border_box(facts: ChildFacts<'_>) -> bool {
    explicit_height(facts) == 0.0 && !vertical_decoration(facts)
}

pub(super) fn explicit_height(facts: ChildFacts<'_>) -> f32 {
    match facts.height {
        super::clearance_owner::OwnerHeight::Explicit(height)
        | super::clearance_owner::OwnerHeight::NonAdjoiningZero(height) => height,
        _ => unreachable!("complete profile validation admitted only used normal heights"),
    }
}

pub(super) fn sequence_work(nodes: usize, tail: usize) -> Option<usize> {
    // 272 fixed actions: sixty admission/kernel and vertical used-value actions; 68
    // for the float's horizontal margins (policy restriction 4, two float used
    // getters 8, identity test 4, three-term outer sum 16, near content edge 16, far
    // content edge 16, band/kernel/owner transfers 4); 62 for its vertical margins
    // (two used getters 8, identity test 4, three-term outer height 16, content top
    // 16, content bottom 14, span/kernel/owner transfers 4); and 20 for a new-BFC B1
    // (the leading-context exception, at most two evaluations of three comparisons
    // 12, and the contact check: context test 2, two published reads 2, strict
    // comparison 2, typed error 2); and 62 for its signed margins (the Sequence
    // exemption from the float's sign scan 4, the call and three argument transfers 4,
    // four used getters 16, two exact three-term outer extents 32, two sign comparisons
    // 4, typed error 2), all charged unconditionally. Then eight bounded
    // per-node actions, every duplicate comparison, 24 arithmetic/group and vertical
    // used-value actions per tail, and 45 constant-size typed margin actions per child.
    // Padding adds 256 fixed actions, 64 per normal and eight per child, prepaid
    // on zero-padding paths too. This covers widened exact folds, four additional
    // F publications, two additional normal publications and copied border/content
    // observations. No credit is taken for the removed decoration rejection.
    let children = nodes.checked_sub(2)?;
    let linear = nodes.checked_mul(8)?;
    let pairs = nodes.checked_mul(nodes.checked_sub(1)?)?.checked_div(2)?;
    let kernels = tail.checked_mul(24)?;
    let margin_actions = children.checked_mul(45)?;
    272usize
        .checked_add(linear)?
        .checked_add(pairs)?
        .checked_add(kernels)?
        .checked_add(margin_actions)?
        .checked_add(256)?
        .checked_add(nodes.checked_sub(3)?.checked_mul(64)?)?
        .checked_add(children.checked_mul(8)?)?
        // CFH: 4 category/policy/role actions plus 4 returned-scalar/role actions
        // per child; 8 fixed Option and two downstream scalar-transfer actions.
        .checked_add(children.checked_mul(8)?)?
        .checked_add(8)?
        // SNV: 24 unconditional actions for the wider interval guard and admission.
        .checked_add(24)?
        // TNB: 30 per tail, charged on every path: the widened context exception 6
        // (policy, role and context comparisons) and the per-tail guard 24 (context test
        // 2, predicate call and return 4, four published reads 4, four comparisons 8,
        // conjunction chain 4, typed error 2). The fixed B1 charges stay without credit.
        .checked_add(tail.checked_mul(30)?)?
        // CBR: 146 fixed actions and 56 per normal, charged on every path, without credit
        // for the removed decoration and isolation rejections. Per normal: the border
        // argument and two stage names 3, two border reads 2, two fast-path comparisons
        // 4, two widened folds 8, two padding zero tests 4, two padding publications 32,
        // and the padding span's return, kernel and record transfers 3. Fixed: per float
        // axis 61 (two border reads 2, two undecorated comparisons 4, two widened
        // outer-extent terms 8, widened near-content and far-border folds 8, two padding
        // zero tests 4, two padding publications 32, and the band or span's return,
        // kernel and owner transfers 3), the float border read 1, the extents check's
        // four widened terms 16 and four border reads 4, and P's policy argument 1 and
        // comparison 2.
        .checked_add(146)?
        .checked_add(nodes.checked_sub(3)?.checked_mul(56)?)?
        // G6a: 62 per normal, charged on every path: the admission predicate 30 (zero
        // test 2, policy comparison 2, context comparison 2, the decoration test 20 (call
        // and return 4, four reads 4, four comparisons 8, disjunction chain 4), two
        // connectives 4) and the widened contact guard 32 (facts transfer 1, call and
        // return 4, height read and zero test 3, the decoration test 20, negation and
        // conjunction 2, the widened disjunction 2).
        .checked_add(nodes.checked_sub(3)?.checked_mul(62)?)?
        // REL: 4 per P and child, charged on every path: the policy flag test 2 and the
        // relative position comparison 2 of the widened display test.
        .checked_add(nodes.checked_sub(1)?.checked_mul(4)?)?
        // CTB (the user's U1=A; CT-15): conservative prepaid policy charges, no credit.
        // Fixed 49: scan setup 3, collapse decision 26, float top selection 3,
        // initial previous-normal record 5, leading member 4, barrier extra member 4,
        // and initial top-margin used getter 4. Per child 3: role test and flag argument.
        // Per normal 18: sign scan 14 and admission clause 4. Per tail 17: leading
        // match 2, collapsed read 2, top-margin reset 1, three widened-group units of 4.
        // Each group unit prepays extraction, padding and extra extrema handling,
        // including unused slots and absent clearance; these are not measured counts.
        .checked_add(49)?
        .checked_add(nodes.checked_sub(2)?.checked_mul(3)?)?
        .checked_add(nodes.checked_sub(3)?.checked_mul(18)?)?
        .checked_add(tail.checked_mul(17)?)?
        // CTT: conservative source-bound policy additions, no old charge removed.
        // Fixed311: member count16, three complete byte helpers78, their gates30,
        // second reserve19, complete through-B1 seed158, owner/receipt transfers10.
        // Per tail262: through decision/call37, top append/span70, local actual
        // descriptors61, extra Exact terms10, bottom append/reference updates81,
        // source mapping3. Existing final record output transfers remain prepaid.
        .checked_add(311)?
        .checked_add(tail.checked_mul(262)?)?
        // NZS-D5: twelve per child for category/policy/zero validation and twelve
        // per normal for separated membership, through provenance and scalar dispatch.
        // These conservative policy charges retain all CT15/CTT units without credit.
        // NZS-FLIP-CORE-WORK: the FLIP stage omits only these new charges.
        .checked_add(children.checked_mul(12)?)?
        .checked_add(nodes.checked_sub(3)?.checked_mul(12)?)?
        // S4: twelve additional units per normal, charged on every path: two
        // cached scalar bindings (2), two finite predicate call/return/tests (6),
        // and two disjunctions (4). All prior charges remain without credit.
        // S4-FLIP-WORK: omit only this addition in the finite accounting stage.
        .checked_add(nodes.checked_sub(3)?.checked_mul(12)?)
}

/// Requested retained allocations for a Sequence source count, without allocating.
/// Inline owner storage (including the terminal span) is not a retained allocation.
/// Returns None for fewer than five sources or count/size/isize overflow.
pub fn sequence_requested_retained_bytes(nodes: usize) -> Option<usize> {
    let tail = nodes.checked_sub(4)?;
    if tail == 0 {
        return None;
    }
    retained_bytes(tail, member_slots(nodes)?)
}

fn member_slots(nodes: usize) -> Option<usize> {
    nodes.checked_sub(3)?.checked_mul(2)
}

fn retained_bytes(record_capacity: usize, member_capacity: usize) -> Option<usize> {
    let records = record_capacity.checked_mul(size_of::<SequenceNormalPlacement>())?;
    let members = member_capacity.checked_mul(size_of::<MarginMember>())?;
    let bytes = records.checked_add(members)?;
    (bytes <= isize::MAX as usize).then_some(bytes)
}

#[cfg(test)]
fn build_with_reserve<'s>(
    sources: &'s [SourceNode],
    input: ClearanceSequenceInput<'s, '_>,
    limits: ClearanceSequenceLimits,
    reserve_records: impl FnOnce(
        &mut Vec<SequenceNormalPlacement>,
        usize,
    ) -> Result<(), TryReserveError>,
) -> Result<ClearanceSequenceOwner<'s>, ClearanceSequenceError> {
    build_with_reserves(
        sources,
        input,
        limits,
        reserve_records,
        |members, requested| members.try_reserve_exact(requested),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        formatting::{
            block::AutoLength,
            clearance::ClearSide,
            clearance_owner::{
                ChildContext, OwnerContent, OwnerEdges, OwnerGenerated, OwnerHeight,
                OwnerProfileReason, ParentBfc,
            },
            display::{Display, DisplayResolution, Float, Flow, Position},
            floats::Band,
        },
        identity::IdAllocator,
        source::Metadata,
    };
    use std::num::NonZeroU64;

    fn fixture() -> (Vec<SourceNode>, ClearanceSequenceLimits) {
        let mut ids = IdAllocator::new(NonZeroU64::new(71).unwrap());
        let mut sources: Vec<_> = (0..5)
            .map(|_| SourceNode {
                instance: ids.allocate().unwrap(),
                parent: None,
                children: vec![],
                metadata: Metadata::default(),
            })
            .collect();
        let parent = sources[1].instance;
        sources[0].children.push(parent);
        sources[1].parent = Some(sources[0].instance);
        sources[1].children = sources[2..].iter().map(|node| node.instance).collect();
        for source in &mut sources[2..] {
            source.parent = Some(parent);
        }
        let limits = ClearanceSequenceLimits {
            sources: NonZeroUsize::new(5).unwrap(),
            work: NonZeroUsize::new(2141).unwrap(),
            retained_bytes: NonZeroUsize::new(sequence_requested_retained_bytes(5).unwrap())
                .unwrap(),
        };
        (sources, limits)
    }

    fn display() -> DisplayResolution {
        DisplayResolution {
            display: Display::Block,
            position: Position::Static,
            float: Float::None,
            flow: Flow::Normal,
            flex: false,
            grid: false,
            sticky: false,
        }
    }

    fn edges() -> OwnerEdges {
        OwnerEdges {
            top: 0.,
            right: 0.,
            bottom: 0.,
            left: 0.,
        }
    }

    fn input(sources: &[SourceNode]) -> (OuterFacts<'_>, ParentFacts<'_>, Vec<ChildFacts<'_>>) {
        let outer = OuterFacts {
            source: &sources[0],
            display: display(),
            content: OwnerContent::NoOwnContentNonReplaced,
            generated: OwnerGenerated::None,
        };
        let parent = ParentFacts {
            source: &sources[1],
            display: display(),
            content: OwnerContent::NoOwnContentNonReplaced,
            generated: OwnerGenerated::None,
            bfc: ParentBfc::NewWithEmptyPrefix,
            content_top: 0.,
            containing: Band {
                left: 0.,
                right: 100.,
            },
            border: OwnerEdges {
                top: 1.,
                bottom: 1.,
                ..edges()
            },
            padding: edges(),
        };
        let mut children: Vec<_> = sources[2..]
            .iter()
            .map(|source| ChildFacts {
                source,
                display: display(),
                context: ChildContext::OrdinaryNonBfc,
                content: OwnerContent::NoOwnContentNonReplaced,
                generated: OwnerGenerated::None,
                height: OwnerHeight::Explicit(1.),
                margin: edges().into(),
                border: edges(),
                padding: edges(),
                clear: ClearSide::None,
            })
            .collect();
        children[1].display.float = Float::Left;
        children[1].display.flow = Flow::Floating;
        children[1].context = ChildContext::FloatBfc;
        children[2].clear = ClearSide::Both;
        (outer, parent, children)
    }

    #[test]
    fn genuine_allocation_error_is_preserved_as_the_source() {
        let (sources, limits) = fixture();
        let (outer, mut parent, mut children) = input(&sources);
        parent.content_top = f32::MAX;
        children[0].margin.top = AutoLength::Value(f32::MAX);
        children[0].margin.left = AutoLength::Auto;
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Auto;
        let expected = Vec::<u8>::new()
            .try_reserve_exact(usize::MAX)
            .unwrap_err()
            .to_string();
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 140.,
            },
            limits,
            |records, _| records.try_reserve_exact(usize::MAX),
        )
        .unwrap_err();
        let ClearanceSequenceError::Allocation(actual) = error else {
            panic!("expected allocation error, got {error:?}");
        };
        assert_eq!(actual.to_string(), expected);
    }

    #[test]
    fn requested_floor_precedes_reserve_and_spare_capacity_is_charged() {
        let (sources, mut limits) = fixture();
        let (outer, parent, mut children) = input(&sources);
        children[0].margin.left = AutoLength::Auto;
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Auto;
        limits.retained_bytes =
            NonZeroUsize::new(sequence_requested_retained_bytes(5).unwrap() - 1).unwrap();
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.,
            },
            limits,
            |_, _| panic!("reserve reached before requested byte floor"),
        )
        .unwrap_err();
        assert!(matches!(error, ClearanceSequenceError::Bytes));

        let (sources, limits) = fixture();
        let (outer, parent, mut children) = input(&sources);
        children[0].margin.left = AutoLength::Auto;
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Auto;
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.,
            },
            limits,
            |records, requested| records.try_reserve_exact(requested + 1),
        )
        .unwrap_err();
        assert!(matches!(error, ClearanceSequenceError::Bytes));
    }

    #[test]
    fn sequence_work_overflow_is_rejected_before_build_phases() {
        assert_eq!(sequence_work(usize::MAX, usize::MAX), None);

        let (sources, mut limits) = fixture();
        let (outer, parent, mut children) = input(&sources);
        children[0].margin.left = AutoLength::Auto;
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Auto;
        children[2].padding.left = 1.0;
        limits.work = NonZeroUsize::new(480).unwrap();
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.0,
            },
            limits,
            |_, _| panic!("reserve reached before work rejection"),
        )
        .unwrap_err();
        assert!(matches!(error, ClearanceSequenceError::Work));
    }

    #[test]
    fn complete_profile_and_byte_preflight_precede_reserve_with_auto_facts() {
        let (sources, limits) = fixture();
        let (outer, parent, mut children) = input(&sources);
        children[0].margin.left = AutoLength::Auto;
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Auto;
        children[2].height = OwnerHeight::Auto;
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.0,
            },
            limits,
            |_, _| panic!("reserve reached before complete profile validation"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ClearanceSequenceError::Profile(ClearanceOwnerError::Unsupported {
                source: 4,
                reason: OwnerProfileReason::HeightKind,
            })
        ));

        let (sources, mut limits) = fixture();
        let (outer, parent, mut children) = input(&sources);
        children[0].margin.left = AutoLength::Auto;
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Auto;
        limits.retained_bytes =
            NonZeroUsize::new(sequence_requested_retained_bytes(5).unwrap() - 1).unwrap();
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.0,
            },
            limits,
            |_, _| panic!("reserve reached before requested byte floor"),
        )
        .unwrap_err();
        assert!(matches!(error, ClearanceSequenceError::Bytes));

        let (sources, mut limits) = fixture();
        let (outer, parent, mut children) = input(&sources);
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Auto;
        limits.retained_bytes =
            NonZeroUsize::new(sequence_requested_retained_bytes(5).unwrap() - 1).unwrap();
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: f32::NAN,
            },
            limits,
            |_, _| panic!("reserve reached before float-width domain validation"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ClearanceSequenceError::Profile(ClearanceOwnerError::InvalidNumber {
                source: 3,
                field: "float_width",
            })
        ));

        let (sources, mut limits) = fixture();
        let (outer, parent, mut children) = input(&sources);
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Auto;
        limits.retained_bytes =
            NonZeroUsize::new(sequence_requested_retained_bytes(5).unwrap() - 1).unwrap();
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 140.0,
            },
            limits,
            |_, _| panic!("reserve reached before requested byte floor"),
        )
        .unwrap_err();
        assert!(matches!(error, ClearanceSequenceError::Bytes));
    }

    fn vertical_auto(children: &mut [ChildFacts<'_>]) {
        for child in children.iter_mut() {
            child.margin.top = AutoLength::Auto;
            child.margin.bottom = AutoLength::Auto;
        }
        children[1].margin.left = AutoLength::Auto;
        children[1].margin.right = AutoLength::Auto;
    }

    #[test]
    fn vertical_auto_profiles_reach_real_reserve_failures_only_after_admission() {
        let real = Vec::<u8>::new()
            .try_reserve_exact(usize::MAX)
            .unwrap_err()
            .to_string();

        // Valid vertical Auto profiles, width 140 and a first kernel that would
        // overflow: the injected genuine reserve failure is returned with its source.
        let (sources, limits) = fixture();
        let (outer, mut parent, mut children) = input(&sources);
        vertical_auto(&mut children);
        parent.content_top = f32::MAX;
        children[0].margin.top = AutoLength::Value(f32::MAX);
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 140.0,
            },
            limits,
            |records, _| records.try_reserve_exact(usize::MAX),
        )
        .unwrap_err();
        let ClearanceSequenceError::Allocation(actual) = &error else {
            panic!("expected allocation error, got {error:?}");
        };
        assert_eq!(actual.to_string(), real);
        let source = std::error::Error::source(&error).unwrap();
        assert!(source.is::<TryReserveError>());
        assert_eq!(source.to_string(), real);

        // Paired control: the ordinary reserve succeeds and the kernel failure is
        // reached, proving the allocation failure was not a masked kernel error.
        let mut requested = None;
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 140.0,
            },
            limits,
            |records, count| {
                requested = Some(count);
                records.try_reserve_exact(count)
            },
        )
        .unwrap_err();
        assert_eq!(requested, Some(1));
        assert!(matches!(
            error,
            ClearanceSequenceError::Placement {
                source: 2,
                error: ClearanceOwnerError::Unrepresentable {
                    stage: "before_top"
                },
            }
        ));

        // Invalid later profile with vertical Auto never reaches reserve.
        let (outer, parent, mut children) = input(&sources);
        vertical_auto(&mut children);
        children[2].height = OwnerHeight::Auto;
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 140.0,
            },
            limits,
            |_, _| panic!("reserve reached before complete profile validation"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ClearanceSequenceError::Profile(ClearanceOwnerError::Unsupported {
                source: 4,
                reason: OwnerProfileReason::HeightKind,
            })
        ));

        // A finite negative margin is admitted and reaches the genuine reserve failure.
        let (outer, parent, mut children) = input(&sources);
        vertical_auto(&mut children);
        children[2].margin.bottom = AutoLength::Value(-1.0);
        let mut calls = 0;
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.0,
            },
            limits,
            |records, _| {
                calls += 1;
                records.try_reserve_exact(usize::MAX)
            },
        )
        .unwrap_err();
        assert_eq!(calls, 1);
        let ClearanceSequenceError::Allocation(actual) = &error else {
            panic!("expected allocation error, got {error:?}");
        };
        assert_eq!(actual.to_string(), real);
        assert!(
            std::error::Error::source(&error)
                .unwrap()
                .is::<TryReserveError>()
        );

        // A nonfinite later profile still cannot reach reserve.
        children[2].margin.bottom = AutoLength::Value(f32::NAN);
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.0,
            },
            limits,
            |_, _| panic!("reserve reached before complete profile validation"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ClearanceSequenceError::Profile(ClearanceOwnerError::InvalidNumber {
                source: 4,
                field: "margin",
            })
        ));
    }

    #[test]
    fn vertical_auto_work_is_prepaid_and_storage_is_runtime_sized() {
        // One short of the new five-source bound fails before reserve; the exact
        // bound reaches reserve once with the tail count and succeeds.
        let (sources, mut limits) = fixture();
        let (outer, parent, mut children) = input(&sources);
        vertical_auto(&mut children);
        for short in [2140, 480, 419, 337, 269, 213] {
            limits.work = NonZeroUsize::new(short).unwrap();
            let error = build_with_reserve(
                &sources,
                ClearanceSequenceInput {
                    outer,
                    parent,
                    children: &children,
                    float_width: 140.0,
                },
                limits,
                |_, _| panic!("reserve reached before work rejection"),
            )
            .unwrap_err();
            assert!(matches!(error, ClearanceSequenceError::Work));
        }
        limits.work = NonZeroUsize::new(2141).unwrap();
        let mut calls = 0;
        limits.retained_bytes = NonZeroUsize::MAX;
        let owner = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 140.0,
            },
            limits,
            |records, requested| {
                calls += 1;
                assert_eq!(requested, 1);
                records.try_reserve_exact(requested)
            },
        )
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(owner.receipt().work, 2141);
        assert_eq!(
            owner.receipt().retained_bytes,
            owner.receipt().record_capacity * size_of::<SequenceNormalPlacement>()
                + owner.receipt().member_capacity * size_of::<MarginMember>()
        );

        // Spare actual capacity is charged against the runtime-sized limit.
        limits.retained_bytes =
            NonZeroUsize::new(sequence_requested_retained_bytes(5).unwrap()).unwrap();
        let error = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 140.0,
            },
            limits,
            |records, requested| records.try_reserve_exact(requested + 1),
        )
        .unwrap_err();
        assert!(matches!(error, ClearanceSequenceError::Bytes));

        // The checked byte helper rejects products beyond isize::MAX.
        let size = size_of::<SequenceNormalPlacement>();
        let largest = isize::MAX as usize / size;
        assert_eq!(retained_bytes(largest, 0), Some(largest * size));
        assert_eq!(retained_bytes(largest + 1, 0), None);
        assert_eq!(retained_bytes(usize::MAX, 0), None);
        assert_eq!(sequence_work(5, 1), Some(2141));
        assert_eq!(sequence_work(6, 2), Some(2791));
        assert_eq!(sequence_work(7, 3), Some(3442));
        assert_eq!(sequence_work(1, 0), None);
    }
    #[test]
    fn cp_task1_actual_representation_measurement() {
        println!(
            "CP_REPRESENTATION SequenceNormalPlacement {} {}",
            size_of::<SequenceNormalPlacement>(),
            std::mem::align_of::<SequenceNormalPlacement>()
        );
        println!(
            "CTT_REPRESENTATION GroupSpan {} {}",
            size_of::<GroupSpan>(),
            std::mem::align_of::<GroupSpan>()
        );
        println!(
            "CTT_REPRESENTATION MarginMember {} {}",
            size_of::<MarginMember>(),
            std::mem::align_of::<MarginMember>()
        );
        println!(
            "CTT_REPRESENTATION ClearanceSequenceOwner {} {}",
            size_of::<ClearanceSequenceOwner<'_>>(),
            std::mem::align_of::<ClearanceSequenceOwner<'_>>()
        );
        println!(
            "CTT_REPRESENTATION ClearanceSequenceReceipt {} {}",
            size_of::<ClearanceSequenceReceipt>(),
            std::mem::align_of::<ClearanceSequenceReceipt>()
        );
        println!(
            "CP_REPRESENTATION ChildFacts {} {}",
            size_of::<ChildFacts<'_>>(),
            std::mem::align_of::<ChildFacts<'_>>()
        );
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
        {
            assert_eq!(
                (size_of::<GroupSpan>(), std::mem::align_of::<GroupSpan>()),
                (24, 8)
            );
            assert_eq!(
                (
                    size_of::<MarginMember>(),
                    std::mem::align_of::<MarginMember>()
                ),
                (16, 8)
            );
            assert_eq!(sequence_requested_retained_bytes(5), Some(200));
            assert_eq!(sequence_requested_retained_bytes(6), Some(368));
            assert_eq!(sequence_requested_retained_bytes(7), Some(536));
            assert_eq!(
                (
                    size_of::<SequenceNormalPlacement>(),
                    std::mem::align_of::<SequenceNormalPlacement>()
                ),
                (136, 8)
            );
            assert_eq!(
                (
                    size_of::<ChildFacts<'_>>(),
                    std::mem::align_of::<ChildFacts<'_>>()
                ),
                (96, 8)
            );
        }
    }
    fn cp_capacity_fixture(count: usize) -> (Vec<SourceNode>, ClearanceSequenceLimits) {
        let mut ids = IdAllocator::new(NonZeroU64::new(71).unwrap());
        let mut sources: Vec<_> = (0..count)
            .map(|_| SourceNode {
                instance: ids.allocate().unwrap(),
                parent: None,
                children: vec![],
                metadata: Metadata::default(),
            })
            .collect();
        let parent = sources[1].instance;
        sources[0].children.push(parent);
        sources[1].parent = Some(sources[0].instance);
        sources[1].children = sources[2..].iter().map(|node| node.instance).collect();
        for source in &mut sources[2..] {
            source.parent = Some(parent);
        }
        let limits = ClearanceSequenceLimits {
            sources: NonZeroUsize::new(count).unwrap(),
            work: NonZeroUsize::new(sequence_work(count, count - 4).unwrap()).unwrap(),
            retained_bytes: NonZeroUsize::MAX,
        };
        (sources, limits)
    }

    #[test]
    fn cp_injected_capacity_is_charged_for_the_same_vector() {
        for count in 5..=7 {
            let requested_tail = count - 4;
            let (sources, mut limits) = cp_capacity_fixture(count);
            let (outer, parent, mut children) = input(&sources);
            children[0].padding.top = 2.0;
            let input = ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.0,
            };
            limits.retained_bytes = NonZeroUsize::MAX;

            // Requested floors and work bounds reject before the allocator seam.
            let work = sequence_work(count, requested_tail).unwrap();
            limits.work = NonZeroUsize::new(work - 1).unwrap();
            assert!(matches!(
                build_with_reserve(&sources, input, limits, |_, _| panic!("work admission")),
                Err(ClearanceSequenceError::Work)
            ));
            limits.work = NonZeroUsize::new(work).unwrap();
            limits.retained_bytes =
                NonZeroUsize::new(sequence_requested_retained_bytes(count).unwrap() - 1).unwrap();
            assert!(matches!(
                build_with_reserve(&sources, input, limits, |_, _| panic!(
                    "requested byte admission"
                )),
                Err(ClearanceSequenceError::Bytes)
            ));
            limits.retained_bytes = NonZeroUsize::MAX;
            // Overflow operands are synthetic usize guards, not enormous allocated trees.
            assert_eq!(sequence_work(usize::MAX, requested_tail), None);
            assert_eq!(sequence_work(count, usize::MAX), None);
            assert_eq!(retained_bytes(usize::MAX - requested_tail, 0), None);
            let real = Vec::<u8>::new().try_reserve_exact(usize::MAX).unwrap_err();
            let error = build_with_reserve(&sources, input, limits, |records, requested| {
                assert_eq!(requested, requested_tail);
                records.try_reserve_exact(usize::MAX)
            })
            .unwrap_err();
            assert!(matches!(error, ClearanceSequenceError::Allocation(_)));
            assert!(
                std::error::Error::source(&error)
                    .unwrap()
                    .is::<TryReserveError>()
            );
            assert_eq!(
                std::error::Error::source(&error).unwrap().to_string(),
                real.to_string()
            );

            // Under-reserve is a separate contract failure, before placement.
            let error = build_with_reserve(&sources, input, limits, |_, requested| {
                assert_eq!(requested, requested_tail);
                Ok(())
            })
            .unwrap_err();
            assert!(matches!(error, ClearanceSequenceError::Bytes));

            // Boxed-slice conversion removes spare capacity. This is an initialized
            // placement vector produced by the public builder, then cleared in place.
            let mut exact = build_clearance_sequence(&sources, input, limits)
                .unwrap()
                .tail
                .into_boxed_slice()
                .into_vec();
            assert_eq!(exact.len(), requested_tail);
            assert_eq!(exact.capacity(), requested_tail);
            exact.clear();
            limits.retained_bytes =
                NonZeroUsize::new(sequence_requested_retained_bytes(count).unwrap()).unwrap();
            let owner = build_with_reserve(&sources, input, limits, move |records, requested| {
                assert_eq!(requested, requested_tail);
                *records = exact;
                Ok(())
            })
            .unwrap();
            assert_eq!(owner.receipt().record_capacity, requested_tail);
            assert_eq!(
                owner.receipt().retained_bytes,
                requested_tail * size_of::<SequenceNormalPlacement>()
                    + owner.receipt().member_capacity * size_of::<MarginMember>()
            );

            // Every case measures its own allocation BEFORE the call and moves that
            // exact vector into the seam. No two independent allocations are equated.
            for offset in [-1isize, 0, 1] {
                let mut prepared = Vec::<SequenceNormalPlacement>::new();
                prepared.try_reserve_exact(requested_tail + 1).unwrap();
                let capacity = prepared.capacity();
                assert!(capacity > requested_tail);
                let bytes = capacity * size_of::<SequenceNormalPlacement>()
                    + 2 * (count - 3) * size_of::<MarginMember>();
                limits.retained_bytes =
                    NonZeroUsize::new(bytes.checked_add_signed(offset).unwrap()).unwrap();
                let mut called = false;
                let result = build_with_reserve(&sources, input, limits, |records, requested| {
                    called = true;
                    assert_eq!(requested, requested_tail);
                    *records = prepared;
                    Ok(())
                });
                assert!(called);
                if offset < 0 {
                    assert!(matches!(result, Err(ClearanceSequenceError::Bytes)));
                } else {
                    let receipt = result.unwrap().receipt();
                    assert_eq!(receipt.record_capacity, capacity);
                    assert_eq!(receipt.retained_bytes, bytes);
                }
            }
        }
    }

    // Insert within clearance_sequence.rs::tests; uses that module's fixture/input.
    #[test]
    fn cfh_checked_height_bits_and_profile_order() {
        let (sources, _) = fixture();
        let (_, _, mut children) = input(&sources);
        children[1].height = OwnerHeight::Auto;
        assert_eq!(
            validate_sequence_child(children[1], 3, true, false)
                .unwrap()
                .to_bits(),
            0
        );
        for value in [-0.0_f32, 0.0, 2.0, f32::MAX] {
            children[1].height = OwnerHeight::Explicit(value);
            assert_eq!(
                validate_sequence_child(children[1], 3, true, false)
                    .unwrap()
                    .to_bits(),
                value.to_bits()
            );
        }
        children[1].height = OwnerHeight::Auto;
        children[1].content = OwnerContent::TextOrRichText;
        assert!(matches!(
            validate_sequence_child(children[1], 3, true, false),
            Err(ClearanceOwnerError::Unsupported {
                source: 3,
                reason: OwnerProfileReason::Content
            })
        ));
    }

    #[test]
    fn cfh_auto_profile_before_reserve_and_actual_capacity() {
        let (sources, mut limits) = fixture();
        let (outer, parent, mut children) = input(&sources);
        children[1].height = OwnerHeight::Auto;
        children[1].margin.bottom = AutoLength::Value(-1.0);
        let failed = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.0,
            },
            limits,
            |_, _| panic!("reserve before complete Auto profile"),
        );
        assert!(matches!(
            failed,
            Err(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: 3,
                    reason: OwnerProfileReason::NegativeFloatOuterExtent,
                }
            ))
        ));
        children[1].margin.bottom = AutoLength::Value(0.0);
        children[2].clear = ClearSide::None;
        children[1].height = OwnerHeight::Explicit(0.0);
        build_clearance_sequence(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.0,
            },
            limits,
        )
        .unwrap();
        children[1].height = OwnerHeight::Auto;
        let mut prepared = Vec::new();
        prepared.try_reserve_exact(3).unwrap();
        let capacity = prepared.capacity();
        limits.retained_bytes = NonZeroUsize::new(
            capacity * size_of::<SequenceNormalPlacement>() + 4 * size_of::<MarginMember>(),
        )
        .unwrap();
        let result = build_with_reserve(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.0,
            },
            limits,
            |records, requested| {
                assert_eq!(requested, 1);
                *records = prepared;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result.receipt().record_capacity, capacity);
        assert_eq!(
            result.receipt().retained_bytes,
            capacity * size_of::<SequenceNormalPlacement>()
                + result.receipt().member_capacity * size_of::<MarginMember>()
        );
    }

    // Prospective additions inside clearance_sequence.rs #[cfg(test)] mod tests.
    // Uses the existing fixture/input/cp_capacity_fixture helpers. Runtime is unrun.

    #[test]
    fn ctt_requested_bytes_cover_both_vectors_and_checked_arithmetic() {
        for nodes in 0..5 {
            assert_eq!(sequence_requested_retained_bytes(nodes), None);
        }
        for nodes in 5..=21 {
            let records = nodes - 4;
            let members = 2 * (nodes - 3);
            assert_eq!(
                sequence_requested_retained_bytes(nodes),
                Some(
                    records * size_of::<SequenceNormalPlacement>()
                        + members * size_of::<MarginMember>()
                )
            );
        }
        assert_eq!(sequence_requested_retained_bytes(usize::MAX), None);
        assert_eq!(member_slots(usize::MAX), None);
        assert_eq!(retained_bytes(0, usize::MAX), None);
        assert_eq!(retained_bytes(usize::MAX, 0), None);
        let max_members = isize::MAX as usize / size_of::<MarginMember>();
        assert_eq!(
            retained_bytes(0, max_members),
            Some(max_members * size_of::<MarginMember>())
        );
        assert_eq!(retained_bytes(0, max_members + 1), None);
        let max_records = isize::MAX as usize / size_of::<SequenceNormalPlacement>();
        assert_eq!(
            retained_bytes(max_records, 0),
            Some(max_records * size_of::<SequenceNormalPlacement>())
        );
        assert_eq!(retained_bytes(max_records, max_members), None);
    }

    #[test]
    fn ctt_record_gate_precedes_second_reserve_and_geometry() {
        let (sources, mut limits) = cp_capacity_fixture(9);
        let (outer, mut parent, mut children) = input(&sources);
        // Finite profile terms overflow B1 publication if geometry is reached.
        parent.content_top = f32::MAX;
        children[0].margin.top = AutoLength::Value(f32::MAX);
        let input = ClearanceSequenceInput {
            outer,
            parent,
            children: &children,
            float_width: 140.0,
        };
        let requested = sequence_requested_retained_bytes(9).unwrap();
        limits.retained_bytes = NonZeroUsize::new(requested - 1).unwrap();
        assert!(matches!(
            build_with_reserves(
                &sources,
                input,
                limits,
                |_, _| panic!("first reserve before requested floor"),
                |_, _| panic!("second reserve before requested floor")
            ),
            Err(ClearanceSequenceError::Bytes)
        ));
        limits.retained_bytes = NonZeroUsize::MAX;
        assert!(matches!(
            build_with_reserves(
                &sources,
                input,
                limits,
                |_, requested| {
                    assert_eq!(requested, 5);
                    Ok(())
                },
                |_, _| panic!("second reserve after short record capacity")
            ),
            Err(ClearanceSequenceError::Bytes)
        ));
        limits.retained_bytes = NonZeroUsize::new(requested).unwrap();
        assert!(matches!(
            build_with_reserves(
                &sources,
                input,
                limits,
                |records, requested| records.try_reserve_exact(requested + 1),
                |_, _| panic!("second reserve after excessive record capacity")
            ),
            Err(ClearanceSequenceError::Bytes)
        ));
    }

    #[test]
    fn ctt_member_reserve_errors_remain_typed_before_geometry() {
        let (sources, limits) = cp_capacity_fixture(5);
        let (outer, mut parent, mut children) = input(&sources);
        parent.content_top = f32::MAX;
        children[0].margin.top = AutoLength::Value(f32::MAX);
        let input = ClearanceSequenceInput {
            outer,
            parent,
            children: &children,
            float_width: 140.0,
        };
        let calls = std::cell::Cell::new(0);
        let expected = Vec::<MarginMember>::new()
            .try_reserve_exact(usize::MAX)
            .unwrap_err();
        let error = build_with_reserves(
            &sources,
            input,
            limits,
            |records, requested| {
                assert_eq!(calls.get(), 0);
                calls.set(1);
                records.try_reserve_exact(requested)
            },
            |members, requested| {
                assert_eq!(calls.get(), 1);
                assert_eq!(requested, 4);
                calls.set(2);
                members.try_reserve_exact(usize::MAX)
            },
        )
        .unwrap_err();
        assert_eq!(calls.get(), 2);
        assert!(matches!(error, ClearanceSequenceError::Allocation(_)));
        let source = std::error::Error::source(&error).unwrap();
        assert!(source.is::<TryReserveError>());
        assert_eq!(source.to_string(), expected.to_string());
        assert!(matches!(
            build_with_reserves(
                &sources,
                input,
                limits,
                |records, requested| records.try_reserve_exact(requested),
                |_, requested| {
                    assert_eq!(requested, 4);
                    Ok(())
                }
            ),
            Err(ClearanceSequenceError::Bytes)
        ));
        // Ownership proof: the local record Vec has no escape on either Err path;
        // ordinary Rust scope destruction releases it. No allocator internals are mocked.
    }

    #[test]
    fn ctt_actual_member_and_joint_spare_capacity_are_charged() {
        for count in 5..=7 {
            for (record_spare, member_spare) in [(0, 0), (1, 0), (0, 1), (1, 2)] {
                for one_short in [false, true] {
                    let (sources, mut limits) = cp_capacity_fixture(count);
                    let (outer, parent, mut children) = input(&sources);
                    children[2].clear = ClearSide::None;
                    let input = ClearanceSequenceInput {
                        outer,
                        parent,
                        children: &children,
                        float_width: 10.0,
                    };
                    let mut record_storage = Vec::new();
                    record_storage
                        .try_reserve_exact(count - 4 + record_spare)
                        .unwrap();
                    let mut member_storage = Vec::new();
                    member_storage
                        .try_reserve_exact(2 * (count - 3) + member_spare)
                        .unwrap();
                    let record_capacity = record_storage.capacity();
                    let member_capacity = member_storage.capacity();
                    let exact = record_capacity * size_of::<SequenceNormalPlacement>()
                        + member_capacity * size_of::<MarginMember>();
                    limits.retained_bytes =
                        NonZeroUsize::new(exact - usize::from(one_short)).unwrap();
                    let result = build_with_reserves(
                        &sources,
                        input,
                        limits,
                        |records, requested| {
                            assert_eq!(requested, count - 4);
                            *records = record_storage;
                            Ok(())
                        },
                        |members, requested| {
                            assert_eq!(requested, 2 * (count - 3));
                            *members = member_storage;
                            Ok(())
                        },
                    );
                    if one_short {
                        assert!(matches!(result, Err(ClearanceSequenceError::Bytes)));
                    } else {
                        let owner = result.unwrap();
                        assert_eq!(owner.receipt().record_capacity, record_capacity);
                        assert_eq!(owner.receipt().member_capacity, member_capacity);
                        assert_eq!(owner.receipt().retained_bytes, exact);
                        assert_eq!(owner.tail().len(), count - 4);
                    }
                }
            }
        }
    }

    #[test]
    fn ctt_complete_profile_validation_precedes_both_reserves() {
        let (sources, limits) = cp_capacity_fixture(5);
        let (outer, parent, mut children) = input(&sources);
        children[2].padding.top = f32::NAN;
        let error = build_with_reserves(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.0,
            },
            limits,
            |_, _| panic!("first reserve before complete profile"),
            |_, _| panic!("second reserve before complete profile"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ClearanceSequenceError::Profile(ClearanceOwnerError::InvalidNumber {
                source: 4,
                field: "padding"
            })
        ));
    }
    #[test]
    fn nzs_validated_zero_scalar_preserves_both_signs() {
        let (sources, _) = fixture();
        let (_, _, children) = input(&sources);
        for source in [2, 4] {
            for zero in [0.0_f32, -0.0_f32] {
                let mut child = children[source - 2];
                child.height = OwnerHeight::NonAdjoiningZero(zero);
                let validated = validate_sequence_child(child, source, false, false).unwrap();
                assert_eq!(validated.to_bits(), zero.to_bits());
                assert_eq!(explicit_height(child).to_bits(), zero.to_bits());
            }
        }
    }

    #[test]
    fn nzs_category_profile_errors_precede_both_retained_allocations() {
        let (sources, mut limits) = fixture();
        let (outer, parent, mut children) = input(&sources);
        children[0].height = OwnerHeight::NonAdjoiningZero(0.);
        children[2].height = OwnerHeight::NonAdjoiningZero(1.);
        limits.retained_bytes = NonZeroUsize::MIN;
        let error = build_with_reserves(
            &sources,
            ClearanceSequenceInput {
                outer,
                parent,
                children: &children,
                float_width: 10.,
            },
            limits,
            |_, _| panic!("record allocation before category validation"),
            |_, _| panic!("member allocation before category validation"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ClearanceSequenceError::Profile(ClearanceOwnerError::Unsupported {
                source: 4,
                reason: OwnerProfileReason::HeightKind,
            })
        ));
        limits.work = NonZeroUsize::new(2140).unwrap();
        assert!(matches!(
            build_with_reserves(
                &sources,
                ClearanceSequenceInput {
                    outer,
                    parent,
                    children: &children,
                    float_width: 10.
                },
                limits,
                |_, _| panic!("record allocation before work validation"),
                |_, _| panic!("member allocation before work validation"),
            ),
            Err(ClearanceSequenceError::Work)
        ));
    }

    #[test]
    fn nzs_height_category_keeps_runtime_sized_storage() {
        println!(
            "NZS_REPRESENTATION OwnerHeight {} {}",
            size_of::<OwnerHeight>(),
            std::mem::align_of::<OwnerHeight>()
        );
        println!(
            "NZS_REPRESENTATION ChildFacts {} {}",
            size_of::<ChildFacts<'_>>(),
            std::mem::align_of::<ChildFacts<'_>>()
        );
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
        {
            assert_eq!(
                (
                    size_of::<OwnerHeight>(),
                    std::mem::align_of::<OwnerHeight>()
                ),
                (8, 4)
            );
            assert_eq!(
                (
                    size_of::<ChildFacts<'_>>(),
                    std::mem::align_of::<ChildFacts<'_>>()
                ),
                (96, 8)
            );
            assert_eq!(
                (
                    size_of::<SequenceNormalPlacement>(),
                    std::mem::align_of::<SequenceNormalPlacement>()
                ),
                (136, 8)
            );
            assert_eq!(
                (
                    size_of::<MarginMember>(),
                    std::mem::align_of::<MarginMember>()
                ),
                (16, 8)
            );
            for (nodes, bytes) in [(5, 200), (6, 368), (7, 536)] {
                assert_eq!(sequence_requested_retained_bytes(nodes), Some(bytes));
            }
        }
    }
}
