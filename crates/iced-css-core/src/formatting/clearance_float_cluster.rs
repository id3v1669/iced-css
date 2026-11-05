//! Bounded ownership for B1, one consecutive explicitly sized float cluster, and
//! a nonempty normal tail in one new BFC. Geometry remains private until success.

use super::{
    clearance::lowest_clearance_float_bottom,
    clearance_owner::{
        BlockVertical, ChildContext, ChildFacts, ClearanceOwnerError, FirstFloatInput,
        FloatBoxInput, FloatBoxKernel, KernelError, MarginMember, OuterFacts, OwnerHeight,
        OwnerProfileReason, ParentFacts, place_first_float, place_float_box, validate_outer,
        validate_sequence_child, validate_sequence_float_extents, validate_sequence_float_width,
        validate_sequence_parent,
    },
    clearance_sequence::{
        SequenceMarginRunView, SequenceNormalPlacement, SequenceNormalView,
        chain::{ChainState, FloatSelection, GroupSpan},
        collapses_through, explicit_height, new_bfc_intersects_float, same_source, sequence_work,
    },
    display::Float,
    floats::{Band, PlacedFloat},
};
use crate::source::SourceNode;
use std::{collections::TryReserveError, mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct ClearanceFloatClusterInput<'s, 'f> {
    pub outer: OuterFacts<'s>,
    pub parent: ParentFacts<'s>,
    /// B1, the consecutive floats, then the nonempty normal tail in source order.
    pub children: &'f [ChildFacts<'s>],
    /// One finite nonnegative used content width for each consecutive float.
    pub float_widths: &'f [f32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearanceFloatClusterLimits {
    pub sources: NonZeroUsize,
    pub floats: NonZeroUsize,
    pub work: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub scratch_bytes: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearanceFloatClusterReceipt {
    pub nodes: usize,
    pub floats: usize,
    pub tail: usize,
    /// Conservative prepaid logical work, independent of measured time.
    pub work: usize,
    pub float_capacity: usize,
    pub record_capacity: usize,
    pub member_capacity: usize,
    pub preceding_capacity: usize,
    pub retained_bytes: usize,
    /// Actual peak preceding-float vector capacity, released before return.
    pub scratch_bytes: usize,
}

#[derive(Debug)]
pub enum ClearanceFloatClusterError {
    Sources,
    Facts,
    Floats,
    ClusterOrder {
        source: usize,
    },
    Work,
    Bytes,
    ScratchBytes,
    Allocation(TryReserveError),
    Profile(ClearanceOwnerError),
    Placement {
        source: usize,
        error: ClearanceOwnerError,
    },
}

impl std::fmt::Display for ClearanceFloatClusterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "clearance float cluster: {self:?}")
    }
}

impl std::error::Error for ClearanceFloatClusterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::Profile(error) | Self::Placement { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FloatClusterFloatPlacement {
    source_index: usize,
    placed_float: PlacedFloat,
    border: Band,
    border_vertical: BlockVertical,
    content: Band,
    content_vertical: BlockVertical,
    padding: Band,
    padding_vertical: BlockVertical,
    bands_visited: usize,
}

impl FloatClusterFloatPlacement {
    pub fn source_index(&self) -> usize {
        self.source_index
    }
    pub fn placed_float(&self) -> PlacedFloat {
        self.placed_float
    }
    pub fn border(&self) -> Band {
        self.border
    }
    pub fn border_vertical(&self) -> BlockVertical {
        self.border_vertical
    }
    pub fn content(&self) -> Band {
        self.content
    }
    pub fn content_vertical(&self) -> BlockVertical {
        self.content_vertical
    }
    pub fn padding(&self) -> Band {
        self.padding
    }
    pub fn padding_vertical(&self) -> BlockVertical {
        self.padding_vertical
    }
    pub fn bands_visited(&self) -> usize {
        self.bands_visited
    }

    fn from_kernel(source_index: usize, value: FloatBoxKernel) -> Self {
        Self {
            source_index,
            placed_float: value.placed_float,
            border: value.float_border,
            border_vertical: value.float_border_vertical,
            content: value.float_content,
            content_vertical: value.float_content_vertical,
            padding: value.float_padding,
            padding_vertical: value.float_padding_vertical,
            bands_visited: value.bands_visited,
        }
    }
}

#[derive(Debug)]
pub struct ClearanceFloatClusterOwner<'s> {
    sources: &'s [SourceNode],
    before: BlockVertical,
    before_content: BlockVertical,
    before_padding: BlockVertical,
    floats: Vec<FloatClusterFloatPlacement>,
    tail: Vec<SequenceNormalPlacement>,
    members: Vec<MarginMember>,
    terminal: GroupSpan,
    receipt: ClearanceFloatClusterReceipt,
}

