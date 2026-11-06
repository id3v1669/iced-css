//! Whole-tree block layout from the real cascade: projects any captured source tree
//! to preorder, evaluates computed box facts for every row and runs the general block
//! flow, then 9.4.3 relative offsets and 10.3.7/10.6.4 absolute placement in preorder.
//! Display none removes a subtree. Inline-level boxes are refused by source, not
//! approximated. Leaf content comes from the caller.
use super::{
    clearance_box_facts::{
        BoxFactMode, BoxLengthFact, BoxMaxFact, BoxOverflow, ClearanceBoxFact,
        ClearanceBoxFactsError, ClearanceBoxFactsLimits, LengthPercentageBoxFact,
        evaluate_box_rows, initial_box_fact, prepare_box_facts,
    },
    clearance_profile::ProjectionView,
    source_display::SourceDisplay,
};
use crate::{
    capture::ConstructionNode,
    host::source_publication::{InitialSourceHost, InitialSourceSnapshot},
};
use iced_css_core::{
    formatting::{
        absolute::{AbsoluteWidthError, AbsoluteWidthInput, absolute_width},
        absolute_height::{AbsoluteHeightError, AbsoluteHeightInput, absolute_height},
        block::{AutoLength, Direction},
        block_flow::{
            Dim, FlowBox, FlowContent, FlowError, FlowGeometry, FlowLimits, FlowRect, FlowViewport,
            layout_block_flow, preferred_content_widths,
        },
        display::{Display, Float, Position},
        floats::FloatSide,
    },
    identity::SourceRef,
    source::{Metadata, SourceNode},
    values::{ConversionError, NumericLimits, bidi::ComputedDirection},
};
use std::num::NonZeroUsize;

/// A captured tree in preorder. Projected 0 is the source root.
pub struct TreeSourceProjection<'h> {
    host: &'h InitialSourceHost,
    snapshot: &'h InitialSourceSnapshot,
    nodes: Vec<SourceNode>,
    original: Vec<usize>,
    projected: Vec<usize>,
}

impl<'h> ProjectionView<'h> for TreeSourceProjection<'h> {
    fn host(&self) -> &'h InitialSourceHost {
        self.host
    }
    fn snapshot(&self) -> &'h InitialSourceSnapshot {
        self.snapshot
    }
    fn nodes(&self) -> &[SourceNode] {
        &self.nodes
    }
    fn original_index(&self, projected: usize) -> Option<usize> {
        self.original.get(projected).copied()
    }
    fn projected_index(&self, original: usize) -> Option<usize> {
        self.projected.get(original).copied()
    }
    fn original_node(&self, projected: usize) -> Option<&'h ConstructionNode> {
        self.snapshot
            .construction()
            .nodes()
            .get(self.original_index(projected)?)
    }
    fn token(&self, projected: usize) -> Option<&'h SourceRef> {
        self.snapshot.token(self.original_index(projected)?)
    }
}

impl TreeSourceProjection<'_> {
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    /// The source node of a projected index.
    pub fn original_index(&self, projected: usize) -> Option<usize> {
        <Self as ProjectionView>::original_index(self, projected)
    }
    /// The projected index of a source node.
    pub fn projected_index(&self, original: usize) -> Option<usize> {
        <Self as ProjectionView>::projected_index(self, original)
    }
}

#[derive(Debug)]
pub enum TreeLayoutError {
    Sources,
    UnpublishedHost,
    Topology {
        original: usize,
    },
    Identity {
        original: usize,
    },
    Facts(ClearanceBoxFactsError),
    /// A display or position outside block flow. Not approximated.
    Unsupported {
        projected: usize,
        property: &'static str,
    },
    Percentage {
        projected: usize,
        error: ConversionError,
    },
    Flow(FlowError),
    AbsoluteWidth {
        projected: usize,
        error: AbsoluteWidthError,
    },
    AbsoluteHeight {
        projected: usize,
        error: AbsoluteHeightError,
    },
}

impl std::fmt::Display for TreeLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "tree layout: {self:?}")
    }
}
impl std::error::Error for TreeLayoutError {}

