//! Bounded actual-CSS composition for one O/P/[B1,F,tail...] sibling capture.
//!
//! The returned geometry owns only copied observations and the core sequence owner;
//! it borrows the dynamic projection's source storage rather than computed rows or
//! temporary child-fact scratch.

use super::{
    clearance_box_facts::{
        AbsoluteBoxFact, BoxLengthFact, BoxMaxFact, BoxOverflow, ClearanceBoxFact, KeywordBoxFact,
        LengthPercentageBoxFact, PercentageBoxFact,
    },
    clearance_bridge::{
        CapturedClearanceError, CapturedClearanceUnsupported, ChildMarginMode,
        ClearanceOuterContentGeometry, auto_length, child_facts_from_row, edges, joined_direction,
        joined_display, same_source, unsupported,
    },
    sibling_box_facts::SiblingBoxFacts,
};
use iced_css_core::{
    formatting::{
        bfc_height::{
            BfcHeightError, BfcHeightInput, BfcHeightMode, BfcHeightPlacement,
            SignedBfcHeightError, SignedBfcHeightInput, SignedNormalMarginError,
            SignedNormalMarginInput, constrain_bfc_height, publish_signed_normal_margin_edges,
            solve_bfc_height, solve_signed_bfc_height,
        },
        block::{
            AutoLength, BlockHorizontalError, BlockHorizontalPlacement, BlockWidthInput,
            BlockWidthResult, Direction, place_block_horizontal,
        },
        block_bottom::{
            BlockBottomError, BlockBottomInput, BlockBottomPlacement, place_block_bottom,
        },
        clearance::{
            ClearSide, ClearanceFloatError, ClearancePresenceError, ClearanceRequirement,
            classify_clearance, lowest_clearance_float_bottom,
        },
        clearance_float_cluster::{
            ClearanceFloatClusterError, ClearanceFloatClusterInput, ClearanceFloatClusterLimits,
            ClearanceFloatClusterOwner, ClearanceFloatClusterReceipt, FloatClusterFloatPlacement,
            build_clearance_float_cluster,
        },
        clearance_owner::{
            BlockVertical, ChildContext, ChildFacts, MarginMember, OuterFacts, OwnerContent,
            OwnerEdges, OwnerGenerated, OwnerHeight, ParentBfc, ParentFacts,
        },
        clearance_sequence::{
            ClearanceSequenceError, ClearanceSequenceInput, ClearanceSequenceLimits,
            ClearanceSequenceOwner, ClearanceSequenceReceipt, SequenceNormalPlacement,
            build_clearance_sequence, sequence_requested_retained_bytes,
        },
        display::{Display, DisplayResolution, Float, Flow, Position},
        first_child_origin::{
            FirstChildOriginError, FirstChildOriginInput, FirstChildOriginPlacement,
            place_first_child_origin,
        },
        floats::{Band, PlacedFloat},
        relative::{RelativeInput, RelativeOffsets, relative_offsets},
    },
    source::SourceNode,
    values::{ConversionError, NumericLimits},
};
use std::{collections::TryReserveError, mem::size_of, num::NonZeroUsize};

/// Independent facade, scratch, retained-output, and core limits.
#[derive(Debug, Clone, Copy)]
pub struct SiblingLayoutLimits {
    pub sources: NonZeroUsize,
    pub work: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub scratch_bytes: NonZeroUsize,
    pub core: ClearanceSequenceLimits,
}

/// Prepaid facade work and actual dynamic capacities retained or released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiblingLayoutReceipt {
    pub nodes: usize,
    pub work: usize,
    pub child_capacity: usize,
    pub retained_bytes: usize,
    pub scratch_bytes: usize,
    pub core: ClearanceSequenceReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalChildKind {
    Normal,
    Float,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrincipalEdges {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

/// One source-linked principal child observation in projected source order.
#[derive(Debug)]
pub struct PrincipalChildBox {
    source_index: usize,
    containing_source_index: usize,
    kind: PrincipalChildKind,
    content: PrincipalEdges,
    padding: PrincipalEdges,
    border: PrincipalEdges,
    used_width: f32,
    normal_width: Option<BlockWidthResult>,
    used_margin_left: f32,
    used_margin_right: f32,
    used_height: f32,
    used_margin_top: f32,
    used_margin_bottom: f32,
    computed_width: BoxLengthFact,
    computed_height: BoxLengthFact,
    computed_margin_left: BoxLengthFact,
    computed_margin_right: BoxLengthFact,
    margin_top: BoxLengthFact,
    margin_bottom: BoxLengthFact,
    clear: KeywordBoxFact<ClearSide>,
    relative: RelativeOffsets,
}

impl PrincipalChildBox {
    pub fn source_index(&self) -> usize {
        self.source_index
    }
    pub fn containing_source_index(&self) -> usize {
        self.containing_source_index
    }
    pub fn kind(&self) -> PrincipalChildKind {
        self.kind
    }
    pub fn content(&self) -> PrincipalEdges {
        self.content
    }
    /// The padding box. Vertically, and for F on both axes, it is the core's published
    /// padding span and band; horizontally, a normal's padding edges are its content
    /// edges moved out by its padding, one f32 step each, from which `border()` steps
    /// out again. It equals `border()` for a child without borders.
    pub fn padding(&self) -> PrincipalEdges {
        self.padding
    }
    pub fn border(&self) -> PrincipalEdges {
        self.border
    }
    pub fn used_width(&self) -> f32 {
        self.used_width
    }
    /// Returns the ordinary block-width result for every normal child.
    /// `None` identifies the admitted float; no normal child has an absent result.
    pub fn normal_width(&self) -> Option<BlockWidthResult> {
        self.normal_width
    }
    /// Returns the retained normal solver margin, or the float category's used value.
    pub fn used_margin_left(&self) -> f32 {
        self.used_margin_left
    }
    /// Returns the retained normal solver margin, or the float category's used value.
    pub fn used_margin_right(&self) -> f32 {
        self.used_margin_right
    }
    pub fn used_height(&self) -> f32 {
        self.used_height
    }
    pub fn used_margin_top(&self) -> f32 {
        self.used_margin_top
    }
    pub fn used_margin_bottom(&self) -> f32 {
        self.used_margin_bottom
    }
    pub fn computed_width(&self) -> BoxLengthFact {
        self.computed_width
    }
    pub fn computed_height(&self) -> BoxLengthFact {
        self.computed_height
    }
    pub fn computed_margin_left(&self) -> BoxLengthFact {
        self.computed_margin_left
    }
    pub fn computed_margin_right(&self) -> BoxLengthFact {
        self.computed_margin_right
    }
    pub fn margin_top(&self) -> BoxLengthFact {
        self.margin_top
    }
    pub fn margin_bottom(&self) -> BoxLengthFact {
        self.margin_bottom
    }
    pub fn clear(&self) -> KeywordBoxFact<ClearSide> {
        self.clear
    }
    /// REL: the used offsets of this child (CSS2.2 9.4.3), solved with P's direction;
    /// four +0 offsets for a static child. Every other accessor keeps the normal-flow
    /// value: the child's visual box is its normal-flow box translated by its own and
    /// P's `left` and `top`.
    pub fn relative_offset(&self) -> RelativeOffsets {
        self.relative
    }
}

/// Copied P observations derived once from actual CSS and published geometry.
#[derive(Debug)]
pub struct SiblingParentGeometry {
    source_index: usize,
    containing_source_index: usize,
    horizontal: BlockHorizontalPlacement,
    origin: FirstChildOriginPlacement,
    height: BfcHeightPlacement,
    bottom: BlockBottomPlacement,
    computed_width: BoxLengthFact,
    computed_height: BoxLengthFact,
    computed_margin_top: BoxLengthFact,
    computed_margin_bottom: BoxLengthFact,
    clear: KeywordBoxFact<ClearSide>,
    clearance: ClearanceRequirement,
    relative: RelativeOffsets,
}

impl SiblingParentGeometry {
    pub fn source_index(&self) -> usize {
        self.source_index
    }
    pub fn containing_source_index(&self) -> usize {
        self.containing_source_index
    }
    pub fn horizontal(&self) -> BlockHorizontalPlacement {
        self.horizontal
    }
    pub fn origin(&self) -> FirstChildOriginPlacement {
        self.origin
    }
    /// P's content height placement. For a computed Auto P whose `min-height` or
    /// `max-height` applied (CSS2.2 10.7), this is the explicit rerun: `auto_extent` is
    /// `None`.
    pub fn height(&self) -> BfcHeightPlacement {
        self.height
    }
    pub fn bottom(&self) -> BlockBottomPlacement {
        self.bottom
    }
    pub fn computed_width(&self) -> BoxLengthFact {
        self.computed_width
    }
    pub fn computed_height(&self) -> BoxLengthFact {
        self.computed_height
    }
    pub fn computed_margin_top(&self) -> BoxLengthFact {
        self.computed_margin_top
    }
    pub fn computed_margin_bottom(&self) -> BoxLengthFact {
        self.computed_margin_bottom
    }
    pub fn clear(&self) -> KeywordBoxFact<ClearSide> {
        self.clear
    }
    pub fn clearance(&self) -> ClearanceRequirement {
        self.clearance
    }
    /// REL: P's used offsets (CSS2.2 9.4.3), solved with O's direction; four +0
    /// offsets for a static P. P's other accessors keep the normal-flow values; the
    /// offsets move P's boxes and its children's together.
    pub fn relative_offset(&self) -> RelativeOffsets {
        self.relative
    }
    pub fn content(&self) -> PrincipalEdges {
        PrincipalEdges {
            left: self.horizontal.content.left,
            right: self.horizontal.content.right,
            top: self.origin.content_top,
            bottom: self.height.content_bottom,
        }
    }
}

/// Source-only-lifetime combined layout output.
#[derive(Debug)]
pub struct CapturedSiblingLayout<'p> {
    sequence: ClearanceSequenceOwner<'p>,
    parent: SiblingParentGeometry,
    children: Vec<PrincipalChildBox>,
    receipt: SiblingLayoutReceipt,
}

impl<'p> CapturedSiblingLayout<'p> {
    pub fn sources(&self) -> &'p [SourceNode] {
        self.sequence.sources()
    }
    pub fn source(&self, index: usize) -> Option<&'p SourceNode> {
        self.sequence.source(index)
    }
    pub fn parent(&self) -> &SiblingParentGeometry {
        &self.parent
    }
    pub fn children(&self) -> &[PrincipalChildBox] {
        &self.children
    }
    pub fn sequence(&self) -> &ClearanceSequenceOwner<'p> {
        &self.sequence
    }
    pub fn receipt(&self) -> SiblingLayoutReceipt {
        self.receipt
    }
}

/// Limits for the opt-in consecutive-float profile. Existing one-float limits are unchanged.
#[derive(Debug, Clone, Copy)]
pub struct FloatClusterLayoutLimits {
    pub sources: NonZeroUsize,
    pub work: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub scratch_bytes: NonZeroUsize,
    pub core: ClearanceFloatClusterLimits,
}

/// Prepaid work and actual peak/retained capacities for the cluster facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatClusterLayoutReceipt {
    pub nodes: usize,
    pub floats: usize,
    pub work: usize,
    pub child_capacity: usize,
    pub width_capacity: usize,
    pub retained_bytes: usize,
    pub scratch_bytes: usize,
    pub core: ClearanceFloatClusterReceipt,
}

/// The complete cluster result borrows only the projection's source storage.
#[derive(Debug)]
pub struct CapturedFloatClusterLayout<'p> {
    cluster: ClearanceFloatClusterOwner<'p>,
    parent: SiblingParentGeometry,
    children: Vec<PrincipalChildBox>,
    receipt: FloatClusterLayoutReceipt,
}

impl<'p> CapturedFloatClusterLayout<'p> {
    pub fn sources(&self) -> &'p [SourceNode] {
        self.cluster.sources()
    }
    pub fn source(&self, index: usize) -> Option<&'p SourceNode> {
        self.cluster.source(index)
    }
    pub fn parent(&self) -> &SiblingParentGeometry {
        &self.parent
    }
    pub fn children(&self) -> &[PrincipalChildBox] {
        &self.children
    }
    pub fn cluster(&self) -> &ClearanceFloatClusterOwner<'p> {
        &self.cluster
    }
    pub fn receipt(&self) -> FloatClusterLayoutReceipt {
        self.receipt
    }
}

/// Existing numeric and admission failures retain their typed error chain.
#[derive(Debug)]
pub enum FloatClusterLayoutError {
    Existing(SiblingLayoutError),
    Core(ClearanceFloatClusterError),
    ClusterOrder { source: usize },
    FloatWidth { source: usize },
    FloatHeight { source: usize },
}

impl From<SiblingLayoutError> for FloatClusterLayoutError {
    fn from(error: SiblingLayoutError) -> Self {
        Self::Existing(error)
    }
}
impl std::fmt::Display for FloatClusterLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "captured float cluster layout: {self:?}")
    }
}
impl std::error::Error for FloatClusterLayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Existing(error) => Some(error),
            Self::Core(error) => Some(error),
            _ => None,
        }
    }
}

/// Normal child's horizontal border publication edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildBorderStage {
    Left,
    Right,
}

/// Failure from facade admission, its one core call, or final principal assembly.
#[derive(Debug)]
pub enum SiblingLayoutError {
    Sources,
    Work,
    Bytes,
    ScratchBytes,
    Allocation(TryReserveError),
    Captured(CapturedClearanceError),
    ParentHorizontal(BlockHorizontalError),
    ParentOrigin(FirstChildOriginError),
    ParentFloatSelection(ClearanceFloatError),
    ParentPresence(ClearancePresenceError),
    UnexpectedParentClearance,
    Core(ClearanceSequenceError),
    ChildHorizontal {
        source: usize,
        error: BlockHorizontalError,
    },
    /// A finite content edge moved out by its padding, or that padding edge moved out by
    /// its border, cannot publish a finite edge.
    ChildBorderPublication {
        source: usize,
        stage: ChildBorderStage,
    },
    /// A failure of the nonnegative Auto or explicit height route, or of the
    /// `min-height`/`max-height` step (`constrain_bfc_height`) after either Auto route.
    Height(BfcHeightError),
    /// Signed Auto normal edge publication, visited in projected source order.
    NormalMargin {
        source: usize,
        error: SignedNormalMarginError,
    },
    SignedHeight(SignedBfcHeightError),
    Bottom(BlockBottomError),
    /// P's computed `overflow: auto` is admitted only when every child's border box and
    /// margin box lie within P's published content box (CSS2.2 §11.1); `source` is the
    /// first child, in source order, that is not proven contained.
    ParentAutoOverflow {
        source: usize,
    },
    /// PCT: a percentage `width`, margin, `right` or `left` of `source` (PPM: also a
    /// padding, `min-width` or `max-width`; `property` is its CSS name) cannot be
    /// resolved against its containing block's width, or (PHT) a child's percentage
    /// `height`, `min-height` or `max-height` against P's explicit used content height, a
    /// child's `top` or `bottom` (PTB, PTA) against P's used content height (explicit, or
    /// after P's bottom under an Auto P), or (OCH) P's `height`, `min-height`,
    /// `max-height`, `top` or `bottom` percentage against O's used content height
    /// (`InvalidContext` when that value is not finite and nonnegative), or (VPH, the
    /// user's U2=A) O's `min-height` or `max-height` percentage against the initial
    /// containing block's height (`source` 0; `InvalidContext` when that value is not
    /// finite and nonnegative).
    Percentage {
        source: usize,
        property: &'static str,
        error: ConversionError,
    },
    FactInvariant {
        source: usize,
        property: &'static str,
    },
}

impl std::fmt::Display for SiblingLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "captured sibling layout: {self:?}")
    }
}

impl std::error::Error for SiblingLayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::Captured(error) => Some(error),
            Self::ParentHorizontal(error) => Some(error),
            Self::ParentOrigin(error) => Some(error),
            Self::ParentFloatSelection(error) => Some(error),
            Self::ParentPresence(error) => Some(error),
            Self::Core(error) => Some(error),
            Self::ChildHorizontal { error, .. } => Some(error),
            Self::Height(error) => Some(error),
            Self::NormalMargin { error, .. } => Some(error),
            Self::SignedHeight(error) => Some(error),
            Self::Bottom(error) => Some(error),
            Self::Percentage { error, .. } => Some(error),
            Self::Sources
            | Self::Work
            | Self::Bytes
            | Self::ScratchBytes
            | Self::UnexpectedParentClearance
            | Self::ChildBorderPublication { .. }
            | Self::ParentAutoOverflow { .. }
            | Self::FactInvariant { .. } => None,
        }
    }
}