impl<'s> ClearanceFloatClusterOwner<'s> {
    pub fn sources(&self) -> &'s [SourceNode] {
        self.sources
    }
    pub fn source(&self, index: usize) -> Option<&'s SourceNode> {
        self.sources.get(index)
    }
    pub fn before(&self) -> BlockVertical {
        self.before
    }
    pub fn before_content(&self) -> BlockVertical {
        self.before_content
    }
    pub fn before_padding(&self) -> BlockVertical {
        self.before_padding
    }
    pub fn floats(&self) -> &[FloatClusterFloatPlacement] {
        &self.floats
    }
    pub fn tail(&self) -> &[SequenceNormalPlacement] {
        &self.tail
    }
    /// Membership always borrows this owner's member arena.
    pub fn tail_views(
        &self,
    ) -> impl ExactSizeIterator<Item = SequenceNormalView<'_>> + DoubleEndedIterator + '_ {
        self.tail
            .iter()
            .map(|record| SequenceNormalView::new(record, &self.members))
    }
    pub fn terminal_margin_run(&self) -> SequenceMarginRunView<'_> {
        SequenceMarginRunView::new(self.terminal, &self.members)
    }
    pub fn receipt(&self) -> ClearanceFloatClusterReceipt {
        self.receipt
    }
}

/// Build a complete, bounded flat BFC owner without partial publication.
///
/// Every float must have an explicit finite nonnegative used content height and
/// width. Float clear selects only earlier matching floats; normal clear retains
/// the existing equality refusal and two-scalar adjoining-margin reference.
///
/// # Errors
/// Returns count, work, allocation/byte, complete source/profile or source-tagged
/// placement errors. All source/profile checks and allocation gates precede
/// geometry. The old single-float APIs and their allocation behavior are unchanged.
pub fn build_clearance_float_cluster<'s>(
    sources: &'s [SourceNode],
    input: ClearanceFloatClusterInput<'s, '_>,
    limits: ClearanceFloatClusterLimits,
) -> Result<ClearanceFloatClusterOwner<'s>, ClearanceFloatClusterError> {
    build_with_reserves(
        sources,
        input,
        limits,
        |values, count| values.try_reserve_exact(count),
        |values, count| values.try_reserve_exact(count),
        |values, count| values.try_reserve_exact(count),
        |values, count| values.try_reserve_exact(count),
    )
}

