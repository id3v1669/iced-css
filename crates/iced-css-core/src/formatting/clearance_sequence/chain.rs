//! Sequence-owned adjoining runs and views. All member storage is reserved by the
//! parent builder before geometry; spans never borrow the temporary input facts.
use super::SequenceNormalPlacement;
use crate::formatting::{
    clearance::{
        ClearancePresenceError, ClearanceRequirement, ClearanceState, classify_clearance,
        lowest_clearance_float_bottom, solve_clearance,
    },
    clearance_owner::{
        BlockVertical, ChildFacts, ClearanceOwnerError, KernelError, MarginGroupId, MarginMember,
        MarginSide, amount_error, normal_vertical, sum,
    },
    floats::PlacedFloat,
    margins::MarginExtrema,
};
use std::num::NonZeroUsize;

/// Private immutable membership snapshot. Its owner alone resolves the range.
#[derive(Debug, Clone, Copy)]
pub(in crate::formatting) struct GroupSpan {
    start: usize,
    len: usize,
    extrema: MarginExtrema,
}

impl GroupSpan {
    fn single(start: usize, value: f32) -> Result<Self, ClearanceOwnerError> {
        let mut extrema = MarginExtrema::default();
        extrema
            .extend(&[value], NonZeroUsize::MIN)
            .map_err(ClearanceOwnerError::Margin)?;
        Ok(Self {
            start,
            len: 1,
            extrema,
        })
    }

    fn extended(mut self, value: f32) -> Result<Self, ClearanceOwnerError> {
        self.extrema
            .extend(&[value], NonZeroUsize::MIN)
            .map_err(ClearanceOwnerError::Margin)?;
        // The builder checked and reserved 2*(N-3) member slots. Each normal adds
        // at most two, so this length and every resolved end remain in that bound.
        self.len += 1;
        Ok(self)
    }

    fn members(self, arena: &[MarginMember]) -> &[MarginMember] {
        &arena[self.start..self.start + self.len]
    }
}

/// Logical placement-group observation, borrowing the immutable owning Sequence.
/// There is no public descriptor/arena constructor and no inline padding member.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SequenceMarginGroupView<'o> {
    id: MarginGroupId,
    members: &'o [MarginMember],
    extrema: MarginExtrema,
}

impl<'o> SequenceMarginGroupView<'o> {
    fn new(id: MarginGroupId, span: GroupSpan, arena: &'o [MarginMember]) -> Self {
        Self {
            id,
            members: span.members(arena),
            extrema: span.extrema,
        }
    }

    pub fn id(&self) -> MarginGroupId {
        self.id
    }

    pub fn members(&self) -> &'o [MarginMember] {
        self.members
    }

    pub fn positive(&self) -> f32 {
        self.extrema.positive()
    }

    pub fn negative(&self) -> f32 {
        self.extrema.negative()
    }
}

/// Final adjoining membership and extrema, without a numeric edge publication.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SequenceMarginRunView<'o> {
    members: &'o [MarginMember],
    extrema: MarginExtrema,
}

impl<'o> SequenceMarginRunView<'o> {
    pub(in crate::formatting) fn new(span: GroupSpan, arena: &'o [MarginMember]) -> Self {
        Self {
            members: span.members(arena),
            extrema: span.extrema,
        }
    }

    pub fn members(&self) -> &'o [MarginMember] {
        self.members
    }

    pub fn positive(&self) -> f32 {
        self.extrema.positive()
    }

    pub fn negative(&self) -> f32 {
        self.extrema.negative()
    }
}