/// Projects the published snapshot of any captured tree.
///
/// # Errors
/// Rejects an unpublished host, a count over `sources`, and a malformed source tree.
pub fn prepare_tree_source_projection<'h>(
    host: &'h InitialSourceHost,
    snapshot: &'h InitialSourceSnapshot,
    sources: NonZeroUsize,
) -> Result<TreeSourceProjection<'h>, TreeLayoutError> {
    use TreeLayoutError as E;
    let published = host.snapshot().ok_or(E::UnpublishedHost)?;
    if !std::ptr::eq(published, snapshot) {
        return Err(E::UnpublishedHost);
    }
    let source = snapshot.construction();
    let count = source.nodes().len();
    if count == 0 || count > sources.get() || snapshot.tokens().len() != count {
        return Err(E::Sources);
    }
    // Sibling order is the construction order, which postorder preserves.
    let mut children = vec![Vec::new(); count];
    for (original, node) in source.nodes().iter().enumerate() {
        match node.parent() {
            Some(parent) if parent < count => children[parent].push(original),
            None if original == source.root() => {}
            _ => return Err(E::Topology { original }),
        }
    }
    let mut order = Vec::with_capacity(count);
    let mut stack = vec![source.root()];
    while let Some(original) = stack.pop() {
        order.push(original);
        stack.extend(children[original].iter().rev().copied());
    }
    if order.len() != count {
        return Err(E::Sources);
    }
    let mut projected = vec![0; count];
    for (index, &original) in order.iter().enumerate() {
        projected[original] = index;
    }
    let mut nodes = Vec::with_capacity(count);
    for &original in &order {
        let token = snapshot.token(original).ok_or(E::Topology { original })?;
        let instance = host
            .engine()
            .instance(token)
            .map_err(|_| E::Identity { original })?;
        nodes.push(SourceNode {
            instance,
            parent: None,
            children: Vec::new(),
            metadata: Metadata::default(),
        });
    }
    for (index, &original) in order.iter().enumerate() {
        nodes[index].parent = source.nodes()[original]
            .parent()
            .map(|parent| nodes[projected[parent]].instance);
        nodes[index].children = children[original]
            .iter()
            .map(|&child| nodes[projected[child]].instance)
            .collect();
    }
    Ok(TreeSourceProjection {
        host,
        snapshot,
        nodes,
        original: order,
        projected,
    })
}

/// Evaluates every row and runs block flow. `content` measures a source leaf by its
/// original index. The result is indexed by projected index; `None` is display none.
///
/// # Errors
/// Returns fact, unsupported-display, percentage and flow errors. Publishes nothing.
pub fn layout_captured_tree(
    projection: &TreeSourceProjection<'_>,
    display: &SourceDisplay<'_, '_, '_>,
    facts: ClearanceBoxFactsLimits,
    viewport: FlowViewport,
    limits: FlowLimits,
    mut content: impl FnMut(usize) -> Option<FlowContent>,
) -> Result<Vec<Option<FlowGeometry>>, TreeLayoutError> {
    use TreeLayoutError as E;
    let count = projection.len();
    let mut prepared = prepare_box_facts(projection, display, facts, BoxFactMode::Sequence, None)
        .map_err(E::Facts)?;
    prepared.absolute_offsets = true;
    let mut rows = vec![initial_box_fact(); count];
    evaluate_box_rows(projection, display, facts, &mut prepared, &mut rows).map_err(E::Facts)?;

    // One arena slot per rendered box; descendants of display none never get one.
    let mut slot: Vec<Option<usize>> = vec![None; count];
    let mut boxes: Vec<FlowBox> = Vec::new();
    let mut offsets: Vec<(usize, [Dim; 4])> = Vec::new();
    let mut positions: Vec<(usize, Position)> = Vec::new();
    for index in 0..count {
        let original = projection.original[index];
        let resolution =
            display
                .resolution(original)
                .ok_or(E::Facts(ClearanceBoxFactsError::SourceJoin {
                    projected: index,
                }))?;
        if resolution.display == Display::None {
            continue;
        }
        let parent = projection.snapshot.construction().nodes()[original]
            .parent()
            .map(|parent| projection.projected[parent]);
        let parent_slot = match parent {
            Some(parent) => match slot[parent] {
                Some(value) => Some(value),
                None => continue,
            },
            None => None,
        };
        if !matches!(resolution.display, Display::Block | Display::ListItem) {
            return Err(E::Unsupported {
                projected: index,
                property: "display",
            });
        }
        let row = &rows[index];
        let numeric = facts.numeric;
        let mut value = flow_box(row, numeric).map_err(|error| E::Percentage {
            projected: index,
            error,
        })?;
        value.direction = display
            .bidi_style(original)
            .map_or(Direction::Ltr, |style| match style.direction() {
                ComputedDirection::LeftToRight => Direction::Ltr,
                ComputedDirection::RightToLeft => Direction::Rtl,
            });
        value.float = match resolution.float {
            Float::Left => Some(FloatSide::Left),
            Float::Right => Some(FloatSide::Right),
            Float::None => None,
        };
        value.out_of_flow = matches!(resolution.position, Position::Absolute | Position::Fixed);
        if value.out_of_flow {
            // 9.7: an absolutely positioned box is never floating.
            value.float = None;
            value.establishes_bfc = true;
        }
        // Absent offsets are all auto: every positioned box is a containing block.
        positions.push((index, resolution.position));
        if let Some(offset) = row.offset {
            offsets.push((
                boxes.len(),
                offset
                    .map(|fact| dim(fact, numeric))
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| E::Percentage {
                        projected: index,
                        error,
                    })?
                    .try_into()
                    .unwrap_or([Dim::Auto; 4]),
            ));
        }
        let leaf = projection.nodes[index].children.is_empty();
        if leaf {
            value.content = content(original);
        }
        let arena = boxes.len();
        slot[index] = Some(arena);
        boxes.push(value);
        if let Some(parent) = parent_slot {
            boxes[parent].children.push(arena);
        }
    }
    if boxes.is_empty() {
        return Ok(vec![None; count]);
    }
    let mut geometry = layout_block_flow(&boxes, viewport, limits).map_err(E::Flow)?;
    // Arena order is preorder, so every box's ancestors have settled before it. A
    // relative offset moves a subtree after flow; floats and later flow do not react.
    let auto = [Dim::Auto; 4];
    for arena in 0..boxes.len() {
        let (index, position) = positions[arena];
        let offset = offsets
            .iter()
            .find(|(at, _)| *at == arena)
            .map_or(auto, |(_, offset)| *offset);
        match position {
            Position::Relative => {
                let basis = parent_content(&boxes, &geometry, arena, viewport);
                let (dx, dy) = relative_shift(&boxes[arena], offset, basis);
                shift(&boxes, &mut geometry, arena, dx, dy);
            }
            Position::Absolute | Position::Fixed => {
                place_absolute(
                    &boxes,
                    &mut geometry,
                    &positions,
                    arena,
                    position,
                    offset,
                    viewport,
                    limits,
                )
                .map_err(|error| error.at(index))?;
            }
            Position::Static => {}
        }
    }
    Ok(slot
        .into_iter()
        .map(|value| value.map(|arena| geometry[arena]))
        .collect())
}

