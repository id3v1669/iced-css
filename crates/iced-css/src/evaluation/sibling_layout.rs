//! Bounded actual-CSS composition for one O/P/[B1,F,tail...] sibling capture.
//!
//! The returned geometry owns only copied observations and the core sequence owner;
//! it borrows the dynamic projection's source storage rather than computed rows or
//! temporary child-fact scratch.

use super::{
    clearance_box_facts::{BoxLengthFact, BoxOverflow, ClearanceBoxFact, KeywordBoxFact},
    clearance_bridge::{
        CapturedClearanceError, CapturedClearanceUnsupported, ClearanceOuterContentGeometry,
        auto_length, child_facts_from_row, edges, joined_direction, joined_display, same_source,
        unsupported,
    },
    sibling_box_facts::SiblingBoxFacts,
};
use iced_css_core::{
    formatting::{
        bfc_height::{
            BfcHeightError, BfcHeightInput, BfcHeightMode, BfcHeightPlacement, solve_bfc_height,
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
            ChildFacts, OuterFacts, OwnerContent, OwnerGenerated, OwnerHeight, ParentBfc,
            ParentFacts,
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
    used_width: f32,
    normal_width: Option<BlockWidthResult>,
    used_height: f32,
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
        self.content
    }
    pub fn border(&self) -> PrincipalEdges {
        self.content
    }
    pub fn used_width(&self) -> f32 {
        self.used_width
    }
    pub fn normal_width(&self) -> Option<BlockWidthResult> {
        self.normal_width
    }
    pub fn used_height(&self) -> f32 {
        self.used_height
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
    Height(BfcHeightError),
    Bottom(BlockBottomError),
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
            Self::Bottom(error) => Some(error),
            Self::Sources
            | Self::Work
            | Self::Bytes
            | Self::ScratchBytes
            | Self::UnexpectedParentClearance
            | Self::FactInvariant { .. } => None,
        }
    }
}

/// Build source-linked P and principal-child geometry from a complete computed batch.
///
/// Facade work `32 + 32*N + 3*(N-2) + 32*(N-3)` is prepaid before any
/// proportional join, profile scan, or allocation. Three constant-size actions
/// per child retain the existing horizontal result and two computed margin facts;
/// 32 actions per normal cover its possible minimum-width category rerun. Each
/// normal still has one horizontal producer invocation. The core is invoked
/// exactly once. Temporary child facts are released before the source-only output
/// is published.
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
    if parent_row.overflow.value != BoxOverflow::Hidden {
        return Err(E::Captured(unsupported(
            1,
            "overflow",
            CapturedClearanceUnsupported::ParentOverflow,
        )));
    }
    for source in 2..node_count {
        let row = fact_row(facts, source, "row")?;
        if row.overflow.value != BoxOverflow::Visible {
            return Err(E::Captured(unsupported(
                source,
                "overflow",
                CapturedClearanceUnsupported::ChildOverflow,
            )));
        }
        let height = match row.height {
            BoxLengthFact::Absolute(value) => OwnerHeight::Explicit(value.pixels),
            BoxLengthFact::Auto { .. } => {
                return Err(E::Captured(unsupported(
                    source,
                    "height",
                    CapturedClearanceUnsupported::HeightKind,
                )));
            }
        };
        let display = joined_display(projection, facts.display(), source).map_err(E::Captured)?;
        child_facts
            .push(child_facts_from_row(nodes, display, row, source, height).map_err(E::Captured)?);
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
        let used_height = explicit_height(child, source)?;
        let (kind, content, used_width, normal_width) = if source == 3 {
            let placed = sequence.placed_float();
            (
                PrincipalChildKind::Float,
                PrincipalEdges {
                    left: placed.left,
                    right: placed.right,
                    top: placed.top,
                    bottom: placed.bottom,
                },
                float_width,
                None,
            )
        } else {
            let vertical = if source == 2 {
                sequence.before()
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
                record.vertical()
            };
            let placed = place_child_horizontal(horizontal, parent_direction, row)
                .map_err(|error| E::ChildHorizontal { source, error })?;
            (
                PrincipalChildKind::Normal,
                PrincipalEdges {
                    left: placed.content.left,
                    right: placed.content.right,
                    top: vertical.top,
                    bottom: vertical.bottom,
                },
                placed.used.width,
                Some(placed.used),
            )
        };
        children.push(PrincipalChildBox {
            source_index: source,
            containing_source_index: 1,
            kind,
            content,
            used_width,
            normal_width,
            used_height,
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
    let height_mode = match computed_height {
        BoxLengthFact::Absolute(value) => BfcHeightMode::Explicit(value.pixels),
        BoxLengthFact::Auto { .. } => {
            let last_source = node_count - 1;
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
            let last_row = fact_row(facts, last_source, "margin-bottom")?;
            let BoxLengthFact::Absolute(margin) = last_row.margin[2] else {
                return Err(E::FactInvariant {
                    source: last_source,
                    property: "margin-bottom",
                });
            };
            BfcHeightMode::Auto {
                last_in_flow_border_bottom: last.vertical().bottom,
                last_in_flow_margin_bottom: margin.pixels,
                float_bottom: sequence.placed_float().bottom,
            }
        }
    };
    let height = solve_bfc_height(BfcHeightInput {
        content_top: origin.content_top,
        mode: height_mode,
    })
    .map_err(E::Height)?;
    let bottom = place_block_bottom(BlockBottomInput {
        content_bottom: height.content_bottom,
        padding_bottom: parent_row.padding[2].pixels,
        border_bottom: parent_row.border_widths()[2],
        margin_bottom: auto_length(parent_row.margin[2]),
    })
    .map_err(E::Bottom)?;

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

fn admitted_display(display: DisplayResolution, floating: bool) -> bool {
    display.display == Display::Block
        && display.position == Position::Static
        && if floating {
            display.flow == Flow::Floating && matches!(display.float, Float::Left | Float::Right)
        } else {
            display.flow == Flow::Normal && display.float == Float::None
        }
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

fn explicit_height(child: ChildFacts<'_>, source: usize) -> Result<f32, SiblingLayoutError> {
    match child.height {
        OwnerHeight::Explicit(height) => Ok(height),
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

fn facade_work(nodes: usize) -> Option<usize> {
    let children = nodes.checked_sub(2)?;
    let normals = nodes.checked_sub(3)?;
    let retained_actions = children.checked_mul(3)?;
    let extra_category_actions = normals.checked_mul(32)?;
    nodes
        .checked_mul(32)?
        .checked_add(32)?
        .checked_add(retained_actions)?
        .checked_add(extra_category_actions)
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
}