/// An owner-bound record and its logical group observations. Every reference has
/// the owner's lifetime, including member slices obtained through temporary views.
///
/// Member slices and iterators keep the owner borrow after temporary views drop.
///
/// ```no_run
/// use iced_css_core::formatting::{
///     clearance_owner::MarginMember,
///     clearance_sequence::ClearanceSequenceOwner,
/// };
///
/// fn hypothetical_members<'o>(
///     owner: &'o ClearanceSequenceOwner<'_>,
/// ) -> &'o [MarginMember] {
///     owner.tail_view(0).unwrap().hypothetical_group().members()
/// }
///
/// fn terminal_members<'o>(
///     owner: &'o ClearanceSequenceOwner<'_>,
/// ) -> &'o [MarginMember] {
///     owner.terminal_run().members()
/// }
/// ```
///
/// ```no_run
/// use iced_css_core::formatting::{
///     clearance_owner::MarginMember,
///     clearance_sequence::{ClearanceSequenceOwner, SequenceNormalPlacement, SequenceNormalView},
/// };
///
/// fn view_from_owner<'o>(
///     owner: &'o ClearanceSequenceOwner<'_>,
/// ) -> SequenceNormalView<'o> {
///     owner.tail_view(0).unwrap()
/// }
///
/// fn placement<'o>(
///     owner: &'o ClearanceSequenceOwner<'_>,
/// ) -> &'o SequenceNormalPlacement {
///     owner.tail_view(0).unwrap().placement()
/// }
///
/// fn first_actual_members<'o>(
///     owner: &'o ClearanceSequenceOwner<'_>,
/// ) -> &'o [MarginMember] {
///     let mut groups = owner.tail_view(0).unwrap().actual_groups();
///     groups.next().unwrap().members()
/// }
/// ```
///
/// ```compile_fail,E0515
/// use iced_css_core::formatting::{
///     clearance_owner::MarginMember,
///     clearance_sequence::ClearanceSequenceOwner,
/// };
///
/// fn dangling_members<'s>(
///     owner: ClearanceSequenceOwner<'s>,
/// ) -> &'s [MarginMember] {
///     owner.tail_view(0).unwrap().hypothetical_group().members()
/// }
/// ```
///
/// ```compile_fail,E0451
/// use iced_css_core::formatting::{
///     clearance_owner::MarginMember,
///     clearance_sequence::{SequenceNormalPlacement, SequenceNormalView},
/// };
///
/// fn forge_view<'o>(
///     placement: &'o SequenceNormalPlacement,
///     members: &'o [MarginMember],
/// ) -> SequenceNormalView<'o> {
///     SequenceNormalView { placement, members }
/// }
/// ```
#[derive(Clone, Copy)]
pub struct SequenceNormalView<'o> {
    placement: &'o SequenceNormalPlacement,
    members: &'o [MarginMember],
}

impl<'o> SequenceNormalView<'o> {
    pub(in crate::formatting) fn new(
        placement: &'o SequenceNormalPlacement,
        members: &'o [MarginMember],
    ) -> Self {
        Self { placement, members }
    }

    pub fn placement(&self) -> &'o SequenceNormalPlacement {
        self.placement
    }

    pub fn hypothetical_group(&self) -> SequenceMarginGroupView<'o> {
        SequenceMarginGroupView::new(
            MarginGroupId::HypotheticalPair,
            self.placement.hypothetical_group,
            self.members,
        )
    }

    pub fn actual_groups(
        &self,
    ) -> impl ExactSizeIterator<Item = SequenceMarginGroupView<'o>> + DoubleEndedIterator + 'o + use<'o>
    {
        let placement = self.placement;
        let members = self.members;
        placement.actual_groups[..placement.actual_len]
            .iter()
            .copied()
            .enumerate()
            .map(move |(index, span)| {
                let id = if placement.actual_len == 1 {
                    MarginGroupId::ActualPair
                } else if index == 0 {
                    MarginGroupId::BeforeBarrier
                } else {
                    MarginGroupId::AfterBarrier
                };
                SequenceMarginGroupView::new(id, span, members)
            })
    }

    pub fn source_index(&self) -> usize {
        self.placement.source_index()
    }

    pub fn vertical(&self) -> BlockVertical {
        self.placement.vertical()
    }

    pub fn content_vertical(&self) -> BlockVertical {
        self.placement.content_vertical()
    }

    pub fn padding_vertical(&self) -> BlockVertical {
        self.placement.padding_vertical()
    }

    pub fn hypothetical_top(&self) -> f32 {
        self.placement.hypothetical_top()
    }

    pub fn zero_clearance_top(&self) -> Option<f32> {
        self.placement.zero_clearance_top()
    }

    pub fn clearance(&self) -> Option<f32> {
        self.placement.clearance()
    }

    pub fn barrier_source_index(&self) -> Option<usize> {
        self.placement.barrier_source_index()
    }
}

/// The caller supplies only earlier same-BFC floats and prepays their selection.
pub(in crate::formatting) struct FloatSelection<'f> {
    pub floats: &'f [PlacedFloat],
    pub limit: NonZeroUsize,
}

#[derive(Clone, Copy)]
pub(in crate::formatting) struct ChainState {
    base: f32,
    subtract: f32,
    pending: GroupSpan,
}

fn append(arena: &mut Vec<MarginMember>, source: usize, side: MarginSide, value: f32) -> usize {
    let index = arena.len();
    // The complete profile/count check proves the pre-reserved 2M bound.
    arena.push(MarginMember {
        source_index: source,
        side,
        value,
    });
    index
}

