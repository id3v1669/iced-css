//! Bounded vertical clearance ownership for exactly `[parent, B1, float, B2]`.
//!
//! Resolved style/content/context facts are explicit caller assertions, not facts
//! inferred from empty source children or captured native measurements. The parent
//! starts a new BFC with no earlier floats and independent top/bottom isolation;
//! ordinary positive-height B1/B2 cannot collapse through. This is not full layout.
use super::{
    arithmetic::Exact,
    clearance::{
        ClearSide, ClearanceError, ClearanceFloatError, ClearancePresenceError,
        ClearanceRequirement, ClearanceState, classify_clearance, lowest_clearance_float_bottom,
        solve_clearance,
    },
    display::{Display, DisplayResolution, Float, Flow, Position},
    floats::{Band, FloatError, FloatRequest, FloatSide, PlacedFloat, place_float},
    margins::{MarginError, MarginExtrema},
};
use crate::source::SourceNode;
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerContent {
    NoOwnContentNonReplaced,
    TextOrRichText,
    Replaced,
    RecipeRequired,
    OpaqueOrUnknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerGenerated {
    None,
    Present,
    Unresolved,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentBfc {
    NewWithEmptyPrefix,
    OtherOrUnknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildContext {
    OrdinaryNonBfc,
    FloatBfc,
    OtherNewBfc,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OwnerHeight {
    /// Used CSS height after applicable upstream min/max processing. For B1/B2,
    /// this also asserts a positive explicit *computed* height category: callers
    /// must not relabel auto, percentage, zero-computed, or native-derived height.
    /// SourceNode cannot prove that provenance; the owner checks the used payload.
    Explicit(f32),
    Auto,
    Percentage(f32),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnerEdges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
#[derive(Debug, Clone, Copy)]
pub struct ParentFacts<'s> {
    pub source: &'s SourceNode,
    pub display: DisplayResolution,
    pub content: OwnerContent,
    pub generated: OwnerGenerated,
    pub bfc: ParentBfc,
    pub content_top: f32,
    pub containing: Band,
    pub border: OwnerEdges,
    pub padding: OwnerEdges,
}
#[derive(Debug, Clone, Copy)]
pub struct ChildFacts<'s> {
    pub source: &'s SourceNode,
    pub display: DisplayResolution,
    pub context: ChildContext,
    pub content: OwnerContent,
    pub generated: OwnerGenerated,
    pub height: OwnerHeight,
    pub margin: OwnerEdges,
    pub border: OwnerEdges,
    pub padding: OwnerEdges,
    pub clear: ClearSide,
}
#[derive(Debug, Clone, Copy)]
pub struct ClearanceOwnerInput<'s> {
    pub parent: ParentFacts<'s>,
    pub before: ChildFacts<'s>,
    pub float: ChildFacts<'s>,
    pub after: ChildFacts<'s>,
    pub float_width: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerProfileReason {
    DisplayOrFlow,
    ChildContext,
    Content,
    GeneratedContent,
    ParentContext,
    ParentIsolation,
    HeightKind,
    NonpositiveBlockHeight,
    NegativeMargin,
    HorizontalMargin,
    ChildDecoration,
    FloatMargin,
    ClearOnBeforeOrFloat,
    FloatTooWide,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearanceOwnerError {
    NodeCount,
    ChildCount {
        source: usize,
    },
    SourcePointer {
        source: usize,
    },
    DuplicateInstance {
        first: usize,
        second: usize,
    },
    ParentMembership {
        source: usize,
    },
    ChildMembership {
        slot: usize,
    },
    InvalidNumber {
        source: usize,
        field: &'static str,
    },
    Unsupported {
        source: usize,
        reason: OwnerProfileReason,
    },
    Unrepresentable {
        stage: &'static str,
    },
    UnsupportedEquality,
    /// The presence classifier required clearance without a selected float.
    /// This indicates an internal primitive contract inconsistency.
    MissingRequiredFloat,
    FloatPlacement(FloatError),
    FloatSelection(ClearanceFloatError),
    Presence(ClearancePresenceError),
    Amount(ClearanceError),
    Margin(MarginError),
}
impl std::fmt::Display for ClearanceOwnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "clearance owner: {self:?}")
    }
}
impl std::error::Error for ClearanceOwnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FloatPlacement(e) => Some(e),
            Self::FloatSelection(e) => Some(e),
            Self::Presence(e) => Some(e),
            Self::Amount(e) => Some(e),
            Self::Margin(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginSide {
    Top,
    Bottom,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarginMember {
    pub source_index: usize,
    pub side: MarginSide,
    pub value: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginGroupId {
    HypotheticalPair,
    ActualPair,
    BeforeBarrier,
    AfterBarrier,
}
/// Immutable owner-local membership. Indices resolve through the owning source slice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarginGroup {
    id: MarginGroupId,
    members: [MarginMember; 2],
    len: usize,
    extrema: MarginExtrema,
}
impl MarginGroup {
    pub fn id(&self) -> MarginGroupId {
        self.id
    }
    pub fn members(&self) -> &[MarginMember] {
        &self.members[..self.len]
    }
    pub fn positive(&self) -> f32 {
        self.extrema.positive()
    }
    pub fn negative(&self) -> f32 {
        self.extrema.negative()
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockVertical {
    pub top: f32,
    pub bottom: f32,
}
/// Borrows actual source storage, independently of the temporary facts wrapper.
#[derive(Debug)]
pub struct ClearanceOwner<'s> {
    sources: &'s [SourceNode],
    before: BlockVertical,
    placed_float: PlacedFloat,
    after: BlockVertical,
    hypothetical_top: f32,
    zero_clearance_top: Option<f32>,
    clearance: Option<f32>,
    hypothetical_group: MarginGroup,
    actual_groups: [MarginGroup; 2],
    actual_len: usize,
}
impl<'s> ClearanceOwner<'s> {
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
    pub fn after(&self) -> BlockVertical {
        self.after
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
        self.clearance.map(|_| 3)
    }
}
fn unsupported(source: usize, reason: OwnerProfileReason) -> ClearanceOwnerError {
    ClearanceOwnerError::Unsupported { source, reason }
}
fn finite(value: f32, source: usize, field: &'static str) -> Result<(), ClearanceOwnerError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ClearanceOwnerError::InvalidNumber { source, field })
    }
}
fn nonnegative(value: f32, source: usize, field: &'static str) -> Result<(), ClearanceOwnerError> {
    finite(value, source, field)?;
    if value < 0. {
        Err(ClearanceOwnerError::InvalidNumber { source, field })
    } else {
        Ok(())
    }
}
fn edge_values(edges: OwnerEdges) -> [f32; 4] {
    [edges.top, edges.right, edges.bottom, edges.left]
}
fn check_edges(
    edges: OwnerEdges,
    source: usize,
    field: &'static str,
    decoration: bool,
) -> Result<(), ClearanceOwnerError> {
    let values = edge_values(edges);
    for value in values {
        finite(value, source, field)?;
    }
    if decoration {
        for value in values {
            nonnegative(value, source, field)?;
        }
    } else if values.into_iter().any(|v| v < 0.) {
        return Err(unsupported(source, OwnerProfileReason::NegativeMargin));
    }
    Ok(())
}
fn display(facts: DisplayResolution, source: usize) -> Result<(), ClearanceOwnerError> {
    let valid = facts.display == Display::Block
        && facts.position == Position::Static
        && if source == 2 {
            facts.flow == Flow::Floating && matches!(facts.float, Float::Left | Float::Right)
        } else {
            facts.flow == Flow::Normal && facts.float == Float::None
        };
    if valid {
        Ok(())
    } else {
        Err(unsupported(source, OwnerProfileReason::DisplayOrFlow))
    }
}
fn content(c: OwnerContent, g: OwnerGenerated, source: usize) -> Result<(), ClearanceOwnerError> {
    if c != OwnerContent::NoOwnContentNonReplaced {
        return Err(unsupported(source, OwnerProfileReason::Content));
    }
    if g != OwnerGenerated::None {
        return Err(unsupported(source, OwnerProfileReason::GeneratedContent));
    }
    Ok(())
}
fn parent(f: ParentFacts<'_>) -> Result<(), ClearanceOwnerError> {
    display(f.display, 0)?;
    content(f.content, f.generated, 0)?;
    if f.bfc != ParentBfc::NewWithEmptyPrefix {
        return Err(unsupported(0, OwnerProfileReason::ParentContext));
    }
    finite(f.content_top, 0, "content_top")?;
    finite(f.containing.left, 0, "containing_left")?;
    finite(f.containing.right, 0, "containing_right")?;
    if f.containing.left > f.containing.right {
        return Err(ClearanceOwnerError::InvalidNumber {
            source: 0,
            field: "containing_order",
        });
    }
    check_edges(f.border, 0, "border", true)?;
    check_edges(f.padding, 0, "padding", true)?;
    if !(f.border.top > 0. || f.padding.top > 0.)
        || !(f.border.bottom > 0. || f.padding.bottom > 0.)
    {
        return Err(unsupported(0, OwnerProfileReason::ParentIsolation));
    }
    Ok(())
}
fn child(f: ChildFacts<'_>, source: usize) -> Result<f32, ClearanceOwnerError> {
    display(f.display, source)?;
    if f.context
        != if source == 2 {
            ChildContext::FloatBfc
        } else {
            ChildContext::OrdinaryNonBfc
        }
    {
        return Err(unsupported(source, OwnerProfileReason::ChildContext));
    }
    content(f.content, f.generated, source)?;
    let OwnerHeight::Explicit(height) = f.height else {
        return Err(unsupported(source, OwnerProfileReason::HeightKind));
    };
    finite(height, source, "height")?;
    if source == 2 {
        nonnegative(height, source, "height")?;
    } else if height <= 0. {
        return Err(unsupported(
            source,
            OwnerProfileReason::NonpositiveBlockHeight,
        ));
    }
    check_edges(f.margin, source, "margin", false)?;
    if source == 2 {
        if edge_values(f.margin).into_iter().any(|v| v != 0.) {
            return Err(unsupported(source, OwnerProfileReason::FloatMargin));
        }
    } else if f.margin.left != 0. || f.margin.right != 0. {
        return Err(unsupported(source, OwnerProfileReason::HorizontalMargin));
    }
    for (edges, field) in [(f.border, "border"), (f.padding, "padding")] {
        check_edges(edges, source, field, true)?;
        if edge_values(edges).into_iter().any(|v| v != 0.) {
            return Err(unsupported(source, OwnerProfileReason::ChildDecoration));
        }
    }
    if source != 3 && f.clear != ClearSide::None {
        return Err(unsupported(
            source,
            OwnerProfileReason::ClearOnBeforeOrFloat,
        ));
    }
    Ok(height)
}
fn sum(terms: &[f32], stage: &'static str) -> Result<f32, ClearanceOwnerError> {
    terms
        .iter()
        .fold(Exact::ZERO, |acc, &term| acc.add(Exact::from_f32(term)))
        .to_f32()
        .map_err(|_| ClearanceOwnerError::Unrepresentable { stage })
}
fn group(
    id: MarginGroupId,
    members: [MarginMember; 2],
    len: usize,
) -> Result<MarginGroup, ClearanceOwnerError> {
    let mut extrema = MarginExtrema::default();
    let values = [members[0].value, members[1].value];
    extrema
        .extend(&values[..len], NonZeroUsize::MIN.saturating_add(1))
        .map_err(ClearanceOwnerError::Margin)?;
    Ok(MarginGroup {
        id,
        members,
        len,
        extrema,
    })
}
fn amount_error(error: ClearanceError) -> ClearanceOwnerError {
    match error {
        ClearanceError::Unrepresentable => ClearanceOwnerError::Unrepresentable {
            stage: "clearance_amount",
        },
        other => ClearanceOwnerError::Amount(other),
    }
}
/// Admit and solve the fixed source profile without allocating owner storage.
///
/// Child counts precede any child traversal; pointer joins and distinct IDs precede
/// reciprocal membership. Facts are checked in source/field order, then geometry
/// consumes published f32 stage boundaries. Each owner sum rounds once from Exact;
/// float placement uses the existing checked producer's actual output.
///
/// # Errors
/// Returns typed shape/profile/measurement errors before geometry. Arithmetic
/// overflow identifies its stage; primitive errors retain their typed causes.
/// Equal hypothetical/relevant-bottom edges remain explicitly unsupported.
pub fn build_clearance_owner<'s>(
    sources: &'s [SourceNode],
    input: ClearanceOwnerInput<'s>,
) -> Result<ClearanceOwner<'s>, ClearanceOwnerError> {
    if sources.len() != 4 {
        return Err(ClearanceOwnerError::NodeCount);
    }
    for (source, node) in sources.iter().enumerate() {
        if node.children.len() != if source == 0 { 3 } else { 0 } {
            return Err(ClearanceOwnerError::ChildCount { source });
        }
    }
    for (source, fact) in [
        input.parent.source,
        input.before.source,
        input.float.source,
        input.after.source,
    ]
    .into_iter()
    .enumerate()
    {
        if !std::ptr::eq(fact, &sources[source]) || fact.instance != sources[source].instance {
            return Err(ClearanceOwnerError::SourcePointer { source });
        }
    }
    for first in 0..4 {
        for second in first + 1..4 {
            if sources[first].instance == sources[second].instance {
                return Err(ClearanceOwnerError::DuplicateInstance { first, second });
            }
        }
    }
    for (source, node) in sources.iter().enumerate() {
        if node.parent
            != if source == 0 {
                None
            } else {
                Some(sources[0].instance)
            }
        {
            return Err(ClearanceOwnerError::ParentMembership { source });
        }
    }
    for (slot, &id) in sources[0].children.iter().enumerate() {
        if id != sources[slot + 1].instance {
            return Err(ClearanceOwnerError::ChildMembership { slot });
        }
    }
    parent(input.parent)?;
    let before_height = child(input.before, 1)?;
    let float_height = child(input.float, 2)?;
    let after_height = child(input.after, 3)?;
    nonnegative(input.float_width, 2, "float_width")?;
    if Exact::from_f32(input.float_width)
        > Exact::from_f32(input.parent.containing.right)
            .sub(Exact::from_f32(input.parent.containing.left))
    {
        return Err(unsupported(2, OwnerProfileReason::FloatTooWide));
    }
    let before_top = sum(
        &[input.parent.content_top, input.before.margin.top],
        "before_top",
    )?;
    let before_bottom = sum(&[before_top, before_height], "before_bottom")?;
    let float_top = sum(&[before_bottom, input.before.margin.bottom], "float_top")?;
    let placed_float = place_float(
        &[],
        input.parent.containing,
        FloatRequest {
            side: match input.float.display.float {
                Float::Right => FloatSide::Right,
                _ => FloatSide::Left,
            },
            width: input.float_width,
            height: float_height,
            containing_top: input.parent.content_top,
            hypothetical_top: float_top,
        },
        NonZeroUsize::MIN,
    )
    .map_err(ClearanceOwnerError::FloatPlacement)?
    .placed;
    let members = [
        MarginMember {
            source_index: 1,
            side: MarginSide::Bottom,
            value: input.before.margin.bottom,
        },
        MarginMember {
            source_index: 3,
            side: MarginSide::Top,
            value: input.after.margin.top,
        },
    ];
    let hypothetical_group = group(MarginGroupId::HypotheticalPair, members, 2)?;
    let hypothetical_top = sum(
        &[before_bottom, hypothetical_group.extrema.collapsed()],
        "hypothetical_top",
    )?;
    let selected =
        lowest_clearance_float_bottom(&[placed_float], input.after.clear, NonZeroUsize::MIN)
            .map_err(ClearanceOwnerError::FloatSelection)?;
    let requirement = classify_clearance(hypothetical_top, selected).map_err(|e| match e {
        ClearancePresenceError::UnsupportedEquality => ClearanceOwnerError::UnsupportedEquality,
        other => ClearanceOwnerError::Presence(other),
    })?;
    let (zero_clearance_top, state, actual_groups, actual_len) = match requirement {
        ClearanceRequirement::Absent => {
            let pair = MarginGroup {
                id: MarginGroupId::ActualPair,
                ..hypothetical_group
            };
            (None, ClearanceState::Absent, [pair, pair], 1)
        }
        ClearanceRequirement::Required => {
            let baseline = sum(
                &[before_bottom, members[0].value, members[1].value],
                "zero_clearance_top",
            )?;
            // Required can only result from Some(bottom); do not invent a
            // primitive validation cause if that internal contract ever changes.
            let bottom = selected.ok_or(ClearanceOwnerError::MissingRequiredFloat)?;
            (
                Some(baseline),
                ClearanceState::Required {
                    zero_clearance_border_top: baseline,
                    lowest_float_bottom: bottom,
                },
                [
                    group(MarginGroupId::BeforeBarrier, [members[0], members[0]], 1)?,
                    group(MarginGroupId::AfterBarrier, [members[1], members[1]], 1)?,
                ],
                2,
            )
        }
    };
    let placement = solve_clearance(hypothetical_top, state).map_err(amount_error)?;
    let after_bottom = sum(&[placement.border_top, after_height], "after_bottom")?;
    Ok(ClearanceOwner {
        sources,
        before: BlockVertical {
            top: before_top,
            bottom: before_bottom,
        },
        placed_float,
        after: BlockVertical {
            top: placement.border_top,
            bottom: after_bottom,
        },
        hypothetical_top,
        zero_clearance_top,
        clearance: placement.clearance,
        hypothetical_group,
        actual_groups,
        actual_len,
    })
}
