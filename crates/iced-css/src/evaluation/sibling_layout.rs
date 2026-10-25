//! Bounded actual-CSS composition for one O/P/[B1,F,tail...] sibling capture.
//!
//! The returned geometry owns only copied observations and the core sequence owner;
//! it borrows the dynamic projection's source storage rather than computed rows or
//! temporary child-fact scratch.

use super::{
    clearance_box_facts::{BoxLengthFact, BoxOverflow, ClearanceBoxFact, KeywordBoxFact},
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
            SignedNormalMarginInput, publish_signed_normal_margin_edges, solve_bfc_height,
            solve_signed_bfc_height,
        },
        block::{
            BlockHorizontalError, BlockHorizontalPlacement, BlockWidthInput, BlockWidthResult,
            Direction, place_block_horizontal,
        },
        block_bottom::{
            BlockBottomError, BlockBottomInput, BlockBottomPlacement, place_block_bottom,
        },
        clearance::{
            ClearSide, ClearanceFloatError, ClearancePresenceError, ClearanceRequirement,
            classify_clearance, lowest_clearance_float_bottom,
        },
        clearance_owner::{
            ChildContext, ChildFacts, OuterFacts, OwnerContent, OwnerGenerated, OwnerHeight,
            ParentBfc, ParentFacts,
        },
        clearance_sequence::{
            ClearanceSequenceError, ClearanceSequenceInput, ClearanceSequenceLimits,
            ClearanceSequenceOwner, ClearanceSequenceReceipt, SequenceNormalPlacement,
            build_clearance_sequence,
        },
        display::{Display, DisplayResolution, Float, Flow, Position},
        first_child_origin::{
            FirstChildOriginError, FirstChildOriginInput, FirstChildOriginPlacement,
            place_first_child_origin,
        },
    },
    source::SourceNode,
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
    pub fn padding(&self) -> PrincipalEdges {
        self.border
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
    /// A finite content edge plus its padding cannot publish a finite border edge.
    ChildBorderPublication {
        source: usize,
        stage: ChildBorderStage,
    },
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
/// Facade work `208 + 32*N + 126*(N-2) + 274*(N-3)` is prepaid before any
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
/// read and comparison, and the context write.
/// The core is invoked exactly once, with computed Auto preserved and
/// bounded vertical and float Auto margins given used value zero. Every child also
/// admits computed `overflow: hidden` or `auto`. Each such child then establishes a block
/// formatting context and has no content, so nothing is clipped, nothing overflows
/// and `auto` presents no scrollbar. The facade labels each such normal child, B1 or a
/// tail, `ChildContext::OtherNewBfc`; the core permits border/margin boundary contact
/// and rejects strict intersection of nonempty intervals at that child's source
/// (CSS2.2 §9.5, Q1). P admits `hidden`, and `auto`
/// only when every child's border box and margin box lie within P's published
/// content box (CSS2.2 §11.1), so that nothing overflows P and `auto` presents no
/// scrollbar; otherwise the build returns `ParentAutoOverflow`. No clip or scroll
/// state is published, and `scroll` stays unsupported on every child. The float's content box is
/// the core's published content band horizontally and its published vertical
/// content span (`sequence().float_content_vertical()`) vertically; its margin box
/// stays `sequence().placed_float()`.
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
    build_with_reserve(
        facts,
        outer,
        limits,
        |scratch, requested| scratch.try_reserve_exact(requested),
        |children, requested| children.try_reserve_exact(requested),
    )
}