impl ChainState {
    pub(in crate::formatting) fn first(
        arena: &mut Vec<MarginMember>,
        before: ChildFacts<'_>,
        before_bottom: f32,
        parent_top: f32,
        through: bool,
    ) -> Result<Self, KernelError> {
        let result = (|| {
            let bottom = before.margin.used_bottom();
            let pending = if through {
                let top = before.margin.used_top();
                let start = append(arena, 2, MarginSide::Top, top);
                let span = GroupSpan::single(start, top)?;
                append(arena, 2, MarginSide::Bottom, bottom);
                span.extended(bottom)?
            } else {
                let start = append(arena, 2, MarginSide::Bottom, bottom);
                GroupSpan::single(start, bottom)?
            };
            Ok(Self {
                base: if through { parent_top } else { before_bottom },
                subtract: 0.0,
                pending,
            })
        })();
        result.map_err(|error| KernelError { source: 2, error })
    }

    pub(in crate::formatting) fn terminal(self) -> GroupSpan {
        self.pending
    }

    pub(in crate::formatting) fn place_next(
        &mut self,
        arena: &mut Vec<MarginMember>,
        next: ChildFacts<'_>,
        next_height: f32,
        source: usize,
        through: bool,
        placed_float: PlacedFloat,
    ) -> Result<SequenceNormalPlacement, KernelError> {
        self.place_next_with_floats(
            arena,
            next,
            next_height,
            source,
            through,
            FloatSelection {
                floats: &[placed_float],
                limit: NonZeroUsize::MIN,
            },
        )
    }

    pub(in crate::formatting) fn place_next_with_floats(
        &mut self,
        arena: &mut Vec<MarginMember>,
        next: ChildFacts<'_>,
        next_height: f32,
        source: usize,
        through: bool,
        selection: FloatSelection<'_>,
    ) -> Result<SequenceNormalPlacement, KernelError> {
        let result = (|| {
            let top = next.margin.used_top();
            let top_index = append(arena, source, MarginSide::Top, top);
            let preceding = self.pending;
            let hypothetical_group = preceding.extended(top)?;
            // Preserve the existing signed collapsed() f32 publication before the
            // Exact fold. Only the cleared-reference subtraction joins that fold.
            let hypothetical_top = sum(
                &[
                    self.base,
                    -self.subtract,
                    hypothetical_group.extrema.collapsed(),
                ],
                "hypothetical_top",
            )?;
            let selected =
                lowest_clearance_float_bottom(selection.floats, next.clear, selection.limit)
                    .map_err(ClearanceOwnerError::FloatSelection)?;
            let requirement =
                classify_clearance(hypothetical_top, selected).map_err(|error| match error {
                    ClearancePresenceError::UnsupportedEquality => {
                        ClearanceOwnerError::UnsupportedEquality
                    }
                    other => ClearanceOwnerError::Presence(other),
                })?;
            let (zero_clearance_top, state, actual_groups, actual_len) = match requirement {
                ClearanceRequirement::Absent => {
                    (None, ClearanceState::Absent, [hypothetical_group; 2], 1)
                }
                ClearanceRequirement::Required => {
                    let baseline = sum(
                        &[
                            self.base,
                            -self.subtract,
                            preceding.extrema.collapsed(),
                            top,
                        ],
                        "zero_clearance_top",
                    )?;
                    let bottom = selected.ok_or(ClearanceOwnerError::MissingRequiredFloat)?;
                    let after = GroupSpan::single(top_index, top)?;
                    (
                        Some(baseline),
                        ClearanceState::Required {
                            zero_clearance_border_top: baseline,
                            lowest_float_bottom: bottom,
                        },
                        [preceding, after],
                        2,
                    )
                }
            };
            let placement = solve_clearance(hypothetical_top, state).map_err(amount_error)?;
            let (content_vertical, padding_vertical, bottom) = normal_vertical(
                placement.border_top,
                next_height,
                next.border,
                next.padding,
                [
                    "after_content_top",
                    "after_padding_top",
                    "after_content_bottom",
                    "after_padding_bottom",
                    "after_bottom",
                ],
            )?;
            let bottom_margin = next.margin.used_bottom();
            let bottom_index = append(arena, source, MarginSide::Bottom, bottom_margin);
            if through {
                if placement.clearance.is_some() {
                    self.base = placement.border_top;
                    self.subtract = top;
                    self.pending = actual_groups[1].extended(bottom_margin)?;
                } else {
                    self.pending = hypothetical_group.extended(bottom_margin)?;
                }
            } else {
                self.base = bottom;
                self.subtract = 0.0;
                self.pending = GroupSpan::single(bottom_index, bottom_margin)?;
            }
            Ok(SequenceNormalPlacement {
                source_index: source,
                vertical: BlockVertical {
                    top: placement.border_top,
                    bottom,
                },
                content_vertical,
                padding_vertical,
                hypothetical_top,
                zero_clearance_top,
                clearance: placement.clearance,
                hypothetical_group,
                actual_groups,
                actual_len,
            })
        })();
        result.map_err(|error| KernelError { source, error })
    }
}
