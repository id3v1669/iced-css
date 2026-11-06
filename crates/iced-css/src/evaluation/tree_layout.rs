//! Whole-tree block layout from the real cascade: projects any captured source tree
//! to preorder, evaluates computed box facts for every row and runs the general block
//! flow. Display none removes a subtree. Inline-level and absolutely positioned boxes
//! are refused by source, not approximated. Leaf content comes from the caller.
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
        block::Direction,
        block_flow::{
            Dim, FlowBox, FlowContent, FlowError, FlowGeometry, FlowLimits, FlowRect, FlowViewport,
            layout_block_flow,
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
    let mut rows = vec![initial_box_fact(); count];
    evaluate_box_rows(projection, display, facts, &mut prepared, &mut rows).map_err(E::Facts)?;

    // One arena slot per rendered box; descendants of display none never get one.
    let mut slot: Vec<Option<usize>> = vec![None; count];
    let mut boxes: Vec<FlowBox> = Vec::new();
    let mut offsets: Vec<(usize, [Dim; 4])> = Vec::new();
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
        if matches!(resolution.position, Position::Absolute | Position::Fixed) {
            return Err(E::Unsupported {
                projected: index,
                property: "position",
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
        if resolution.position == Position::Relative
            && let Some(offset) = row.offset
        {
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
    // 9.4.3: relative offsets move a subtree after flow, in source order. Parents come
    // first and carry their children; floats and later flow do not react.
    for &(arena, offset) in &offsets {
        let parent_width = parent_content(&boxes, &geometry, arena, viewport);
        let (dx, dy) = relative_shift(&boxes[arena], offset, parent_width);
        shift(&boxes, &mut geometry, arena, dx, dy);
    }
    Ok(slot
        .into_iter()
        .map(|value| value.map(|arena| geometry[arena]))
        .collect())
}

fn parent_content(
    boxes: &[FlowBox],
    geometry: &[FlowGeometry],
    arena: usize,
    viewport: FlowViewport,
) -> (f32, Option<f32>) {
    boxes
        .iter()
        .position(|candidate| candidate.children.contains(&arena))
        .map_or((viewport.width, viewport.height), |parent| {
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