fn build_with_reserve<'p, 'h, 'd, 's, 'b, 'r>(
    facts: &SiblingBoxFacts<'p, 'h, 'd, 's, 'b, 'r>,
    outer: ClearanceOuterContentGeometry<'p>,
    limits: SiblingLayoutLimits,
    reserve_scratch: impl FnOnce(&mut Vec<ChildFacts<'p>>, usize) -> Result<(), TryReserveError>,
    reserve_children: impl FnOnce(&mut Vec<PrincipalChildBox>, usize) -> Result<(), TryReserveError>,
) -> Result<CapturedSiblingLayout<'p>, SiblingLayoutError> {
    use SiblingLayoutError as E;

    // NonZeroUsize supplies the initial preflight unit. Nothing proportional has
    // been inspected at this point.
    let projection = facts.projection();
    let nodes = projection.nodes();
    let node_count = nodes.len();
    if node_count < 5 || node_count > limits.sources.get() {
        return Err(E::Sources);
    }
    if !same_source(outer.outer, &nodes[0]) {
        return Err(E::Captured(CapturedClearanceError::GeometrySource));
    }
    let work = facade_work(node_count).ok_or(E::Work)?;
    if work > limits.work.get() {
        return Err(E::Work);
    }

    let child_count = node_count - 2;
    let tail_count = node_count - 4;
    let requested_scratch = checked_bytes::<ChildFacts<'_>>(child_count).ok_or(E::ScratchBytes)?;
    if requested_scratch > limits.scratch_bytes.get() {
        return Err(E::ScratchBytes);
    }
    let requested_core = checked_bytes::<SequenceNormalPlacement>(tail_count).ok_or(E::Bytes)?;
    let requested_children = checked_bytes::<PrincipalChildBox>(child_count).ok_or(E::Bytes)?;
    requested_core
        .checked_add(requested_children)
        .filter(|&bytes| bytes <= isize::MAX as usize && bytes <= limits.retained_bytes.get())
        .ok_or(E::Bytes)?;

    let (mut child_facts, scratch_bytes) =
        allocate_scratch(child_count, limits.scratch_bytes, reserve_scratch)?;

    // Prove the complete actual row/display/token mapping before numerical work.
    for source in 0..node_count {
        fact_row(facts, source, "row")?;
        joined_display(projection, facts.display(), source).map_err(E::Captured)?;
    }
    if facts.row(node_count).is_some() {
        return Err(E::FactInvariant {
            source: node_count,
            property: "row-count",
        });
    }

    for source in 0..node_count {
        let display = joined_display(projection, facts.display(), source).map_err(E::Captured)?;
        if !admitted_display(display, source == 3) {
            return Err(E::Captured(unsupported(
                source,
                "display/position/float",
                CapturedClearanceUnsupported::DisplayOrFlow,
            )));
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
        )));
    }
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
                )));
            }
        }
        let height = match row.height {
            BoxLengthFact::Absolute(value) => OwnerHeight::Explicit(value.pixels),
            BoxLengthFact::Auto { .. } if source == 3 => OwnerHeight::Auto,
            BoxLengthFact::Auto { .. } => {
                return Err(E::Captured(unsupported(
                    source,
                    "height",
                    CapturedClearanceUnsupported::HeightKind,
                )));
            }
        };
        let display = joined_display(projection, facts.display(), source).map_err(E::Captured)?;
        let mut child = child_facts_from_row(
            nodes,
            display,
            row,
            source,
            height,
            ChildMarginMode::Sequence,
        )
        .map_err(E::Captured)?;
        // CSS2.2 §9.4.1: a normal child (B1 or a tail) admitted above with `hidden` or
        // `auto` establishes a new block formatting context. The shared conversion labels
        // normals by position, so the Sequence facade, which alone admits such normals,
        // sets their context here.
        if source != 3 && row.overflow.value != BoxOverflow::Visible {
            child.context = ChildContext::OtherNewBfc;
        }
        child_facts.push(child);
    }
    let float_width = match fact_row(facts, 3, "width")?.width {
        BoxLengthFact::Absolute(value) => value.pixels,
        BoxLengthFact::Auto { .. } => {
            return Err(E::Captured(unsupported(
                3,
                "width",
                CapturedClearanceUnsupported::FloatWidth,
            )));
        }
    };

    // Direction joins are deliberately ordered O then P. O governs P's width
    // equation; P governs every normal child's width equation.
    let outer_direction = joined_direction(projection, facts.display(), 0).map_err(E::Captured)?;
    let parent_direction = joined_direction(projection, facts.display(), 1).map_err(E::Captured)?;
    let parent_border = edges(parent_row.border_widths());
    let parent_padding = edges(parent_row.padding.map(|value| value.pixels));
    let horizontal = place_block_horizontal(
        outer.content_left,
        BlockWidthInput {
            containing_width: outer.content_width,
            direction: outer_direction,
            width: auto_length(parent_row.width),
            margin_left: auto_length(parent_row.margin[3]),
            margin_right: auto_length(parent_row.margin[1]),
            border_left: parent_border.left,
            border_right: parent_border.right,
            padding_left: parent_padding.left,
            padding_right: parent_padding.right,
            min_width: 0.0,
            max_width: None,
        },
    )
    .map_err(E::ParentHorizontal)?;
    let origin = place_first_child_origin(FirstChildOriginInput {
        containing_top: outer.content_top,
        margin_top: auto_length(parent_row.margin[0]),
        border_top: parent_border.top,
        padding_top: parent_padding.top,
    })
    .map_err(E::ParentOrigin)?;
    let selected = lowest_clearance_float_bottom(&[], parent_row.clear.value, NonZeroUsize::MIN)
        .map_err(E::ParentFloatSelection)?;
    let parent_clearance =
        classify_clearance(origin.border_top, selected).map_err(E::ParentPresence)?;
    if parent_clearance != ClearanceRequirement::Absent {
        return Err(E::UnexpectedParentClearance);
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
    let sequence = build_clearance_sequence(
        nodes,
        ClearanceSequenceInput {
            outer: OuterFacts {
                source: &nodes[0],
                display: outer_display,
                content: OwnerContent::NoOwnContentNonReplaced,
                generated: OwnerGenerated::None,
            },
            parent: ParentFacts {
                source: &nodes[1],
                display: parent_display,
                content: OwnerContent::NoOwnContentNonReplaced,
                generated: OwnerGenerated::None,
                bfc: ParentBfc::NewWithEmptyPrefix,
                content_top: origin.content_top,
                containing: horizontal.content,
                border: parent_border,
                padding: parent_padding,
            },
            children: &child_facts,
            float_width,
        },
        core_limits,
    )
    .map_err(E::Core)?;

    sequence
        .receipt()
        .retained_bytes
        .checked_add(requested_children)
        .filter(|&bytes| bytes <= limits.retained_bytes.get())
        .ok_or(E::Bytes)?;
    let (mut children, child_bytes) = allocate_children(
        child_count,
        sequence.receipt().retained_bytes,
        limits.retained_bytes,
        reserve_children,
    )?;

    for source in 2..node_count {
        let row = fact_row(facts, source, "row")?;
        let child = child_facts
            .get(source - 2)
            .copied()
            .ok_or(E::FactInvariant {
                source,
                property: "child-facts",
            })?;
        let used_height = used_child_height(child, source)?;
        let principal = if source == 3 {
            let band = sequence.float_content();
            let span = sequence.float_content_vertical();
            let border_band = sequence.float_border();
            let border_span = sequence.float_border_vertical();
            Principal {
                kind: PrincipalChildKind::Float,
                content: PrincipalEdges {
                    left: band.left,
                    right: band.right,
                    top: span.top,
                    bottom: span.bottom,
                },
                border: PrincipalEdges {
                    left: border_band.left,
                    right: border_band.right,
                    top: border_span.top,
                    bottom: border_span.bottom,
                },
                used_width: float_width,
                normal_width: None,
                used_margin_left: child.margin.float_used_left(),
                used_margin_right: child.margin.float_used_right(),
            }
        } else {
            let (vertical, content_vertical) = if source == 2 {
                (sequence.before(), sequence.before_content())
            } else {
                let record = sequence.tail().get(source - 4).ok_or(E::FactInvariant {
                    source,
                    property: "sequence-tail",
                })?;
                if record.source_index() != source {
                    return Err(E::FactInvariant {
                        source,
                        property: "sequence-source",
                    });
                }
                (record.vertical(), record.content_vertical())
            };
            let placed = place_child_horizontal(horizontal, parent_direction, row)
                .map_err(|error| E::ChildHorizontal { source, error })?;
            let left = child_border_edge(
                placed.content.left,
                child.padding.left,
                source,
                ChildBorderStage::Left,
            )?;
            let right = child_border_edge(
                placed.content.right,
                child.padding.right,
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
        });
    }

    let computed_height = parent_row.height;
    let height = if matches!(computed_height, BoxLengthFact::Auto { .. })
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
            float_bottom: sequence.placed_float().bottom,
        })
        .map_err(E::SignedHeight)?
    } else {
        let height_mode = match computed_height {
            BoxLengthFact::Absolute(value) => BfcHeightMode::Explicit(value.pixels),
            BoxLengthFact::Auto { .. } => {
                let last_source = node_count - 1;
                let last_child = children.last().ok_or(E::FactInvariant {
                    source: last_source,
                    property: "principal-last",
                })?;
                if last_child.source_index() != last_source {
                    return Err(E::FactInvariant {
                        source: last_source,
                        property: "principal-source",
                    });
                }
                let last_in_flow_margin_bottom = last_child.used_margin_bottom();
                let last = sequence.tail().last().ok_or(E::FactInvariant {
                    source: last_source,
                    property: "sequence-tail-last",
                })?;
                if last.source_index() != last_source {
                    return Err(E::FactInvariant {
                        source: last_source,
                        property: "sequence-source",
                    });
                }
                BfcHeightMode::Auto {
                    last_in_flow_border_bottom: last.vertical().bottom,
                    last_in_flow_margin_bottom,
                    float_bottom: sequence.placed_float().bottom,
                }
            }
        };
        solve_bfc_height(BfcHeightInput {
            content_top: origin.content_top,
            mode: height_mode,
        })
        .map_err(E::Height)?
    };
    let bottom = place_block_bottom(BlockBottomInput {
        content_bottom: height.content_bottom,
        padding_bottom: parent_row.padding[2].pixels,
        border_bottom: parent_row.border_widths()[2],
        margin_bottom: auto_length(parent_row.margin[2]),
    })
    .map_err(E::Bottom)?;

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
        let placed = sequence.placed_float();
        for child in &children {
            let border = child.border();
            let margin = if child.kind() == PrincipalChildKind::Float {
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
            if !within(border, area) || !within(margin, area) {
                return Err(E::ParentAutoOverflow {
                    source: child.source_index(),
                });
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
    };
    let core_receipt = sequence.receipt();
    let retained_bytes = core_receipt
        .retained_bytes
        .checked_add(child_bytes)
        .ok_or(E::Bytes)?;
    let receipt = SiblingLayoutReceipt {
        nodes: node_count,
        work,
        child_capacity: children.capacity(),
        retained_bytes,
        scratch_bytes,
        core: core_receipt,
    };
    drop(child_facts);
    Ok(CapturedSiblingLayout {
        sequence,
        parent,
        children,
        receipt,
    })
}

// Named fields instead of a positional tuple (NFH-T2Q-002).
struct Principal {
    kind: PrincipalChildKind,
    content: PrincipalEdges,
    border: PrincipalEdges,
    used_width: f32,
    normal_width: Option<BlockWidthResult>,
    used_margin_left: f32,
    used_margin_right: f32,
}

fn admitted_display(display: DisplayResolution, floating: bool) -> bool {
    display.display == Display::Block
        && display.position == Position::Static
        && if floating {
            display.flow == Flow::Floating && matches!(display.float, Float::Left | Float::Right)
        } else {
            display.flow == Flow::Normal && display.float == Float::None
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

fn fact_row<'a>(
    facts: &'a SiblingBoxFacts<'_, '_, '_, '_, '_, '_>,
    source: usize,
    property: &'static str,
) -> Result<&'a ClearanceBoxFact, SiblingLayoutError> {
    facts
        .row(source)
        .ok_or(SiblingLayoutError::FactInvariant { source, property })
}

fn used_child_height(child: ChildFacts<'_>, source: usize) -> Result<f32, SiblingLayoutError> {
    match child.height {
        OwnerHeight::Explicit(height) => Ok(height),
        OwnerHeight::Auto if source == 3 => Ok(0.0_f32),
        _ => Err(SiblingLayoutError::FactInvariant {
            source,
            property: "height",
        }),
    }
}

fn place_child_horizontal(
    parent: BlockHorizontalPlacement,
    direction: Direction,
    row: &ClearanceBoxFact,
) -> Result<BlockHorizontalPlacement, BlockHorizontalError> {
    let border = row.border_widths();
    place_block_horizontal(
        parent.content.left,
        BlockWidthInput {
            containing_width: parent.used.width,
            direction,
            width: auto_length(row.width),
            margin_left: auto_length(row.margin[3]),
            margin_right: auto_length(row.margin[1]),
            border_left: border[3],
            border_right: border[1],
            padding_left: row.padding[3].pixels,
            padding_right: row.padding[1].pixels,
            min_width: 0.0,
            max_width: None,
        },
    )
}

fn child_border_edge(
    content: f32,
    padding: f32,
    source: usize,
    stage: ChildBorderStage,
) -> Result<f32, SiblingLayoutError> {
    if padding == 0.0 {
        return Ok(content);
    }
    let value = match stage {
        ChildBorderStage::Left => content - padding,
        ChildBorderStage::Right => content + padding,
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
        .checked_add(children.checked_mul(8)?)
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
        assert_eq!(facade_work(5), Some(1294));
        assert_eq!(facade_work(6), Some(1726));
        assert_eq!(facade_work(7), Some(2158));
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
            (264, 8)
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
            let core_bytes = (nodes - 4) * size_of::<SequenceNormalPlacement>();
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
}