enum AbsoluteFailure {
    Width(AbsoluteWidthError),
    Height(AbsoluteHeightError),
    Flow(FlowError),
}
impl AbsoluteFailure {
    fn at(self, projected: usize) -> TreeLayoutError {
        match self {
            Self::Width(error) => TreeLayoutError::AbsoluteWidth { projected, error },
            Self::Height(error) => TreeLayoutError::AbsoluteHeight { projected, error },
            Self::Flow(error) => TreeLayoutError::Flow(error),
        }
    }
}

fn parent_of(boxes: &[FlowBox], arena: usize) -> Option<usize> {
    boxes
        .iter()
        .position(|candidate| candidate.children.contains(&arena))
}

/// 10.1: the padding box of the nearest positioned ancestor, or the initial block.
fn containing_block(
    boxes: &[FlowBox],
    geometry: &[FlowGeometry],
    positions: &[(usize, Position)],
    arena: usize,
    position: Position,
    viewport: FlowViewport,
) -> (FlowRect, Direction) {
    let initial = FlowRect {
        left: viewport.left,
        right: viewport.left + viewport.width,
        top: viewport.top,
        bottom: viewport.top + viewport.height.unwrap_or(0.0),
    };
    if position == Position::Fixed {
        return (initial, boxes[0].direction);
    }
    let mut at = parent_of(boxes, arena);
    while let Some(ancestor) = at {
        if positions[ancestor].1 != Position::Static {
            return (geometry[ancestor].padding, boxes[ancestor].direction);
        }
        at = parent_of(boxes, ancestor);
    }
    (initial, boxes[0].direction)
}

fn subtree(boxes: &[FlowBox], root: usize) -> Vec<usize> {
    let mut order = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        order.push(node);
        stack.extend(boxes[node].children.iter().rev().copied());
    }
    order
}