/// Build source-linked P and principal-child geometry from a complete computed batch.
///
/// Facade work `752 + 32*N + 542*(N-2) + 373*(N-3)` is prepaid before any
/// proportional join, profile scan, or allocation. Twenty fixed actions cover the
/// float content band and vertical content span reads and the float's used
/// horizontal margins; three more cover the former B1/float `hidden`/`auto` source
/// guard and five more the former after-loop B1 relabel, both kept without credit,
/// and twelve more cover P's second admitted value, the `auto` test, the
/// four content-box reads, the four placed-float reads and the containment error.
/// Seventy-four constant-size actions per child cover typed conversion and retained
/// computed/used margin facts, including the two retained used horizontal margins,
/// and the containment test: the loop step, the kind test, four border and four
/// used-margin reads, four margin-edge sums and sixteen comparisons. 66 actions per
/// normal cover Auto width categories, a possible minimum-width category rerun and
/// the two solver-margin reads. Each normal still has one horizontal producer
/// invocation. Padding adds 80 actions per normal for separate copied spans
/// and checked left/right border expansion, plus 16 per child for copied border
/// observations. These additions are prepaid even when padding is zero. Eight more
/// actions per child cover the in-loop new-BFC label: the role test, the overflow
/// read and comparison, and the context write. Child borders add 10 fixed actions
/// for the float padding band and span, 8 per child for the copied padding box and
/// 39 per normal for the two border reads, the two staged border-edge steps, the
/// vertical padding-span read and the padding-edge assembly, prepaid on zero-border
/// paths too. Relative positioning adds 28 fixed actions (P's offset solve 27 and
/// its widened admission test 1) and 38 per child (its offset solve 27, the
/// containment shift 10 and its widened admission test 1), prepaid on static paths.
/// Absolute min/max constraints (MMX) add 22 actions per box (P and each child: four
/// constraint reads, two max category conversions, the width hand-off with a possible
/// max-width rerun, and the height constraint) and, for P alone, 8 for its possible
/// min-width rerun and 28 for its Auto height step: 58 fixed and 22 per child,
/// prepaid when every constraint is default. Percentages (PCT) add 72 fixed actions
/// (P's resolution 57, its two basis validity tests 4, the held-result test 2 and F's
/// width resolved again 9) and 124 per child (its first resolution 57, the held-result
/// test 2, its second resolution 57 and four margin comparisons 8); a PCT resolution was
/// seven longhands of 8 and the basis read, prepaid when no value is a percentage.
/// Padding and width-constraint percentages (PPM) add six longhands to each
/// resolution (105 in all): 64 fixed actions (P's resolution 48 and F's `min-width`
/// and `max-width` resolved again 16) and 104 per child (its two resolutions 96 and
/// four padding comparisons 8), prepaid when no value is a percentage.
/// Height percentages (PHT) add 21 fixed actions (O's height read and its `auto` test
/// 3, P's three category tests and the refusal decision 8, P's three treated-as writes
/// 3, and the children's basis: P's height category test, the two `constrain`
/// comparisons and the basis write 7) and 27 per child (its vertical resolution: three
/// longhands of 8 and the basis read 1, and the basis-presence test 2), prepaid when no
/// value is a percentage.
/// Offset percentages (PTB) add 8 fixed actions (P's offset presence test 2, its two
/// category tests 4 and the refusal decision 2) and 42 per child (two vertical offset
/// resolutions of 21: the offset presence test 2, two longhands of 8, the basis read 1
/// and the basis-presence test 2), prepaid when no value is a percentage.
/// Offset percentages after layout (PTA) add 2 fixed actions (the basis read and its
/// transfer) and 35 per child: the mask for the horizontal solve 10 (the offset presence
/// test 2 and two longhands of 4: category test 2, result write 1, origin copy 1) and the
/// vertical pass 25 besides its resolution, which is PTB's second, moved there (loop
/// step 2, source and row reads 2, the offset presence test 2, two offset reads 2, two
/// category conversions 2, direction read 1, primitive call and return 4, two axis
/// resolutions 8 and two publication writes 2), prepaid when no value is a percentage.
/// O's host-supplied used content height (OCH) adds 75 fixed actions, prepaid on every
/// path (`build_captured_sibling_layout` passes `None`): the input transfer 1, O's
/// explicit-height write 1, the basis selection 4, the explicit-O branch test 2, the
/// basis presence test 2, the validity decision 7, P's height family against the basis
/// 27 (three longhands of 8, the basis read 1 and the basis-presence test 2), P's
/// offsets against it 21 (the PTB unit), the copy of P's resolved `top` and `bottom`
/// into its used row 7 and the vertical pass's basis selection 3.
/// Decorated Auto normals (DAZ, G6b) add 58 actions per normal, prepaid on every path:
/// the separation test of an Auto normal whose constrained tentative height is +0 (call
/// and return 4, the overflow read and comparison 3, the border accessor with two reads
/// and two comparisons 10, two vertical paddings of 13 at their percentage maximum, two
/// padding comparisons 4, the connectives 6, the read of P's solve 2) and the arm's
/// branch and write 3. MFZ (U4) admits a new-BFC normal through the same test and arm,
/// with the same actions.
/// P's offsets against an Auto O (OAH) add 79 fixed actions, prepaid on every path: O's
/// two limit category tests and the refusal decision 6, P's mask 10, the pass decision
/// 4, O's tentative height 4, the negative test and refusal decision 4 (kept without
/// credit since MFZ removed them), O's two limits 4, the constraint 5, P's resolution 21
/// and P's vertical axis 21.
/// O's limits against the initial containing block's height (VPH, the user's U2=A) add 45
/// fixed actions, prepaid on every path (the old entries pass `None`): the input transfer
/// 1, the viewport presence test 2, the validity decision 7, O's limit name selection 3,
/// the outer-limits decision 5 and O's limits against the basis 27 (the PHT unit: three
/// longhands of 8, the basis read 1 and the basis-presence test 2).
/// A collapse-through B1 (CTB, the user's U1=A; CT-15) adds conservative policy charges:
/// 25 fixed, 8 per child and 2 per normal, prepaid on every path without credit.
/// Fixed: adjoining property checks 15, explicit check 8 and refusal 2. Per child:
/// adjoining role/connective/write 4 and explicit used-write/role/connective 4.
/// Per normal: the Auto guard 2. The existing category/constrain and DAZ helper
/// charges are retained once. These policy charges are not measured instruction counts.
/// CTT retains those charges and prepays 57 fixed policy units for the shared
/// two-vector requested-byte helper, including its checked count/products/sum.
/// NZS adds eight per child for the narrow new category and scalar publication,
/// retaining all old charges without credit for removed refusals.
/// As in the PHT, PTB, PTA and OCH units, a call of a private facade helper is not charged.
/// The shared public core byte helper is included explicitly in the CTT unit.
/// The core is invoked exactly once, with computed Auto preserved and
/// bounded vertical and float Auto margins given used value zero. Every child also
/// admits computed `overflow: hidden` or `auto`. Each such child then establishes a block
/// formatting context and has no content, so nothing is clipped, nothing overflows
/// and `auto` presents no scrollbar. The facade labels each such normal child, B1 or a
/// tail, `ChildContext::OtherNewBfc`; the core permits border/margin boundary contact
/// and rejects strict intersection of nonempty intervals, and a zero-height border box
/// strictly inside F's margin interval (G6a), at that child's source
/// (CSS2.2 §9.5, Q1). P admits `hidden`, and `auto`
/// only when every child's border box and margin box lie within P's published
/// content box (CSS2.2 §11.1), so that nothing overflows P and `auto` presents no
/// scrollbar; otherwise the build returns `ParentAutoOverflow`. No clip or scroll
/// state is published, and `scroll` stays unsupported on every child. The float's content box is
/// the core's published content band horizontally and its published vertical
/// content span (`sequence().float_content_vertical()`) vertically; its margin box
/// stays `sequence().placed_float()`.
/// P's computed `border-*-width` facts are admitted on every side and enter P's
/// horizontal solve, first-child origin and bottom. They offset P's content box, and
/// narrow it when P's width is `auto`; the `auto` containment area stays that content
/// box and never includes P's border area. P needs no top or bottom isolation: as a
/// new block formatting context root it never collapses margins with its children
/// (CSS2.2 8.3.1), so a P without border and padding is admitted.
/// Every child (B1, F or a tail) admits computed border widths on every side. A
/// normal's width solve subtracts them; its padding edges are its content edges moved
/// out by its padding, and its border edges are those padding edges moved out by its
/// border, one f32 step each. This staged, inside-out derivation starts from the
/// solver's rounded content edge, so a border edge can differ from the exact border
/// position by the rounding already absorbed in that content edge. Vertically, and
/// for F on both axes, the core publishes the border, padding and content spans. The
/// Q1 contact check, the signed and nonnegative Auto routes and the `auto`
/// containment read the border box, which includes the child's borders. `padding()`
/// is the padding box and equals `border()` for a child without borders. A zero-width
/// child border of any style has the geometry of `border-style: none`. O's widths
/// are captured and unused.
/// P and every child (B1, F or a tail) also admit computed `position: relative`; O
/// stays `static`, and `absolute` and `fixed` stay refused on every source (REL).
/// Relative positioning acts after normal flow and floating (CSS2.2 9.4.3): every
/// published normal-flow field, clearance, the Q1 contact check and P's Auto height
/// read the unshifted boxes. The used offsets are published separately
/// (`relative_offset()`), solved with O's direction for P and with P's direction for
/// each child (`left` wins in `ltr`, `right` in `rtl`; `top` always wins). Under P
/// `auto`, each child's border box and margin box, computed as above, are translated
/// by the child's own `left` and `top`, one f32 addition per edge, before the
/// containment test; P's own offset moves its content box with its children and is
/// not applied. Under P `hidden` shifted children are admitted. A child's `top` or
/// `bottom` percentage (PTB, PTA, CSS2.2 9.3.2) resolves against P's used content height
/// by one exact product rounded once: an explicit P's in the child loop (the PHT basis
/// below) and again, for every child, in the vertical pass after P's bottom; an Auto P's
/// only there, after P's bottom and before the containment test, children in source
/// order. P's own `top` or `bottom` percentage resolves against O's used content height
/// when O's computed `height` is not `auto` and the host supplies it (OCH,
/// `build_captured_sibling_layout_with_outer_height`), at P's vertical step; it is
/// refused with `OuterHeightBasis` under such an O without that value. Under an `auto` O
/// (OAH; CSS2.2 10.6.3, 8.3.1, 10.7) it resolves after P's bottom, before the children's
/// vertical pass, against O's used content height: O is the captured root, whose margins
/// do not collapse, so its tentative height runs from its content top to P's bottom
/// margin edge (one f32 subtraction), and O's `min-height` and `max-height` then
/// constrain it. That O is the root of its CSS document, so that P's bottom margin does
/// not collapse through O, remains the caller's assertion, as for
/// `ClearanceOuterContentGeometry`. A negative tentative height is not greater than O's
/// `max-height` and is smaller than O's `min-height` (0 by default), so 10.7 gives O's
/// `min-height` (MFZ, the user's U3=A); a percentage `min-height` or `max-height` of O
/// resolves against the initial containing block's height when the host supplies it
/// (VPH, `build_captured_sibling_layout_with_viewport_height`, the user's U2=A; CSS2.2
/// 10.1, 10.7), at P's vertical step, and is `IndefiniteOffsetBasis` there without it;
/// the host's O content height is not read. Only P's `top` and `bottom` take the
/// resolved values; its `left` and `right` keep their resolution against O's content
/// width. Each side is resolved on its own, even when 9.4.3 then ignores `bottom` because
/// `top` is not `auto`: an ignored `bottom` percentage, a child's or (OCH, OAH) P's, is
/// resolved (an unrepresentable one is `Percentage`), and P's without a basis is refused
/// like any other (a conservative choice).
/// Every Sequence row's absolute `min-width`, `max-width`, `min-height` and
/// `max-height` apply (MMX, CSS2.2 10.4 and 10.7; of O's, only `min-height` and
/// `max-height` are read, by OAH). P and every normal pass their width constraints to
/// the 10.4 category solver, whose reruns show in `passes`; F's used width is its
/// computed width after the max, then the min rerun. An explicit height of
/// P or of a child is replaced the same way. A child's computed Auto height
/// is tentatively +0: F keeps the Auto category when its constrained height
/// is +0, and a normal with a positive `min-height` is admitted with
/// that used height. An ordinary (`visible`) Auto normal whose constrained height is +0
/// is passed to the core as an explicit +0 when a positive top or bottom border or
/// padding separates its own margins (DAZ, G6b; CSS2.2 10.6.3 "zero, otherwise" and
/// 8.3.1); a vertical padding percentage is read against P's used content width, and it
/// does not separate without P's solve, when it does not resolve or when it rounds to +0.
/// A new-BFC (`hidden` or `auto`) Auto normal whose constrained height is +0 is passed as
/// an explicit +0 too (MFZ): CSS2.2 10.6.7 defines no height for a box without children,
/// and the user's U4=A extends Q3=A (F's childless Auto height is zero) to it; its own
/// margins are not adjoining (8.3.1). NZS preserves computed non-adjacency for other
/// zero-height normals with the explicit NonAdjoiningZero category.
/// An explicit used height of +0 (G6a) is admitted by the core for a normal whose own top
/// and bottom margins are not adjoining (CSS2.2 8.3.1: `hidden` or `auto`, or a positive
/// top or bottom border or padding). An ordinary normal whose computed `height` is zero or
/// `auto` and whose computed `min-height` is zero, neither a percentage nor a length
/// rounded to +0, and without a top or bottom border or padding, collapses through
/// (CTB/CTT, the user's U1=A; CSS2.2 8.3.1, 9.5.1). F's hypothetical top is B1's
/// top border edge when B1 collapses through; subsequent through tails extend the
/// adjoining run unless a normal's used vertical margin is negative
/// (`NonpositiveBlockHeight`). A normal used as +0 whose computed `height` or
/// `min-height` is not such a zero uses separated geometry (NZS), without changing
/// physical decoration, context or sibling margin collapse.
/// P's unsigned Auto-height route keeps the last source normal's border bottom plus
/// its raw used bottom margin, including through tails (CTT-D1). Terminal collapsed
/// extrema do not replace this endpoint; raw-margin containment remains separate.
/// P's Auto height is the tentative placement of either route, then
/// `constrain_bfc_height`: a limit that applies reruns the explicit route on P's content
/// top. Containment, offsets and P's bottom read the constrained boxes, so a constraint
/// that makes a child leave P's content box under P `auto` is `ParentAutoOverflow`.
/// Every Sequence row's percentage `width`, margins, `right` and `left` (PCT, CSS2.2
/// 10.2, 8.3, 9.3.2), paddings, `min-width` and `max-width` (PPM, CSS2.2 8.4, 10.4)
/// resolve against its containing block's width: P's against O's
/// content width, each child's (F included) against P's used content width, by one
/// exact product rounded once to the nearest f32 (`SiblingBoxFacts::numeric_limits()`);
/// the solvers then run unchanged on the resolved values, and the computed accessors
/// publish the percentage facts. P's horizontal solve runs before the child loop and a
/// failure there is held: it is returned after F's `FloatWidth` test, where P's solver
/// error occurred before, so every earlier error keeps its order. An invalid O basis
/// keeps the solver's own error. A child's width-axis percentages resolve after its
/// admission tests and its height facts, and not while P's failure is held (a failure
/// is `Percentage` at that child); the placement loop and F's width resolve each row
/// again, and a failure or a margin or padding that differs there is `FactInvariant`.
/// Every reader of a padding or a width constraint reads a resolved row (PPM): P's
/// solve, origin and bottom, each normal's solve and padding edges, the core's child
/// facts and F's `constrain`. The one exception is DAZ's height-category probe (G6b): it
/// reads a decorated Auto normal's vertical padding percentages from the fact row and
/// resolves them itself, earlier, against the same width with the same primitive and
/// without failing; one that does not resolve, or that rounds to +0, does not separate by
/// itself. Computed non-adjacency supplies a separate NZS category; literal adjoining
/// zeros retain CTB/CTT behavior. The later resolved row owns any percentage error.
/// Every Sequence row's percentage `height`, `min-height` and `max-height` (PHT, CSS2.2
/// 10.5, 10.7) resolves against its containing block's height when that height is
/// specified explicitly; otherwise `height` is used as `auto`, `min-height` as 0 and
/// `max-height` as `none`. P's containing block is O; O's computed `height` other than
/// `auto` is specified explicitly (O is the captured root, so its own percentage refers
/// to the initial containing block, CSS2.2 10.5), and P's height percentages then
/// resolve against O's used content height supplied by the host (OCH), or are refused
/// with `OuterHeightBasis` without it; under an `auto` O they take those indefinite
/// rules. The resolution, its failure (`Percentage` at P; `InvalidContext` for a
/// supplied value that is not finite and nonnegative) and the refusal follow P's
/// `overflow` test and precede every child error and P's held horizontal error. Each
/// child's basis is P's explicit used content height (P's `height` length, or its
/// percentage resolved against O's, after its limits), known before the child loop, or
/// none when P's used height is `auto`. A child's height facts resolve in the order
/// `height`, `min-height`, `max-height`, after its admission tests and before the held P
/// check (their basis does not depend on P's horizontal solve); a failure is `Percentage`
/// at that child. P's height routes and `constrain_bfc_height` select on P's used height
/// category and limits. The computed accessors publish the percentage facts. Vertical
/// paddings and margins keep the width basis. P's `top`/`bottom` percentage resolution or
/// refusal (PTB, OCH) follows its height-percentage resolution or refusal and precedes
/// every child error; under an `auto` O only the refusal for O's percentage limits, or
/// (VPH, the user's U2=A) their resolution against the initial containing block's height
/// and its failure, sits there, and P's offset resolution (OAH; since MFZ a negative
/// tentative height takes O's `min-height`) follows P's bottom. A child's `top` and
/// `bottom` percentages resolve, under an explicit P, after its height facts and category
/// and before the held P check, and again in the vertical pass after P's bottom (PTA);
/// under an Auto P they resolve only in that pass, after P's bottom and before the `auto`
/// containment test, where a failure is `Percentage`.
/// Temporary child facts are released before the source-only output is published.
///
/// # Errors
/// Returns the first failure in the documented facade order and never publishes a
/// partial combined result.
pub fn build_captured_sibling_layout<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterContentGeometry<'p>,
    limits: SiblingLayoutLimits,
) -> Result<CapturedSiblingLayout<'p>, SiblingLayoutError> {
    build_captured_sibling_layout_with_outer_height(facts, outer, None, limits)
}