fn build_with_reserves<'s>(
    sources: &'s [SourceNode],
    input: ClearanceFloatClusterInput<'s, '_>,
    limits: ClearanceFloatClusterLimits,
    reserve_floats: impl FnOnce(
        &mut Vec<FloatClusterFloatPlacement>,
        usize,
    ) -> Result<(), TryReserveError>,
    reserve_tail: impl FnOnce(&mut Vec<SequenceNormalPlacement>, usize) -> Result<(), TryReserveError>,
    reserve_members: impl FnOnce(&mut Vec<MarginMember>, usize) -> Result<(), TryReserveError>,
    reserve_preceding: impl FnOnce(&mut Vec<PlacedFloat>, usize) -> Result<(), TryReserveError>,
) -> Result<ClearanceFloatClusterOwner<'s>, ClearanceFloatClusterError> {
    use ClearanceFloatClusterError as Error;
    let nodes = sources.len();
    if nodes < 5 || nodes > limits.sources.get() {
        return Err(Error::Sources);
    }
    if input.children.len() != nodes - 2 {
        return Err(Error::Facts);
    }
    // The width count gives a bounded proposed partition before any source scan.
    // Full role checks below authenticate that partition, including reentry.
    let count = input.float_widths.len();
    if count == 0 || count > limits.floats.get() || count > nodes - 4 {
        return Err(Error::Floats);
    }
    let tail_count = nodes - count - 3;
    let work = cluster_work(nodes, count, tail_count).ok_or(Error::Work)?;
    if work > limits.work.get() {
        return Err(Error::Work);
    }
    validate_graph(sources, input)?;
    validate_outer(input.outer, 0).map_err(Error::Profile)?;
    validate_sequence_parent(input.parent, 1).map_err(Error::Profile)?;
    let through_eligible = input.children[..1]
        .iter()
        .chain(&input.children[count + 1..])
        .all(|facts| {
            let top = facts.margin.used_top();
            let bottom = facts.margin.used_bottom();
            (top >= 0.0 || top.is_finite()) && (bottom >= 0.0 || bottom.is_finite())
        });
    for (offset, facts) in input.children.iter().copied().enumerate() {
        let floating = offset >= 1 && offset <= count;
        if (facts.display.float != Float::None) != floating {
            return Err(Error::ClusterOrder { source: offset + 2 });
        }
        validate_sequence_child(facts, offset + 2, floating, !floating && through_eligible)
            .map_err(Error::Profile)?;
        if floating && !matches!(facts.height, OwnerHeight::Explicit(_)) {
            return Err(Error::Profile(ClearanceOwnerError::Unsupported {
                source: offset + 2,
                reason: OwnerProfileReason::HeightKind,
            }));
        }
    }
    // Preserve the one-float ordering: complete child profiles, width domain,
    // then exact outer extents, all before the first reserve.
    for (index, width) in input.float_widths.iter().copied().enumerate() {
        validate_sequence_float_width(width, index + 3).map_err(Error::Profile)?;
    }
    for (index, width) in input.float_widths.iter().copied().enumerate() {
        let facts = input.children[index + 1];
        validate_sequence_float_extents(facts, width, explicit_height(facts), index + 3)
            .map_err(Error::Profile)?;
    }
    let member_count = tail_count
        .checked_add(1)
        .and_then(|value| value.checked_mul(2))
        .ok_or(Error::Bytes)?;
    let requested = retained_bytes(count, tail_count, member_count).ok_or(Error::Bytes)?;
    if requested > limits.retained_bytes.get() {
        return Err(Error::Bytes);
    }
    let requested_scratch = scratch_bytes(count).ok_or(Error::ScratchBytes)?;
    if requested_scratch > limits.scratch_bytes.get() {
        return Err(Error::ScratchBytes);
    }

    let mut floats = Vec::new();
    reserve_floats(&mut floats, count).map_err(Error::Allocation)?;
    if !floats.is_empty() || floats.capacity() < count {
        return Err(Error::Bytes);
    }
    if retained_bytes(floats.capacity(), tail_count, member_count).ok_or(Error::Bytes)?
        > limits.retained_bytes.get()
    {
        return Err(Error::Bytes);
    }
    let mut tail = Vec::new();
    reserve_tail(&mut tail, tail_count).map_err(Error::Allocation)?;
    if !tail.is_empty() || tail.capacity() < tail_count {
        return Err(Error::Bytes);
    }
    if retained_bytes(floats.capacity(), tail.capacity(), member_count).ok_or(Error::Bytes)?
        > limits.retained_bytes.get()
    {
        return Err(Error::Bytes);
    }
    let mut members = Vec::new();
    reserve_members(&mut members, member_count).map_err(Error::Allocation)?;
    if !members.is_empty() || members.capacity() < member_count {
        return Err(Error::Bytes);
    }
    let retained = retained_bytes(floats.capacity(), tail.capacity(), members.capacity())
        .ok_or(Error::Bytes)?;
    if retained > limits.retained_bytes.get() {
        return Err(Error::Bytes);
    }
    let mut preceding = Vec::new();
    reserve_preceding(&mut preceding, count).map_err(Error::Allocation)?;
    if !preceding.is_empty() || preceding.capacity() < count {
        return Err(Error::ScratchBytes);
    }
    let scratch = scratch_bytes(preceding.capacity()).ok_or(Error::ScratchBytes)?;
    if scratch > limits.scratch_bytes.get() {
        return Err(Error::ScratchBytes);
    }

    let before_facts = input.children[0];
    let through = collapses_through(before_facts);
    let first = place_first_float(FirstFloatInput {
        parent: input.parent,
        before: before_facts,
        before_height: explicit_height(before_facts),
        before_source: 2,
        float: input.children[1],
        float_height: explicit_height(input.children[1]),
        float_width: input.float_widths[0],
        float_source: 3,
        before_collapses_through: through,
    })
    .map_err(placement)?;
    check_contact(before_facts, first.before, first.placed_float, 2)?;
    floats.push(FloatClusterFloatPlacement {
        source_index: 3,
        placed_float: first.placed_float,
        border: first.float_border,
        border_vertical: first.float_border_vertical,
        content: first.float_content,
        content_vertical: first.float_content_vertical,
        padding: first.float_padding,
        padding_vertical: first.float_padding_vertical,
        bands_visited: first.bands_visited,
    });
    preceding.push(first.placed_float);
    for index in 1..count {
        let source = index + 3;
        let facts = input.children[index + 1];
        let selected = lowest_clearance_float_bottom(
            &preceding,
            facts.clear,
            NonZeroUsize::new(index).expect("later float has a preceding float"),
        )
        .map_err(|error| Error::Placement {
            source,
            error: ClearanceOwnerError::FloatSelection(error),
        })?;
        let hypothetical_top = match selected {
            Some(bottom) if bottom > first.float_hypothetical_top => bottom,
            _ => first.float_hypothetical_top,
        };
        // index+1 is bounded by the already admitted nonempty count.
        let next = place_float_box(
            FloatBoxInput {
                parent: input.parent,
                float: facts,
                float_height: explicit_height(facts),
                float_width: input.float_widths[index],
                float_source: source,
                hypothetical_top,
            },
            &preceding,
            NonZeroUsize::new(index + 1).expect("positive band count"),
        )
        .map_err(placement)?;
        check_contact(before_facts, first.before, next.placed_float, 2)?;
        preceding.push(next.placed_float);
        floats.push(FloatClusterFloatPlacement::from_kernel(source, next));
    }
    let mut chain = ChainState::first(
        &mut members,
        before_facts,
        first.before.bottom,
        input.parent.content_top,
        through,
    )
    .map_err(placement)?;
    let selection_limit = NonZeroUsize::new(count).expect("nonempty validated cluster");
    for (index, facts) in input.children[count + 1..].iter().copied().enumerate() {
        let source = count + 3 + index;
        let next = chain
            .place_next_with_floats(
                &mut members,
                facts,
                explicit_height(facts),
                source,
                collapses_through(facts),
                FloatSelection {
                    floats: &preceding,
                    limit: selection_limit,
                },
            )
            .map_err(placement)?;
        for float in &preceding {
            check_contact(facts, next.vertical(), *float, source)?;
        }
        tail.push(next);
    }
    Ok(ClearanceFloatClusterOwner {
        sources,
        before: first.before,
        before_content: first.before_content,
        before_padding: first.before_padding,
        receipt: ClearanceFloatClusterReceipt {
            nodes,
            floats: count,
            tail: tail_count,
            work,
            float_capacity: floats.capacity(),
            record_capacity: tail.capacity(),
            member_capacity: members.capacity(),
            preceding_capacity: preceding.capacity(),
            retained_bytes: retained,
            scratch_bytes: scratch,
        },
        floats,
        tail,
        members,
        terminal: chain.terminal(),
    })
}