#[allow(clippy::too_many_arguments)]
fn place_absolute(
    boxes: &[FlowBox],
    geometry: &mut [FlowGeometry],
    positions: &[(usize, Position)],
    arena: usize,
    position: Position,
    offset: [Dim; 4],
    viewport: FlowViewport,
    limits: FlowLimits,
) -> Result<(), AbsoluteFailure> {
    let value = &boxes[arena];
    let (block, direction) =
        containing_block(boxes, geometry, positions, arena, position, viewport);
    let cb_width = block.right - block.left;
    let cb_height = block.bottom - block.top;
    let auto = |dim: Dim, basis: f32| match dim {
        Dim::Auto => AutoLength::Auto,
        Dim::Px(number) => AutoLength::Value(number),
        Dim::Percent(number) => AutoLength::Value(basis * number / 100.0),
    };
    let zero = |dim: Dim, basis: f32| match auto(dim, basis) {
        AutoLength::Auto => 0.0,
        AutoLength::Value(number) => number,
    };
    let padding = value.padding.map(|dim| zero(dim, cb_width));
    let (preferred_min_width, preferred_width) = preferred_content_widths(boxes, arena);
    let static_border = geometry[arena].border;
    let width = absolute_width(AbsoluteWidthInput {
        containing_width: cb_width,
        containing_direction: direction,
        static_direction: value.direction,
        static_left: static_border.left - block.left,
        static_right: block.right - static_border.right,
        left: auto(offset[3], cb_width),
        width: auto(value.width, cb_width),
        right: auto(offset[1], cb_width),
        margin_left: auto(value.margin[3], cb_width),
        margin_right: auto(value.margin[1], cb_width),
        border_left: value.border[3],
        padding_left: padding[3],
        padding_right: padding[1],
        border_right: value.border[1],
        preferred_min_width,
        preferred_width,
        min_width: zero(value.min_width, cb_width),
        max_width: match value.max_width {
            Dim::Auto => None,
            other => Some(zero(other, cb_width)),
        },
    })
    .map_err(AbsoluteFailure::Width)?;
    // The contents form a new BFC: lay the subtree out alone, once to learn its 10.6.7
    // height and again with the used height so percentages see a definite basis.
    let nodes = subtree(boxes, arena);
    let local = |node: usize| nodes.iter().position(|candidate| *candidate == node);
    let solve = |height: Dim| -> Result<Vec<FlowGeometry>, FlowError> {
        let mut arena_copy: Vec<FlowBox> = nodes.iter().map(|&node| boxes[node].clone()).collect();
        for item in &mut arena_copy {
            item.children = item.children.iter().filter_map(|&c| local(c)).collect();
        }
        let root = &mut arena_copy[0];
        root.out_of_flow = false;
        root.width = Dim::Px(width.width);
        root.min_width = Dim::Auto;
        root.max_width = Dim::Auto;
        root.height = height;
        root.min_height = Dim::Auto;
        root.max_height = Dim::Auto;
        root.margin = [Dim::Px(0.0); 4];
        layout_block_flow(
            &arena_copy,
            FlowViewport {
                left: 0.0,
                top: 0.0,
                width: padding[3] + padding[1] + value.border[3] + value.border[1] + width.width,
                height: Some(cb_height),
            },
            limits,
        )
    };
    let specified = match value.height {
        Dim::Percent(_) if viewport.height.is_none() && position == Position::Fixed => Dim::Auto,
        other => other,
    };
    let first = solve(Dim::Auto).map_err(AbsoluteFailure::Flow)?;
    let content_height = first[0].content.bottom - first[0].content.top;
    let height = absolute_height(AbsoluteHeightInput {
        containing_height: cb_height,
        static_top: static_border.top - block.top,
        top: auto(offset[0], cb_height),
        height: auto(specified, cb_height),
        bottom: auto(offset[2], cb_height),
        margin_top: auto(value.margin[0], cb_width),
        margin_bottom: auto(value.margin[2], cb_width),
        border_top: value.border[0],
        padding_top: padding[0],
        padding_bottom: padding[2],
        border_bottom: value.border[2],
        content_height,
        min_height: zero(value.min_height, cb_height),
        max_height: match value.max_height {
            Dim::Auto => None,
            other => Some(zero(other, cb_height)),
        },
    })
    .map_err(AbsoluteFailure::Height)?;
    let second = solve(Dim::Px(height.height)).map_err(AbsoluteFailure::Flow)?;
    let dx = block.left + width.left + width.margin_left;
    let dy = block.top + height.top + height.margin_top;
    for (index, &node) in nodes.iter().enumerate() {
        let mut placed = second[index];
        for rect in [&mut placed.border, &mut placed.padding, &mut placed.content] {
            rect.left += dx;
            rect.right += dx;
            rect.top += dy;
            rect.bottom += dy;
        }
        if index == 0 {
            placed.margin = [
                height.margin_top,
                width.margin_right,
                height.margin_bottom,
                width.margin_left,
            ];
        }
        geometry[node] = placed;
    }
    Ok(())
}

