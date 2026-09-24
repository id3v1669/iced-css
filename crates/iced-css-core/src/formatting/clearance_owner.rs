//! Bounded vertical clearance ownership for a four-source region, optionally
//! beneath a sole outer source retained in the full five-source graph.
//!
//! Resolved style/content/context facts are explicit caller assertions, not facts
//! inferred from empty source children or captured native measurements. The parent
//! starts a new BFC with no earlier floats and independent top/bottom isolation;
//! ordinary positive-height B1/B2 cannot collapse through. This is not full layout.
use super::{
    arithmetic::Exact,
    block::AutoLength,
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
/// Computed child margins for the bounded clearance inputs.
/// Auto remains a category, never a substituted computed zero. The explicit
/// used-value methods resolve only the bounded vertical category; admission and
/// validation remain separate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChildMargins {
    pub top: AutoLength,
    pub right: AutoLength,
    pub bottom: AutoLength,
    pub left: AutoLength,
}

impl ChildMargins {
    pub fn used_top(self) -> f32 {
        match self.top {
            AutoLength::Auto => 0.0,
            AutoLength::Value(value) => value,
        }
    }

    pub fn used_bottom(self) -> f32 {
        match self.bottom {
            AutoLength::Auto => 0.0,
            AutoLength::Value(value) => value,
        }
    }
}

