//! Bounded vertical clearance ownership for one parent containing B1, one float,
//! and one or more following normal siblings.
//!
//! Admission validates the complete source graph and every supplied profile before
//! reserving output storage or publishing geometry. The returned owner borrows only
//! the source slice; copied placement records do not retain the input facts slice.

use super::{
    clearance_owner::{
        BlockVertical, ChildContext, ChildFacts, ClearanceOwnerError, FirstFloatInput, KernelError,
        MarginGroup, NormalKernel, OuterFacts, ParentFacts, place_first_float, place_next_normal,
        validate_outer, validate_parent, validate_sequence_child, validate_sequence_float_extents,
        validate_sequence_float_width,
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
    hypothetical_top: f32,
    zero_clearance_top: Option<f32>,
    clearance: Option<f32>,
    hypothetical_group: MarginGroup,
    actual_groups: [MarginGroup; 2],
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
    pub fn hypothetical_top(&self) -> f32 {
        self.hypothetical_top
    }
    pub fn zero_clearance_top(&self) -> Option<f32> {
        self.zero_clearance_top
    }
    pub fn clearance(&self) -> Option<f32> {
        self.clearance
    }
    pub fn hypothetical_group(&self) -> &MarginGroup {
        &self.hypothetical_group
    }
    pub fn actual_groups(&self) -> &[MarginGroup] {
        &self.actual_groups[..self.actual_len]
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
    float_border: Band,
    float_border_vertical: BlockVertical,
    placed_float: PlacedFloat,
    float_content: Band,
    float_content_vertical: BlockVertical,
    tail: Vec<SequenceNormalPlacement>,
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
    /// With zero padding on this axis, the predecessor margin-offset near/far
    /// path is retained; if the used margins are also zero, copy the placer band.
    /// With padding, publish the side-selected near border edge, near content
    /// edge, far content edge, then far border edge. Each stage rounds once.
    /// Signed margins may place border/content outside the margin box; no span
    /// is clamped. Nonnegative padding ensures border contains content.
    pub fn float_content(&self) -> Band {
        self.float_content
    }
    /// Copied float content span, independently published from its margin box.
    /// With zero vertical padding, retain the predecessor margin-offset stages;
    /// if both used vertical margins are also zero, copy the placer span.
    /// Otherwise publish border top, content top, content bottom and border
    /// bottom in that order. Signed margins may move either span outside the
    /// margin box; nonnegative padding keeps border around content. No clamp.
    pub fn float_content_vertical(&self) -> BlockVertical {
        self.float_content_vertical
    }
    pub fn tail(&self) -> &[SequenceNormalPlacement] {
        &self.tail
    }
    pub fn receipt(&self) -> ClearanceSequenceReceipt {
        self.receipt
    }
}

/// Validate and solve the complete source-owned sibling sequence.
///
/// Source/count/work checks precede the complete graph and profile scan. All
/// profile checks, including the last child and float-width domain, precede the
/// fallible record allocation; allocation precedes every numerical kernel.
/// Sequence normals admit finite signed absolute or Auto margins on every edge.
/// Floats admit computed Auto or finite signed
/// absolute margins on all four edges (Auto is used zero) while both exact outer
/// extents are nonnegative; otherwise, after every child's profile and the float-width
/// domain, the profile rejects the float with `NegativeFloatOuterExtent`. The float is
/// placed by its margin box, formed from content extents, padding and used
/// margins. Content and border bands/spans are published separately; finite
/// nonnegative absolute padding is admitted on all Sequence children. B1 and the float admit any computed `clear`. No float precedes either
/// of them in the parent's new block formatting context, so CSS2.2 §9.5.2 and float
/// constraint #10 select nothing; the value is not read after admission. Every
/// Sequence normal (B1 or a tail) may instead carry `ChildContext::OtherNewBfc`, a
/// normal-flow block that establishes a new block formatting context. Right after
/// that normal's placement, `NewBfcFloatContact` rejects intersection of its nonempty
/// published border interval with F's nonempty margin interval, tagged with its
/// source. Contact and empty intervals pass (Q1); signed margins make rejection reachable.
/// Childless F admits computed Auto height with positive-zero used content height;
/// normal children retain positive explicit height and Legacy admission is unchanged.
/// Work `560 + 8*N + N*(N-1)/2 + 54*(N-4) + 61*(N-2) + 64*(N-3)` is prepaid,
/// including constant-size typed margin validation and used-value service for each
/// child and the float's outer-extent sign check, outer-extent, content-band and
/// vertical content stages.
///
/// # Errors
/// Returns bounded admission/allocation failures, complete-profile failures, or a
/// placement failure tagged with the actual source whose stage failed.
pub fn build_clearance_sequence<'s>(
    sources: &'s [SourceNode],
    input: ClearanceSequenceInput<'s, '_>,
    limits: ClearanceSequenceLimits,
) -> Result<ClearanceSequenceOwner<'s>, ClearanceSequenceError> {
    build_with_reserve(sources, input, limits, |records, requested| {
        records.try_reserve_exact(requested)
    })
}