fn parent_content(
    boxes: &[FlowBox],
    geometry: &[FlowGeometry],
    arena: usize,
    viewport: FlowViewport,
) -> (f32, Option<f32>) {
    parent_of(boxes, arena).map_or((viewport.width, viewport.height), |parent| {
        let content: FlowRect = geometry[parent].content;
        (
            content.right - content.left,
            Some(content.bottom - content.top),
        )
    })
}

fn relative_shift(value: &FlowBox, offset: [Dim; 4], basis: (f32, Option<f32>)) -> (f32, f32) {
    let horizontal = |dim: Dim| match dim {
        Dim::Auto => None,
        Dim::Px(number) => Some(number),
        Dim::Percent(number) => Some(basis.0 * number / 100.0),
    };
    let vertical = |dim: Dim| match dim {
        Dim::Auto => None,
        Dim::Px(number) => Some(number),
        Dim::Percent(number) => basis.1.map(|height| height * number / 100.0),
    };
    let [top, right, bottom, left] = offset;
    // 9.4.3: top wins over bottom; the direction's start edge wins horizontally.
    let dy = vertical(top)
        .or_else(|| vertical(bottom).map(|v| -v))
        .unwrap_or(0.0);
    let (first, second) = match value.direction {
        Direction::Ltr => (horizontal(left), horizontal(right).map(|v| -v)),
        Direction::Rtl => (horizontal(right).map(|v| -v), horizontal(left)),
    };
    (first.or(second).unwrap_or(0.0), dy)
}

fn shift(boxes: &[FlowBox], geometry: &mut [FlowGeometry], node: usize, dx: f32, dy: f32) {
    let target = &mut geometry[node];
    for rect in [&mut target.border, &mut target.padding, &mut target.content] {
        rect.left += dx;
        rect.right += dx;
        rect.top += dy;
        rect.bottom += dy;
    }
    for &child in &boxes[node].children {
        shift(boxes, geometry, child, dx, dy);
    }
}

fn dim(fact: BoxLengthFact, numeric: NumericLimits) -> Result<Dim, ConversionError> {
    Ok(match fact {
        BoxLengthFact::Absolute(value) => Dim::Px(value.pixels),
        BoxLengthFact::Auto { .. } => Dim::Auto,
        BoxLengthFact::Percentage(value) => {
            Dim::Percent(value.value.percentage_finite_f32(100.0, numeric)?.value)
        }
    })
}

fn length_dim(
    fact: LengthPercentageBoxFact,
    numeric: NumericLimits,
) -> Result<Dim, ConversionError> {
    Ok(match fact {
        LengthPercentageBoxFact::Absolute(value) => Dim::Px(value.pixels),
        LengthPercentageBoxFact::Percentage(value) => {
            Dim::Percent(value.value.percentage_finite_f32(100.0, numeric)?.value)
        }
    })
}

fn max_dim(fact: BoxMaxFact, numeric: NumericLimits) -> Result<Dim, ConversionError> {
    Ok(match fact {
        BoxMaxFact::Absolute(value) => Dim::Px(value.pixels),
        BoxMaxFact::None { .. } => Dim::Auto,
        BoxMaxFact::Percentage(value) => {
            Dim::Percent(value.value.percentage_finite_f32(100.0, numeric)?.value)
        }
    })
}

fn flow_box(row: &ClearanceBoxFact, numeric: NumericLimits) -> Result<FlowBox, ConversionError> {
    let side = |index: usize| dim(row.margin[index], numeric);
    let pad = |index: usize| length_dim(row.padding[index], numeric);
    Ok(FlowBox {
        clear: row.clear.value,
        establishes_bfc: row.overflow.value != BoxOverflow::Visible,
        width: dim(row.width, numeric)?,
        min_width: length_dim(row.min_width, numeric)?,
        max_width: max_dim(row.max_width, numeric)?,
        height: dim(row.height, numeric)?,
        min_height: length_dim(row.min_height, numeric)?,
        max_height: max_dim(row.max_height, numeric)?,
        margin: [side(0)?, side(1)?, side(2)?, side(3)?],
        padding: [pad(0)?, pad(1)?, pad(2)?, pad(3)?],
        border: row.border_widths(),
        ..FlowBox::default()
    })
}