/// OCH (CSS2.2 10.5, 10.7, 9.3.2): `build_captured_sibling_layout` with
/// `outer_content_height`, O's used content height as established by the host (after
/// O's own `min-height`/`max-height`), the basis of P's height, `top` and `bottom`
/// percentages when O's computed `height` is not `auto`. It is read only then: under an
/// `auto` O a supplied value is ignored (O's height depends on its content), and `None`
/// keeps the `OuterHeightBasis` refusal. O's percentage `height` counts as specified
/// explicitly because O is the captured root, whose containing block is the initial
/// containing block (CSS2.2 10.1, 10.5); a host whose O is not the root of its CSS
/// document, so that O's percentage height would be used as `auto`, must pass `None`.
/// Of P's offsets, only `top` and `bottom` take this basis; its `left` and `right` keep
/// O's content width. Agreement with upstream CSS layout remains the caller's assertion,
/// as for `ClearanceOuterContentGeometry`. It supplies no initial containing block height
/// (VPH: `build_captured_sibling_layout_with_viewport_height` with `None`).
///
/// # Errors
/// As `build_captured_sibling_layout`; a supplied value that is not finite and
/// nonnegative is `Percentage { source: 1, .. }` with `InvalidContext` when P holds such
/// a percentage, at the position of the `OuterHeightBasis` refusal. With a valid value,
/// P's `height`, `min-height`, `max-height`, `top` or `bottom` percentage (in that order)
/// whose resolved value exceeds the finite f32 range is `Percentage { source: 1,
/// property, error: Unrepresentable }`, at the same position; the old entry cannot
/// produce these at that position (under an `auto` O both entries give P's `top` or
/// `bottom` `Percentage` after P's bottom, OAH). A digit or budget failure of that
/// resolution under the supplied `NumericLimits` is the same `Percentage` with that
/// error, at the same position.
pub fn build_captured_sibling_layout_with_outer_height<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterContentGeometry<'p>,
    outer_content_height: Option<f32>,
    limits: SiblingLayoutLimits,
) -> Result<CapturedSiblingLayout<'p>, SiblingLayoutError> {
    build_captured_sibling_layout_with_viewport_height(
        facts,
        outer,
        outer_content_height,
        None,
        limits,
    )
}

/// VPH (CSS2.2 10.1, 10.7; the user's U2=A): the OCH entry with `viewport_height`, the
/// height of the initial containing block (for continuous media, the viewport's). O is
/// the captured root, so a percentage `min-height` or `max-height` of O refers to it. It
/// is read only where the other entries refuse with `IndefiniteOffsetBasis`: when O's
/// computed `height` is `auto`, P's `top` or `bottom` is a percentage and O's
/// `min-height` or `max-height` is a percentage. O's limits then resolve against it at
/// P's vertical step (`min-height` before `max-height`, each by one exact product rounded
/// once) and constrain O's used content height, the basis of P's `top` and `bottom` (OAH;
/// a negative tentative height takes O's used `min-height`, MFZ). Otherwise the value is
/// not read and not validated, and `None` keeps the refusal. It is never the basis of
/// O's `height` (the host's `outer_content_height` stays its only source, OCH) nor of P's
/// or a child's height percentages. That O is the root of its CSS document, so that its
/// containing block is the initial containing block, remains the caller's assertion, as
/// for `ClearanceOuterContentGeometry`: a host whose O is not that root must pass `None`.
///
/// # Errors
/// As `build_captured_sibling_layout_with_outer_height`. When the value is read, one that
/// is not finite and nonnegative (-0.0 is admitted) is `Percentage { source: 0, property,
/// error: InvalidContext }`, `property` being `"min-height"` when O's `min-height` is a
/// percentage and `"max-height"` otherwise, and a resolved limit beyond the finite f32
/// range is `Percentage { source: 0, property, error: Unrepresentable }`, `min-height`
/// first; both at the position of the `IndefiniteOffsetBasis` refusal, which `None`
/// keeps: after P's `overflow` test and before every child error and P's held horizontal
/// error. A digit or budget failure of that resolution under the supplied
/// `NumericLimits` is the same `Percentage` with that error, at the same position.
pub fn build_captured_sibling_layout_with_viewport_height<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterContentGeometry<'p>,
    outer_content_height: Option<f32>,
    viewport_height: Option<f32>,
    limits: SiblingLayoutLimits,
) -> Result<CapturedSiblingLayout<'p>, SiblingLayoutError> {
    build_with_reserve(
        facts,
        outer,
        outer_content_height,
        viewport_height,
        limits,
        |scratch, requested| scratch.try_reserve_exact(requested),
        |children, requested| children.try_reserve_exact(requested),
    )
}

#[derive(Clone, Copy)]
enum LayoutProfile {
    Single,
    Cluster(ClearanceFloatClusterLimits),
}

enum GeometryOwner<'p> {
    Single(ClearanceSequenceOwner<'p>),
    Cluster(ClearanceFloatClusterOwner<'p>),
}
impl<'p> GeometryOwner<'p> {
    fn view(&self) -> GeometryView<'_, 'p> {
        match self {
            Self::Single(owner) => GeometryView::Single(owner),
            Self::Cluster(owner) => GeometryView::Cluster(owner),
        }
    }
}
struct GeometryOutput<'p> {
    owner: GeometryOwner<'p>,
    parent: SiblingParentGeometry,
    children: Vec<PrincipalChildBox>,
    nodes: usize,
    work: usize,
    retained_bytes: usize,
    scratch_bytes: usize,
    width_capacity: usize,
}

fn build_with_reserve<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterContentGeometry<'p>,
    outer_content_height: Option<f32>,
    viewport_height: Option<f32>,
    limits: SiblingLayoutLimits,
    reserve_scratch: impl FnOnce(&mut Vec<ChildFacts<'p>>, usize) -> Result<(), TryReserveError>,
    reserve_children: impl FnOnce(&mut Vec<PrincipalChildBox>, usize) -> Result<(), TryReserveError>,
) -> Result<CapturedSiblingLayout<'p>, SiblingLayoutError> {
    let result = build_geometry(
        facts,
        outer,
        outer_content_height,
        viewport_height,
        limits,
        LayoutProfile::Single,
        (reserve_scratch, reserve_children),
    )
    .map_err(|error| match error {
        FloatClusterLayoutError::Existing(error) => error,
        // Single never enters cluster admission or its core builder.
        _ => unreachable!("single profile produced a cluster-only error"),
    })?;
    let GeometryOwner::Single(sequence) = result.owner else {
        unreachable!("single profile owner")
    };
    let receipt = SiblingLayoutReceipt {
        nodes: result.nodes,
        work: result.work,
        child_capacity: result.children.capacity(),
        retained_bytes: result.retained_bytes,
        scratch_bytes: result.scratch_bytes,
        core: sequence.receipt(),
    };
    Ok(CapturedSiblingLayout {
        sequence,
        parent: result.parent,
        children: result.children,
        receipt,
    })
}

/// Lay out a childless B1, consecutive explicitly sized floats, and a normal tail.
///
/// # Errors
/// Rejects invalid joins, cluster order, automatic float dimensions, exceeded budgets,
/// unsupported profiles and numerical publication failures. No partial owner escapes.
pub fn build_captured_float_cluster_layout<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterContentGeometry<'p>,
    outer_content_height: Option<f32>,
    viewport_height: Option<f32>,
    limits: FloatClusterLayoutLimits,
) -> Result<CapturedFloatClusterLayout<'p>, FloatClusterLayoutError> {
    let common = SiblingLayoutLimits {
        sources: limits.sources,
        work: limits.work,
        retained_bytes: limits.retained_bytes,
        scratch_bytes: limits.scratch_bytes,
        core: ClearanceSequenceLimits {
            sources: limits.core.sources,
            work: limits.core.work,
            retained_bytes: limits.core.retained_bytes,
        },
    };
    let result = build_geometry(
        facts,
        outer,
        outer_content_height,
        viewport_height,
        common,
        LayoutProfile::Cluster(limits.core),
        (
            |rows, count| rows.try_reserve_exact(count),
            |rows, count| rows.try_reserve_exact(count),
        ),
    )?;
    let GeometryOwner::Cluster(cluster) = result.owner else {
        unreachable!("cluster profile owner")
    };
    let core = cluster.receipt();
    let receipt = FloatClusterLayoutReceipt {
        nodes: result.nodes,
        floats: core.floats,
        work: result.work,
        child_capacity: result.children.capacity(),
        width_capacity: result.width_capacity,
        retained_bytes: result.retained_bytes,
        scratch_bytes: result.scratch_bytes,
        core,
    };
    Ok(CapturedFloatClusterLayout {
        cluster,
        parent: result.parent,
        children: result.children,
        receipt,
    })
}