fn placement(failure: KernelError) -> ClearanceFloatClusterError {
    ClearanceFloatClusterError::Placement {
        source: failure.source,
        error: failure.error,
    }
}

fn check_contact(
    facts: ChildFacts<'_>,
    border: BlockVertical,
    float: PlacedFloat,
    source: usize,
) -> Result<(), ClearanceFloatClusterError> {
    if facts.context == ChildContext::OtherNewBfc && new_bfc_intersects_float(facts, border, float)
    {
        Err(ClearanceFloatClusterError::Placement {
            source,
            error: ClearanceOwnerError::NewBfcFloatContact,
        })
    } else {
        Ok(())
    }
}

fn validate_graph(
    sources: &[SourceNode],
    input: ClearanceFloatClusterInput<'_, '_>,
) -> Result<(), ClearanceFloatClusterError> {
    let nodes = sources.len();
    let profile = ClearanceFloatClusterError::Profile;
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
    Ok(())
}

fn retained_bytes(floats: usize, tail: usize, members: usize) -> Option<usize> {
    let bytes = floats
        .checked_mul(size_of::<FloatClusterFloatPlacement>())?
        .checked_add(tail.checked_mul(size_of::<SequenceNormalPlacement>())?)?
        .checked_add(members.checked_mul(size_of::<MarginMember>())?)?;
    (bytes <= isize::MAX as usize).then_some(bytes)
}

fn scratch_bytes(floats: usize) -> Option<usize> {
    let bytes = floats.checked_mul(size_of::<PlacedFloat>())?;
    (bytes <= isize::MAX as usize).then_some(bytes)
}

fn cluster_work(nodes: usize, floats: usize, tail: usize) -> Option<usize> {
    // Existing graph/profile/normal/arena work is conservatively prepaid with the
    // larger single-float tail. No existing charges are removed or refunded.
    let base = sequence_work(nodes, nodes.checked_sub(4)?)?;
    let earlier = floats.checked_sub(1)?;
    // Each later float j scans j prior floats for clear/validation/top, and at
    // most j+1 narrowing/next-edge bands: sum 32*j*(j+1), j=1..K-1.
    let bands = floats
        .checked_mul(earlier)?
        .checked_mul(floats.checked_add(1)?)?
        .checked_div(3)?
        .checked_mul(32)?;
    base.checked_add(earlier.checked_mul(1062)?)?
        .checked_add(bands)?
        // All-float normal clear and contact scans, including B1 contact.
        .checked_add(floats.checked_mul(tail.checked_add(1)?)?.checked_mul(128)?)?
        // Consecutive role admission, float clear setup and record transfers.
        .checked_add(floats.checked_mul(32)?)
}

#[cfg(test)]
#[path = "clearance_float_cluster_tests.rs"]
mod tests;