impl From<OwnerEdges> for ChildMargins {
    fn from(edges: OwnerEdges) -> Self {
        Self {
            top: AutoLength::Value(edges.top),
            right: AutoLength::Value(edges.right),
            bottom: AutoLength::Value(edges.bottom),
            left: AutoLength::Value(edges.left),
        }
    }
}
/// Explicit outer source facts; no root formatting context or geometry is asserted.
#[derive(Debug, Clone, Copy)]
pub struct OuterFacts<'s> {
    pub source: &'s SourceNode,
    pub display: DisplayResolution,
    pub content: OwnerContent,
    pub generated: OwnerGenerated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerSourceIndices {
    pub parent: usize,
    pub before: usize,
    pub float: usize,
    pub after: usize,
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
    pub margin: ChildMargins,
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
    AutoVerticalMargin,
    AutoHorizontalMargin,
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
        source: usize,
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
    indices: OwnerSourceIndices,
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
    pub fn source_indices(&self) -> OwnerSourceIndices {
        self.indices
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
        self.clearance.map(|_| self.indices.after)
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
fn display(
    facts: DisplayResolution,
    source: usize,
    floating: bool,
) -> Result<(), ClearanceOwnerError> {
    let valid = facts.display == Display::Block
        && facts.position == Position::Static
        && if floating {
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
fn parent(f: ParentFacts<'_>, source: usize) -> Result<(), ClearanceOwnerError> {
    display(f.display, source, false)?;
    content(f.content, f.generated, source)?;
    if f.bfc != ParentBfc::NewWithEmptyPrefix {
        return Err(unsupported(source, OwnerProfileReason::ParentContext));
    }
    finite(f.content_top, source, "content_top")?;
    finite(f.containing.left, source, "containing_left")?;
    finite(f.containing.right, source, "containing_right")?;
    if f.containing.left > f.containing.right {
        return Err(ClearanceOwnerError::InvalidNumber {
            source,
            field: "containing_order",
        });
    }
    check_edges(f.border, source, "border", true)?;
    check_edges(f.padding, source, "padding", true)?;
    if !(f.border.top > 0. || f.padding.top > 0.)
        || !(f.border.bottom > 0. || f.padding.bottom > 0.)
    {
        return Err(unsupported(source, OwnerProfileReason::ParentIsolation));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChildMarginPolicy {
    Legacy,
    Sequence,
}

fn child(
    f: ChildFacts<'_>,
    source: usize,
    floating: bool,
    after: bool,
    margin_policy: ChildMarginPolicy,
) -> Result<f32, ClearanceOwnerError> {
    display(f.display, source, floating)?;
    if f.context
        != if floating {
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
    if floating {
        nonnegative(height, source, "height")?;
    } else if height <= 0. {
        return Err(unsupported(
            source,
            OwnerProfileReason::NonpositiveBlockHeight,
        ));
    }
    let values = [f.margin.top, f.margin.right, f.margin.bottom, f.margin.left];
    for value in values {
        if let AutoLength::Value(value) = value {
            finite(value, source, "margin")?;
        }
    }
    if !floating && margin_policy == ChildMarginPolicy::Sequence {
        if matches!(f.margin.top, AutoLength::Value(v) if v < 0.0)
            || matches!(f.margin.bottom, AutoLength::Value(v) if v < 0.0)
        {
            return Err(unsupported(source, OwnerProfileReason::NegativeMargin));
        }
    } else {
        if values
            .into_iter()
            .any(|v| matches!(v, AutoLength::Value(v) if v < 0.0))
        {
            return Err(unsupported(source, OwnerProfileReason::NegativeMargin));
        }
        if margin_policy == ChildMarginPolicy::Legacy {
            if matches!(f.margin.top, AutoLength::Auto)
                || matches!(f.margin.bottom, AutoLength::Auto)
            {
                return Err(unsupported(source, OwnerProfileReason::AutoVerticalMargin));
            }
            if matches!(f.margin.right, AutoLength::Auto)
                || matches!(f.margin.left, AutoLength::Auto)
            {
                return Err(unsupported(
                    source,
                    OwnerProfileReason::AutoHorizontalMargin,
                ));
            }
        }
        if floating {
            if values
                .into_iter()
                .any(|v| matches!(v, AutoLength::Value(v) if v != 0.0))
            {
                return Err(unsupported(source, OwnerProfileReason::FloatMargin));
            }
        } else if f.margin.left != AutoLength::Value(0.0)
            || f.margin.right != AutoLength::Value(0.0)
        {
            return Err(unsupported(source, OwnerProfileReason::HorizontalMargin));
        }
    }
    for (edges, field) in [(f.border, "border"), (f.padding, "padding")] {
        check_edges(edges, source, field, true)?;
        if edge_values(edges).into_iter().any(|v| v != 0.) {
            return Err(unsupported(source, OwnerProfileReason::ChildDecoration));
        }
    }
    if !after && f.clear != ClearSide::None {
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

pub(super) fn validate_outer(
    facts: OuterFacts<'_>,
    source: usize,
) -> Result<(), ClearanceOwnerError> {
    display(facts.display, source, false)?;
    content(facts.content, facts.generated, source)
}

pub(super) fn validate_parent(
    facts: ParentFacts<'_>,
    source: usize,
) -> Result<(), ClearanceOwnerError> {
    parent(facts, source)
}

pub(super) fn validate_child(
    facts: ChildFacts<'_>,
    source: usize,
    floating: bool,
    after: bool,
) -> Result<f32, ClearanceOwnerError> {
    child(facts, source, floating, after, ChildMarginPolicy::Legacy)
}

pub(super) fn validate_sequence_child(
    facts: ChildFacts<'_>,
    source: usize,
    floating: bool,
    after: bool,
) -> Result<f32, ClearanceOwnerError> {
    child(facts, source, floating, after, ChildMarginPolicy::Sequence)
}

pub(super) fn validate_float_width(
    parent: ParentFacts<'_>,
    width: f32,
    source: usize,
) -> Result<(), ClearanceOwnerError> {
    nonnegative(width, source, "float_width")?;
    if Exact::from_f32(width)
        > Exact::from_f32(parent.containing.right).sub(Exact::from_f32(parent.containing.left))
    {
        Err(unsupported(source, OwnerProfileReason::FloatTooWide))
    } else {
        Ok(())
    }
}

pub(super) fn validate_sequence_float_width(
    width: f32,
    source: usize,
) -> Result<(), ClearanceOwnerError> {
    nonnegative(width, source, "float_width")
}

pub(super) struct KernelError {
    pub source: usize,
    pub error: ClearanceOwnerError,
}

pub(super) struct FirstFloatKernel {
    pub before: BlockVertical,
    pub placed_float: PlacedFloat,
}

pub(super) struct FirstFloatInput<'s> {
    pub parent: ParentFacts<'s>,
    pub before: ChildFacts<'s>,
    pub before_height: f32,
    pub before_source: usize,
    pub float: ChildFacts<'s>,
    pub float_height: f32,
    pub float_width: f32,
    pub float_source: usize,
}

pub(super) struct NormalKernel {
    pub vertical: BlockVertical,
    pub hypothetical_top: f32,
    pub zero_clearance_top: Option<f32>,
    pub clearance: Option<f32>,
    pub hypothetical_group: MarginGroup,
    pub actual_groups: [MarginGroup; 2],
    pub actual_len: usize,
}

pub(super) fn place_first_float(
    input: FirstFloatInput<'_>,
) -> Result<FirstFloatKernel, KernelError> {
    let before_top = sum(
        &[input.parent.content_top, input.before.margin.used_top()],
        "before_top",
    )
    .map_err(|error| KernelError {
        source: input.before_source,
        error,
    })?;
    let before_bottom =
        sum(&[before_top, input.before_height], "before_bottom").map_err(|error| KernelError {
            source: input.before_source,
            error,
        })?;
    let float_top = sum(
        &[before_bottom, input.before.margin.used_bottom()],
        "float_top",
    )
    .map_err(|error| KernelError {
        source: input.float_source,
        error,
    })?;
    let placed_float = place_float(
        &[],
        input.parent.containing,
        // A float admitted here has zero absolute or Auto margins. Both
        // categories have used-zero margins, so content dimensions remain the
        // unchanged margin-box dimensions for this zero-decoration profile.
        FloatRequest {
            side: match input.float.display.float {
                Float::Right => FloatSide::Right,
                _ => FloatSide::Left,
            },
            width: input.float_width,
            height: input.float_height,
            containing_top: input.parent.content_top,
            hypothetical_top: float_top,
        },
        NonZeroUsize::MIN,
    )
    .map_err(|error| KernelError {
        source: input.float_source,
        error: ClearanceOwnerError::FloatPlacement(error),
    })?
    .placed;
    Ok(FirstFloatKernel {
        before: BlockVertical {
            top: before_top,
            bottom: before_bottom,
        },
        placed_float,
    })
}

pub(super) fn place_next_normal(
    previous_border_bottom: f32,
    previous_bottom_margin: f32,
    previous_source: usize,
    next: ChildFacts<'_>,
    next_height: f32,
    next_source: usize,
    placed_float: PlacedFloat,
) -> Result<NormalKernel, KernelError> {
    let result = (|| {
        let members = [
            MarginMember {
                source_index: previous_source,
                side: MarginSide::Bottom,
                value: previous_bottom_margin,
            },
            MarginMember {
                source_index: next_source,
                side: MarginSide::Top,
                value: next.margin.used_top(),
            },
        ];
        let hypothetical_group = group(MarginGroupId::HypotheticalPair, members, 2)?;
        let hypothetical_top = sum(
            &[
                previous_border_bottom,
                hypothetical_group.extrema.collapsed(),
            ],
            "hypothetical_top",
        )?;
        let selected =
            lowest_clearance_float_bottom(&[placed_float], next.clear, NonZeroUsize::MIN)
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
                let pair = MarginGroup {
                    id: MarginGroupId::ActualPair,
                    ..hypothetical_group
                };
                (None, ClearanceState::Absent, [pair, pair], 1)
            }
            ClearanceRequirement::Required => {
                let baseline = sum(
                    &[previous_border_bottom, members[0].value, members[1].value],
                    "zero_clearance_top",
                )?;
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
        let bottom = sum(&[placement.border_top, next_height], "after_bottom")?;
        Ok(NormalKernel {
            vertical: BlockVertical {
                top: placement.border_top,
                bottom,
            },
            hypothetical_top,
            zero_clearance_top,
            clearance: placement.clearance,
            hypothetical_group,
            actual_groups,
            actual_len,
        })
    })();
    result.map_err(|error| KernelError {
        source: next_source,
        error,
    })
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
            return Err(ClearanceOwnerError::ChildMembership { source: 0, slot });
        }
    }
    solve_region(
        sources,
        OwnerSourceIndices {
            parent: 0,
            before: 1,
            float: 2,
            after: 3,
        },
        input,
    )
}
/// Admit the entire `[outer, parent, B1, float, B2]` graph, then solve its inner region.
/// The outer facts do not establish a root BFC or solve the outer box.
/// # Errors
/// Rejects invalid full-document membership or unsupported facts before geometry.
pub fn build_nested_clearance_owner<'s>(
    sources: &'s [SourceNode],
    outer: OuterFacts<'s>,
    input: ClearanceOwnerInput<'s>,
) -> Result<ClearanceOwner<'s>, ClearanceOwnerError> {
    if sources.len() != 5 {
        return Err(ClearanceOwnerError::NodeCount);
    }
    for (source, expected) in [1, 3, 0, 0, 0].into_iter().enumerate() {
        if sources[source].children.len() != expected {
            return Err(ClearanceOwnerError::ChildCount { source });
        }
    }
    for (source, fact) in [
        outer.source,
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
    for first in 0..5 {
        for second in first + 1..5 {
            if sources[first].instance == sources[second].instance {
                return Err(ClearanceOwnerError::DuplicateInstance { first, second });
            }
        }
    }
    for (source, parent) in [None, Some(0), Some(1), Some(1), Some(1)]
        .into_iter()
        .enumerate()
    {
        if sources[source].parent != parent.map(|p| sources[p].instance) {
            return Err(ClearanceOwnerError::ParentMembership { source });
        }
    }
    for (source, children) in [(0, &[1][..]), (1, &[2, 3, 4][..])] {
        for (slot, &child) in children.iter().enumerate() {
            if sources[source].children[slot] != sources[child].instance {
                return Err(ClearanceOwnerError::ChildMembership { source, slot });
            }
        }
    }
    validate_outer(outer, 0)?;
    solve_region(
        sources,
        OwnerSourceIndices {
            parent: 1,
            before: 2,
            float: 3,
            after: 4,
        },
        input,
    )
}
fn solve_region<'s>(
    sources: &'s [SourceNode],
    indices: OwnerSourceIndices,
    input: ClearanceOwnerInput<'s>,
) -> Result<ClearanceOwner<'s>, ClearanceOwnerError> {
    validate_parent(input.parent, indices.parent)?;
    let before_height = validate_child(input.before, indices.before, false, false)?;
    let float_height = validate_child(input.float, indices.float, true, false)?;
    let after_height = validate_child(input.after, indices.after, false, true)?;
    validate_float_width(input.parent, input.float_width, indices.float)?;
    let first = place_first_float(FirstFloatInput {
        parent: input.parent,
        before: input.before,
        before_height,
        before_source: indices.before,
        float: input.float,
        float_height,
        float_width: input.float_width,
        float_source: indices.float,
    })
    .map_err(|failure| failure.error)?;
    let after = place_next_normal(
        first.before.bottom,
        input.before.margin.used_bottom(),
        indices.before,
        input.after,
        after_height,
        indices.after,
        first.placed_float,
    )
    .map_err(|failure| failure.error)?;
    Ok(ClearanceOwner {
        sources,
        indices,
        before: first.before,
        placed_float: first.placed_float,
        after: after.vertical,
        hypothetical_top: after.hypothetical_top,
        zero_clearance_top: after.zero_clearance_top,
        clearance: after.clearance,
        hypothetical_group: after.hypothetical_group,
        actual_groups: after.actual_groups,
        actual_len: after.actual_len,
    })
}