fn build_geometry<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterContentGeometry<'p>,
    outer_content_height: Option<f32>,
    viewport_height: Option<f32>,
    limits: SiblingLayoutLimits,
    profile: LayoutProfile,
    reserves: (
        impl FnOnce(&mut Vec<ChildFacts<'p>>, usize) -> Result<(), TryReserveError>,
        impl FnOnce(&mut Vec<PrincipalChildBox>, usize) -> Result<(), TryReserveError>,
    ),
) -> Result<GeometryOutput<'p>, FloatClusterLayoutError> {
    use SiblingLayoutError as E;
    let (reserve_scratch, reserve_children) = reserves;

    // NonZeroUsize supplies the initial preflight unit. Nothing proportional has
    // been inspected at this point.
    let projection = facts.projection();
    let nodes = projection.nodes();
    let node_count = nodes.len();
    if node_count < 5 || node_count > limits.sources.get() {
        return Err(E::Sources.into());
    }
    if !same_source(outer.outer, &nodes[0]) {
        return Err(E::Captured(CapturedClearanceError::GeometrySource).into());
    }
    let mut work = facade_work(node_count).ok_or(E::Work)?;
    if work > limits.work.get() {
        return Err(E::Work.into());
    }

    // The existing base work prepays the complete joins and role scan. Cluster
    // classification adds no role vector and occurs before numerical work.
    let float_count = match profile {
        LayoutProfile::Single => 1,
        LayoutProfile::Cluster(core) => {
            let count = classify_cluster(facts)?;
            if node_count > core.sources.get() {
                return Err(FloatClusterLayoutError::Core(
                    ClearanceFloatClusterError::Sources,
                ));
            }
            if count > core.floats.get() {
                return Err(FloatClusterLayoutError::Core(
                    ClearanceFloatClusterError::Floats,
                ));
            }
            work = work
                .checked_add(count.checked_mul(512).ok_or(E::Work)?)
                .and_then(|value| {
                    node_count
                        .checked_mul(count)?
                        .checked_mul(128)?
                        .checked_add(value)
                })
                .ok_or(E::Work)?;
            if work > limits.work.get() {
                return Err(E::Work.into());
            }
            count
        }
    };
    let is_float = |source: usize| match profile {
        LayoutProfile::Single => source == 3,
        LayoutProfile::Cluster(_) => {
            source >= 3
                && joined_display(projection, facts.display(), source)
                    .is_ok_and(|display| display.flow == Flow::Floating)
        }
    };
    let child_count = node_count - 2;
    let requested_scratch = checked_bytes::<ChildFacts<'_>>(child_count).ok_or(E::ScratchBytes)?;
    if requested_scratch > limits.scratch_bytes.get() {
        return Err(E::ScratchBytes.into());
    }
    let requested_core = match profile {
        LayoutProfile::Single => sequence_requested_retained_bytes(node_count),
        LayoutProfile::Cluster(_) => cluster_requested_retained(node_count, float_count),
    }
    .ok_or(E::Bytes)?;
    let requested_children = checked_bytes::<PrincipalChildBox>(child_count).ok_or(E::Bytes)?;
    requested_core
        .checked_add(requested_children)
        .filter(|&bytes| bytes <= isize::MAX as usize && bytes <= limits.retained_bytes.get())
        .ok_or(E::Bytes)?;

    if matches!(profile, LayoutProfile::Cluster(_)) {
        // Reject the whole simultaneous scratch floor before the first reserve.
        requested_scratch
            .checked_add(checked_bytes::<f32>(float_count).ok_or(E::ScratchBytes)?)
            .and_then(|sum| sum.checked_add(checked_bytes::<PlacedFloat>(float_count)?))
            .filter(|&sum| sum <= isize::MAX as usize && sum <= limits.scratch_bytes.get())
            .ok_or(E::ScratchBytes)?;
    }
    let (mut child_facts, scratch_bytes) =
        allocate_scratch(child_count, limits.scratch_bytes, reserve_scratch)?;

    let mut widths = Vec::new();
    let width_bytes = if let LayoutProfile::Cluster(_) = profile {
        let requested = checked_bytes::<f32>(float_count).ok_or(E::ScratchBytes)?;
        let core_floor = checked_bytes::<PlacedFloat>(float_count).ok_or(E::ScratchBytes)?;
        scratch_bytes
            .checked_add(requested)
            .and_then(|sum| sum.checked_add(core_floor))
            .filter(|&sum| sum <= isize::MAX as usize && sum <= limits.scratch_bytes.get())
            .ok_or(E::ScratchBytes)?;
        widths
            .try_reserve_exact(float_count)
            .map_err(E::Allocation)?;
        let actual = checked_bytes::<f32>(widths.capacity()).ok_or(E::ScratchBytes)?;
        scratch_bytes
            .checked_add(actual)
            .and_then(|sum| sum.checked_add(core_floor))
            .filter(|&sum| sum <= isize::MAX as usize && sum <= limits.scratch_bytes.get())
            .ok_or(E::ScratchBytes)?;
        actual
    } else {
        0
    };

    if matches!(profile, LayoutProfile::Single) {
        // Prove the complete actual row/display/token mapping before numerical work.
        for source in 0..node_count {
            fact_row(facts, source, "row")?;
            joined_display(projection, facts.display(), source).map_err(E::Captured)?;
        }
        if facts.row(node_count).is_some() {
            return Err(E::FactInvariant {
                source: node_count,
                property: "row-count",
            }
            .into());
        }

        for source in 0..node_count {
            let display =
                joined_display(projection, facts.display(), source).map_err(E::Captured)?;
            if !admitted_display(display, source == 3, source == 0) {
                return Err(E::Captured(unsupported(
                    source,
                    "display/position/float",
                    CapturedClearanceUnsupported::DisplayOrFlow,
                ))
                .into());
            }
        }
    }

    let parent_row = fact_row(facts, 1, "row")?;
    // CSS2.2 §9.4.1: P establishes a block formatting context with `hidden` or `auto`;
    // `auto` is kept only if nothing can overflow P, checked once P's height is known.
    if !matches!(
        parent_row.overflow.value,
        BoxOverflow::Hidden | BoxOverflow::Auto
    ) {
        return Err(E::Captured(unsupported(
            1,
            "overflow",
            CapturedClearanceUnsupported::ParentOverflow,
        ))
        .into());
    }
    let numeric = facts.numeric_limits();
    // PHT, PTB, OCH (CSS2.2 10.5, 10.7, 9.3.2): P's containing block is O. O's computed
    // `height` other than `auto` is specified explicitly (O is the captured root, so its
    // own percentage refers to the initial containing block); P's height, `top` and
    // `bottom` percentages then resolve against `outer_content_height`, O's used content
    // height from the host and its only source, in the order `height`, `min-height`,
    // `max-height`, `top`, `bottom`; without it they are refused (`OuterHeightBasis`),
    // and a value that is not finite and nonnegative is `InvalidContext`. Under an
    // `auto` O the input is not read: P's height percentages take the indefinite rules
    // and its offset percentages are refused (`IndefiniteOffsetBasis`) only when O's
    // `min-height` or `max-height` is a percentage and the host supplies no initial
    // containing block height (VPH, the user's U2=A); otherwise they resolve after P's
    // bottom (OAH). Every failure here follows P's `overflow` test and precedes every
    // child error. The children's basis is P's explicit used content height, known here.
    let outer_row = fact_row(facts, 0, "height")?;
    let outer_explicit = !matches!(outer_row.height, BoxLengthFact::Auto { .. });
    let outer_basis = outer_content_height.filter(|_| outer_explicit);
    let parent_offset_percentage = match parent_row.offset {
        Some([BoxLengthFact::Percentage(_), ..]) => Some("top"),
        Some([_, _, BoxLengthFact::Percentage(_), _]) => Some("bottom"),
        _ => None,
    };
    if outer_explicit {
        let property = if matches!(parent_row.height, BoxLengthFact::Percentage(_)) {
            Some("height")
        } else if matches!(
            parent_row.min_height,
            LengthPercentageBoxFact::Percentage(_)
        ) {
            Some("min-height")
        } else if matches!(parent_row.max_height, BoxMaxFact::Percentage(_)) {
            Some("max-height")
        } else {
            parent_offset_percentage
        };
        if let Some(property) = property {
            let Some(basis) = outer_basis else {
                return Err(E::Captured(unsupported(
                    1,
                    property,
                    CapturedClearanceUnsupported::OuterHeightBasis,
                ))
                .into());
            };
            let valid = basis.is_finite() && basis >= 0.0;
            if !valid {
                return Err(E::Percentage {
                    source: 1,
                    property,
                    error: ConversionError::InvalidContext,
                }
                .into());
            }
        }
    } else if let Some(property) =
        parent_offset_percentage.filter(|_| outer_limit_percentage(outer_row))
    {
        // VPH (the user's U2=A; CSS2.2 10.1, 10.7): O is the captured root, so its
        // percentage limits refer to the initial containing block's height, supplied by
        // the host or refused; a value that is not finite and nonnegative is O's
        // `InvalidContext`.
        let Some(viewport) = viewport_height else {
            return Err(E::Captured(unsupported(
                1,
                property,
                CapturedClearanceUnsupported::IndefiniteOffsetBasis,
            ))
            .into());
        };
        let valid = viewport.is_finite() && viewport >= 0.0;
        if !valid {
            let property = if matches!(outer_row.min_height, LengthPercentageBoxFact::Percentage(_))
            {
                "min-height"
            } else {
                "max-height"
            };
            return Err(E::Percentage {
                source: 0,
                property,
                error: ConversionError::InvalidContext,
            }
            .into());
        }
    }
    // VPH: O's used limits for the OAH pass, resolved here, at the refusal's position (a
    // percentage only with a valid viewport height, checked above).
    let outer_limits = match parent_offset_percentage {
        Some(_) if !outer_explicit => {
            Some(resolve_heights(outer_row, viewport_height, numeric, 0)?)
        }
        _ => None,
    };
    let parent_vertical = resolve_heights(parent_row, outer_basis, numeric, 1)?;
    let parent_offset = resolve_vertical_offsets(parent_row.offset, outer_basis, numeric, 1)?;
    let child_basis = match parent_vertical.height {
        BoxLengthFact::Absolute(value) => Some(constrain(
            value.pixels,
            parent_vertical.min,
            parent_vertical.max,
        )),
        _ => None,
    };
    // PCT (Q-6): P's horizontal solve runs here, before the children whose basis it is;
    // a failure is held and returned at F's width step.
    let parent_solve = solve_parent(facts, outer.content_left, outer.content_width, parent_row);
    // DAZ (G6b): the height category's view of P's solve (the vertical padding basis).
    let parent_solved = parent_solve.as_ref().ok();
    for source in 2..node_count {
        let row = fact_row(facts, source, "row")?;
        // CSS2.2 §9.4.1: the float establishes a block formatting context, and so does
        // every normal child (B1 or a tail) with `hidden` or `auto`; the label is set below
        // when its facts are converted; the core permits boundary contact with F's
        // margin box (§9.5, Q1). No child has content, so §11.1.1 `hidden` clips nothing and `auto`
        // has no overflow to scroll. `scroll` stays unsupported (Q4).
        match row.overflow.value {
            BoxOverflow::Visible => {}
            BoxOverflow::Hidden | BoxOverflow::Auto => {}
            _ => {
                return Err(E::Captured(unsupported(
                    source,
                    "overflow",
                    CapturedClearanceUnsupported::ChildOverflow,
                ))
                .into());
            }
        }
        // MMX (CSS2.2 10.7): the core receives the used height after the min/max
        // reruns. A computed Auto height is tentatively +0 (10.6.3 for a normal, 10.6.7
        // for F without content): F keeps the Auto category when its constrained
        // height is +0 (Q-6); a normal with a positive `min-height` is admitted with
        // that height (it cannot collapse through, 8.3.1); DAZ (G6b): an ordinary one
        // whose positive top or bottom border or padding separates its own margins is
        // an explicit +0 ("zero, otherwise", 10.6.3); MFZ (U4=A): so is a new-BFC one
        // (10.6.7 defines no childless height; Q3=A extended). NZS keeps computed
        // non-adjacency for other used-zero normals. PHT (10.5, 10.7): height facts resolve against
        // P's explicit used content height or take the indefinite rules, after the
        // admission tests above and before the held P check below (the basis does not
        // depend on P's horizontal solve). An explicit +0 goes to the core (G6a). CTB (the
        // user's U1=A): an ordinary undecorated B1 whose own margins adjoin goes as +0 and
        // may collapse through. NZS normals with a non-adjoining computed category
        // carry their zero used height without joining their own top and bottom margins.
        let vertical = resolve_heights(row, child_basis, numeric, source)?;
        if matches!(profile, LayoutProfile::Cluster(_))
            && is_float(source)
            && !matches!(vertical.height, BoxLengthFact::Absolute(_))
        {
            return Err(FloatClusterLayoutError::FloatHeight { source });
        }
        let auto_height = constrain(0.0, vertical.min, vertical.max);
        // CTB/CTT (CSS2.2 8.3.1, 10.5, 10.7): a normal's margins adjoin only with a computed
        // `height` of zero or `auto` and a computed `min-height` of zero; a percentage is
        // neither, nor is a length rounded to +0 (CT-2).
        let adjoining = !is_float(source)
            && matches!(row.min_height, LengthPercentageBoxFact::Absolute(min)
                if min.pixels == 0.0 && !min.rounded_to_zero)
            && match row.height {
                BoxLengthFact::Absolute(height) => height.pixels == 0.0 && !height.rounded_to_zero,
                BoxLengthFact::Auto { .. } => true,
                BoxLengthFact::Percentage(_) => false,
            };
        let height = match vertical.height {
            BoxLengthFact::Absolute(value) => {
                let used = constrain(value.pixels, vertical.min, vertical.max);
                // NZS: carry computed non-adjacency explicitly for the formerly
                // refused zero; existing physically separated cases keep their category.
                if !is_float(source)
                    && used == 0.0
                    && !adjoining
                    && !separated_auto(row, parent_solved, numeric)
                {
                    OwnerHeight::NonAdjoiningZero(used)
                } else {
                    OwnerHeight::Explicit(used)
                }
            }
            BoxLengthFact::Auto { .. } if is_float(source) && auto_height == 0.0 => {
                OwnerHeight::Auto
            }
            BoxLengthFact::Auto { .. } if auto_height > 0.0 => OwnerHeight::Explicit(auto_height),
            BoxLengthFact::Auto { .. } if separated_auto(row, parent_solved, numeric) => {
                OwnerHeight::Explicit(auto_height)
            }
            // CTB (10.6.3 "zero, otherwise"): the core decides between separated,
            // collapse-through and `NonpositiveBlockHeight`.
            BoxLengthFact::Auto { .. } if adjoining => OwnerHeight::Explicit(auto_height),
            BoxLengthFact::Auto { .. } if !is_float(source) && auto_height == 0.0 => {
                OwnerHeight::NonAdjoiningZero(auto_height)
            }
            BoxLengthFact::Auto { .. } => {
                return Err(E::Captured(unsupported(
                    source,
                    "height",
                    CapturedClearanceUnsupported::HeightKind,
                ))
                .into());
            }
            // PHT: `resolve_heights` leaves no height percentage.
            BoxLengthFact::Percentage(_) => {
                return Err(E::FactInvariant {
                    source,
                    property: "height",
                }
                .into());
            }
        };
        // PTB (CSS2.2 9.3.2): `top` and `bottom` percentages resolve against the same
        // basis, after the height facts and category and before the held P check; under an
        // Auto P they wait for P's used height (PTA). This pass fixes the error order; the
        // vertical pass after P's bottom resolves them again for the used offsets.
        resolve_vertical_offsets(row.offset, child_basis, numeric, source)?;
        let display = joined_display(projection, facts.display(), source).map_err(E::Captured)?;
        // PCT: after its admission tests and its height facts (PHT), the child's width-axis
        // percentages resolve against P's used content width; while P's failure is held,
        // only admission and the height facts continue.
        if matches!(profile, LayoutProfile::Cluster(_))
            && is_float(source)
            && matches!(row.width, BoxLengthFact::Auto { .. })
        {
            return Err(FloatClusterLayoutError::FloatWidth { source });
        }
        let Ok(parent) = &parent_solve else {
            continue;
        };
        let used = resolved_row(row, parent.horizontal.used.width, numeric, source)?;
        if matches!(profile, LayoutProfile::Cluster(_)) && is_float(source) {
            let BoxLengthFact::Absolute(width) = used.width else {
                return Err(E::FactInvariant {
                    source,
                    property: "width",
                }
                .into());
            };
            widths.push(constrain(
                width.pixels,
                length_operand(used.min_width, source, "min-width")?,
                max_value(used.max_width, source, "max-width")?,
            ));
        }
        let mut child = child_facts_from_row(
            nodes,
            display,
            &used,
            source,
            height,
            ChildMarginMode::Sequence,
        )
        .map_err(E::Captured)?;
        // CSS2.2 §9.4.1: a normal child (B1 or a tail) admitted above with `hidden` or
        // `auto` establishes a new block formatting context. The shared conversion labels
        // normals by position, so the Sequence facade, which alone admits such normals,
        // sets their context here.
        child.context = if is_float(source) {
            ChildContext::FloatBfc
        } else if row.overflow.value != BoxOverflow::Visible {
            ChildContext::OtherNewBfc
        } else {
            ChildContext::OrdinaryNonBfc
        };
        child_facts.push(child);
    }
    // MMX (CSS2.2 10.3.5 with 10.4): F's margins do not depend on its width, so the
    // min/max reruns only replace the width.
    let float_row = fact_row(facts, 3, "width")?;
    if matches!(profile, LayoutProfile::Single)
        && matches!(float_row.width, BoxLengthFact::Auto { .. })
    {
        return Err(E::Captured(unsupported(
            3,
            "width",
            CapturedClearanceUnsupported::FloatWidth,
        ))
        .into());
    }
    // PCT: the held P solve is returned here (Q-6); F's width is then resolved again
    // against P's used content width (C-1), where a failure is an invariant breach.
    let ParentSolve {
        outer_direction,
        parent_direction,
        used: parent_used,
        border: parent_border,
        padding: parent_padding,
        horizontal,
    } = parent_solve?;
    // PPM (PL-18): F's `min-width` and `max-width` are resolved there too, so
    // `constrain` reads resolved values only.
    let float_width =
        if matches!(profile, LayoutProfile::Single) {
            let Ok(BoxLengthFact::Absolute(float_width)) =
                resolve_length(float_row.width, horizontal.used.width, numeric)
            else {
                return Err(E::FactInvariant {
                    source: 3,
                    property: "width",
                }
                .into());
            };
            let float_min =
                resolve_length_percentage(float_row.min_width, horizontal.used.width, numeric)
                    .map_err(|_| E::FactInvariant {
                        source: 3,
                        property: "min-width",
                    })?;
            let float_max = resolve_max(float_row.max_width, horizontal.used.width, numeric)
                .map_err(|_| E::FactInvariant {
                    source: 3,
                    property: "max-width",
                })?;
            constrain(
                float_width.pixels,
                length_operand(float_min, 3, "min-width")?,
                max_value(float_max, 3, "max-width")?,
            )
        } else {
            0.0
        };
    let origin = place_first_child_origin(FirstChildOriginInput {
        containing_top: outer.content_top,
        margin_top: operand(parent_used.margin[0], 1, "margin-top")?,
        border_top: parent_border.top,
        padding_top: parent_padding.top,
    })
    .map_err(E::ParentOrigin)?;
    let selected = lowest_clearance_float_bottom(&[], parent_row.clear.value, NonZeroUsize::MIN)
        .map_err(E::ParentFloatSelection)?;
    let parent_clearance =
        classify_clearance(origin.border_top, selected).map_err(E::ParentPresence)?;
    if parent_clearance != ClearanceRequirement::Absent {
        return Err(E::UnexpectedParentClearance.into());
    }

    let remaining_for_core = limits
        .retained_bytes
        .get()
        .checked_sub(requested_children)
        .and_then(NonZeroUsize::new)
        .ok_or(E::Bytes)?;
    let mut core_limits = limits.core;
    core_limits.retained_bytes = NonZeroUsize::new(
        core_limits
            .retained_bytes
            .get()
            .min(remaining_for_core.get()),
    )
    .ok_or(E::Bytes)?;
    let outer_display = joined_display(projection, facts.display(), 0).map_err(E::Captured)?;
    let parent_display = joined_display(projection, facts.display(), 1).map_err(E::Captured)?;
    // REL: P's containing block is O's content box, so O's direction resolves P's
    // `left`/`right` overconstraint. OCH: P's `top`/`bottom` percentages were resolved
    // against O's used content height at P's vertical step (under an explicit O; under an
    // `auto` O they are still percentages here and are masked below); only those two
    // sides are copied into the row resolved against O's width (its `left`/`right` stay).
    let parent_used = ClearanceBoxFact {
        offset: parent_used
            .offset
            .zip(parent_offset)
            .map(|(mut used, resolved)| {
                used[0] = resolved[0];
                used[2] = resolved[2];
                used
            }),
        ..parent_used
    };
    // OAH: under an `auto` O, P's `top` and `bottom` percentages count as `auto` here (the
    // PTA mask); its vertical axis is solved after P's bottom (below).
    let parent_masked = vertical_percentages_as_auto(&parent_used);
    let mut parent_relative =
        used_relative_offsets(parent_display, &parent_masked, outer_direction, 1)?;
    let outer_facts = OuterFacts {
        source: &nodes[0],
        display: outer_display,
        content: OwnerContent::NoOwnContentNonReplaced,
        generated: OwnerGenerated::None,
    };
    let parent_facts = ParentFacts {
        source: &nodes[1],
        display: parent_display,
        content: OwnerContent::NoOwnContentNonReplaced,
        generated: OwnerGenerated::None,
        bfc: ParentBfc::NewWithEmptyPrefix,
        content_top: origin.content_top,
        containing: horizontal.content,
        border: parent_border,
        padding: parent_padding,
    };
    let owner = match profile {
        LayoutProfile::Single => GeometryOwner::Single(
            build_clearance_sequence(
                nodes,
                ClearanceSequenceInput {
                    outer: outer_facts,
                    parent: parent_facts,
                    children: &child_facts,
                    float_width,
                },
                core_limits,
            )
            .map_err(E::Core)?,
        ),
        LayoutProfile::Cluster(mut cluster_limits) => {
            cluster_limits.retained_bytes = NonZeroUsize::new(
                cluster_limits
                    .retained_bytes
                    .get()
                    .min(remaining_for_core.get()),
            )
            .ok_or(E::Bytes)?;
            let available_scratch = limits
                .scratch_bytes
                .get()
                .checked_sub(scratch_bytes)
                .and_then(|value| value.checked_sub(width_bytes))
                .and_then(NonZeroUsize::new)
                .ok_or(E::ScratchBytes)?;
            cluster_limits.scratch_bytes = NonZeroUsize::new(
                cluster_limits
                    .scratch_bytes
                    .get()
                    .min(available_scratch.get()),
            )
            .ok_or(E::ScratchBytes)?;
            GeometryOwner::Cluster(
                build_clearance_float_cluster(
                    nodes,
                    ClearanceFloatClusterInput {
                        outer: outer_facts,
                        parent: parent_facts,
                        children: &child_facts,
                        float_widths: &widths,
                    },
                    cluster_limits,
                )
                .map_err(FloatClusterLayoutError::Core)?,
            )
        }
    };
    let sequence = owner.view();

    sequence
        .retained_bytes()
        .checked_add(requested_children)
        .filter(|&bytes| bytes <= limits.retained_bytes.get())
        .ok_or(E::Bytes)?;
    let (mut children, child_bytes) = allocate_children(
        child_count,
        sequence.retained_bytes(),
        limits.retained_bytes,
        reserve_children,
    )?;

    let mut float_ordinal = 0_usize;
    let mut tail_ordinal = 0_usize;
    for source in 2..node_count {
        let row = fact_row(facts, source, "row")?;
        let child = child_facts
            .get(source - 2)
            .copied()
            .ok_or(E::FactInvariant {
                source,
                property: "child-facts",
            })?;
        let used_height = used_child_height(child, source, is_float(source))?;
        // PCT (C-1): the row is resolved again against P's used content width; a failure,
        // or a margin (PPM: or a padding) that differs from the first resolution (kept in
        // `child`), is an invariant breach.
        let used = resolved_row_with_width(
            row,
            horizontal.used.width,
            numeric,
            source,
            !(matches!(profile, LayoutProfile::Cluster(_)) && is_float(source)),
        )
        .map_err(|_| E::FactInvariant {
            source,
            property: "percentage",
        })?;
        let margins = [
            child.margin.top,
            child.margin.right,
            child.margin.bottom,
            child.margin.left,
        ];
        let paddings = [
            child.padding.top,
            child.padding.right,
            child.padding.bottom,
            child.padding.left,
        ];
        if used.margin.map(auto_length) != margins.map(Some)
            || padding_operands(&used, source)? != paddings
        {
            return Err(E::FactInvariant {
                source,
                property: "percentage",
            }
            .into());
        }
        // REL: a child's containing block is P's content box (P's direction). PTA: the
        // vertical axis is solved after P's bottom (below); here `top` and `bottom`
        // percentages count as `auto`.
        let masked = vertical_percentages_as_auto(&used);
        let relative = used_relative_offsets(child.display, &masked, parent_direction, source)?;
        let principal = if is_float(source) {
            let float = sequence.float(source).ok_or(E::FactInvariant {
                source,
                property: "cluster-float",
            })?;
            let band = float.content;
            let span = float.content_vertical;
            let padding_band = float.padding;
            let padding_span = float.padding_vertical;
            let border_band = float.border;
            let border_span = float.border_vertical;
            Principal {
                kind: PrincipalChildKind::Float,
                content: PrincipalEdges {
                    left: band.left,
                    right: band.right,
                    top: span.top,
                    bottom: span.bottom,
                },
                padding: PrincipalEdges {
                    left: padding_band.left,
                    right: padding_band.right,
                    top: padding_span.top,
                    bottom: padding_span.bottom,
                },
                border: PrincipalEdges {
                    left: border_band.left,
                    right: border_band.right,
                    top: border_span.top,
                    bottom: border_span.bottom,
                },
                used_width: match profile {
                    LayoutProfile::Single => float_width,
                    LayoutProfile::Cluster(_) => {
                        let width = *widths.get(float_ordinal).ok_or(E::FactInvariant {
                            source,
                            property: "float-width",
                        })?;
                        float_ordinal += 1;
                        width
                    }
                },
                normal_width: None,
                used_margin_left: child.margin.float_used_left(),
                used_margin_right: child.margin.float_used_right(),
            }
        } else {
            let (vertical, padding_vertical, content_vertical) = if source == 2 {
                (
                    sequence.before(),
                    sequence.before_padding(),
                    sequence.before_content(),
                )
            } else {
                let record = sequence.tail().get(tail_ordinal).ok_or(E::FactInvariant {
                    source,
                    property: "sequence-tail",
                })?;
                tail_ordinal += 1;
                if record.source_index() != source {
                    return Err(E::FactInvariant {
                        source,
                        property: "sequence-source",
                    }
                    .into());
                }
                (
                    record.vertical(),
                    record.padding_vertical(),
                    record.content_vertical(),
                )
            };
            let placed = place_child_horizontal(horizontal, parent_direction, &used, source)?;
            // CBR D3: the padding edges move out from the solver's content edges, then the
            // border edges move out from the published padding edges (staged, inside-out).
            let padding_left = child_border_edge(
                placed.content.left,
                child.padding.left,
                source,
                ChildBorderStage::Left,
            )?;
            let padding_right = child_border_edge(
                placed.content.right,
                child.padding.right,
                source,
                ChildBorderStage::Right,
            )?;
            let left = child_border_edge(
                padding_left,
                child.border.left,
                source,
                ChildBorderStage::Left,
            )?;
            let right = child_border_edge(
                padding_right,
                child.border.right,
                source,
                ChildBorderStage::Right,
            )?;
            Principal {
                kind: PrincipalChildKind::Normal,
                content: PrincipalEdges {
                    left: placed.content.left,
                    right: placed.content.right,
                    top: content_vertical.top,
                    bottom: content_vertical.bottom,
                },
                padding: PrincipalEdges {
                    left: padding_left,
                    right: padding_right,
                    top: padding_vertical.top,
                    bottom: padding_vertical.bottom,
                },
                border: PrincipalEdges {
                    left,
                    right,
                    top: vertical.top,
                    bottom: vertical.bottom,
                },
                used_width: placed.used.width,
                normal_width: Some(placed.used),
                used_margin_left: placed.used.margin_left,
                used_margin_right: placed.used.margin_right,
            }
        };
        children.push(PrincipalChildBox {
            source_index: source,
            containing_source_index: 1,
            kind: principal.kind,
            content: principal.content,
            padding: principal.padding,
            border: principal.border,
            used_width: principal.used_width,
            normal_width: principal.normal_width,
            used_margin_left: principal.used_margin_left,
            used_margin_right: principal.used_margin_right,
            used_height,
            used_margin_top: child.margin.used_top(),
            used_margin_bottom: child.margin.used_bottom(),
            computed_width: row.width,
            computed_height: row.height,
            computed_margin_left: row.margin[3],
            computed_margin_right: row.margin[1],
            margin_top: row.margin[0],
            margin_bottom: row.margin[2],
            clear: row.clear,
            relative,
        });
    }

    let greatest_float_bottom = sequence.greatest_float_bottom();
    let computed_height = parent_row.height;
    // PHT (PM-19): P's routes select on its used height category and limits from its
    // vertical step (a percentage `height` used as `auto` takes the Auto routes, a
    // percentage limit is 0 or `none`); `computed_height` is published unchanged.
    let (parent_min_height, parent_max_height) = (parent_vertical.min, parent_vertical.max);
    // S4/Q2=B: the signed route includes every normal's raw published margin
    // edges, including actual through participants. Group endpoints do not replace
    // these observations. The unsigned route keeps CTT-D1's source-last rule;
    // the later Auto-overflow check remains independent raw-box containment.
    let tentative = if matches!(parent_vertical.height, BoxLengthFact::Auto { .. })
        && children.iter().any(|child| {
            child.kind() == PrincipalChildKind::Normal
                && (child.used_margin_top() < 0.0 || child.used_margin_bottom() < 0.0)
        }) {
        let mut extrema: Option<(f32, f32)> = None;
        for child in &children {
            if child.kind() != PrincipalChildKind::Normal {
                continue;
            }
            let edges = publish_signed_normal_margin_edges(SignedNormalMarginInput {
                border_top: child.border().top,
                border_bottom: child.border().bottom,
                margin_top: child.used_margin_top(),
                margin_bottom: child.used_margin_bottom(),
            })
            .map_err(|error| E::NormalMargin {
                source: child.source_index(),
                error,
            })?;
            extrema = Some(match extrema {
                None => (edges.top, edges.bottom),
                Some((top, bottom)) => (top.min(edges.top), bottom.max(edges.bottom)),
            });
        }
        let (normal_margin_top, normal_margin_bottom) = extrema.ok_or(E::FactInvariant {
            source: 1,
            property: "normal-extrema",
        })?;
        solve_signed_bfc_height(SignedBfcHeightInput {
            content_top: origin.content_top,
            normal_margin_top,
            normal_margin_bottom,
            float_bottom: greatest_float_bottom,
        })
        .map_err(E::SignedHeight)?
    } else {
        let height_mode = match parent_vertical.height {
            // MMX (CSS2.2 10.7): the rerun with an explicit computed height yields it.
            BoxLengthFact::Absolute(value) => BfcHeightMode::Explicit(constrain(
                value.pixels,
                parent_min_height,
                parent_max_height,
            )),
            // PHT: `resolve_heights` leaves no height percentage.
            BoxLengthFact::Percentage(_) => {
                return Err(E::FactInvariant {
                    source: 1,
                    property: "height",
                }
                .into());
            }
            BoxLengthFact::Auto { .. } => {
                // Floats are out of flow: the source-last normal may precede them.
                let last_child = children
                    .iter()
                    .rev()
                    .find(|child| child.kind() == PrincipalChildKind::Normal)
                    .ok_or(E::FactInvariant {
                        source: 1,
                        property: "principal-last",
                    })?;
                let last_source = last_child.source_index();
                let last_in_flow_margin_bottom = last_child.used_margin_bottom();
                let last_bottom = match sequence.tail().last() {
                    Some(last) if last.source_index() == last_source => last.vertical().bottom,
                    None if last_source == 2 => sequence.before().bottom,
                    _ => {
                        return Err(E::FactInvariant {
                            source: last_source,
                            property: "sequence-source",
                        }
                        .into());
                    }
                };
                BfcHeightMode::Auto {
                    last_in_flow_border_bottom: last_bottom,
                    last_in_flow_margin_bottom,
                    float_bottom: greatest_float_bottom,
                }
            }
        };
        solve_bfc_height(BfcHeightInput {
            content_top: origin.content_top,
            mode: height_mode,
        })
        .map_err(E::Height)?
    };
    // MMX (CSS2.2 10.7): either Auto route above gives the tentative placement, which
    // is constrained once by `min-height`/`max-height`; P's bottom and the `auto`
    // containment use the result. An explicit height is already constrained.
    let height = match parent_vertical.height {
        BoxLengthFact::Auto { .. } => constrain_bfc_height(
            origin.content_top,
            tentative,
            parent_min_height,
            parent_max_height,
        )
        .map_err(E::Height)?,
        BoxLengthFact::Absolute(_) => tentative,
        // PHT: `resolve_heights` leaves no height percentage.
        BoxLengthFact::Percentage(_) => {
            return Err(E::FactInvariant {
                source: 1,
                property: "height",
            }
            .into());
        }
    };
    // PPM (PL-18): P's bottom reads its resolved row and border edges.
    let bottom = place_block_bottom(BlockBottomInput {
        content_bottom: height.content_bottom,
        padding_bottom: length_operand(parent_used.padding[2], 1, "padding-bottom")?,
        border_bottom: parent_border.bottom,
        margin_bottom: operand(parent_used.margin[2], 1, "margin-bottom")?,
    })
    .map_err(E::Bottom)?;
    // OAH (G4d-3b-P; CSS2.2 9.3.2, 9.4.3): under an `auto` O, P's `top` and `bottom`
    // percentages resolve here, after P's bottom and before the children's vertical pass,
    // against O's used content height (`outer_auto_height`); O's limits were resolved at
    // P's step (VPH, the user's U2=A: a percentage one against the initial containing
    // block's height, or refused there without it). P's vertical offset axis is then
    // solved; its horizontal axis was solved with them counted as `auto`. A host value is
    // not read.
    if let Some(outer_limits) = outer_limits {
        let basis = outer_auto_height(&outer_limits, outer.content_top, bottom.margin_bottom);
        let offset = resolve_vertical_offsets(parent_row.offset, Some(basis), numeric, 1)?;
        if let Some([top, _, end, _]) = offset {
            let vertical = relative_offsets(RelativeInput {
                left: AutoLength::Auto,
                right: AutoLength::Auto,
                top: operand(top, 1, "top")?,
                bottom: operand(end, 1, "bottom")?,
                containing_direction: outer_direction,
            })
            .map_err(|_| E::FactInvariant {
                source: 1,
                property: "relative-offset",
            })?;
            parent_relative.top = vertical.top;
            parent_relative.bottom = vertical.bottom;
        }
    }
    // PTA (CSS2.2 9.3.2, 9.4.3): offsets apply once layout is done, so each child's
    // `top` and `bottom` resolve against P's used content height, explicit or Auto, and
    // its vertical offset axis is solved here, after P's bottom and before the `auto`
    // containment test, children in source order. Under an explicit P the child loop
    // resolved them against the same value, so a failure here needs an Auto P. The facts
    // hold `offset` only on a relatively positioned Sequence row, so a static child has
    // none, is skipped here and keeps `NO_OFFSETS` (`used_relative_offsets` tests the
    // display instead).
    // OCH (PTA code review I-1, PA-13): under an explicit P the basis is `child_basis`,
    // the value the child loop resolved against, so the error order and the published
    // offsets share one value; under an Auto P it is P's final used content height.
    let basis = child_basis.unwrap_or(height.used_height);
    for child in &mut children {
        let source = child.source_index();
        let row = fact_row(facts, source, "row")?;
        let offset = resolve_vertical_offsets(row.offset, Some(basis), numeric, source)?;
        let Some([top, _, bottom, _]) = offset else {
            continue;
        };
        let vertical = relative_offsets(RelativeInput {
            left: AutoLength::Auto,
            right: AutoLength::Auto,
            top: operand(top, source, "top")?,
            bottom: operand(bottom, source, "bottom")?,
            containing_direction: parent_direction,
        })
        .map_err(|_| E::FactInvariant {
            source,
            property: "relative-offset",
        })?;
        child.relative.top = vertical.top;
        child.relative.bottom = vertical.bottom;
    }

    // CSS2.2 §11.1/§11.1.1: `auto` on P is admitted only when no content of P lies outside
    // P's content box, so nothing overflows P, `auto` presents no scrollbar (Architecture 60)
    // and the layout is the `hidden` layout. Every child's border box and margin box must lie
    // within P's published content box, edges included. A normal's margin edges are its
    // published border edges moved by its used margins in nearest-even f32 steps, as P's Auto
    // height uses the last bottom on the nonnegative route or signed extrema; the
    // float's margin box is `placed_float()`. Containment remains a separate check.
    if parent_row.overflow.value == BoxOverflow::Auto {
        let area = PrincipalEdges {
            left: horizontal.content.left,
            right: horizontal.content.right,
            top: origin.content_top,
            bottom: height.content_bottom,
        };
        for child in &children {
            let border = child.border();
            let margin = if child.kind() == PrincipalChildKind::Float {
                let placed = sequence
                    .float(child.source_index())
                    .ok_or(E::FactInvariant {
                        source: child.source_index(),
                        property: "cluster-float",
                    })?
                    .placed;
                PrincipalEdges {
                    left: placed.left,
                    right: placed.right,
                    top: placed.top,
                    bottom: placed.bottom,
                }
            } else {
                PrincipalEdges {
                    left: border.left - child.used_margin_left(),
                    right: border.right + child.used_margin_right(),
                    top: border.top - child.used_margin_top(),
                    bottom: border.bottom + child.used_margin_bottom(),
                }
            };
            // REL (Q4=C): the child's own offsets translate both boxes after they are
            // computed as above, one f32 addition per edge; P's offset is not applied.
            let (dx, dy) = (child.relative.left, child.relative.top);
            let border = shift(border, dx, dy);
            let margin = shift(margin, dx, dy);
            if !within(border, area) || !within(margin, area) {
                return Err(E::ParentAutoOverflow {
                    source: child.source_index(),
                }
                .into());
            }
        }
    }

    let parent = SiblingParentGeometry {
        source_index: 1,
        containing_source_index: 0,
        horizontal,
        origin,
        height,
        bottom,
        computed_width: parent_row.width,
        computed_height,
        computed_margin_top: parent_row.margin[0],
        computed_margin_bottom: parent_row.margin[2],
        clear: parent_row.clear,
        clearance: parent_clearance,
        relative: parent_relative,
    };
    let retained_bytes = sequence
        .retained_bytes()
        .checked_add(child_bytes)
        .ok_or(E::Bytes)?;
    let scratch_bytes = scratch_bytes
        .checked_add(width_bytes)
        .and_then(|bytes| bytes.checked_add(sequence.scratch_bytes()))
        .ok_or(E::ScratchBytes)?;
    if scratch_bytes > limits.scratch_bytes.get() {
        return Err(E::ScratchBytes.into());
    }
    let width_capacity = widths.capacity();
    drop(child_facts);
    drop(widths);
    Ok(GeometryOutput {
        owner,
        parent,
        children,
        nodes: node_count,
        work,
        retained_bytes,
        scratch_bytes,
        width_capacity,
    })
}

