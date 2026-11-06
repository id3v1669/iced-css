//! Whole-tree block layout from the real cascade: projects any captured source tree
//! to preorder, evaluates computed box facts for every row and runs the general block
//! flow, then 9.4.3 relative offsets and 10.3.7/10.6.4 absolute placement in preorder.
//! Display none removes a subtree. Consecutive inline-level siblings share one anonymous
//! block (9.2.1.1) holding their text runs in source order; inline elements are flattened
//! into it. Floats among them are placed before the anonymous block. Table and
//! inline-block displays are refused by source. Other leaf content comes from the caller.
use super::{
    clearance_box_facts::{
        BoxFactMode, BoxFactTarget, BoxLengthFact, BoxMaxFact, BoxOverflow, ClearanceBoxFact,
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
    cascade::{
        DefaultedValueRef,
        rule::{RuleTarget, cascade_positioned_rules},
        winner_table::PropertyWinners,
    },
    formatting::{
        absolute::{AbsoluteWidthError, AbsoluteWidthInput, absolute_width},
        absolute_height::{AbsoluteHeightError, AbsoluteHeightInput, absolute_height},
        block::{AutoLength, Direction},
        block_flow::{
            Dim, FlowBox, FlowContent, FlowError, FlowGeometry, FlowLimits, FlowRect, FlowViewport,
            layout_block_flow_measured, preferred_content_widths,
        },
        clearance::ClearSide,
        display::{Display, Float, Position},
        floats::FloatSide,
    },
    identity::SourceRef,
    selectors::PseudoElement,
    source::{Metadata, SourceNode},
    values::{
        ConversionError, LengthTarget, NumericLimits, RelativeUnit,
        bidi::ComputedDirection,
        color::Color,
        computed_color::{
            ColorComputationContext, ColorComputationError, ComputedColor, compute_color,
        },
        font_size::{FontSizeError, FontSizeTable},
        font_weight::{FontWeight, compute_font_weight},
        property::{ContentItem, FontFamily, SpecifiedValue},
        registry::PropertyId,
        text_align::{ComputedTextAlign, compute_text_align},
    },
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
    FontMedium(FontSizeError),
    Paint {
        property: &'static str,
    },
    Color {
        projected: usize,
        error: ColorComputationError,
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

/// Where one text run of an inline block comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunSource {
    /// A captured text node, by projected index.
    Node(usize),
    /// The `::before` content (with any list marker) of a projected element.
    Before(usize),
    /// The `::after` content of a projected element.
    After(usize),
}

/// One `::before` or `::after`, as text in its element. Box properties other than
/// `display` and `clear` do not apply to it here, and `counter()` items produce nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedRun {
    pub text: String,
    pub size: f32,
    pub color: Option<[f32; 3]>,
    pub background: Option<[f32; 3]>,
    pub visible: bool,
    pub style: TextStyle,
    /// `display` other than inline: the run gets a block of its own.
    pub block: bool,
    pub clear: ClearSide,
}

/// The appended structure being built: arena slots and their per-slot records.
#[derive(Default)]
struct Build {
    boxes: Vec<FlowBox>,
    positions: Vec<(usize, Position)>,
    anonymous: Vec<bool>,
    clipped: Vec<bool>,
    runs: Vec<Vec<RunSource>>,
    /// The anonymous block still accepting inline siblings, per slot.
    open: Vec<Option<usize>>,
}

impl Build {
    fn anonymous_box(&mut self, target: usize, owner: usize, clear: ClearSide) -> usize {
        let arena = self.boxes.len();
        self.boxes.push(FlowBox {
            direction: self.boxes[target].direction,
            clear,
            ..FlowBox::default()
        });
        self.positions.push((owner, Position::Static));
        self.anonymous.push(true);
        self.clipped.push(false);
        self.runs.push(Vec::new());
        self.open.push(None);
        self.boxes[target].children.push(arena);
        arena
    }

    fn inline_group(&mut self, target: usize, owner: usize) -> usize {
        if let Some(group) = self.open[target] {
            return group;
        }
        let group = self.anonymous_box(target, owner, ClearSide::None);
        self.open[target] = Some(group);
        group
    }

    /// Adds generated text to an element's content: in the current run, or its own block.
    fn generated(&mut self, target: usize, owner: usize, run: RunSource, value: &GeneratedRun) {
        if value.block {
            self.open[target] = None;
            let arena = self.anonymous_box(target, owner, value.clear);
            self.runs[arena].push(run);
        } else {
            let group = self.inline_group(target, owner);
            self.runs[group].push(run);
        }
    }
}

/// Width-independent box tree, ready to lay out at any viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedTree {
    /// Arena order is preorder; indices differ from projected ones after display none.
    pub boxes: Vec<FlowBox>,
    pub offsets: Vec<(usize, [Dim; 4])>,
    pub positions: Vec<(usize, Position)>,
    /// Arena slot of each projected index; `None` is display none.
    pub slot: Vec<Option<usize>>,
    /// Computed font size in pixels, per projected index.
    pub font_sizes: Vec<f32>,
    /// Whether the arena slot is an anonymous block around inline content, per slot.
    pub anonymous: Vec<bool>,
    /// Projected indices of the text runs each arena slot holds, in source order.
    pub runs: Vec<Vec<RunSource>>,
    /// The enclosing inline element of each projected index, if any.
    pub inline_parent: Vec<Option<usize>>,
    /// Whether the arena slot clips its contents to its padding box (11.1.1). The root's
    /// `overflow` applies to the viewport, so it never clips here.
    pub clipped: Vec<bool>,
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
    // The `medium` font size in pixels; `em`, `ex` and font keywords need it.
    font_medium: Option<f32>,
    viewport: FlowViewport,
    limits: FlowLimits,
    content: impl FnMut(usize) -> Option<FlowContent>,
) -> Result<Vec<Option<FlowGeometry>>, TreeLayoutError> {
    prepare_captured_tree(projection, display, facts, font_medium, content, &[])?
        .layout(viewport, limits)
}