fn build_with_reserve<'s>(
    sources: &'s [SourceNode],
    input: ClearanceSequenceInput<'s, '_>,
    limits: ClearanceSequenceLimits,
    reserve: impl FnOnce(&mut Vec<SequenceNormalPlacement>, usize) -> Result<(), TryReserveError>,
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
    validate_parent(input.parent, 1).map_err(profile)?;
    let mut checked_float_height = None;
    for (offset, facts) in input.children.iter().copied().enumerate() {
        let source = offset + 2;
        let height = validate_sequence_child(facts, source, source == 3).map_err(profile)?;
        if source == 3 {
            checked_float_height = Some(height);
        }
    }
    let float_height =
        checked_float_height.expect("validated Sequence shape contains source-3 float");
    validate_sequence_float_width(input.float_width, 3).map_err(profile)?;
    validate_sequence_float_extents(input.children[1], input.float_width, float_height, 3)
        .map_err(profile)?;

    let requested_bytes = retained_bytes(tail_count).ok_or(ClearanceSequenceError::Bytes)?;
    if requested_bytes > limits.retained_bytes.get() {
        return Err(ClearanceSequenceError::Bytes);
    }
    let mut tail = Vec::new();
    reserve(&mut tail, tail_count).map_err(ClearanceSequenceError::Allocation)?;
    if tail.capacity() < tail_count {
        return Err(ClearanceSequenceError::Bytes);
    }
    let actual_bytes = retained_bytes(tail.capacity()).ok_or(ClearanceSequenceError::Bytes)?;
    if actual_bytes > limits.retained_bytes.get() {
        return Err(ClearanceSequenceError::Bytes);
    }

    let before_height = explicit_height(input.children[0]);
    let first = place_first_float(FirstFloatInput {
        parent: input.parent,
        before: input.children[0],
        before_height,
        before_source: 2,
        float: input.children[1],
        float_height,
        float_width: input.float_width,
        float_source: 3,
    })
    .map_err(placement)?;
    // CSS2.2 §9.5, Q1: reject intersection of nonempty published intervals.
    // This bounded core has no normal horizontal geometry for avoidance search.
    // Signed margins may put F above B1; rounded-empty B1 and F remain admitted.
    if input.children[0].context == ChildContext::OtherNewBfc
        && new_bfc_intersects_float(first.before, first.placed_float)
    {
        return Err(ClearanceSequenceError::Placement {
            source: 2,
            error: ClearanceOwnerError::NewBfcFloatContact,
        });
    }
    let mut previous_bottom = first.before.bottom;
    let mut previous_margin = input.children[0].margin.used_bottom();
    let mut previous_source = 2;
    for (offset, facts) in input.children[2..].iter().copied().enumerate() {
        let source = offset + 4;
        let next = place_next_normal(
            previous_bottom,
            previous_margin,
            previous_source,
            facts,
            explicit_height(facts),
            source,
            first.placed_float,
        )
        .map_err(placement)?;
        // CSS2.2 §9.5, Q1: a new-BFC tail obeys the B1 rule, checked right after its own
        // placement so that the first failing source in source order is reported.
        if facts.context == ChildContext::OtherNewBfc
            && new_bfc_intersects_float(next.vertical, first.placed_float)
        {
            return Err(ClearanceSequenceError::Placement {
                source,
                error: ClearanceOwnerError::NewBfcFloatContact,
            });
        }
        previous_bottom = next.vertical.bottom;
        previous_margin = facts.margin.used_bottom();
        previous_source = source;
        tail.push(record(source, next));
    }
    Ok(ClearanceSequenceOwner {
        sources,
        before: first.before,
        before_content: first.before_content,
        float_border: first.float_border,
        float_border_vertical: first.float_border_vertical,
        placed_float: first.placed_float,
        float_content: first.float_content,
        float_content_vertical: first.float_content_vertical,
        receipt: ClearanceSequenceReceipt {
            nodes,
            tail: tail_count,
            work,
            record_capacity: tail.capacity(),
            retained_bytes: actual_bytes,
        },
        tail,
    })
}