struct FloatGeometry {
    placed: PlacedFloat,
    border: Band,
    border_vertical: BlockVertical,
    padding: Band,
    padding_vertical: BlockVertical,
    content: Band,
    content_vertical: BlockVertical,
}

enum GeometryView<'o, 'p> {
    Single(&'o ClearanceSequenceOwner<'p>),
    Cluster(&'o ClearanceFloatClusterOwner<'p>),
}
impl GeometryView<'_, '_> {
    fn before(&self) -> BlockVertical {
        match self {
            Self::Single(v) => v.before(),
            Self::Cluster(v) => v.before(),
        }
    }
    fn before_content(&self) -> BlockVertical {
        match self {
            Self::Single(v) => v.before_content(),
            Self::Cluster(v) => v.before_content(),
        }
    }
    fn before_padding(&self) -> BlockVertical {
        match self {
            Self::Single(v) => v.before_padding(),
            Self::Cluster(v) => v.before_padding(),
        }
    }
    fn tail(&self) -> &[SequenceNormalPlacement] {
        match self {
            Self::Single(v) => v.tail(),
            Self::Cluster(v) => v.tail(),
        }
    }
    fn retained_bytes(&self) -> usize {
        match self {
            Self::Single(v) => v.receipt().retained_bytes,
            Self::Cluster(v) => v.receipt().retained_bytes,
        }
    }
    fn scratch_bytes(&self) -> usize {
        match self {
            Self::Single(_) => 0,
            Self::Cluster(v) => v.receipt().scratch_bytes,
        }
    }
    fn float(&self, source: usize) -> Option<FloatGeometry> {
        match self {
            Self::Single(v) if source == 3 => Some(FloatGeometry {
                placed: v.placed_float(),
                border: v.float_border(),
                border_vertical: v.float_border_vertical(),
                content: v.float_content(),
                content_vertical: v.float_content_vertical(),
                padding: v.float_padding(),
                padding_vertical: v.float_padding_vertical(),
            }),
            Self::Single(_) => None,
            Self::Cluster(v) => {
                let floats = v.floats();
                let f = &floats[floats
                    .binary_search_by_key(&source, |f| f.source_index())
                    .ok()?];
                Some(FloatGeometry {
                    placed: f.placed_float(),
                    border: f.border(),
                    border_vertical: f.border_vertical(),
                    content: f.content(),
                    content_vertical: f.content_vertical(),
                    padding: f.padding(),
                    padding_vertical: f.padding_vertical(),
                })
            }
        }
    }
    fn greatest_float_bottom(&self) -> f32 {
        match self {
            Self::Single(v) => v.placed_float().bottom,
            Self::Cluster(v) => {
                let mut floats = v.floats().iter();
                let first = floats
                    .next()
                    .expect("validated nonempty float cluster")
                    .placed_float()
                    .bottom;
                floats.fold(first, |bottom, f| bottom.max(f.placed_float().bottom))
            }
        }
    }
}

fn cluster_requested_retained(nodes: usize, floats: usize) -> Option<usize> {
    let tail = nodes.checked_sub(floats)?.checked_sub(3)?;
    let members = tail.checked_add(1)?.checked_mul(2)?;
    checked_bytes::<FloatClusterFloatPlacement>(floats)?
        .checked_add(checked_bytes::<SequenceNormalPlacement>(tail)?)?
        .checked_add(checked_bytes::<MarginMember>(members)?)
        .filter(|&bytes| bytes <= isize::MAX as usize)
}