/// Evaluates every row into the width-independent box tree.
///
/// # Errors
/// Returns fact, unsupported-display and percentage errors.
pub fn prepare_captured_tree(
    projection: &TreeSourceProjection<'_>,
    display: &SourceDisplay<'_, '_, '_>,
    facts: ClearanceBoxFactsLimits,
    font_medium: Option<f32>,
    mut content: impl FnMut(usize) -> Option<FlowContent>,
    // Per projected index, `::before` and `::after`. May be empty.
    generated: &[[Option<GeneratedRun>; 2]],
) -> Result<PreparedTree, TreeLayoutError> {
    use TreeLayoutError as E;
    let count = projection.len();
    let font = font_medium
        .map(FontSizeTable::new)
        .transpose()
        .map_err(E::FontMedium)?;
    let mut prepared = prepare_box_facts(projection, display, facts, BoxFactMode::Sequence, font)
        .map_err(E::Facts)?;
    prepared.absolute_offsets = true;
    let mut rows = vec![initial_box_fact(); count];
    evaluate_box_rows(projection, display, facts, &mut prepared, &mut rows).map_err(E::Facts)?;

    // One arena slot per rendered box; descendants of display none never get one.
    let mut slot: Vec<Option<usize>> = vec![None; count];
    let mut build = Build::default();
    let pseudo = |projected: usize, side: usize| {
        generated
            .get(projected)
            .and_then(|pair| pair[side].as_ref())
    };
    let mut offsets: Vec<(usize, [Dim; 4])> = Vec::new();
    let font_sizes: Vec<f32> = rows
        .iter()
        .map(|row| row.font_size.map_or(16.0, |size| size.pixels))
        .collect();
    // The arena slot children of each projected row attach to; a block parent for itself
    // and, for an inline element, the block container it flattens into.
    let mut container: Vec<Option<usize>> = vec![None; count];
    let mut inline_element = vec![false; count];
    let mut inline_parent: Vec<Option<usize>> = vec![None; count];
    // Whether the projected row is a block text leaf, whose generated runs join its own.
    let mut text_block = vec![false; count];
    // Open elements, innermost last: an element's `::after` follows all its content.
    let mut ancestors: Vec<usize> = Vec::new();
    for index in 0..=count {
        let source_parent = (index < count)
            .then(|| {
                projection.snapshot.construction().nodes()[projection.original[index]].parent()
            })
            .flatten()
            .map(|parent| projection.projected[parent]);
        while let Some(&top) = ancestors.last() {
            if index < count && Some(top) == source_parent {
                break;
            }
            ancestors.pop();
            if let (Some(value), Some(target)) = (pseudo(top, 1), container[top]) {
                if text_block[top] {
                    build.runs[target].push(RunSource::After(top));
                } else {
                    build.generated(target, top, RunSource::After(top), value);
                }
            }
        }
        if index == count {
            break;
        }
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
            Some(parent) => match container[parent] {
                Some(value) => Some(value),
                None => continue,
            },
            None => None,
        };
        let source_node = &projection.snapshot.construction().nodes()[original];
        let is_text = source_node.text().is_some() && projection.nodes[index].children.is_empty();
        if resolution.display == Display::Inline
            && let Some(target) = parent_slot
        {
            let group = build.inline_group(target, index);
            slot[index] = Some(group);
            container[index] = Some(target);
            inline_element[index] = true;
            inline_parent[index] = parent.filter(|&outer| inline_element[outer]);
            if let Some(value) = pseudo(index, 0) {
                build.generated(target, index, RunSource::Before(index), value);
            }
            if is_text {
                let group = build.inline_group(target, index);
                build.runs[group].push(RunSource::Node(index));
            }
            ancestors.push(index);
            continue;
        }
        if parent.is_some() && !matches!(resolution.display, Display::Block | Display::ListItem) {
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
        build.positions.push((index, resolution.position));
        if let Some(offset) = row.offset {
            offsets.push((
                build.boxes.len(),
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
        if projection.nodes[index].children.is_empty() && !is_text {
            value.content = content(original);
        }
        let arena = build.boxes.len();
        slot[index] = Some(arena);
        container[index] = Some(arena);
        build.anonymous.push(false);
        build
            .clipped
            .push(parent.is_some() && rows[index].overflow.value != BoxOverflow::Visible);
        build.runs.push(Vec::new());
        build.open.push(None);
        let is_float = value.float.is_some();
        let in_flow = !is_float && !value.out_of_flow;
        build.boxes.push(value);
        if let Some(target) = parent_slot {
            match build.open[target] {
                Some(group) if is_float => {
                    // A float belongs at its line's top, and the run avoids it: placing
                    // it before the whole run is exact for a float on the first line.
                    let at = build.boxes[target]
                        .children
                        .iter()
                        .position(|child| *child == group)
                        .unwrap_or(build.boxes[target].children.len());
                    build.boxes[target].children.insert(at, arena);
                }
                _ => {
                    if in_flow {
                        build.open[target] = None;
                    }
                    build.boxes[target].children.push(arena);
                }
            }
        }
        // A text leaf holds its runs directly, so it stays a leaf.
        text_block[index] = is_text;
        if let Some(value) = pseudo(index, 0) {
            if is_text {
                build.runs[arena].push(RunSource::Before(index));
            } else {
                build.generated(arena, index, RunSource::Before(index), value);
            }
        }
        if is_text {
            build.runs[arena].push(RunSource::Node(index));
        }
        ancestors.push(index);
    }
    let Build {
        boxes,
        positions,
        anonymous,
        clipped,
        runs,
        open: _,
    } = build;
    Ok(PreparedTree {
        boxes,
        offsets,
        positions,
        slot,
        font_sizes,
        anonymous,
        runs,
        inline_parent,
        clipped,
    })
}

impl PreparedTree {
    /// Lays the tree out; indexed by projected index, `None` is display none.
    ///
    /// # Errors
    /// Returns flow and absolute placement errors.
    pub fn layout(
        &self,
        viewport: FlowViewport,
        limits: FlowLimits,
    ) -> Result<Vec<Option<FlowGeometry>>, TreeLayoutError> {
        self.layout_measured(viewport, limits, &mut |_, _| None)
    }

    /// The geometry of each arena slot's box mapped to projected indices.
    #[must_use]
    pub fn by_projected(&self, arena: &[FlowGeometry]) -> Vec<Option<FlowGeometry>> {
        self.slot
            .iter()
            .map(|value| value.and_then(|slot| arena.get(slot).copied()))
            .collect()
    }

    /// Lays the tree out, measuring leaves through `text_height(arena, width)`.
    ///
    /// # Errors
    /// Returns flow and absolute placement errors.
    pub fn layout_measured(
        &self,
        viewport: FlowViewport,
        limits: FlowLimits,
        text_height: &mut dyn FnMut(usize, f32) -> Option<f32>,
    ) -> Result<Vec<Option<FlowGeometry>>, TreeLayoutError> {
        Ok(self.by_projected(&self.layout_slots(viewport, limits, text_height)?))
    }

    /// Lays the tree out, by arena slot. Anonymous blocks have no projected index.
    ///
    /// # Errors
    /// Returns flow and absolute placement errors.
    pub fn layout_slots(
        &self,
        viewport: FlowViewport,
        limits: FlowLimits,
        text_height: &mut dyn FnMut(usize, f32) -> Option<f32>,
    ) -> Result<Vec<FlowGeometry>, TreeLayoutError> {
        use TreeLayoutError as E;
        let Self {
            boxes,
            offsets,
            positions,
            slot,
            font_sizes: _,
            anonymous: _,
            runs: _,
            inline_parent: _,
            clipped: _,
        } = self;
        let _ = slot;
        if boxes.is_empty() {
            return Ok(Vec::new());
        }
        let mut geometry =
            layout_block_flow_measured(boxes, viewport, limits, text_height).map_err(E::Flow)?;
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
                    let basis = parent_content(boxes, &geometry, arena, viewport);
                    let direction = parent_of(boxes, arena)
                        .map_or(boxes[arena].direction, |parent| boxes[parent].direction);
                    let (dx, dy) = relative_shift(direction, offset, basis);
                    shift(boxes, &mut geometry, arena, dx, dy);
                }
                Position::Absolute | Position::Fixed => {
                    place_absolute(
                        boxes,
                        &mut geometry,
                        positions,
                        arena,
                        position,
                        offset,
                        viewport,
                        limits,
                        text_height,
                    )
                    .map_err(|error| error.at(index))?;
                }
                Position::Static => {}
            }
        }
        Ok(geometry)
    }
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
    text_height: &mut dyn FnMut(usize, f32) -> Option<f32>,
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
        static_direction: parent_of(boxes, arena)
            .map_or(value.direction, |parent| boxes[parent].direction),
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
    let mut solve = |height: Dim| -> Result<Vec<FlowGeometry>, FlowError> {
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
        // The percentages' basis is the containing block, not this synthetic viewport.
        root.padding = padding.map(Dim::Px);
        layout_block_flow_measured(
            &arena_copy,
            FlowViewport {
                left: 0.0,
                top: 0.0,
                width: padding[3] + padding[1] + value.border[3] + value.border[1] + width.width,
                height: Some(cb_height),
            },
            limits,
            &mut |local_node, used| text_height(nodes[local_node], used),
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
        if let Some(rect) = &mut placed.leaf_band {
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
        // 9.3.2: a percentage of an auto-height containing block computes to auto.
        let height = (boxes[parent].height != Dim::Auto).then_some(content.bottom - content.top);
        (content.right - content.left, height)
    })
}

fn relative_shift(direction: Direction, offset: [Dim; 4], basis: (f32, Option<f32>)) -> (f32, f32) {
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
    // 9.4.3: top wins over bottom; the containing block's start edge wins horizontally.
    let dy = vertical(top)
        .or_else(|| vertical(bottom).map(|v| -v))
        .unwrap_or(0.0);
    let (first, second) = match direction {
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
    if let Some(rect) = &mut target.leaf_band {
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

/// Cascaded paint colors of one box. `None` means transparent or not representable.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoxPaint {
    /// 11.2: a hidden box lays out but paints nothing and takes no input.
    pub visible: bool,
    /// The computed `z-index`; `None` is auto. It only matters on positioned boxes.
    pub z_index: Option<i32>,
    /// Width and color of the outline; `None` draws none.
    pub outline: Option<(f32, [f32; 3])>,
    /// The computed `color`, for this box's text; `None` is transparent.
    pub color: Option<[f32; 3]>,
    /// Red, green, blue in 0 to 1.
    pub background: Option<[f32; 3]>,
    /// Top, right, bottom, left.
    pub border: [Option<[f32; 3]>; 4],
}

fn rgb(color: &Color, numeric: NumericLimits) -> Option<[f32; 3]> {
    match color {
        Color::Srgb([r, g, b]) => Some([*r, *g, *b].map(|value| f32::from(value) / 255.0)),
        Color::IntegerRgb([r, g, b]) => {
            Some([r, g, b].map(|value| f32::from(value.clamped_byte()) / 255.0))
        }
        Color::PercentageRgb([r, g, b]) => {
            // 4.3.6: out of range clips, including values too long to convert.
            let unit = |value: &iced_css_core::values::Decimal| match value
                .percentage_finite_f32(1.0, numeric)
            {
                Ok(converted) => converted.value.clamp(0.0, 1.0),
                Err(_) if value.is_negative() => 0.0,
                Err(_) => 1.0,
            };
            Some([unit(r), unit(g), unit(b)])
        }
        // A system palette and outline invert have no fixed value here.
        Color::Transparent | Color::Invert | Color::System(_) => None,
    }
}

static BLACK: Color = Color::Srgb([0, 0, 0]);

/// Everything `evaluate_tree_paint` computes per projected row.
pub struct TreePaint {
    pub paints: Vec<BoxPaint>,
    pub styles: Vec<TextStyle>,
    pub generated: Vec<[Option<GeneratedRun>; 2]>,
}

/// 12.5.1: the marker text of list item `ordinal` (from 1), as inside-marker text.
fn list_marker(kind: &str, ordinal: usize) -> Option<String> {
    let number = ordinal.to_string();
    let alpha = |base: u8| {
        let mut value = ordinal;
        let mut out = Vec::new();
        while value > 0 {
            value -= 1;
            out.push(base + (value % 26) as u8);
            value /= 26;
        }
        out.reverse();
        String::from_utf8_lossy(&out).into_owned()
    };
    let roman = |value: usize| {
        const TABLE: [(usize, &str); 13] = [
            (1000, "m"),
            (900, "cm"),
            (500, "d"),
            (400, "cd"),
            (100, "c"),
            (90, "xc"),
            (50, "l"),
            (40, "xl"),
            (10, "x"),
            (9, "ix"),
            (5, "v"),
            (4, "iv"),
            (1, "i"),
        ];
        let mut rest = value;
        let mut out = String::new();
        for (amount, text) in TABLE {
            while rest >= amount {
                out.push_str(text);
                rest -= amount;
            }
        }
        out
    };
    let label = match kind {
        "none" => return None,
        "disc" => return Some("\u{2022}\u{a0}".to_owned()),
        "circle" => return Some("\u{25e6}\u{a0}".to_owned()),
        "square" => return Some("\u{25aa}\u{a0}".to_owned()),
        "decimal-leading-zero" => format!("{ordinal:02}"),
        "lower-alpha" | "lower-latin" => alpha(b'a'),
        "upper-alpha" | "upper-latin" => alpha(b'A'),
        "lower-roman" => roman(ordinal),
        "upper-roman" => roman(ordinal).to_uppercase(),
        // Greek, Armenian and Georgian fall back to decimal here.
        _ => number,
    };
    Some(format!("{label}.\u{a0}"))
}

/// A computed line height, keeping the number form that inherits as a number (10.8.1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextLineHeight {
    #[default]
    Normal,
    Multiplier(f32),
    Px(f32),
}

/// Computed text properties of one row.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// 100 to 900.
    pub weight: u16,
    pub italic: bool,
    pub line_height: TextLineHeight,
    /// The first listed family; a generic is its keyword.
    pub family: Option<String>,
    pub underline: bool,
    pub line_through: bool,
    /// `left`, `center` or `right`, after the initial value's direction.
    pub align: &'static str,
    pub white_space: &'static str,
    pub text_transform: &'static str,
}

fn line_height_of(
    value: &SpecifiedValue,
    font_size: f32,
    numeric: NumericLimits,
) -> TextLineHeight {
    let px = |converted: Result<f32, ConversionError>| {
        converted.map_or(TextLineHeight::Normal, TextLineHeight::Px)
    };
    match value {
        SpecifiedValue::Number(number) => number
            .scaled_finite_f32(1.0, numeric)
            .map_or(TextLineHeight::Normal, |converted| {
                TextLineHeight::Multiplier(converted.value)
            }),
        SpecifiedValue::Length(LengthTarget::Absolute(length)) => {
            px(length.to_nonnegative_f32(numeric).map(|c| c.value))
        }
        SpecifiedValue::Length(LengthTarget::Percentage(number)) => px(number
            .percentage_finite_f32(font_size, numeric)
            .map(|c| c.value)),
        SpecifiedValue::Length(LengthTarget::Relative(number, unit)) => {
            let basis = match unit {
                RelativeUnit::Em => font_size,
                RelativeUnit::Ex => font_size * 0.5,
            };
            px(number.scaled_finite_f32(basis, numeric).map(|c| c.value))
        }
        _ => TextLineHeight::Normal,
    }
}

fn family_of(list: &[FontFamily]) -> Option<String> {
    list.first().map(|first| match first {
        FontFamily::Quoted(name) => name.clone(),
        FontFamily::Identifiers(words) => words.join(" "),
        FontFamily::Generic(keyword) => keyword.as_str().to_owned(),
    })
}

/// Cascades background and border colors for each projected row; `color` is computed
/// along the way, since it is both inherited and the initial border color.
///
/// # Errors
/// Returns a named property, cascade or color computation failure.
pub fn evaluate_tree_paint<'r>(
    projection: &TreeSourceProjection<'_>,
    display: &SourceDisplay<'_, '_, 'r>,
    limits: ClearanceBoxFactsLimits,
    font_sizes: &[f32],
) -> Result<TreePaint, TreeLayoutError> {
    use TreeLayoutError as E;
    let id = |name: &'static str| PropertyId::lookup(name).ok_or(E::Paint { property: name });
    let foreground = id("color")?;
    let background = id("background-color")?;
    let border = [
        id("border-top-color")?,
        id("border-right-color")?,
        id("border-bottom-color")?,
        id("border-left-color")?,
    ];
    let bundle = display.bundle();
    let snapshot = projection.snapshot;
    let source = snapshot
        .selector_source(limits.selectors)
        .map_err(|error| E::Facts(ClearanceBoxFactsError::Source(error)))?;
    // Per row: color, background, then the four border colors, for `inherit`.
    let mut colors: Vec<[ComputedColor<'r>; 6]> = Vec::with_capacity(projection.len());
    let mut paints = Vec::with_capacity(projection.len());
    let mut styles: Vec<TextStyle> = Vec::with_capacity(projection.len());
    let mut weights: Vec<FontWeight> = Vec::with_capacity(projection.len());
    let mut aligns: Vec<ComputedTextAlign> = Vec::with_capacity(projection.len());
    let mut visibility: Vec<bool> = Vec::with_capacity(projection.len());
    let mut z_indices: Vec<Option<i32>> = Vec::with_capacity(projection.len());
    let outline_color = id("outline-color")?;
    let mut generated: Vec<[Option<GeneratedRun>; 2]> = Vec::with_capacity(projection.len());
    let mut marker_types: Vec<Option<String>> = Vec::with_capacity(projection.len());
    // Each list parent's running item count.
    let mut items = vec![0_usize; projection.len()];
    let nodes = snapshot.construction().nodes();
    for (index, &font_size) in font_sizes.iter().enumerate() {
        let original = projection.original[index];
        let mut winners = PropertyWinners::new();
        cascade_positioned_rules(
            &mut winners,
            bundle.offers().iter().copied(),
            RuleTarget {
                source: &source,
                node: original,
                pseudo: None,
            },
            bundle.sheets(),
            limits.per_target,
        )
        .map_err(|error| {
            E::Facts(ClearanceBoxFactsError::Cascade {
                original,
                target: BoxFactTarget::Element,
                error,
            })
        })?;
        let parent = snapshot.construction().nodes()[original]
            .parent()
            .map(|parent| colors[projection.projected[parent]]);
        let parent_of_slot = |slot: usize| parent.map(|row: [ComputedColor<'r>; 6]| row[slot]);
        let current = compute_color(
            &winners,
            foreground,
            ColorComputationContext {
                parent: parent_of_slot(0),
                current_color: None,
                user_agent_color: Some(&BLACK),
            },
        )
        .map_err(|error| E::Color {
            projected: index,
            error,
        })?;
        let parent_index = snapshot.construction().nodes()[original]
            .parent()
            .map(|parent| projection.projected[parent]);
        let inherited = parent_index.map(|parent| &styles[parent]);
        let defaulted = |name: &'static str| {
            let property = id(name)?;
            winners
                .defaulted(property, parent_index.is_some())
                .map_err(|_| E::Paint { property: name })
        };
        let visible = match defaulted("visibility")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                keyword.as_str() == "visible"
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.is_none_or(|parent| visibility[parent])
            }
            _ => true,
        };
        let z_index = match defaulted("z-index")? {
            DefaultedValueRef::Compute(SpecifiedValue::Integer(value)) => {
                Some(value.saturating_i32())
            }
            DefaultedValueRef::ParentComputed => parent_index.and_then(|parent| z_indices[parent]),
            _ => None,
        };
        let outline_width = match defaulted("outline-style")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(style))
                if !matches!(style.as_str(), "none" | "hidden") =>
            {
                match defaulted("outline-width")? {
                    DefaultedValueRef::Compute(SpecifiedValue::Keyword(width)) => {
                        Some(match width.as_str() {
                            "thin" => 1.0,
                            "thick" => 5.0,
                            _ => 3.0,
                        })
                    }
                    DefaultedValueRef::Compute(SpecifiedValue::Length(LengthTarget::Absolute(
                        length,
                    ))) => length
                        .to_nonnegative_f32(limits.numeric)
                        .ok()
                        .map(|c| c.value),
                    _ => Some(3.0),
                }
            }
            _ => None,
        }
        .filter(|width| *width > 0.0);
        let parent_weight = parent_index.map_or(FontWeight::NORMAL, |parent| weights[parent]);
        let weight = match defaulted("font-weight")? {
            DefaultedValueRef::Compute(value) => compute_font_weight(value, parent_weight)
                .map_err(|_| E::Paint {
                    property: "font-weight",
                })?,
            DefaultedValueRef::ParentComputed => parent_weight,
            DefaultedValueRef::Initial(_) => FontWeight::NORMAL,
        };
        let italic = match defaulted("font-style")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                keyword.as_str() != "normal"
            }
            DefaultedValueRef::ParentComputed => inherited.is_some_and(|style| style.italic),
            _ => false,
        };
        let line_height = match defaulted("line-height")? {
            DefaultedValueRef::Compute(value) => line_height_of(value, font_size, limits.numeric),
            DefaultedValueRef::ParentComputed => {
                inherited.map_or(TextLineHeight::Normal, |style| style.line_height)
            }
            DefaultedValueRef::Initial(_) => TextLineHeight::Normal,
        };
        let family = match defaulted("font-family")? {
            DefaultedValueRef::Compute(SpecifiedValue::FontFamily(list)) => family_of(list),
            DefaultedValueRef::ParentComputed => inherited.and_then(|style| style.family.clone()),
            _ => None,
        };
        let resolution = display.resolution(original);
        // 16.3.1: decorations propagate down, except into floats and absolute boxes.
        let detached = resolution.is_some_and(|value| {
            value.float != Float::None
                || matches!(value.position, Position::Absolute | Position::Fixed)
        });
        let carried = inherited.filter(|_| !detached);
        let (mut underline, mut line_through) = carried.map_or((false, false), |style| {
            (style.underline, style.line_through)
        });
        if let DefaultedValueRef::Compute(SpecifiedValue::TextDecoration(own)) =
            defaulted("text-decoration")?
        {
            underline |= own.underline();
            line_through |= own.line_through();
        }
        let parent_align = parent_index.map(|parent| aligns[parent]);
        let align = compute_text_align(&winners, parent_align).map_err(|_| E::Paint {
            property: "text-align",
        })?;
        weights.push(weight);
        aligns.push(align);
        let rtl = display
            .bidi_style(original)
            .is_some_and(|style| style.direction() == ComputedDirection::RightToLeft);
        let keywords = display.text_keywords(original);
        styles.push(TextStyle {
            weight: weight.number(),
            italic,
            line_height,
            family,
            underline,
            line_through,
            align: match align {
                ComputedTextAlign::Right => "right",
                ComputedTextAlign::Center => "center",
                ComputedTextAlign::DirectionalInitial if rtl => "right",
                // A renderer paragraph cannot justify: justified text stays start-aligned.
                ComputedTextAlign::Justify if rtl => "right",
                _ => "left",
            },
            white_space: keywords.map_or("normal", |value| value.white_space().keyword().as_str()),
            text_transform: keywords
                .map_or("none", |value| value.text_transform().keyword().as_str()),
        });
        let side = |property: PropertyId, slot: usize| {
            compute_color(
                &winners,
                property,
                ColorComputationContext {
                    parent: parent_of_slot(slot),
                    current_color: Some(current),
                    user_agent_color: Some(&BLACK),
                },
            )
            .map_err(|error| E::Color {
                projected: index,
                error,
            })
        };
        let row = [
            current,
            side(background, 1)?,
            side(border[0], 2)?,
            side(border[1], 3)?,
            side(border[2], 4)?,
            side(border[3], 5)?,
        ];
        colors.push(row);
        let paint = |slot: usize| rgb(row[slot].value(), limits.numeric);
        // An outline's initial `invert` has no value without the backdrop: it uses `color`.
        let outline = outline_width.map(|width| {
            let color = compute_color(
                &winners,
                outline_color,
                ColorComputationContext {
                    parent: None,
                    current_color: Some(current),
                    user_agent_color: Some(&BLACK),
                },
            )
            .ok()
            .and_then(|color| rgb(color.value(), limits.numeric))
            .or_else(|| paint(0))
            .unwrap_or([0.0; 3]);
            (width, color)
        });
        paints.push(BoxPaint {
            // A system color has no palette here: text stays visible, in black.
            color: match row[0].value() {
                Color::System(_) | Color::Invert => Some([0.0; 3]),
                _ => paint(0),
            },
            background: paint(1),
            border: [paint(2), paint(3), paint(4), paint(5)],
            visible,
            z_index,
            outline,
        });
        visibility.push(visible);
        z_indices.push(z_index);
        // Generated content: text in the element, inheriting from it.
        let marker_type = match defaulted("list-style-type")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                Some(keyword.as_str().to_owned())
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.and_then(|parent| marker_types[parent].clone())
            }
            _ => None,
        };
        let marker = if display
            .resolution(original)
            .is_some_and(|value| value.display == Display::ListItem)
        {
            let ordinal = parent_index.map_or(1, |parent| {
                items[parent] += 1;
                items[parent]
            });
            list_marker(marker_type.as_deref().unwrap_or("disc"), ordinal)
        } else {
            None
        };
        marker_types.push(marker_type);
        let mut pair = [None, None];
        for (side, pseudo) in [PseudoElement::Before, PseudoElement::After]
            .into_iter()
            .enumerate()
        {
            let mut winners = PropertyWinners::new();
            cascade_positioned_rules(
                &mut winners,
                bundle.offers().iter().copied(),
                RuleTarget {
                    source: &source,
                    node: original,
                    pseudo: Some(pseudo),
                },
                bundle.sheets(),
                limits.per_target,
            )
            .map_err(|error| {
                E::Facts(ClearanceBoxFactsError::Cascade {
                    original,
                    target: if side == 0 {
                        BoxFactTarget::Before
                    } else {
                        BoxFactTarget::After
                    },
                    error,
                })
            })?;
            let cascaded = |name: &'static str| {
                let property = id(name)?;
                winners
                    .defaulted(property, true)
                    .map_err(|_| E::Paint { property: name })
            };
            let mut content = None;
            if let DefaultedValueRef::Compute(SpecifiedValue::Content(list)) = cascaded("content")?
            {
                let mut text = String::new();
                for item in list {
                    match item {
                        ContentItem::Text(value) => text.push_str(value),
                        ContentItem::Attribute(name) => {
                            if let Some(value) = nodes[original].metadata().attribute(name) {
                                text.push_str(value);
                            }
                        }
                        ContentItem::Quote(keyword) => match keyword.as_str() {
                            "open-quote" => text.push('\u{201c}'),
                            "close-quote" => text.push('\u{201d}'),
                            _ => {}
                        },
                        ContentItem::Uri(_) | ContentItem::Counter { .. } => {}
                    }
                }
                content = Some(text);
            }
            let hide_display = matches!(
                cascaded("display")?,
                DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword))
                    if keyword.as_str() == "none"
            );
            if hide_display {
                content = None;
            }
            let block = matches!(
                cascaded("display")?,
                DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword))
                    if keyword.as_str() != "inline"
            );
            let clear = match cascaded("clear")? {
                DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                    match keyword.as_str() {
                        "left" => ClearSide::Left,
                        "right" => ClearSide::Right,
                        "both" => ClearSide::Both,
                        _ => ClearSide::None,
                    }
                }
                _ => ClearSide::None,
            };
            let marker_text = if side == 0 { marker.clone() } else { None };
            if content.is_none() && marker_text.is_none() {
                continue;
            }
            let text = format!(
                "{}{}",
                marker_text.unwrap_or_default(),
                content.unwrap_or_default()
            );
            let owner = styles[index].clone();
            let size = match cascaded("font-size")? {
                DefaultedValueRef::Compute(SpecifiedValue::Length(target)) => match target {
                    LengthTarget::Absolute(length) => length
                        .to_nonnegative_f32(limits.numeric)
                        .map_or(font_size, |c| c.value),
                    LengthTarget::Relative(number, RelativeUnit::Em) => number
                        .scaled_finite_f32(font_size, limits.numeric)
                        .map_or(font_size, |c| c.value),
                    LengthTarget::Relative(number, RelativeUnit::Ex) => number
                        .scaled_finite_f32(font_size * 0.5, limits.numeric)
                        .map_or(font_size, |c| c.value),
                    LengthTarget::Percentage(number) => number
                        .percentage_finite_f32(font_size, limits.numeric)
                        .map_or(font_size, |c| c.value),
                    LengthTarget::Auto => font_size,
                },
                _ => font_size,
            };
            let weight = match cascaded("font-weight")? {
                DefaultedValueRef::Compute(value) => {
                    compute_font_weight(value, weight).map_or(owner.weight, FontWeight::number)
                }
                _ => owner.weight,
            };
            let italic = match cascaded("font-style")? {
                DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                    keyword.as_str() != "normal"
                }
                _ => owner.italic,
            };
            let (mut underline, mut line_through) = (owner.underline, owner.line_through);
            if let DefaultedValueRef::Compute(SpecifiedValue::TextDecoration(own)) =
                cascaded("text-decoration")?
            {
                underline |= own.underline();
                line_through |= own.line_through();
            }
            let pseudo_color = compute_color(
                &winners,
                foreground,
                ColorComputationContext {
                    parent: Some(current),
                    current_color: None,
                    user_agent_color: Some(&BLACK),
                },
            )
            .map_err(|error| E::Color {
                projected: index,
                error,
            })?;
            let pseudo_background = compute_color(
                &winners,
                background,
                ColorComputationContext {
                    parent: Some(row[1]),
                    current_color: Some(pseudo_color),
                    user_agent_color: Some(&BLACK),
                },
            )
            .map_err(|error| E::Color {
                projected: index,
                error,
            })?;
            pair[side] = Some(GeneratedRun {
                text,
                size,
                color: rgb(pseudo_color.value(), limits.numeric),
                background: rgb(pseudo_background.value(), limits.numeric),
                visible,
                style: TextStyle {
                    weight,
                    italic,
                    underline,
                    line_through,
                    ..owner
                },
                block,
                clear,
            });
        }
        generated.push(pair);
    }
    Ok(TreePaint {
        paints,
        styles,
        generated,
    })
}