fn same_source(actual: &SourceNode, expected: &SourceNode) -> bool {
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
fn new_bfc_intersects_float(border: BlockVertical, float: PlacedFloat) -> bool {
    border.top < border.bottom
        && float.top < float.bottom
        && float.top < border.bottom
        && border.top < float.bottom
}

fn explicit_height(facts: ChildFacts<'_>) -> f32 {
    match facts.height {
        super::clearance_owner::OwnerHeight::Explicit(height) => height,
        _ => unreachable!("complete profile validation admitted only explicit height"),
    }
}

fn record(source_index: usize, placement: NormalKernel) -> SequenceNormalPlacement {
    SequenceNormalPlacement {
        source_index,
        vertical: placement.vertical,
        content_vertical: placement.content_vertical,
        hypothetical_top: placement.hypothetical_top,
        zero_clearance_top: placement.zero_clearance_top,
        clearance: placement.clearance,
        hypothetical_group: placement.hypothetical_group,
        actual_groups: placement.actual_groups,
        actual_len: placement.actual_len,
    }
}

fn sequence_work(nodes: usize, tail: usize) -> Option<usize> {
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
        .checked_add(tail.checked_mul(30)?)
}

fn retained_bytes(capacity: usize) -> Option<usize> {
    let bytes = capacity.checked_mul(size_of::<SequenceNormalPlacement>())?;
    (bytes <= isize::MAX as usize).then_some(bytes)
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
            work: NonZeroUsize::new(975).unwrap(),
            retained_bytes: NonZeroUsize::new(size_of::<SequenceNormalPlacement>()).unwrap(),
        };
        (sources, limits)
    }

    fn display() -> DisplayResolution {
        DisplayResolution {
            display: Display::Block,
            position: Position::Static,
            float: Float::None,
            flow: Flow::Normal,
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
            NonZeroUsize::new(size_of::<SequenceNormalPlacement>() - 1).unwrap();
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
            NonZeroUsize::new(size_of::<SequenceNormalPlacement>() - 1).unwrap();
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
            NonZeroUsize::new(size_of::<SequenceNormalPlacement>() - 1).unwrap();
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
            NonZeroUsize::new(size_of::<SequenceNormalPlacement>() - 1).unwrap();
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
        for short in [974, 480, 419, 337, 269, 213] {
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
        limits.work = NonZeroUsize::new(975).unwrap();
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
        assert_eq!(owner.receipt().work, 975);
        assert_eq!(
            owner.receipt().retained_bytes,
            owner.receipt().record_capacity * size_of::<SequenceNormalPlacement>()
        );

        // Spare actual capacity is charged against the runtime-sized limit.
        limits.retained_bytes = NonZeroUsize::new(size_of::<SequenceNormalPlacement>()).unwrap();
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
        assert_eq!(retained_bytes(largest), Some(largest * size));
        assert_eq!(retained_bytes(largest + 1), None);
        assert_eq!(retained_bytes(usize::MAX), None);
        assert_eq!(sequence_work(5, 1), Some(975));
        assert_eq!(sequence_work(6, 2), Some(1167));
        assert_eq!(sequence_work(7, 3), Some(1360));
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
            "CP_REPRESENTATION ChildFacts {} {}",
            size_of::<ChildFacts<'_>>(),
            std::mem::align_of::<ChildFacts<'_>>()
        );
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
        {
            assert_eq!(
                (
                    size_of::<SequenceNormalPlacement>(),
                    std::mem::align_of::<SequenceNormalPlacement>()
                ),
                (224, 8)
            );
            assert_eq!(
                (
                    size_of::<ChildFacts<'_>>(),
                    std::mem::align_of::<ChildFacts<'_>>()
                ),
                (88, 8)
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
                NonZeroUsize::new(requested_tail * size_of::<SequenceNormalPlacement>() - 1)
                    .unwrap();
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
            assert_eq!(retained_bytes(usize::MAX - requested_tail), None);
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
                NonZeroUsize::new(requested_tail * size_of::<SequenceNormalPlacement>()).unwrap();
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
            );

            // Every case measures its own allocation BEFORE the call and moves that
            // exact vector into the seam. No two independent allocations are equated.
            for offset in [-1isize, 0, 1] {
                let mut prepared = Vec::<SequenceNormalPlacement>::new();
                prepared.try_reserve_exact(requested_tail + 1).unwrap();
                let capacity = prepared.capacity();
                assert!(capacity > requested_tail);
                let bytes = capacity * size_of::<SequenceNormalPlacement>();
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
            validate_sequence_child(children[1], 3, true)
                .unwrap()
                .to_bits(),
            0
        );
        for value in [-0.0_f32, 0.0, 2.0, f32::MAX] {
            children[1].height = OwnerHeight::Explicit(value);
            assert_eq!(
                validate_sequence_child(children[1], 3, true)
                    .unwrap()
                    .to_bits(),
                value.to_bits()
            );
        }
        children[1].height = OwnerHeight::Auto;
        children[1].content = OwnerContent::TextOrRichText;
        assert!(matches!(
            validate_sequence_child(children[1], 3, true),
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
        limits.retained_bytes =
            NonZeroUsize::new(capacity * size_of::<SequenceNormalPlacement>()).unwrap();
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
        );
    }
}