fn classify_cluster(
    facts: &SiblingBoxFacts<'_, '_, '_, '_, '_, '_>,
) -> Result<usize, FloatClusterLayoutError> {
    use SiblingLayoutError as E;
    let projection = facts.projection();
    let count = projection.nodes().len();
    for source in 0..count {
        fact_row(facts, source, "row")?;
        joined_display(projection, facts.display(), source).map_err(E::Captured)?;
    }
    if facts.row(count).is_some() {
        return Err(E::FactInvariant {
            source: count,
            property: "row-count",
        }
        .into());
    }
    let mut floats = 0usize;
    for source in 0..count {
        let display = joined_display(projection, facts.display(), source).map_err(E::Captured)?;
        let floating = display.flow == Flow::Floating;
        if floating && source < 3 {
            return Err(FloatClusterLayoutError::ClusterOrder { source });
        }
        if !admitted_display(display, floating, source == 0) {
            return Err(E::Captured(unsupported(
                source,
                "display/position/float",
                CapturedClearanceUnsupported::DisplayOrFlow,
            ))
            .into());
        }
        if floating {
            floats = floats.checked_add(1).ok_or(E::Sources)?;
        }
    }
    if floats == 0 {
        return Err(FloatClusterLayoutError::ClusterOrder { source: count - 1 });
    }
    Ok(floats)
}

// Named fields instead of a positional tuple (NFH-T2Q-002).
struct Principal {
    kind: PrincipalChildKind,
    content: PrincipalEdges,
    padding: PrincipalEdges,
    border: PrincipalEdges,
    used_width: f32,
    normal_width: Option<BlockWidthResult>,
    used_margin_left: f32,
    used_margin_right: f32,
}

/// `outer` (O) admits only `static`; P and the children also admit `relative` (REL).
fn admitted_display(display: DisplayResolution, floating: bool, outer: bool) -> bool {
    display.display == Display::Block
        && (display.position == Position::Static
            || (!outer && display.position == Position::Relative))
        && if floating {
            display.flow == Flow::Floating && matches!(display.float, Float::Left | Float::Right)
        } else {
            display.flow == Flow::Normal && display.float == Float::None
        }
}

/// REL: four +0 offsets, published for every static box.
const NO_OFFSETS: RelativeOffsets = RelativeOffsets {
    left: 0.0,
    right: 0.0,
    top: 0.0,
    bottom: 0.0,
};

/// REL (CSS2.2 9.4.3): the used offsets of a relatively positioned box, solved with
/// its containing block's `direction`; a static box gets `NO_OFFSETS` without reading
/// any offset fact. The facts guarantee a resolved, finite offset row, so both
/// failures are invariant breaches (Q-9).
fn used_relative_offsets(
    display: DisplayResolution,
    row: &ClearanceBoxFact,
    direction: Direction,
    source: usize,
) -> Result<RelativeOffsets, SiblingLayoutError> {
    if display.position != Position::Relative {
        return Ok(NO_OFFSETS);
    }
    let [top, right, bottom, left] = row.offset.ok_or(SiblingLayoutError::FactInvariant {
        source,
        property: "offset",
    })?;
    relative_offsets(RelativeInput {
        left: operand(left, source, "left")?,
        right: operand(right, source, "right")?,
        top: operand(top, source, "top")?,
        bottom: operand(bottom, source, "bottom")?,
        containing_direction: direction,
    })
    .map_err(|_| SiblingLayoutError::FactInvariant {
        source,
        property: "relative-offset",
    })
}

/// REL: `edges` translated by (`dx`, `dy`), one f32 addition per edge.
fn shift(edges: PrincipalEdges, dx: f32, dy: f32) -> PrincipalEdges {
    PrincipalEdges {
        left: edges.left + dx,
        right: edges.right + dx,
        top: edges.top + dy,
        bottom: edges.bottom + dy,
    }
}

/// Every edge coordinate of `edges` lies in `area` on its own axis, bounds included.
fn within(edges: PrincipalEdges, area: PrincipalEdges) -> bool {
    [edges.left, edges.right]
        .into_iter()
        .all(|x| (area.left..=area.right).contains(&x))
        && [edges.top, edges.bottom]
            .into_iter()
            .all(|y| (area.top..=area.bottom).contains(&y))
}

/// MMX (CSS2.2 10.4, 10.7): the used size after the min/max reruns of a size whose
/// rules give the computed value back (an explicit size, F's width, a tentative Auto
/// height): greater than `max` gives `max`, then smaller than `min` gives `min`, so
/// `min` wins. No arithmetic, so no rounding; zero is +0.
fn constrain(value: f32, min: f32, max: Option<f32>) -> f32 {
    let value = max.filter(|&max| value > max).unwrap_or(value);
    let value = if value < min { min } else { value };
    if value == 0.0 { 0.0 } else { value }
}

/// MMX: a computed `max-*` fact as the solver's optional maximum. PPM: a percentage
/// left in a row resolved by `resolved_row` is an invariant breach.
fn max_value(
    fact: BoxMaxFact,
    source: usize,
    property: &'static str,
) -> Result<Option<f32>, SiblingLayoutError> {
    match fact {
        BoxMaxFact::Absolute(value) => Ok(Some(value.pixels)),
        BoxMaxFact::None { .. } => Ok(None),
        BoxMaxFact::Percentage(_) => Err(SiblingLayoutError::FactInvariant { source, property }),
    }
}

/// OAH (CSS2.2 10.7, 10.1): O's `min-height` or `max-height` is a percentage. O is the
/// captured root, so it refers to the initial containing block's height, which the host
/// may supply (VPH, the user's U2=A); without it P's `top`/`bottom` percentage under an
/// `auto` O stays refused at P's step.
fn outer_limit_percentage(outer: &ClearanceBoxFact) -> bool {
    matches!(outer.min_height, LengthPercentageBoxFact::Percentage(_))
        || matches!(outer.max_height, BoxMaxFact::Percentage(_))
}

/// OAH (CSS2.2 10.6.3, 8.3.1, 10.7): O's used content height under an `auto` O, the
/// basis of P's `top` and `bottom` percentages (9.3.2). O is the captured root (10.1,
/// 10.5): its margins do not collapse, P is its only in-flow child and P's offsets are
/// ignored, so the tentative height is the distance from O's content top to P's bottom
/// margin edge, one f32 subtraction. O's used `min-height` and `max-height` from P's
/// step (`limits`; VPH, the user's U2=A: a percentage one resolved against the initial
/// containing block's height) then constrain it (10.7): MFZ (the user's U3=A) reads 10.7
/// literally, so a negative tentative height, which is not greater than `max-height` and
/// is smaller than `min-height` (0 by default), gives `min-height`.
fn outer_auto_height(limits: &VerticalUsed, content_top: f32, margin_bottom: f32) -> f32 {
    constrain(margin_bottom - content_top, limits.min, limits.max)
}

/// PHT: the used height facts of a row: `height` is `Absolute` or `Auto` (never a
/// percentage), `min` the used `min-height` and `max` the used `max-height` (`None` for
/// `none`).
struct VerticalUsed {
    height: BoxLengthFact,
    min: f32,
    max: Option<f32>,
}

/// PHT (CSS2.2 10.5, 10.7): the used `height`, `min-height` and `max-height` of `row` in
/// the fixed order `height`, `min-height`, `max-height`, so that the first failing
/// property is deterministic. With `Some(basis)`, the containing block's explicit used
/// content height, a percentage resolves by one exact product rounded once
/// (`resolve_percentage`; a failure is `Percentage` at `source`). With `None` (the
/// containing block's height depends on content) a `height` percentage is used as `auto`
/// with its origin, a `min-height` percentage as 0 and a `max-height` percentage as
/// `none`. Absolute, `auto` and `none` facts pass through.
fn resolve_heights(
    row: &ClearanceBoxFact,
    basis: Option<f32>,
    limits: NumericLimits,
    source: usize,
) -> Result<VerticalUsed, SiblingLayoutError> {
    let resolve = |percentage: PercentageBoxFact, basis: f32, property: &'static str| {
        resolve_percentage(percentage, basis, limits).map_err(|error| {
            SiblingLayoutError::Percentage {
                source,
                property,
                error,
            }
        })
    };
    let height = match (row.height, basis) {
        (BoxLengthFact::Percentage(percentage), Some(basis)) => {
            BoxLengthFact::Absolute(resolve(percentage, basis, "height")?)
        }
        (BoxLengthFact::Percentage(percentage), None) => BoxLengthFact::Auto {
            origin: percentage.origin,
        },
        (other, _) => other,
    };
    let min = match (row.min_height, basis) {
        (LengthPercentageBoxFact::Absolute(value), _) => value.pixels,
        (LengthPercentageBoxFact::Percentage(percentage), Some(basis)) => {
            resolve(percentage, basis, "min-height")?.pixels
        }
        (LengthPercentageBoxFact::Percentage(_), None) => 0.0,
    };
    let max = match (row.max_height, basis) {
        (BoxMaxFact::Absolute(value), _) => Some(value.pixels),
        (BoxMaxFact::None { .. }, _) => None,
        (BoxMaxFact::Percentage(percentage), Some(basis)) => {
            Some(resolve(percentage, basis, "max-height")?.pixels)
        }
        (BoxMaxFact::Percentage(_), None) => None,
    };
    Ok(VerticalUsed { height, min, max })
}

/// DAZ (G6b; CSS2.2 10.6.3, 8.3.1): an ordinary (`visible`) Auto normal has no in-flow
/// children or line boxes, so its tentative height is zero ("zero, otherwise"); the
/// caller reaches this test only when the constrained height is +0. A positive top or
/// bottom border or padding separates its own top and bottom margins, so it cannot
/// collapse through and goes to the core as an explicit +0 (G6a). MFZ: a new-BFC
/// (`hidden` or `auto`) Auto normal has no height defined by 10.6.7 without children;
/// the user's U4=A extends Q3=A to it, so it is zero, and its own margins are not
/// adjoining (8.3.1), as the core's `own_margins_separated` reads: it is separated
/// before any padding is read. A vertical padding percentage resolves against P's used
/// content width as in `resolved_row` (CSS2.2 8.4); without P's solve (held, PCT Q-6),
/// when it does not resolve, or when it rounds to +0, it does not separate by itself
/// (the core's `vertical_decoration` tests the same edges with `> 0.`); when it is the
/// only possible physical separation, this helper returns false. NZS carries computed
/// non-adjacency separately; CTB/CTT retains the literal adjoining category. The caller
/// also asks this helper for an explicit normal used as +0.
fn separated_auto(
    row: &ClearanceBoxFact,
    parent: Option<&ParentSolve>,
    limits: NumericLimits,
) -> bool {
    let padding = |fact: LengthPercentageBoxFact| match (fact, parent) {
        (LengthPercentageBoxFact::Absolute(value), _) => value.pixels,
        (LengthPercentageBoxFact::Percentage(percentage), Some(parent)) => {
            resolve_percentage(percentage, parent.horizontal.used.width, limits)
                .map_or(0.0, |value| value.pixels)
        }
        (LengthPercentageBoxFact::Percentage(_), None) => 0.0,
    };
    let border = row.border_widths();
    row.overflow.value != BoxOverflow::Visible
        || border[0] > 0.0
        || border[2] > 0.0
        || padding(row.padding[0]) > 0.0
        || padding(row.padding[2]) > 0.0
}

/// PTB, PTA (CSS2.2 9.3.2, 9.4.3): `offset` of a relatively positioned row with its `top`
/// and `bottom` percentages resolved, in that order, against `basis`, the containing
/// block's used content height, by one exact product rounded once (`resolve_percentage`;
/// a failure is `Percentage` at `source`); `right` and `left` are left as they are
/// (`resolved_row`). With `None` a percentage is left as it is: for a child, P's height
/// is `auto` and not known yet (the pass after P's bottom resolves it); for P, O supplies
/// no basis at P's step: under an explicit O without a host value that step has refused
/// such a percentage (OCH), and under an `auto` O the OAH pass after P's bottom resolves
/// it with `Some` (or refuses it). Each side is resolved on its own, even when 9.4.3 then
/// ignores `bottom` because `top` is not `auto` (a conservative choice, as REL refuses an
/// ignored `bottom` in `em`). A static row (`None`) passes through.
fn resolve_vertical_offsets(
    offset: Option<[BoxLengthFact; 4]>,
    basis: Option<f32>,
    limits: NumericLimits,
    source: usize,
) -> Result<Option<[BoxLengthFact; 4]>, SiblingLayoutError> {
    let Some(mut offset) = offset else {
        return Ok(None);
    };
    for (side, property) in [(0, "top"), (2, "bottom")] {
        let BoxLengthFact::Percentage(percentage) = offset[side] else {
            continue;
        };
        let Some(basis) = basis else {
            continue;
        };
        let resolved = resolve_percentage(percentage, basis, limits).map_err(|error| {
            SiblingLayoutError::Percentage {
                source,
                property,
                error,
            }
        })?;
        offset[side] = BoxLengthFact::Absolute(resolved);
    }
    Ok(Some(offset))
}

/// PPM: the solver operand of a padding or `min-width` fact of a row resolved by
/// `resolved_row`; a percentage left there is an invariant breach.
fn length_operand(
    fact: LengthPercentageBoxFact,
    source: usize,
    property: &'static str,
) -> Result<f32, SiblingLayoutError> {
    fact.absolute()
        .map(|value| value.pixels)
        .ok_or(SiblingLayoutError::FactInvariant { source, property })
}

/// PPM: the four paddings (top, right, bottom, left) of a resolved row as solver
/// operands.
fn padding_operands(row: &ClearanceBoxFact, source: usize) -> Result<[f32; 4], SiblingLayoutError> {
    Ok([
        length_operand(row.padding[0], source, "padding-top")?,
        length_operand(row.padding[1], source, "padding-right")?,
        length_operand(row.padding[2], source, "padding-bottom")?,
        length_operand(row.padding[3], source, "padding-left")?,
    ])
}

/// PCT: P's resolved row and horizontal solve, held by the caller (Q-6).
struct ParentSolve {
    outer_direction: Direction,
    parent_direction: Direction,
    used: ClearanceBoxFact,
    border: OwnerEdges,
    padding: OwnerEdges,
    horizontal: BlockHorizontalPlacement,
}

/// PCT: the two direction joins (deliberately ordered O then P: O governs P's width
/// equation, P every normal child's), then P's percentages resolved against O's content
/// width and P's horizontal solve, in their order before PCT. An invalid O basis (a
/// nonfinite left, or a width that is nonfinite or negative) keeps the solver's own
/// error unconditionally: P's percentages are then never resolved (they become +0, see
/// `zeroed_row`) and the solver refuses the origin or `containing_width` before it reads
/// any operand. PPM: P's paddings, `min-width` and `max-width` are resolved, and zeroed,
/// with the others, so every operand read below succeeds before the solver. This
/// mirrors `place_block_horizontal`'s first checks; T18 and R18 pin the order.
fn solve_parent(
    facts: &SiblingBoxFacts<'_, '_, '_, '_, '_, '_>,
    content_left: f32,
    content_width: f32,
    parent_row: &ClearanceBoxFact,
) -> Result<ParentSolve, SiblingLayoutError> {
    use SiblingLayoutError as E;
    let projection = facts.projection();
    let outer_direction = joined_direction(projection, facts.display(), 0).map_err(E::Captured)?;
    let parent_direction = joined_direction(projection, facts.display(), 1).map_err(E::Captured)?;
    let valid = content_left.is_finite() && content_width.is_finite() && content_width >= 0.0;
    let used = if valid {
        resolved_row(parent_row, content_width, facts.numeric_limits(), 1)?
    } else {
        zeroed_row(parent_row)
    };
    let border = edges(used.border_widths());
    let padding = edges(padding_operands(&used, 1)?);
    let horizontal = place_block_horizontal(
        content_left,
        BlockWidthInput {
            containing_width: content_width,
            direction: outer_direction,
            width: operand(used.width, 1, "width")?,
            margin_left: operand(used.margin[3], 1, "margin-left")?,
            margin_right: operand(used.margin[1], 1, "margin-right")?,
            border_left: border.left,
            border_right: border.right,
            padding_left: padding.left,
            padding_right: padding.right,
            min_width: length_operand(used.min_width, 1, "min-width")?,
            max_width: max_value(used.max_width, 1, "max-width")?,
        },
    )
    .map_err(E::ParentHorizontal)?;
    Ok(ParentSolve {
        outer_direction,
        parent_direction,
        used,
        border,
        padding,
        horizontal,
    })
}

/// PCT (CSS2.2 10.2, 8.3, 9.3.2) and PPM (8.4, 10.4): `percentage` resolved against
/// `basis`, the containing block's width (PHT, PTB, PTA: its used content height), by
/// one exact product rounded once to the nearest f32 (zero is +0), keeping its origin
/// and recording the rounding flag.
fn resolve_percentage(
    percentage: PercentageBoxFact,
    basis: f32,
    limits: NumericLimits,
) -> Result<AbsoluteBoxFact, ConversionError> {
    let resolved = percentage.value.percentage_finite_f32(basis, limits)?;
    Ok(AbsoluteBoxFact {
        pixels: resolved.value,
        rounded_to_zero: resolved.rounded_to_zero,
        origin: percentage.origin,
    })
}

/// PCT: `fact` with a percentage resolved (`resolve_percentage`); any other fact is
/// returned unchanged.
fn resolve_length(
    fact: BoxLengthFact,
    basis: f32,
    limits: NumericLimits,
) -> Result<BoxLengthFact, ConversionError> {
    match fact {
        BoxLengthFact::Percentage(percentage) => Ok(BoxLengthFact::Absolute(resolve_percentage(
            percentage, basis, limits,
        )?)),
        other => Ok(other),
    }
}

