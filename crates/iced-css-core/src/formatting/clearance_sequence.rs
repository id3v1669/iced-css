//! Bounded vertical clearance ownership for one parent containing B1, one float,
//! and one or more following normal siblings.
//!
//! Admission validates the complete source graph and every supplied profile before
//! reserving output storage or publishing geometry. The returned owner borrows only
//! the source slice; copied placement records do not retain the input facts slice.

use super::{
    clearance_owner::{
        BlockVertical, ChildFacts, ClearanceOwnerError, FirstFloatInput, KernelError, MarginGroup,
        NormalKernel, OuterFacts, ParentFacts, place_first_float, place_next_normal,
        validate_float_width, validate_outer, validate_parent, validate_sequence_child,
    },
    floats::PlacedFloat,
};
use crate::source::SourceNode;
use std::{collections::TryReserveError, mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct ClearanceSequenceInput<'s, 'f> {
    pub outer: OuterFacts<'s>,
    pub parent: ParentFacts<'s>,
    /// Exact projected children B1, F, and tail in source order.
    pub children: &'f [ChildFacts<'s>],
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
    placed_float: PlacedFloat,
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
    pub fn placed_float(&self) -> PlacedFloat {
        self.placed_float
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
/// profile checks, including the last child and float endpoint fit, precede the
/// fallible record allocation; allocation precedes every numerical kernel.
/// Sequence normals admit genuine finite signed absolute or Auto horizontal margins;
/// floats retain zero margins, and this vertical solve does not publish a new
/// horizontal placement. Work
/// `32 + 8*N + N*(N-1)/2 + 16*(N-4) + 33*(N-2)` is prepaid, including
/// constant-size typed margin validation for each child.
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
    for (offset, facts) in input.children.iter().copied().enumerate() {
        let source = offset + 2;
        validate_sequence_child(facts, source, source == 3, source >= 4).map_err(profile)?;
    }
    validate_float_width(input.parent, input.float_width, 3).map_err(profile)?;

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
    let float_height = explicit_height(input.children[1]);
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
    let mut previous_bottom = first.before.bottom;
    let mut previous_margin = input.children[0].margin.bottom;
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
        previous_bottom = next.vertical.bottom;
        previous_margin = facts.margin.bottom;
        previous_source = source;
        tail.push(record(source, next));
    }
    Ok(ClearanceSequenceOwner {
        sources,
        before: first.before,
        placed_float: first.placed_float,
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
        hypothetical_top: placement.hypothetical_top,
        zero_clearance_top: placement.zero_clearance_top,
        clearance: placement.clearance,
        hypothetical_group: placement.hypothetical_group,
        actual_groups: placement.actual_groups,
        actual_len: placement.actual_len,
    }
}

fn sequence_work(nodes: usize, tail: usize) -> Option<usize> {
    // 32 fixed admission/kernel actions, eight bounded per-node actions, every
    // duplicate comparison, sixteen fixed arithmetic/group actions per tail,
    // and 33 constant-size typed margin actions per child.
    let children = nodes.checked_sub(2)?;
    let linear = nodes.checked_mul(8)?;
    let pairs = nodes.checked_mul(nodes.checked_sub(1)?)?.checked_div(2)?;
    let kernels = tail.checked_mul(16)?;
    let margin_actions = children.checked_mul(33)?;
    32usize
        .checked_add(linear)?
        .checked_add(pairs)?
        .checked_add(kernels)?
        .checked_add(margin_actions)
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
            work: NonZeroUsize::new(197).unwrap(),
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
        children[0].margin.top = f32::MAX;
        children[0].margin.left = AutoLength::Auto;
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
                float_width: 10.,
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
        children[2].padding.left = 1.0;
        limits.work = NonZeroUsize::new(196).unwrap();
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
    }
}
