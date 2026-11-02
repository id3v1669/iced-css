//! Bounded vertical clearance ownership for a four-source region, optionally
//! beneath a sole outer source retained in the full five-source graph.
//!
//! Resolved style/content/context facts are explicit caller assertions, not facts
//! inferred from empty source children or captured native measurements. The parent
//! starts a new BFC with no earlier floats. The Legacy owners also require
//! independent top/bottom isolation (a nonzero border or padding on each side); the
//! Sequence owner does not, because a new BFC root never collapses margins with its
//! in-flow children (CSS2.2 8.3.1). Ordinary positive-height B1/B2 cannot collapse
//! through; nor can a Sequence normal whose own top and bottom margins are not adjoining
//! (G6a), which may have zero height. CTB/CTT (the user's U1=A): a zero-height ordinary
//! Sequence normal whose own margins adjoin admits finite signed vertical margins (S4).
//! NaN and negative-infinity normal margins retain the earlier eligibility refusal;
//! positive infinity reaches source-ordered finite validation. The Sequence owner also admits a relatively
//! positioned P or child (REL, CSS2.2 9.4.3) and computes its normal-flow geometry, which
//! the offsets do not
//! change; the outer source and every Legacy source stay static. This is not full
//! layout.
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
    /// Used CSS height after applicable upstream min/max processing (CSS2.2 10.7). For
    /// B1/B2 this also asserts a positive used height of an explicit computed height,
    /// or (MMX, Sequence only) of a normal whose computed Auto or zero height is raised
    /// by a positive `min-height` (10.6.3 gives the tentative zero; such a box cannot
    /// collapse through, 8.3.1). (G6a) A Sequence normal may also assert +0 when its own
    /// top and bottom margins are not adjoining (8.3.1): it establishes a new block
    /// formatting context or has a nonzero top or bottom border or padding. (CTB/CTT, the
    /// user's U1=A) An ordinary undecorated Sequence normal asserting +0 or -0 asserts that
    /// its own margins adjoin: its computed `height` is zero or `auto` and its computed
    /// `min-height` is zero, neither a percentage nor a length rounded to zero. A Sequence
    /// percentage `height` resolved by the caller against its containing block's explicit
    /// used content height (PHT, CSS2.2 10.5) is
    /// such a used length; callers must not relabel native-derived height. SourceNode
    /// cannot prove that provenance; the owner checks the used payload.
    Explicit(f32),
    /// NZS (CSS2.2 8.3.1): a Sequence normal's used height is exactly +0 or -0,
    /// but its computed height or minimum does not qualify for own-margin adjacency.
    /// This caller assertion preserves the zero payload; the owner cannot recover
    /// the original CSS facts. Sequence normals validate finiteness and exact zero.
    /// Legacy owners and the Sequence float reject this category as HeightKind.
    NonAdjoiningZero(f32),
    /// Used as Auto: a computed Auto height, or a Sequence percentage `height` used as
    /// `auto` because its containing block's height depends on content (PHT, CSS2.2
    /// 10.5); only the childless Sequence float resolves this to positive zero.
    /// Legacy owners and every normal child reject this category.
    Auto,
    /// An unresolved percentage: every owner rejects it (`HeightKind`).
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
/// used-value methods resolve the bounded vertical category (`used_top`,
/// `used_bottom`) and the floating nonreplaced horizontal category
/// (`float_used_left`, `float_used_right`); admission and validation remain
/// separate.
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

    /// Used value for the floating nonreplaced category only (CSS2.2 §10.3.5):
    /// Auto is +0; an absolute payload keeps its bits. Normal children use the
    /// block width solver instead. This does not validate or extend admission.
    pub fn float_used_left(self) -> f32 {
        match self.left {
            AutoLength::Auto => 0.0,
            AutoLength::Value(value) => value,
        }
    }

    /// Used value for the floating nonreplaced category only (CSS2.2 §10.3.5):
    /// Auto is +0; an absolute payload keeps its bits. Normal children use the
    /// block width solver instead. This does not validate or extend admission.
    pub fn float_used_right(self) -> f32 {
        match self.right {
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
    /// Legacy only: P has zero border and padding on its top side or on its bottom
    /// side. Not produced by the Sequence owner, whose P is a new BFC root.
    ParentIsolation,
    HeightKind,
    /// A normal's used height is negative, or +0 while its own top and bottom margins may
    /// adjoin (Sequence, G6a) and a normal's used vertical margin is NaN or negative
    /// infinity (S4 preserves this invalid-input precedence), or always (Legacy).
    NonpositiveBlockHeight,
    NegativeMargin,
    AutoVerticalMargin,
    AutoHorizontalMargin,
    HorizontalMargin,
    /// Legacy only: any nonzero child border or padding. Not produced by the Sequence
    /// owner, which admits finite nonnegative child borders and padding.
    ChildDecoration,
    /// Under `ChildMarginPolicy::Legacy`: any nonzero absolute float margin on any
    /// edge. Not produced under `ChildMarginPolicy::Sequence`.
    FloatMargin,
    /// Legacy fixed and nested owners (and so the captured-Legacy bridge) only:
    /// `clear` other than none on B1 or the float. Not produced by the Sequence
    /// owner: no float precedes either box in the parent's new block formatting
    /// context, so CSS2.2 §9.5.2 gives their `clear` no effect.
    ClearOnBeforeOrFloat,
    FloatTooWide,
    /// Sequence owner only: an outer extent of the float is negative, that is
    /// `margin-left + width + margin-right` or `margin-top + height + margin-bottom`,
    /// summed exactly from the used values. CSS2.2 §8.3 admits negative margins, but the
    /// §9.5.1 placer takes a nonnegative margin box.
    NegativeFloatOuterExtent,
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
    /// Sequence-only rejection of intersecting nonempty border and F margin intervals
    /// when a normal (B1 or a tail) establishes a BFC; the placement source names that
    /// normal. (G6a, Q-3) A zero-height border box of such a normal strictly inside F's
    /// margin interval is rejected too. Edge contact and the other empty intervals pass.
    /// Signed margins make this conservative vertical guard reachable.
    NewBfcFloatContact,
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
/// A group's role. `HypotheticalPair` and `ActualPair` name the adjoining margins of a tail
/// and the normal before it: two margins, or three after a B1 that collapses through (CTB,
/// the user's U1=A); the barrier groups hold the margins on either side of clearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginGroupId {
    HypotheticalPair,
    ActualPair,
    BeforeBarrier,
    AfterBarrier,
}
/// Immutable owner-local membership. Indices resolve through the owning source slice. A
/// group has at most three members: after a B1 that collapses through (CTB, the user's
/// U1=A; CSS2.2 8.3.1), the first tail's hypothetical and actual groups list B1's top and
/// bottom margins and the tail's top margin, and its `BeforeBarrier` B1's two margins.
/// Unused slots repeat the last member.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarginGroup {
    id: MarginGroupId,
    members: [MarginMember; 3],
    len: usize,
    extrema: MarginExtrema,
}
impl MarginGroup {
    pub fn id(&self) -> MarginGroupId {
        self.id
    }
    /// The members in source order: two, one, or (CTB) three.
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
/// `relative` admits computed `position: relative` besides `static` (Sequence P and
/// children only, REL).
fn display(
    facts: DisplayResolution,
    source: usize,
    floating: bool,
    relative: bool,
) -> Result<(), ClearanceOwnerError> {
    let valid = facts.display == Display::Block
        && (facts.position == Position::Static
            || (relative && facts.position == Position::Relative))
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
fn parent(
    f: ParentFacts<'_>,
    source: usize,
    policy: ChildMarginPolicy,
) -> Result<(), ClearanceOwnerError> {
    display(
        f.display,
        source,
        false,
        policy == ChildMarginPolicy::Sequence,
    )?;
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
    // Legacy only. The Sequence P is a new BFC root, which never collapses margins
    // with its in-flow children (CSS2.2 8.3.1), so it needs no top/bottom isolation.
    if policy == ChildMarginPolicy::Legacy
        && (!(f.border.top > 0. || f.padding.top > 0.)
            || !(f.border.bottom > 0. || f.padding.bottom > 0.))
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

/// G6a (CSS2.2 8.3.1): a nonzero top or bottom border or padding separates a box's own
/// top and bottom margins, so they are not adjoining. Left and right edges never do.
pub(super) fn vertical_decoration(f: ChildFacts<'_>) -> bool {
    f.border.top > 0. || f.border.bottom > 0. || f.padding.top > 0. || f.padding.bottom > 0.
}

/// G6a (CSS2.2 8.3.1): the own top and bottom margins of a Sequence normal are not
/// adjoining when it establishes a new block formatting context or a top or bottom
/// border or padding separates them, so its zero used height cannot collapse through.
/// NZS also admits a caller-asserted computed non-adjoining zero. Its numerical
/// payload is validated before this predicate is reached. Legacy normals never qualify.
fn own_margins_separated(f: ChildFacts<'_>, policy: ChildMarginPolicy) -> bool {
    policy == ChildMarginPolicy::Sequence
        && (f.context == ChildContext::OtherNewBfc
            || vertical_decoration(f)
            || matches!(f.height, OwnerHeight::NonAdjoiningZero(_)))
}

/// `collapse_through` (CTB/CTT, the user's U1=A): a Sequence normal whose own margins
/// may adjoin is admitted, and its margins collapse through (CSS2.2 8.3.1).
fn child(
    f: ChildFacts<'_>,
    source: usize,
    floating: bool,
    margin_policy: ChildMarginPolicy,
    collapse_through: bool,
) -> Result<f32, ClearanceOwnerError> {
    display(
        f.display,
        source,
        floating,
        margin_policy == ChildMarginPolicy::Sequence,
    )?;
    // A Sequence normal (B1 or a tail) may establish a new block formatting context
    // (CSS2.2 §9.4.1); the Sequence owner permits boundary contact with the float's
    // margin box and rejects strict nonempty intersection, and a zero-height border box
    // strictly inside that margin box (G6a). Legacy contexts stay fixed.
    if f.context
        != if floating {
            ChildContext::FloatBfc
        } else {
            ChildContext::OrdinaryNonBfc
        }
        && !(margin_policy == ChildMarginPolicy::Sequence
            && !floating
            && f.context == ChildContext::OtherNewBfc)
    {
        return Err(unsupported(source, OwnerProfileReason::ChildContext));
    }
    content(f.content, f.generated, source)?;
    let height = match f.height {
        OwnerHeight::Explicit(height) => height,
        OwnerHeight::NonAdjoiningZero(height)
            if margin_policy == ChildMarginPolicy::Sequence && !floating =>
        {
            height
        }
        OwnerHeight::Auto if margin_policy == ChildMarginPolicy::Sequence && floating => 0.0_f32,
        _ => return Err(unsupported(source, OwnerProfileReason::HeightKind)),
    };
    finite(height, source, "height")?;
    // NZS: a finite nonzero payload violates the zero-only category contract.
    // Keep this after finiteness and before any margin or decoration validation.
    if matches!(f.height, OwnerHeight::NonAdjoiningZero(_)) && height != 0.0 {
        return Err(unsupported(source, OwnerProfileReason::HeightKind));
    }
    if floating {
        nonnegative(height, source, "height")?;
    } else if height < 0.
        || (height == 0. && !own_margins_separated(f, margin_policy) && !collapse_through)
    {
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
    // Sequence normals admit finite signed and Auto margins on every edge.
    // Keep them outside the Legacy horizontal/category restrictions below.
    if floating || margin_policy == ChildMarginPolicy::Legacy {
        // CSS2.2 §8.3: a Sequence float's margins may be negative; its outer extents are
        // checked once its width is known (`validate_sequence_float_extents`).
        if !(floating && margin_policy == ChildMarginPolicy::Sequence)
            && values
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
            if margin_policy == ChildMarginPolicy::Legacy
                && values
                    .into_iter()
                    .any(|value| matches!(value, AutoLength::Value(v) if v != 0.0))
            {
                return Err(unsupported(source, OwnerProfileReason::FloatMargin));
            }
        } else if f.margin.left != AutoLength::Value(0.0)
            || f.margin.right != AutoLength::Value(0.0)
        {
            return Err(unsupported(source, OwnerProfileReason::HorizontalMargin));
        }
    }
    // Sequence children admit finite nonnegative borders and padding on every side
    // (CBR); Legacy children keep both zero.
    for (edges, field) in [(f.border, "border"), (f.padding, "padding")] {
        check_edges(edges, source, field, true)?;
        if margin_policy == ChildMarginPolicy::Legacy
            && edge_values(edges).into_iter().any(|v| v != 0.)
        {
            return Err(unsupported(source, OwnerProfileReason::ChildDecoration));
        }
    }
    Ok(height)
}
pub(super) fn sum(terms: &[f32], stage: &'static str) -> Result<f32, ClearanceOwnerError> {
    terms
        .iter()
        .fold(Exact::ZERO, |acc, &term| acc.add(Exact::from_f32(term)))
        .to_f32()
        .map_err(|_| ClearanceOwnerError::Unrepresentable { stage })
}
fn group(
    id: MarginGroupId,
    members: [MarginMember; 3],
    len: usize,
) -> Result<MarginGroup, ClearanceOwnerError> {
    let mut extrema = MarginExtrema::default();
    let values = [members[0].value, members[1].value, members[2].value];
    extrema
        .extend(&values[..len], NonZeroUsize::MIN.saturating_add(2))
        .map_err(ClearanceOwnerError::Margin)?;
    Ok(MarginGroup {
        id,
        members,
        len,
        extrema,
    })
}
pub(super) fn amount_error(error: ClearanceError) -> ClearanceOwnerError {
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
    display(facts.display, source, false, false)?;
    content(facts.content, facts.generated, source)
}

pub(super) fn validate_parent(
    facts: ParentFacts<'_>,
    source: usize,
) -> Result<(), ClearanceOwnerError> {
    parent(facts, source, ChildMarginPolicy::Legacy)
}

/// Sequence parent profile: `validate_parent` without the Legacy isolation test.
pub(super) fn validate_sequence_parent(
    facts: ParentFacts<'_>,
    source: usize,
) -> Result<(), ClearanceOwnerError> {
    parent(facts, source, ChildMarginPolicy::Sequence)
}

pub(super) fn validate_child(
    facts: ChildFacts<'_>,
    source: usize,
    floating: bool,
    after: bool,
) -> Result<f32, ClearanceOwnerError> {
    let height = child(facts, source, floating, ChildMarginPolicy::Legacy, false)?;
    // Legacy owners keep the four-source restriction on B1 and the float.
    if !after && facts.clear != ClearSide::None {
        return Err(unsupported(
            source,
            OwnerProfileReason::ClearOnBeforeOrFloat,
        ));
    }
    Ok(height)
}

/// `collapse_through`: as for `child`; S4 admits finite signed normal margins
/// while preserving the existing NaN/negative-infinity eligibility boundary.
pub(super) fn validate_sequence_child(
    facts: ChildFacts<'_>,
    source: usize,
    floating: bool,
    collapse_through: bool,
) -> Result<f32, ClearanceOwnerError> {
    child(
        facts,
        source,
        floating,
        ChildMarginPolicy::Sequence,
        collapse_through,
    )
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

/// Sequence float only (CSS2.2 §8.3, §10.3.5, §10.6.6): each used margin may be negative,
/// but both seven-term outer extents (near margin, near border, near padding,
/// content extent, far padding, far border, far margin), summed exactly, must be
/// nonnegative because the
/// §9.5.1 placer takes a nonnegative margin box. Complete child profiles and the
/// width domain have already validated finite terms and nonnegative borders, padding
/// and extents.
pub(super) fn validate_sequence_float_extents(
    facts: ChildFacts<'_>,
    width: f32,
    height: f32,
    source: usize,
) -> Result<(), ClearanceOwnerError> {
    let margin = facts.margin;
    let extent = |terms: [f32; 7]| {
        terms
            .into_iter()
            .fold(Exact::ZERO, |sum, term| sum.add(Exact::from_f32(term)))
    };
    if extent([
        margin.float_used_left(),
        facts.border.left,
        facts.padding.left,
        width,
        facts.padding.right,
        facts.border.right,
        margin.float_used_right(),
    ]) < Exact::ZERO
        || extent([
            margin.used_top(),
            facts.border.top,
            facts.padding.top,
            height,
            facts.padding.bottom,
            facts.border.bottom,
            margin.used_bottom(),
        ]) < Exact::ZERO
    {
        return Err(unsupported(
            source,
            OwnerProfileReason::NegativeFloatOuterExtent,
        ));
    }
    Ok(())
}

pub(super) struct KernelError {
    pub source: usize,
    pub error: ClearanceOwnerError,
}

pub(super) struct FirstFloatKernel {
    pub before: BlockVertical,
    pub before_content: BlockVertical,
    /// B1 padding span: on a side without border its edge is the border edge.
    pub before_padding: BlockVertical,
    pub float_border: Band,
    pub float_border_vertical: BlockVertical,
    pub placed_float: PlacedFloat,
    pub float_content: Band,
    pub float_content_vertical: BlockVertical,
    /// Float padding band and span: on a side without border each edge is the border
    /// edge, on a side without padding the content edge.
    pub float_padding: Band,
    pub float_padding_vertical: BlockVertical,
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
    /// CTB (the user's U1=A): B1 collapses through, so F's hypothetical top is B1's top
    /// border edge (CSS2.2 8.3.1, 9.5.1).
    pub before_collapses_through: bool,
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

/// The in-flow normal before a placed tail: its border bottom `edge` and its bottom
/// margin, or (CTB, the user's U1=A) for a B1 that collapses through, P's content top as
/// `edge` (B1's top margin edge) and both of B1's margins, which adjoin the tail's top
/// margin (CSS2.2 8.3.1) unless clearance separates them (9.5.2).
#[derive(Clone, Copy)]
pub(super) struct PreviousNormal {
    pub edge: f32,
    pub source: usize,
    pub top_margin: Option<f32>,
    pub bottom_margin: f32,
}

// Border top is already published by origin/clearance. Preserve the undecorated
// path (zero vertical padding and border), including its stage and signed-zero
// semantics, without extra arithmetic. Otherwise the content top and the border
// bottom each round once from an exact sum that includes the border (CBR D2); each
// padding edge is a side publication between two published neighbours, so its
// stage cannot fail. `stages` names the content top, padding top, content bottom,
// padding bottom and border bottom. Returns the content span, the padding span and
// the border bottom.
pub(super) fn normal_vertical(
    border_top: f32,
    height: f32,
    border: OwnerEdges,
    padding: OwnerEdges,
    stages: [&'static str; 5],
) -> Result<(BlockVertical, BlockVertical, f32), ClearanceOwnerError> {
    let [
        top_stage,
        padding_top_stage,
        content_bottom_stage,
        padding_bottom_stage,
        border_bottom_stage,
    ] = stages;
    if padding.top == 0.0 && padding.bottom == 0.0 && border.top == 0.0 && border.bottom == 0.0 {
        let bottom = sum(&[border_top, height], border_bottom_stage)?;
        let content = BlockVertical {
            top: border_top,
            bottom,
        };
        return Ok((content, content, bottom));
    }
    let top = sum(&[border_top, border.top, padding.top], top_stage)?;
    let bottom = sum(&[top, height], content_bottom_stage)?;
    let border_bottom = sum(
        &[bottom, padding.bottom, border.bottom],
        border_bottom_stage,
    )?;
    let padding_top = if border.top == 0.0 {
        border_top
    } else {
        sum(&[border_top, border.top], padding_top_stage)?
    };
    let padding_bottom = if border.bottom == 0.0 {
        border_bottom
    } else {
        sum(&[bottom, padding.bottom], padding_bottom_stage)?
    };
    Ok((
        BlockVertical { top, bottom },
        BlockVertical {
            top: padding_top,
            bottom: padding_bottom,
        },
        border_bottom,
    ))
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
    let (before_content, before_padding, before_bottom) = normal_vertical(
        before_top,
        input.before_height,
        input.before.border,
        input.before.padding,
        [
            "before_content_top",
            "before_padding_top",
            "before_content_bottom",
            "before_padding_bottom",
            "before_bottom",
        ],
    )
    .map_err(|error| KernelError {
        source: input.before_source,
        error,
    })?;
    // CTB (the user's U1=A; CSS2.2 9.5.1 rule 4, 8.3.1): after a B1 that collapses
    // through, F's hypothetical top is B1's top border edge, placed as with a nonzero
    // bottom border; the placer's containing-top clamp still applies.
    let float_top = if input.before_collapses_through {
        before_top
    } else {
        sum(
            &[before_bottom, input.before.margin.used_bottom()],
            "float_top",
        )
        .map_err(|error| KernelError {
            source: input.float_source,
            error,
        })?
    };
    let ml = input.float.margin.float_used_left();
    let mr = input.float.margin.float_used_right();
    let padding = input.float.padding;
    let border = input.float.border;
    // An axis is undecorated when its padding and border terms are all zero.
    let horizontal_undecorated =
        padding.left == 0.0 && padding.right == 0.0 && border.left == 0.0 && border.right == 0.0;
    let identity = ml == 0.0 && mr == 0.0;
    let tagged = |error| KernelError {
        source: input.float_source,
        error,
    };
    let outer_width = if horizontal_undecorated {
        if identity {
            input.float_width
        } else {
            sum(&[ml, input.float_width, mr], "float_outer_width").map_err(tagged)?
        }
    } else {
        sum(
            &[
                ml,
                border.left,
                padding.left,
                input.float_width,
                padding.right,
                border.right,
                mr,
            ],
            "float_outer_width",
        )
        .map_err(tagged)?
    };
    let mt = input.float.margin.used_top();
    let mb = input.float.margin.used_bottom();
    let vertical_identity = mt == 0.0 && mb == 0.0;
    let vertical_undecorated =
        padding.top == 0.0 && padding.bottom == 0.0 && border.top == 0.0 && border.bottom == 0.0;
    let outer_height = if vertical_undecorated {
        if vertical_identity {
            input.float_height
        } else {
            sum(&[mt, input.float_height, mb], "float_outer_height").map_err(tagged)?
        }
    } else {
        sum(
            &[
                mt,
                border.top,
                padding.top,
                input.float_height,
                padding.bottom,
                border.bottom,
                mb,
            ],
            "float_outer_height",
        )
        .map_err(tagged)?
    };
    let placed_float = place_float(
        &[],
        input.parent.containing,
        // The request is the float's margin box. Each extent includes both border and
        // padding edges and both used margins, summed once. The axis-specific
        // undecorated path retains the predecessor's identity and rounded extent
        // behavior.
        FloatRequest {
            side: match input.float.display.float {
                Float::Right => FloatSide::Right,
                _ => FloatSide::Left,
            },
            width: outer_width,
            height: outer_height,
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
    let (float_content, float_padding, float_border) = if horizontal_undecorated {
        let content = if identity {
            Band {
                left: placed_float.left,
                right: placed_float.right,
            }
        } else {
            match placed_float.side {
                FloatSide::Left => {
                    let left =
                        sum(&[placed_float.left, ml], "float_content_left").map_err(tagged)?;
                    let right =
                        sum(&[left, input.float_width], "float_content_right").map_err(tagged)?;
                    Band { left, right }
                }
                FloatSide::Right => {
                    let right =
                        sum(&[placed_float.right, -mr], "float_content_right").map_err(tagged)?;
                    let left =
                        sum(&[right, -input.float_width], "float_content_left").map_err(tagged)?;
                    Band { left, right }
                }
            }
        };
        (content, content, content)
    } else {
        // Content and border edges round once from their anchors; each padding edge is
        // a side publication between two published neighbours (CBR D2).
        match placed_float.side {
            FloatSide::Left => {
                let border_left =
                    sum(&[placed_float.left, ml], "float_border_left").map_err(tagged)?;
                let left = sum(
                    &[border_left, border.left, padding.left],
                    "float_content_left",
                )
                .map_err(tagged)?;
                let right =
                    sum(&[left, input.float_width], "float_content_right").map_err(tagged)?;
                let border_right = sum(&[right, padding.right, border.right], "float_border_right")
                    .map_err(tagged)?;
                let padding_left = if border.left == 0.0 {
                    border_left
                } else {
                    sum(&[border_left, border.left], "float_padding_left").map_err(tagged)?
                };
                let padding_right = if border.right == 0.0 {
                    border_right
                } else {
                    sum(&[right, padding.right], "float_padding_right").map_err(tagged)?
                };
                (
                    Band { left, right },
                    Band {
                        left: padding_left,
                        right: padding_right,
                    },
                    Band {
                        left: border_left,
                        right: border_right,
                    },
                )
            }
            FloatSide::Right => {
                let border_right =
                    sum(&[placed_float.right, -mr], "float_border_right").map_err(tagged)?;
                let right = sum(
                    &[border_right, -border.right, -padding.right],
                    "float_content_right",
                )
                .map_err(tagged)?;
                let left =
                    sum(&[right, -input.float_width], "float_content_left").map_err(tagged)?;
                let border_left = sum(&[left, -padding.left, -border.left], "float_border_left")
                    .map_err(tagged)?;
                let padding_right = if border.right == 0.0 {
                    border_right
                } else {
                    sum(&[border_right, -border.right], "float_padding_right").map_err(tagged)?
                };
                let padding_left = if border.left == 0.0 {
                    border_left
                } else {
                    sum(&[left, -padding.left], "float_padding_left").map_err(tagged)?
                };
                (
                    Band { left, right },
                    Band {
                        left: padding_left,
                        right: padding_right,
                    },
                    Band {
                        left: border_left,
                        right: border_right,
                    },
                )
            }
        }
    };
    // The near (top) edge is no longer bounded by the margin box: a negative top margin,
    // or a positive one beside a negative bottom margin, can carry `placed.top + mt` past
    // the finite range, which returns its named stage (SFM design).
    let (float_content_vertical, float_padding_vertical, float_border_vertical) =
        if vertical_undecorated {
            let content = if vertical_identity {
                BlockVertical {
                    top: placed_float.top,
                    bottom: placed_float.bottom,
                }
            } else {
                let top = sum(&[placed_float.top, mt], "float_content_top").map_err(tagged)?;
                let bottom =
                    sum(&[top, input.float_height], "float_content_bottom").map_err(tagged)?;
                BlockVertical { top, bottom }
            };
            (content, content, content)
        } else {
            let border_top = sum(&[placed_float.top, mt], "float_border_top").map_err(tagged)?;
            let top =
                sum(&[border_top, border.top, padding.top], "float_content_top").map_err(tagged)?;
            let bottom = sum(&[top, input.float_height], "float_content_bottom").map_err(tagged)?;
            let border_bottom = sum(
                &[bottom, padding.bottom, border.bottom],
                "float_border_bottom",
            )
            .map_err(tagged)?;
            let padding_top = if border.top == 0.0 {
                border_top
            } else {
                sum(&[border_top, border.top], "float_padding_top").map_err(tagged)?
            };
            let padding_bottom = if border.bottom == 0.0 {
                border_bottom
            } else {
                sum(&[bottom, padding.bottom], "float_padding_bottom").map_err(tagged)?
            };
            (
                BlockVertical { top, bottom },
                BlockVertical {
                    top: padding_top,
                    bottom: padding_bottom,
                },
                BlockVertical {
                    top: border_top,
                    bottom: border_bottom,
                },
            )
        };
    Ok(FirstFloatKernel {
        before_content,
        before_padding,
        float_border,
        float_border_vertical,
        before: BlockVertical {
            top: before_top,
            bottom: before_bottom,
        },
        placed_float,
        float_content,
        float_content_vertical,
        float_padding,
        float_padding_vertical,
    })
}

pub(super) fn place_next_normal(
    previous: PreviousNormal,
    next: ChildFacts<'_>,
    next_height: f32,
    next_source: usize,
    placed_float: PlacedFloat,
) -> Result<NormalKernel, KernelError> {
    let result = (|| {
        let bottom = MarginMember {
            source_index: previous.source,
            side: MarginSide::Bottom,
            value: previous.bottom_margin,
        };
        let top = MarginMember {
            source_index: next_source,
            side: MarginSide::Top,
            value: next.margin.used_top(),
        };
        // CTB (the user's U1=A; CSS2.2 8.3.1): after a B1 that collapses through, its top
        // and bottom margins adjoin the tail's top margin, from P's content top; the
        // margins before a barrier are B1's pair. Unused slots repeat the last member.
        let (members, len, before, before_len) = match previous.top_margin {
            Some(value) => {
                let leading = MarginMember {
                    source_index: previous.source,
                    side: MarginSide::Top,
                    value,
                };
                ([leading, bottom, top], 3, [leading, bottom, bottom], 2)
            }
            None => ([bottom, top, top], 2, [bottom, bottom, bottom], 1),
        };
        let hypothetical_group = group(MarginGroupId::HypotheticalPair, members, len)?;
        let hypothetical_top = sum(
            &[previous.edge, hypothetical_group.extrema.collapsed()],
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
                // 9.5.2: clearance separates the tail's top margin from the margins before
                // it, which still collapse with each other (CTB: B1's pair).
                let before = group(MarginGroupId::BeforeBarrier, before, before_len)?;
                let baseline = sum(
                    &[previous.edge, before.extrema.collapsed(), top.value],
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
                        before,
                        group(MarginGroupId::AfterBarrier, [top, top, top], 1)?,
                    ],
                    2,
                )
            }
        };
        let placement = solve_clearance(hypothetical_top, state).map_err(amount_error)?;
        let (_, _, bottom) = normal_vertical(
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
        before_collapses_through: false,
    })
    .map_err(|failure| failure.error)?;
    // Legacy float margins are validated all zero, so only the identity path is
    // reachable on both axes here, and `first.float_content` and
    // `first.float_content_vertical` equal the placed margin box.
    let after = place_next_normal(
        PreviousNormal {
            edge: first.before.bottom,
            source: indices.before,
            top_margin: None,
            bottom_margin: input.before.margin.used_bottom(),
        },
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