/// PPM: a padding or `min-width` fact with a percentage resolved; an absolute fact is
/// returned unchanged.
fn resolve_length_percentage(
    fact: LengthPercentageBoxFact,
    basis: f32,
    limits: NumericLimits,
) -> Result<LengthPercentageBoxFact, ConversionError> {
    match fact {
        LengthPercentageBoxFact::Percentage(percentage) => Ok(LengthPercentageBoxFact::Absolute(
            resolve_percentage(percentage, basis, limits)?,
        )),
        other => Ok(other),
    }
}

/// PPM: a `max-width` fact with a percentage resolved; an absolute or `none` fact is
/// returned unchanged.
fn resolve_max(
    fact: BoxMaxFact,
    basis: f32,
    limits: NumericLimits,
) -> Result<BoxMaxFact, ConversionError> {
    match fact {
        BoxMaxFact::Percentage(percentage) => Ok(BoxMaxFact::Absolute(resolve_percentage(
            percentage, basis, limits,
        )?)),
        other => Ok(other),
    }
}

/// PCT: a copy of `row` whose percentages are resolved against `basis`, in the fixed
/// order `width`, `margin-top`, `margin-right`, `margin-bottom`, `margin-left`, `right`,
/// `left`, then (PPM) `padding-top`, `padding-right`, `padding-bottom`, `padding-left`,
/// `min-width`, `max-width`, so that the first failing property is deterministic; a
/// failure is `Percentage` at `source`. PHT: the height facts are not resolved here (see
/// `resolve_heights`); vertical paddings keep the width basis (CSS2.2 8.4). PTB: `top`
/// and `bottom` are not resolved here either (see `resolve_vertical_offsets`). It never
/// rewrites offset sides 0 and 2: every reader of P's and the children's `top` and
/// `bottom` (P's vertical step, the copy of P's resolved sides into P's used row, the
/// child loop, the OAH pass and the vertical pass) takes them from the fact row. EML and
/// EXF: an `em` or `ex` offset is already absolute in the fact row (a computed value,
/// CSS2.2 6.1.2), so no unit is converted here.
fn resolved_row(
    row: &ClearanceBoxFact,
    basis: f32,
    limits: NumericLimits,
    source: usize,
) -> Result<ClearanceBoxFact, SiblingLayoutError> {
    resolved_row_with_width(row, basis, limits, source, true)
}

fn resolved_row_with_width(
    row: &ClearanceBoxFact,
    basis: f32,
    limits: NumericLimits,
    source: usize,
    resolve_width: bool,
) -> Result<ClearanceBoxFact, SiblingLayoutError> {
    let resolve = |fact: BoxLengthFact, property: &'static str| {
        resolve_length(fact, basis, limits).map_err(|error| SiblingLayoutError::Percentage {
            source,
            property,
            error,
        })
    };
    let resolve_lp = |fact: LengthPercentageBoxFact, property: &'static str| {
        resolve_length_percentage(fact, basis, limits).map_err(|error| {
            SiblingLayoutError::Percentage {
                source,
                property,
                error,
            }
        })
    };
    let mut used = *row;
    if resolve_width {
        used.width = resolve(row.width, "width")?;
    }
    used.margin[0] = resolve(row.margin[0], "margin-top")?;
    used.margin[1] = resolve(row.margin[1], "margin-right")?;
    used.margin[2] = resolve(row.margin[2], "margin-bottom")?;
    used.margin[3] = resolve(row.margin[3], "margin-left")?;
    if let Some(offset) = used.offset.as_mut() {
        offset[1] = resolve(offset[1], "right")?;
        offset[3] = resolve(offset[3], "left")?;
    }
    used.padding[0] = resolve_lp(row.padding[0], "padding-top")?;
    used.padding[1] = resolve_lp(row.padding[1], "padding-right")?;
    used.padding[2] = resolve_lp(row.padding[2], "padding-bottom")?;
    used.padding[3] = resolve_lp(row.padding[3], "padding-left")?;
    if resolve_width {
        used.min_width = resolve_lp(row.min_width, "min-width")?;
        used.max_width = resolve_max(row.max_width, basis, limits).map_err(|error| {
            SiblingLayoutError::Percentage {
                source,
                property: "max-width",
                error,
            }
        })?;
    }
    Ok(used)
}

/// PCT: a copy of `row` whose percentages are +0 with their origin, without
/// the percentage primitive, for an invalid O basis that the P solver refuses before it
/// reads any operand (PPM: paddings, `min-width` and `max-width` included). PHT: the
/// height facts are left as they are; no height reader reads this row. PTB: P's `top` and
/// `bottom` percentages are refused at P's vertical step, before this row exists, or
/// (OCH) resolved at that step against O's used content height, or (OAH, under an `auto`
/// O) left as percentages and resolved after P's bottom; the P solver error this row
/// leads to is returned before any of them is copied into P's used row or resolved.
fn zeroed_row(row: &ClearanceBoxFact) -> ClearanceBoxFact {
    let absolute = |percentage: PercentageBoxFact| AbsoluteBoxFact {
        pixels: 0.0,
        rounded_to_zero: false,
        origin: percentage.origin,
    };
    let zero = |fact| match fact {
        BoxLengthFact::Percentage(percentage) => BoxLengthFact::Absolute(absolute(percentage)),
        other => other,
    };
    let zero_lp = |fact| match fact {
        LengthPercentageBoxFact::Percentage(percentage) => {
            LengthPercentageBoxFact::Absolute(absolute(percentage))
        }
        other => other,
    };
    let mut used = *row;
    used.width = zero(row.width);
    used.margin = row.margin.map(zero);
    used.offset = row.offset.map(|offset| offset.map(zero));
    used.padding = row.padding.map(zero_lp);
    used.min_width = zero_lp(row.min_width);
    if let BoxMaxFact::Percentage(percentage) = row.max_width {
        used.max_width = BoxMaxFact::Absolute(absolute(percentage));
    }
    used
}

/// PTA: a copy of `row` whose `top` and `bottom` percentages count as `auto` (origin
/// kept), for the placement loop's horizontal solve and (OAH) P's; the vertical axis is
/// solved after P's bottom.
fn vertical_percentages_as_auto(row: &ClearanceBoxFact) -> ClearanceBoxFact {
    let mut masked = *row;
    if let Some(offset) = masked.offset.as_mut() {
        for side in [0, 2] {
            if let BoxLengthFact::Percentage(percentage) = offset[side] {
                offset[side] = BoxLengthFact::Auto {
                    origin: percentage.origin,
                };
            }
        }
    }
    masked
}

/// PCT: the solver operand of a fact of a row resolved by `resolved_row`; a percentage
/// left there is an invariant breach.
fn operand(
    fact: BoxLengthFact,
    source: usize,
    property: &'static str,
) -> Result<AutoLength, SiblingLayoutError> {
    auto_length(fact).ok_or(SiblingLayoutError::FactInvariant { source, property })
}

fn fact_row<'a>(
    facts: &'a SiblingBoxFacts<'_, '_, '_, '_, '_, '_>,
    source: usize,
    property: &'static str,
) -> Result<&'a ClearanceBoxFact, SiblingLayoutError> {
    facts
        .row(source)
        .ok_or(SiblingLayoutError::FactInvariant { source, property })
}

fn used_child_height(
    child: ChildFacts<'_>,
    source: usize,
    floating: bool,
) -> Result<f32, SiblingLayoutError> {
    match child.height {
        OwnerHeight::Explicit(height) => Ok(height),
        OwnerHeight::NonAdjoiningZero(height) if !floating => Ok(height),
        OwnerHeight::Auto if floating => Ok(0.0_f32),
        _ => Err(SiblingLayoutError::FactInvariant {
            source,
            property: "height",
        }),
    }
}

/// A normal child's horizontal solve on its resolved row (PCT: `resolved_row`).
fn place_child_horizontal(
    parent: BlockHorizontalPlacement,
    direction: Direction,
    row: &ClearanceBoxFact,
    source: usize,
) -> Result<BlockHorizontalPlacement, SiblingLayoutError> {
    let border = row.border_widths();
    let padding = padding_operands(row, source)?;
    place_block_horizontal(
        parent.content.left,
        BlockWidthInput {
            containing_width: parent.used.width,
            direction,
            width: operand(row.width, source, "width")?,
            margin_left: operand(row.margin[3], source, "margin-left")?,
            margin_right: operand(row.margin[1], source, "margin-right")?,
            border_left: border[3],
            border_right: border[1],
            padding_left: padding[3],
            padding_right: padding[1],
            min_width: length_operand(row.min_width, source, "min-width")?,
            max_width: max_value(row.max_width, source, "max-width")?,
        },
    )
    .map_err(|error| SiblingLayoutError::ChildHorizontal { source, error })
}

/// Moves a published edge outward by a nonnegative width (a padding, then a border):
/// a zero width copies the edge; otherwise one f32 step that must stay finite, with
/// zero published as +0.
fn child_border_edge(
    edge: f32,
    width: f32,
    source: usize,
    stage: ChildBorderStage,
) -> Result<f32, SiblingLayoutError> {
    if width == 0.0 {
        return Ok(edge);
    }
    let value = match stage {
        ChildBorderStage::Left => edge - width,
        ChildBorderStage::Right => edge + width,
    };
    if !value.is_finite() {
        return Err(SiblingLayoutError::ChildBorderPublication { source, stage });
    }
    Ok(if value == 0.0 { 0.0 } else { value })
}

fn facade_work(nodes: usize) -> Option<usize> {
    let children = nodes.checked_sub(2)?;
    let normals = nodes.checked_sub(3)?;
    let child_actions = children.checked_mul(74)?;
    let category_actions = normals.checked_mul(66)?;
    nodes
        .checked_mul(32)?
        .checked_add(80)?
        .checked_add(child_actions)?
        .checked_add(category_actions)?
        .checked_add(normals.checked_mul(80)?)?
        .checked_add(children.checked_mul(16)?)?
        // CFH: four conversion/category/role actions and four used-height
        // category/role/publication actions per child, including explicit paths.
        .checked_add(children.checked_mul(8)?)?
        // SNV: prepaid route scan, normal-edge fold and aggregate publication.
        .checked_add(128)?
        .checked_add(children.checked_mul(20)?)?
        .checked_add(normals.checked_mul(128)?)?
        // TNB: eight per child for the in-loop new-BFC label.
        .checked_add(children.checked_mul(8)?)?
        // CBR: 10 fixed (the float padding band and span: two accessor returns, four
        // reads and four assembly writes), 8 per child (the padding box into `Principal`
        // and into `PrincipalChildBox`) and 39 per normal (two border reads, two staged
        // border-edge steps of 15, the padding-span accessor return and two reads, and
        // four assembly writes), charged on every path.
        .checked_add(10)?
        .checked_add(children.checked_mul(8)?)?
        .checked_add(normals.checked_mul(39)?)?
        // REL: 28 fixed (P's offset solve: position test 2, four offset reads 4, four
        // category conversions 4, direction read 1, primitive call and return 4, two
        // axis resolutions 8, four publication writes 4; and P's widened admission
        // comparison 1) and 38 per child (its offset solve 27, the containment shift:
        // two offset reads 2 and eight additions 8, and its widened admission
        // comparison 1), charged on every path.
        .checked_add(28)?
        .checked_add(children.checked_mul(38)?)?
        // MMX: 58 fixed and 22 per child, charged on every path. Per box (P and each
        // child) 22: four constraint reads 4, two max category conversions 2, the width
        // hand-off 10 (P and normals: two solver input transfers 2 and a possible
        // max-width rerun 8, a normal's possible min-width rerun being prepaid in its 66;
        // F: two clamp comparisons 4, the result transfer 1 and the category tests 5)
        // and the height constraint 6 (two comparisons 4, the Auto category test and
        // label 2). P also: its possible min-width rerun 8 (D-9; no existing unit covers
        // P) and its Auto height step 28 (primitive call and return 4, exact span 4, two
        // exact comparisons 4, a possible explicit rerun publication 16).
        .checked_add(58)?
        .checked_add(children.checked_mul(22)?)?
        // PCT: 72 fixed and 124 per child, charged on every path. A PCT resolution was 57:
        // seven longhands of 8 (category test 2, primitive call and return 4, result
        // write 1, origin copy 1) and the basis read 1. P: its resolution 57, the two
        // basis validity tests 4, the held-result test 2 and F's width resolved again 9
        // (one longhand and the basis read). Per child: its first resolution 57, the
        // held-result test 2, its second resolution 57 and four margin comparisons 8.
        .checked_add(72)?
        .checked_add(children.checked_mul(124)?)?
        // PPM: a resolution covers six more longhands of 8 (the four paddings,
        // `min-width` and `max-width`; 105 in all): 64 fixed (P's resolution 48 and F's
        // `min-width` and `max-width` resolved again 16) and 104 per child (its two
        // resolutions 96 and four padding comparisons 8), charged on every path.
        .checked_add(64)?
        .checked_add(children.checked_mul(104)?)?
        // PHT: 21 fixed and 27 per child, charged on every path. P: O's height read 1 and
        // its `auto` test 2, P's three category tests 6 and the refusal decision 2, the
        // three treated-as writes 3, and the children's basis (P's height category test 2,
        // the two `constrain` comparisons 4 and the basis write 1). Per child: its vertical
        // resolution, three longhands of 8 (category test 2, primitive call and return 4,
        // result write 1, origin copy 1) and the basis read 1, and the basis-presence
        // test 2.
        .checked_add(21)?
        .checked_add(children.checked_mul(27)?)?
        // PTB: 8 fixed and 42 per child, charged on every path. P: its offset presence
        // test 2, the two category tests 4 and the refusal decision 2. Per child: two
        // vertical offset resolutions of 21 (the offset presence test 2, two longhands of
        // 8 (category test 2, primitive call and return 4, result write 1, origin copy 1),
        // the basis read 1 and the basis-presence test 2).
        .checked_add(8)?
        .checked_add(children.checked_mul(42)?)?
        // PTA: 2 fixed and 35 per child, charged on every path. P: the basis read 1 and its
        // transfer 1. Per child: the mask 10 (the offset presence test 2 and two longhands
        // of 4: category test 2, result write 1, origin copy 1) and the vertical pass 25
        // besides its resolution (PTB's second, moved there): loop step 2, source read 1,
        // row read 1, the offset presence test 2, two offset reads 2, two category
        // conversions 2, direction read 1, primitive call and return 4, two axis
        // resolutions 8 and two publication writes 2.
        .checked_add(2)?
        .checked_add(children.checked_mul(35)?)?
        // DAZ (G6b): 58 per normal, charged on every path: the separation test (call and
        // return 4, the overflow read and comparison 3, the border accessor with two reads
        // and two comparisons 10, two vertical paddings of 13 at their percentage maximum:
        // read 1, category test 2, solve presence test 2, width read 1, primitive call and
        // return 4, result test 2, value read 1; two padding comparisons 4, the connectives
        // 6 and the read of P's solve 2) and the arm's branch and write 3.
        .checked_add(normals.checked_mul(58)?)?
        // OCH: 75 fixed, charged on every path (the old entry passes `None`; the entry
        // delegation is not charged, as no entry call was before). P: the input transfer
        // 1, O's explicit-height write 1, the basis selection 4 (input read 1,
        // explicit-height test 2, write 1), the explicit-O branch test 2, the `let`-`else`
        // basis presence test 2, the validity decision 7 (`is_finite` 2, `>= 0.0` 2, the
        // write 1 and the branch test 2), the height family against the basis 27 (the PHT
        // child unit: three longhands of 8, the basis read 1 and the basis-presence test
        // 2), the offsets against it 21 (the PTB unit), the copy of the resolved `top` and
        // `bottom` into P's used row 7 (presence test 2, two reads 2, two writes 2, the
        // offset write 1) and the vertical pass's basis selection 3 (read 1, presence test
        // 2).
        .checked_add(75)?
        // OAH: 79 fixed, charged on every path. P's step: O's two limit category tests 4
        // and the refusal decision 2. P's mask for its horizontal solve 10 (the PTA mask
        // unit). The pass: the auto-O and percentage-presence decision 4, O's tentative
        // height 4 (two reads 2, the subtraction 1, the write 1), the negative test and
        // refusal decision 4 (kept without credit since MFZ removed them), O's two limit
        // reads and conversions 4, the constraint 5 (two comparisons 4, the write 1), P's
        // resolution 21 (the PTB unit) and P's vertical axis 21 (the offset presence test
        // 2, two offset reads 2, two category conversions 2, direction read 1, primitive
        // call and return 4, two axis resolutions 8 and two publication writes 2).
        .checked_add(79)?
        // VPH (the user's U2=A): 45 fixed, charged on every path (the old entries pass
        // `None`; the entry delegation is not charged, OCH). P's step: the input transfer
        // 1, the `let`-`else` viewport presence test 2, the validity decision 7
        // (`is_finite` 2, `>= 0.0` 2, the write 1 and the branch test 2), O's limit name
        // selection 3 (category test 2, write 1), the outer-limits decision 5 (presence
        // test 2, explicit-O test 2, write 1) and O's limits against the basis 27 (the PHT
        // unit: three longhands of 8, the basis read 1 and the basis-presence test 2). The
        // OAH pass decision 4 and O's two limit reads 4 stay charged above, kept without
        // credit (the pass now tests the carried limits).
        .checked_add(45)?
        // CTB (the user's U1=A; CT-15): conservative prepaid policy charges, no credit.
        // Fixed 25: adjoining property checks 15, explicit check 8, refusal 2.
        // Per child 8: adjoining role/connective/write 4 and explicit
        // used-write/role/connective 4. Per normal 2: the Auto guard.
        // Property checks include category, zero and rounding tests and connectives.
        // Existing category/constrain and DAZ helper charges remain prepaid once.
        .checked_add(25)?
        .checked_add(nodes.checked_sub(2)?.checked_mul(8)?)?
        .checked_add(nodes.checked_sub(3)?.checked_mul(2)?)?
        // CTT: shared requested-byte helper57: entry/argument5, tail count4,
        // zero-tail guard4, member_slots16, two-vector byte helper26, mapping2.
        // The old record-only byte charge remains, without credit.
        .checked_add(57)?
        // NZS-D5: eight per child, six for the narrow category branch/write and
        // two for validated scalar publication, retaining all earlier charges.
        // NZS-FLIP-FACADE-WORK: the FLIP stage omits only this new charge.
        .checked_add(nodes.checked_sub(2)?.checked_mul(8)?)
}

fn checked_bytes<T>(count: usize) -> Option<usize> {
    count
        .checked_mul(size_of::<T>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
}

fn allocate_scratch<'p>(
    count: usize,
    limit: NonZeroUsize,
    reserve: impl FnOnce(&mut Vec<ChildFacts<'p>>, usize) -> Result<(), TryReserveError>,
) -> Result<(Vec<ChildFacts<'p>>, usize), SiblingLayoutError> {
    let requested =
        checked_bytes::<ChildFacts<'_>>(count).ok_or(SiblingLayoutError::ScratchBytes)?;
    if requested > limit.get() {
        return Err(SiblingLayoutError::ScratchBytes);
    }
    let mut scratch = Vec::new();
    reserve(&mut scratch, count).map_err(SiblingLayoutError::Allocation)?;
    let actual = checked_bytes::<ChildFacts<'_>>(scratch.capacity())
        .filter(|&bytes| scratch.capacity() >= count && bytes <= limit.get())
        .ok_or(SiblingLayoutError::ScratchBytes)?;
    Ok((scratch, actual))
}

fn allocate_children(
    count: usize,
    core_bytes: usize,
    limit: NonZeroUsize,
    reserve: impl FnOnce(&mut Vec<PrincipalChildBox>, usize) -> Result<(), TryReserveError>,
) -> Result<(Vec<PrincipalChildBox>, usize), SiblingLayoutError> {
    let requested = checked_bytes::<PrincipalChildBox>(count).ok_or(SiblingLayoutError::Bytes)?;
    core_bytes
        .checked_add(requested)
        .filter(|&bytes| bytes <= isize::MAX as usize && bytes <= limit.get())
        .ok_or(SiblingLayoutError::Bytes)?;
    let mut children = Vec::new();
    reserve(&mut children, count).map_err(SiblingLayoutError::Allocation)?;
    let actual = checked_bytes::<PrincipalChildBox>(children.capacity())
        .filter(|&bytes| {
            children.capacity() >= count
                && core_bytes
                    .checked_add(bytes)
                    .is_some_and(|sum| sum <= isize::MAX as usize && sum <= limit.get())
        })
        .ok_or(SiblingLayoutError::Bytes)?;
    Ok((children, actual))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn nz(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    #[test]
    fn scratch_and_child_capacity_surplus_are_charged() {
        let scratch_floor = 4 * size_of::<ChildFacts<'_>>();
        let error = allocate_scratch(4, nz(scratch_floor), |scratch, requested| {
            scratch.try_reserve_exact(requested + 1)
        })
        .unwrap_err();
        assert!(matches!(error, SiblingLayoutError::ScratchBytes));

        let child_floor = 4 * size_of::<PrincipalChildBox>();
        let error = allocate_children(4, 1, nz(child_floor + 1), |children, requested| {
            children.try_reserve_exact(requested + 1)
        })
        .unwrap_err();
        assert!(matches!(error, SiblingLayoutError::Bytes));
    }

    #[test]
    fn genuine_allocation_causes_are_retained() {
        let real = Vec::<u8>::new().try_reserve_exact(usize::MAX).unwrap_err();
        let error = allocate_scratch(4, NonZeroUsize::MAX, |scratch, _| {
            scratch.try_reserve_exact(usize::MAX)
        })
        .unwrap_err();
        assert!(matches!(error, SiblingLayoutError::Allocation(_)));
        assert!(error.source().unwrap().is::<TryReserveError>());
        assert_eq!(error.source().unwrap().to_string(), real.to_string());

        let error = allocate_children(4, 0, NonZeroUsize::MAX, |children, _| {
            children.try_reserve_exact(usize::MAX)
        })
        .unwrap_err();
        assert!(matches!(error, SiblingLayoutError::Allocation(_)));
        assert!(error.source().unwrap().is::<TryReserveError>());
        assert_eq!(error.source().unwrap().to_string(), real.to_string());
    }

    #[test]
    fn facade_work_overflow_is_rejected() {
        assert_eq!(facade_work(usize::MAX), None);
    }

    #[test]
    fn vertical_auto_facade_work_and_runtime_storage_boundaries() {
        assert_eq!(facade_work(5), Some(3284));
        assert_eq!(facade_work(6), Some(4231));
        assert_eq!(facade_work(7), Some(5178));
        assert_eq!(facade_work(2), None);

        // Checked byte products reject anything beyond isize::MAX at runtime size.
        let size = size_of::<PrincipalChildBox>();
        let largest = isize::MAX as usize / size;
        assert_eq!(
            checked_bytes::<PrincipalChildBox>(largest),
            Some(largest * size)
        );
        assert_eq!(checked_bytes::<PrincipalChildBox>(largest + 1), None);
        let scratch = size_of::<ChildFacts<'_>>();
        assert_eq!(
            checked_bytes::<ChildFacts<'_>>(isize::MAX as usize / scratch + 1),
            None
        );

        // Bind success to each prepared vector's measured capacity before moving it.
        let mut prepared = Vec::<PrincipalChildBox>::new();
        prepared.try_reserve_exact(4).unwrap();
        let capacity = prepared.capacity();
        let (children, bytes) =
            allocate_children(4, 7, nz(7 + capacity * size), move |children, count| {
                assert_eq!(count, 4);
                *children = prepared;
                Ok(())
            })
            .unwrap();
        assert_eq!(children.capacity(), capacity);
        assert_eq!(bytes, capacity * size);
        let mut prepared = Vec::<ChildFacts<'_>>::new();
        prepared.try_reserve_exact(4).unwrap();
        let capacity = prepared.capacity();
        let (records, bytes) =
            allocate_scratch(4, nz(capacity * scratch), move |records, count| {
                assert_eq!(count, 4);
                *records = prepared;
                Ok(())
            })
            .unwrap();
        assert_eq!(records.capacity(), capacity);
        assert_eq!(bytes, capacity * scratch);

        // Core bytes plus requested children beyond isize::MAX or the limit fail
        // before any reserve; the combined surplus includes core bytes.
        let error = allocate_children(4, isize::MAX as usize, NonZeroUsize::MAX, |_, _| {
            panic!("reserve reached beyond isize::MAX")
        })
        .unwrap_err();
        assert!(matches!(error, SiblingLayoutError::Bytes));
        let error = allocate_children(4, 1, nz(4 * size), |_, _| {
            panic!("reserve reached beyond the combined limit")
        })
        .unwrap_err();
        assert!(matches!(error, SiblingLayoutError::Bytes));
        let error = allocate_scratch(4, nz(4 * scratch - 1), |_, _| {
            panic!("reserve reached below the scratch floor")
        })
        .unwrap_err();
        assert!(matches!(error, SiblingLayoutError::ScratchBytes));

        // Real TryReserveError from each seam keeps its source.
        let real = Vec::<u8>::new().try_reserve_exact(usize::MAX).unwrap_err();
        let error = allocate_children(4, 7, nz(7 + 4 * size), |children, _| {
            children.try_reserve_exact(usize::MAX)
        })
        .unwrap_err();
        assert!(error.source().unwrap().is::<TryReserveError>());
        assert_eq!(error.source().unwrap().to_string(), real.to_string());
    }
    #[test]
    fn cp_task1_actual_representation_measurement() {
        println!(
            "CP_REPRESENTATION PrincipalChildBox {} {}",
            size_of::<PrincipalChildBox>(),
            std::mem::align_of::<PrincipalChildBox>()
        );
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
        assert_eq!(
            (
                size_of::<PrincipalChildBox>(),
                std::mem::align_of::<PrincipalChildBox>()
            ),
            (392, 8)
        );
    }

    // Initialized test values allow boxed-slice conversion to make exact capacities
    // 3/4/5 without assuming any allocator's reserve_exact behavior.
    fn cp_capacity_child(source: &SourceNode) -> ChildFacts<'_> {
        use iced_css_core::formatting::clearance_owner::OwnerEdges;
        let zero = OwnerEdges {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        };
        ChildFacts {
            source,
            display: DisplayResolution {
                display: Display::Block,
                position: Position::Static,
                float: Float::None,
                flow: Flow::Normal,
                flex: false,
                grid: false,
                sticky: false,
            },
            context: ChildContext::OrdinaryNonBfc,
            content: OwnerContent::NoOwnContentNonReplaced,
            generated: OwnerGenerated::None,
            height: OwnerHeight::Explicit(1.0),
            margin: zero.into(),
            border: zero,
            padding: OwnerEdges { top: 2.0, ..zero },
            clear: ClearSide::None,
        }
    }
    fn cp_capacity_principal() -> PrincipalChildBox {
        use super::super::clearance_box_facts::{AbsoluteBoxFact, BoxFactOrigin};
        let length = BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels: 0.0,
            rounded_to_zero: false,
            origin: BoxFactOrigin::Specified,
        });
        let edges = PrincipalEdges {
            left: 0.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0,
        };
        PrincipalChildBox {
            source_index: 2,
            containing_source_index: 1,
            kind: PrincipalChildKind::Normal,
            content: edges,
            padding: edges,
            border: edges,
            used_width: 0.0,
            normal_width: None,
            used_margin_left: 0.0,
            used_margin_right: 0.0,
            used_height: 0.0,
            used_margin_top: 0.0,
            used_margin_bottom: 0.0,
            computed_width: length,
            computed_height: length,
            computed_margin_left: length,
            computed_margin_right: length,
            margin_top: length,
            margin_bottom: length,
            clear: KeywordBoxFact {
                value: ClearSide::None,
                origin: BoxFactOrigin::Specified,
            },
            relative: NO_OFFSETS,
        }
    }

    #[test]
    fn cp_injected_scratch_and_child_capacity_boundaries() {
        use iced_css_core::{identity::IdAllocator, source::Metadata};
        let mut ids = IdAllocator::new(std::num::NonZeroU64::new(712).unwrap());
        let source = SourceNode {
            instance: ids.allocate().unwrap(),
            parent: None,
            children: vec![],
            metadata: Metadata::default(),
        };
        for nodes in 5..=7 {
            let count = nodes - 2;
            let core_bytes = sequence_requested_retained_bytes(nodes).unwrap();
            let scratch_floor = count * size_of::<ChildFacts<'_>>();
            let child_floor = count * size_of::<PrincipalChildBox>();
            assert!(matches!(
                allocate_scratch(count, NonZeroUsize::MAX, |_, _| Ok(())),
                Err(SiblingLayoutError::ScratchBytes)
            ));
            assert!(matches!(
                allocate_children(count, core_bytes, NonZeroUsize::MAX, |_, _| Ok(())),
                Err(SiblingLayoutError::Bytes)
            ));
            assert!(matches!(
                allocate_scratch(count, nz(scratch_floor - 1), |_, _| panic!("scratch floor")),
                Err(SiblingLayoutError::ScratchBytes)
            ));
            assert!(matches!(
                allocate_children(
                    count,
                    core_bytes,
                    nz(core_bytes + child_floor - 1),
                    |_, _| panic!("combined floor")
                ),
                Err(SiblingLayoutError::Bytes)
            ));
            // Synthetic oversized operands cover checked arithmetic without allocations.
            assert_eq!(facade_work(usize::MAX - nodes), None);
            assert!(matches!(
                allocate_scratch(usize::MAX - count, NonZeroUsize::MAX, |_, _| panic!(
                    "scratch integer guard"
                )),
                Err(SiblingLayoutError::ScratchBytes)
            ));
            assert!(matches!(
                allocate_children(
                    usize::MAX - count,
                    core_bytes,
                    NonZeroUsize::MAX,
                    |_, _| panic!("child integer guard")
                ),
                Err(SiblingLayoutError::Bytes)
            ));
            for huge_core in [isize::MAX as usize, usize::MAX - child_floor + 1] {
                assert!(matches!(
                    allocate_children(count, huge_core, NonZeroUsize::MAX, |_, _| panic!(
                        "combined integer guard"
                    )),
                    Err(SiblingLayoutError::Bytes)
                ));
            }
            let real = Vec::<u8>::new().try_reserve_exact(usize::MAX).unwrap_err();
            for error in [
                allocate_scratch(count, NonZeroUsize::MAX, |v, requested| {
                    assert_eq!(requested, count);
                    v.try_reserve_exact(usize::MAX)
                })
                .unwrap_err(),
                allocate_children(count, core_bytes, NonZeroUsize::MAX, |v, requested| {
                    assert_eq!(requested, count);
                    v.try_reserve_exact(usize::MAX)
                })
                .unwrap_err(),
            ] {
                assert!(matches!(error, SiblingLayoutError::Allocation(_)));
                assert!(error.source().unwrap().is::<TryReserveError>());
                assert_eq!(error.source().unwrap().to_string(), real.to_string());
            }
            for offset in [0usize, 1] {
                let mut exact = (0..count)
                    .map(|_| cp_capacity_child(&source))
                    .collect::<Box<[_]>>()
                    .into_vec();
                assert_eq!(exact.capacity(), count);
                exact.clear();
                let (v, bytes) =
                    allocate_scratch(count, nz(scratch_floor + offset), move |v, requested| {
                        assert_eq!(requested, count);
                        *v = exact;
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(v.capacity(), count);
                assert_eq!(bytes, scratch_floor);
                let mut exact = (0..count)
                    .map(|_| cp_capacity_principal())
                    .collect::<Box<[_]>>()
                    .into_vec();
                assert_eq!(exact.capacity(), count);
                exact.clear();
                let (v, bytes) = allocate_children(
                    count,
                    core_bytes,
                    nz(core_bytes + child_floor + offset),
                    move |v, requested| {
                        assert_eq!(requested, count);
                        *v = exact;
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(v.capacity(), count);
                assert_eq!(bytes, child_floor);
            }
            // Bind every boundary to its own prepared allocation, passed unchanged.
            for offset in [-1isize, 0, 1] {
                let mut prepared = Vec::<ChildFacts<'_>>::new();
                prepared.try_reserve_exact(count + 1).unwrap();
                let capacity = prepared.capacity();
                assert!(capacity > count);
                let wanted = capacity * size_of::<ChildFacts<'_>>();
                let mut called = false;
                let result = allocate_scratch(
                    count,
                    nz(wanted.checked_add_signed(offset).unwrap()),
                    |v, requested| {
                        called = true;
                        assert_eq!(requested, count);
                        *v = prepared;
                        Ok(())
                    },
                );
                assert!(called);
                if offset < 0 {
                    assert!(matches!(result, Err(SiblingLayoutError::ScratchBytes)));
                } else {
                    let (v, bytes) = result.unwrap();
                    assert_eq!(v.capacity(), capacity);
                    assert_eq!(bytes, wanted);
                }
                let mut prepared = Vec::<PrincipalChildBox>::new();
                prepared.try_reserve_exact(count + 1).unwrap();
                let capacity = prepared.capacity();
                assert!(capacity > count);
                let wanted = capacity * size_of::<PrincipalChildBox>();
                let mut called = false;
                let result = allocate_children(
                    count,
                    core_bytes,
                    nz((core_bytes + wanted).checked_add_signed(offset).unwrap()),
                    |v, requested| {
                        called = true;
                        assert_eq!(requested, count);
                        *v = prepared;
                        Ok(())
                    },
                );
                assert!(called);
                if offset < 0 {
                    assert!(matches!(result, Err(SiblingLayoutError::Bytes)));
                } else {
                    let (v, bytes) = result.unwrap();
                    assert_eq!(v.capacity(), capacity);
                    assert_eq!(bytes, wanted);
                }
            }
        }
    }
    #[test]
    fn nzs_nonadjoining_zero_used_height_preserves_both_signs() {
        use iced_css_core::{identity::IdAllocator, source::Metadata};
        let mut ids = IdAllocator::new(std::num::NonZeroU64::new(713).unwrap());
        let source = SourceNode {
            instance: ids.allocate().unwrap(),
            parent: None,
            children: vec![],
            metadata: Metadata::default(),
        };
        for index in [2, 4] {
            for zero in [0.0_f32, -0.0_f32] {
                let mut child = cp_capacity_child(&source);
                child.height = OwnerHeight::NonAdjoiningZero(zero);
                assert_eq!(
                    used_child_height(child, index, false).unwrap().to_bits(),
                    zero.to_bits()
                );
                assert!(matches!(
                    used_child_height(child, 3, true),
                    Err(SiblingLayoutError::FactInvariant {
                        source: 3,
                        property: "height"
                    })
                ));
            }
        }
    }
}
