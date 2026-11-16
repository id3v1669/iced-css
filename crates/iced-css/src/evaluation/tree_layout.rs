//! Whole-tree block layout from the real cascade: projects any captured source tree
//! to preorder, evaluates computed box facts for every row and runs the general block
//! flow, then 9.4.3 relative offsets and 10.3.7/10.6.4 absolute placement in preorder.
//! Display none removes a subtree. Consecutive inline-level siblings share one anonymous
//! block (9.2.1.1) holding their text runs in source order; inline elements are flattened
//! into it. Floats among them are placed before the anonymous block. Inline-blocks form
//! lines in which text takes boxes of its own. Other leaf content comes from the caller.
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
            AutoRepeat, Breadth, Dim, FlexAlign, FlexContainer, FlexItem, FlexSpace, FlexWrap,
            FlowBox, FlowContent, FlowError, FlowGeometry, FlowLimits, FlowRect, FlowViewport,
            GridContainer, GridItem, GridLine, RowAlign, TableRole, TrackList, TrackSize,
            content_boxes, grid_areas, layout_block_flow_baselines, preferred_content_widths,
        },
        clearance::ClearSide,
        display::{Display, Float, Position},
        floats::FloatSide,
    },
    identity::SourceRef,
    selectors::PseudoElement,
    source::{Metadata, SourceNode},
    values::{
        ConversionError, LengthTarget, NumericLimits,
        bidi::ComputedDirection,
        color::Color,
        computed_color::{
            ColorComputationContext, ColorComputationError, ComputedColor, compute_color,
        },
        font_size::{FontSizeError, FontSizeTable},
        font_weight::{FontWeight, compute_font_weight},
        property::{
            AngleUnit, ContentItem, EasingValue, FontFamily, GRID_LIMIT, GradientDirection,
            GridBreadth, GridRepeat, GridTrackEntry, GridTrackSize, GridValue, SpecifiedValue,
            StepPosition as StepValue, TransitionProperty, TransitionValue,
        },
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

/// The box of a block-level `::before` or `::after`: its sizes, sides and paint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeneratedFrame {
    pub width: Dim,
    pub height: Dim,
    pub margin: [Dim; 4],
    pub padding: [Dim; 4],
    pub border: [f32; 4],
    pub paint: BoxPaint,
}

/// One `::before` or `::after`, as text in its element. A block-level one has a box of
/// its own (`frame`); an inline one takes the frame's sides around its text.
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
    pub frame: Option<GeneratedFrame>,
    /// The byte length of an inside list marker at the start of `text`.
    pub marker: Option<usize>,
}

/// The appended structure being built: arena slots and their per-slot records.
#[derive(Default)]
struct Build {
    boxes: Vec<FlowBox>,
    positions: Vec<(usize, Position)>,
    anonymous: Vec<bool>,
    clipped: Vec<bool>,
    scrolls: Vec<bool>,
    runs: Vec<Vec<RunSource>>,
    /// The anonymous block still accepting inline siblings, per slot.
    open: Vec<Option<usize>>,
    /// Block-level generated content's own boxes, by arena slot, in slot order.
    generated_boxes: Vec<(usize, BoxPaint)>,
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
        self.scrolls.push(false);
        self.runs.push(Vec::new());
        self.open.push(None);
        self.boxes[target].children.push(arena);
        arena
    }

    /// 17.2.1: the parent a box with `role` (`None` is any non-table box) actually goes in,
    /// building the anonymous table parts it is missing. Consecutive misplaced siblings
    /// share them, through the same open slot inline runs use.
    fn table_parent(&mut self, mut target: usize, owner: usize, role: Option<TableRole>) -> usize {
        // Blocks and inlines under a table part need a cell; cells need a row; rows and
        // groups and captions need a table.
        loop {
            let here = self.boxes[target].table;
            let group = |role: Option<TableRole>| {
                matches!(
                    role,
                    Some(TableRole::HeaderGroup | TableRole::BodyGroup | TableRole::FooterGroup)
                )
            };
            let in_table = matches!(here, Some(TableRole::Table { .. }));
            let in_row = here == Some(TableRole::Row);
            let wanted = match role {
                // Only a cell or caption or plain block holds these.
                // Rule 1 of stage 2: a row, as for a cell, so a cell beside it shares it.
                None | Some(TableRole::Table { .. }) => {
                    if in_table || group(here) {
                        Some(TableRole::Row)
                    } else if in_row {
                        Some(TableRole::Cell)
                    } else {
                        None
                    }
                }
                Some(TableRole::Cell) => {
                    if in_row {
                        None
                    } else if in_table || group(here) {
                        Some(TableRole::Row)
                    } else {
                        Some(TableRole::Table {
                            spacing: [0.0, 0.0],
                        })
                    }
                }
                Some(TableRole::Row) => {
                    if in_table || group(here) {
                        None
                    } else if in_row {
                        Some(TableRole::Cell)
                    } else {
                        Some(TableRole::Table {
                            spacing: [0.0, 0.0],
                        })
                    }
                }
                Some(_) => {
                    if in_table {
                        None
                    } else if group(here) {
                        Some(TableRole::Row)
                    } else if in_row {
                        Some(TableRole::Cell)
                    } else {
                        Some(TableRole::Table {
                            spacing: [0.0, 0.0],
                        })
                    }
                }
            };
            let Some(wanted) = wanted else {
                return target;
            };
            let reuse = self.open[target].filter(|open| {
                matches!(
                    (self.boxes[*open].table, wanted),
                    (Some(TableRole::Table { .. }), TableRole::Table { .. })
                        | (Some(TableRole::Cell), TableRole::Cell)
                ) || self.boxes[*open].table == Some(wanted)
            });
            target = match reuse {
                Some(open) => open,
                None => {
                    let made = self.anonymous_box(target, owner, ClearSide::None);
                    self.boxes[made].table = Some(wanted);
                    self.boxes[made].establishes_bfc =
                        matches!(wanted, TableRole::Table { .. } | TableRole::Cell);
                    self.open[target] = Some(made);
                    made
                }
            };
        }
    }

    fn inline_group(&mut self, target: usize, owner: usize) -> usize {
        // An open anonymous table part is not a run: a run starts fresh after it.
        if let Some(group) = self.open[target].filter(|open| self.boxes[*open].table.is_none()) {
            return group;
        }
        let group = self.anonymous_box(target, owner, ClearSide::None);
        self.open[target] = Some(group);
        group
    }

    /// The box text goes in on a line of inline-blocks: the piece the last text opened, or
    /// a new one. Each piece sits in the line as one box, breaking only inside itself.
    fn row_text(&mut self, group: usize, owner: usize) -> usize {
        if let Some(&last) = self.boxes[group].children.last()
            && self.anonymous[last]
            && self.boxes[last].table.is_none()
        {
            return last;
        }
        let piece = self.anonymous_box(group, owner, ClearSide::None);
        self.boxes[piece].establishes_bfc = true;
        piece
    }

    /// Adds generated text to an element's content: in the current run, or its own block.
    fn generated(
        &mut self,
        rows: &[usize],
        target: usize,
        owner: usize,
        run: RunSource,
        value: &GeneratedRun,
    ) {
        if value.block {
            self.open[target] = None;
            let arena = self.anonymous_box(target, owner, value.clear);
            self.runs[arena].push(run);
            if let Some(frame) = value.frame {
                let made = &mut self.boxes[arena];
                made.width = frame.width;
                made.height = frame.height;
                made.margin = frame.margin;
                made.padding = frame.padding;
                made.border = frame.border;
                self.generated_boxes.push((arena, frame.paint));
            }
        } else {
            let group = self.inline_group(target, owner);
            let holder = if rows.contains(&group) {
                self.row_text(group, owner)
            } else {
                group
            };
            self.runs[holder].push(run);
        }
    }
}

/// An inline element's margin, border and padding in pixels, top right bottom left. A
/// percentage has no line width to resolve against here: 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InlineBoxSides {
    pub margin: [f32; 4],
    pub border: [f32; 4],
    pub padding: [f32; 4],
    /// Percentage margins and paddings, of the containing block's width, sized at layout.
    pub percent_margin: [f32; 4],
    pub percent_padding: [f32; 4],
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
    /// The box sides of each inline element, by projected index.
    pub inline_boxes: Vec<Option<InlineBoxSides>>,
    /// Each table's `table-column` boxes in column order, direct or in a direct column group:
    /// table arena slot, the column's projected index, and its group's projected index.
    pub table_columns: Vec<(usize, usize, Option<usize>)>,
    /// Block-level generated content's own boxes and paint, by arena slot, in slot order.
    pub generated_boxes: Vec<(usize, BoxPaint)>,
    /// 17.6.2.1 per stretch of edge: a collapsed cell's side that loses the stretch it
    /// shares with a neighbour spanning differently, as (cell, side, winning neighbour),
    /// by arena slot. The side is not drawn along the winner.
    pub border_cuts: Vec<(usize, usize, usize)>,
    /// A cell's side that clears a wider neighbour in layout but draws its own width there,
    /// as (cell, side, width), by arena slot.
    pub thin_sides: Vec<(usize, usize, f32)>,
    /// A collapsed cell's side that gives way along all of its edge and draws nothing,
    /// as (cell, side), by arena slot, sorted.
    pub collapsed_losers: Vec<(usize, usize)>,
    /// A collapsed cell's side that draws the border of its table, row or row group, which
    /// won there, as (cell, side, table or row or row group), by arena slot.
    pub table_borders: Vec<(usize, usize, usize)>,
    /// 17.6.2: the border widths of rows, row groups, columns and column groups, which their
    /// boxes do not keep: only a collapsed table's cells draw them, where they win. As
    /// (projected index, widths), sorted by index; all zero is left out.
    pub layer_borders: Vec<(usize, [f32; 4])>,
    /// A line of inline-blocks whose first line takes a percentage `text-indent`, of its
    /// block's containing block's width, as (row, percent), by arena slot.
    pub row_indents: Vec<(usize, f32)>,
    /// Whether the arena slot clips its contents to its padding box (11.1.1). The root's
    /// `overflow` applies to the viewport, so it never clips here.
    pub clipped: Vec<bool>,
    /// Whether `overflow` is `auto` or `scroll`, so the box scrolls vertically (11.1.1).
    /// The root's `visible` counts as `auto`: it scrolls the viewport.
    pub scrolls: Vec<bool>,
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
    // An inline element's sides, as its line runs take them.
    let inline_sides = |at: usize| {
        let sides = flow_box(&rows[at], facts.numeric).ok()?;
        let pixels = |dims: [Dim; 4]| {
            dims.map(|value| match value {
                Dim::Px(pixels) | Dim::Mix(pixels, _) => pixels,
                _ => 0.0,
            })
        };
        let percent = |dims: [Dim; 4]| {
            dims.map(|value| match value {
                Dim::Percent(percent) | Dim::Mix(_, percent) => percent.max(0.0),
                _ => 0.0,
            })
        };
        Some(InlineBoxSides {
            margin: pixels(sides.margin),
            border: sides.border,
            padding: pixels(sides.padding),
            percent_margin: percent(sides.margin),
            percent_padding: percent(sides.padding),
        })
    };
    // 10.8: whether those sides take any room.
    let sided = |at: usize| {
        inline_sides(at).is_some_and(|sides| {
            sides
                .margin
                .iter()
                .chain(&sides.border)
                .chain(&sides.padding)
                .chain(&sides.percent_margin)
                .chain(&sides.percent_padding)
                .any(|side| *side > 0.0)
        })
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
    let mut inline_boxes: Vec<Option<InlineBoxSides>> = vec![None; count];
    let mut table_columns: Vec<(usize, usize, Option<usize>)> = Vec::new();
    let mut layer_borders: Vec<(usize, [f32; 4])> = Vec::new();
    // Each projected row's parent element, by projected index.
    let mut source_parent: Vec<Option<usize>> = vec![None; count];
    // The table arena slot of each column group that sits directly in a table.
    let mut group_table: Vec<Option<usize>> = vec![None; count];
    // Whether the projected row is a block text leaf, whose generated runs join its own.
    let mut text_block = vec![false; count];
    let mut inline_rows: Vec<usize> = Vec::new();
    // Open elements, innermost last: an element's `::after` follows all its content.
    let mut ancestors: Vec<usize> = Vec::new();
    for index in 0..=count {
        // Skip absent boxes first: their subtrees must not close any open element.
        let mut resolved = None;
        if index < count {
            let original = projection.original[index];
            let resolution = display.resolution(original).ok_or(E::Facts(
                ClearanceBoxFactsError::SourceJoin { projected: index },
            ))?;
            if resolution.display == Display::None {
                continue;
            }
            let parent = projection.snapshot.construction().nodes()[original]
                .parent()
                .map(|parent| projection.projected[parent]);
            let parent_slot = match parent {
                Some(parent) => match container[parent] {
                    Some(value) => Some(value),
                    // A column group's own columns are read; nothing else in it is.
                    None if group_table[parent].is_some()
                        && resolution.display == Display::TableColumn =>
                    {
                        None
                    }
                    None => continue,
                },
                None => None,
            };
            resolved = Some((original, resolution, parent, parent_slot));
        }
        while let Some(&top) = ancestors.last() {
            if resolved.is_some_and(|(_, _, parent, _)| Some(top) == parent) {
                break;
            }
            ancestors.pop();
            if let (Some(value), Some(target)) = (pseudo(top, 1), container[top]) {
                if text_block[top] {
                    build.runs[target].push(RunSource::After(top));
                } else {
                    let target = build.table_parent(target, top, None);
                    build.generated(&inline_rows, target, top, RunSource::After(top), value);
                }
            }
        }
        let Some((original, resolution, parent, parent_slot)) = resolved else {
            break;
        };
        source_parent[index] = parent;
        // 17.2.1: anonymous table parts wrap consecutive siblings only. One opened for an
        // element in another parent, such as inside an inline element now closed, ends here;
        // one the parent opened itself, for its `:before`, goes on with its children.
        if let Some(target) = parent_slot
            && let Some(open) = build.open[target]
            && build.anonymous[open]
            && build.boxes[open].table.is_some()
            && Some(build.positions[open].0) != parent
            && source_parent[build.positions[open].0] != parent
        {
            build.open[target] = None;
        }
        let source_node = &projection.snapshot.construction().nodes()[original];
        let is_text = source_node.text().is_some() && projection.nodes[index].children.is_empty();
        if resolution.display == Display::Inline
            && let Some(target) = parent_slot
        {
            // 17.2.1: blank text between table parts generates nothing; other inline
            // content goes in an anonymous cell.
            let in_part = matches!(
                build.boxes[target].table,
                Some(
                    TableRole::Table { .. }
                        | TableRole::HeaderGroup
                        | TableRole::BodyGroup
                        | TableRole::FooterGroup
                        | TableRole::Row
                )
            );
            let open_part = build.open[target]
                .and_then(|open| build.boxes[open].table)
                .filter(|role| *role != TableRole::Cell);
            if (open_part.is_some() || (in_part && build.open[target].is_none()))
                && is_text
                && source_node
                    .text()
                    .is_some_and(|text| text.trim().is_empty())
            {
                continue;
            }
            let target = build.table_parent(target, index, None);
            let group = build.inline_group(target, index);
            slot[index] = Some(group);
            container[index] = Some(target);
            inline_element[index] = true;
            inline_parent[index] = parent.filter(|&outer| inline_element[outer]);
            inline_boxes[index] = inline_sides(index);
            if let Some(value) = pseudo(index, 0) {
                build.generated(&inline_rows, target, index, RunSource::Before(index), value);
            }
            if is_text {
                let group = build.inline_group(target, index);
                let blank = source_node
                    .text()
                    .is_some_and(|text| text.trim().is_empty());
                if inline_rows.contains(&group) {
                    // White space between inline-blocks generates nothing here; inside a
                    // piece of text it separates words as usual.
                    let in_piece = build.boxes[group].children.last().is_some_and(|last| {
                        build.anonymous[*last] && build.boxes[*last].table.is_none()
                    });
                    if !blank || in_piece {
                        let piece = build.row_text(group, index);
                        build.runs[piece].push(RunSource::Node(index));
                        slot[index] = Some(piece);
                    }
                } else {
                    build.runs[group].push(RunSource::Node(index));
                }
            }
            // 10.8: an inline element with no content but padding, a border or a margin
            // still makes a box. A zero-width run holds its place, for its sides to wrap.
            let has_sides = sided(index);
            // No content: no children, only white space, which collapses away (16.6.1), or
            // only inline elements with none either, no sides and no generated content.
            // Children follow their parent in projection order, the source's pre-order.
            let blank = || {
                let source_nodes = projection.snapshot.construction().nodes();
                let mut open = vec![index];
                while let Some(node) = open.pop() {
                    let node_original = projection.original[node];
                    let wanted = projection.nodes[node].children.len();
                    for at in (node + 1..projection.original.len())
                        .filter(|&at| {
                            source_nodes[projection.original[at]].parent() == Some(node_original)
                        })
                        .take(wanted)
                    {
                        if let Some(text) = source_nodes[projection.original[at]].text() {
                            // CSS white space only: a no-break space is content.
                            if !text.chars().all(|character| {
                                matches!(character, ' ' | '\t' | '\n' | '\r' | '\u{C}')
                            }) {
                                return false;
                            }
                            continue;
                        }
                        match display
                            .resolution(projection.original[at])
                            .map(|value| value.display)
                        {
                            Some(Display::None) => {}
                            // One with sides holds a place of its own: content here.
                            Some(Display::Inline)
                                if !sided(at)
                                    && pseudo(at, 0).is_none()
                                    && pseudo(at, 1).is_none() =>
                            {
                                open.push(at);
                            }
                            _ => return false,
                        }
                    }
                }
                true
            };
            if !is_text
                && has_sides
                && pseudo(index, 0).is_none()
                && pseudo(index, 1).is_none()
                && blank()
            {
                let group = build.inline_group(target, index);
                if inline_rows.contains(&group) {
                    let piece = build.row_text(group, index);
                    build.runs[piece].push(RunSource::Node(index));
                } else {
                    build.runs[group].push(RunSource::Node(index));
                }
            }
            ancestors.push(index);
            continue;
        }
        // 17.2: a `table-column` directly in a table, or in a column group directly in one,
        // gives its width to fixed tables and its background to its cells.
        if matches!(
            resolution.display,
            Display::TableColumn | Display::TableColumnGroup
        ) {
            let in_table = parent_slot
                .filter(|table| matches!(build.boxes[*table].table, Some(TableRole::Table { .. })));
            if resolution.display == Display::TableColumnGroup {
                group_table[index] = in_table;
                // Open while its columns are read, so they do not close its ancestors; it has
                // no box, so closing it generates nothing.
                if in_table.is_some() {
                    ancestors.push(index);
                    layer_borders.push((index, rows[index].border_widths()));
                }
            } else if let Some((table, group)) = in_table.map(|table| (table, None)).or_else(|| {
                parent.and_then(|group| group_table[group].map(|table| (table, Some(group))))
            }) {
                let width = flow_box(&rows[index], facts.numeric)
                    .map_err(|error| E::Percentage {
                        projected: index,
                        error,
                    })?
                    .width;
                build.boxes[table].column_widths.push(width);
                table_columns.push((table, index, group));
                layer_borders.push((index, rows[index].border_widths()));
            }
            continue;
        }
        let in_row = matches!(
            resolution.display,
            Display::InlineBlock | Display::InlineTable
        );
        let table_role = table_role_of(resolution.display);
        // 17.2.1: a table part that needs an anonymous table inside an inline element gets an
        // inline-table: it sits in a line of inline-blocks, as an `inline-table` does.
        let inline_table = parent.is_some_and(|parent| inline_element[parent])
            && table_role.is_some_and(|role| !matches!(role, TableRole::Table { .. }))
            && parent_slot.is_some_and(|target| {
                !matches!(
                    build.boxes[target].table,
                    Some(
                        TableRole::Table { .. }
                            | TableRole::HeaderGroup
                            | TableRole::BodyGroup
                            | TableRole::FooterGroup
                            | TableRole::Row
                    )
                )
            });
        // 17.2.1: a misplaced table part, or a non-table box in one, gets anonymous parts.
        let parent_slot = if inline_table {
            parent_slot
        } else {
            parent_slot.map(|target| build.table_parent(target, index, table_role))
        };
        // 9.2.2: inline-blocks in a run form lines of their own, and text beside them takes
        // a box in those lines. So a line never breaks between words of different pieces.
        if (in_row || inline_table)
            && let Some(target) = parent_slot
            && !build.open[target].is_some_and(|group| inline_rows.contains(&group))
        {
            // Blank text before the first inline-block generates nothing either; other text
            // moves into a piece of the line.
            let source_nodes = projection.snapshot.construction().nodes();
            let mut earlier = Vec::new();
            if let Some(group) = build.open[target] {
                let has_content = build.runs[group].iter().any(|run| match run {
                    // An element's own run is an empty inline's box: content too.
                    RunSource::Node(at) => source_nodes[projection.original[*at]]
                        .text()
                        .is_none_or(|text| !text.trim().is_empty()),
                    _ => true,
                });
                earlier = std::mem::take(&mut build.runs[group]);
                if !has_content {
                    earlier.clear();
                }
            }
            let group = build.inline_group(target, index);
            inline_rows.push(group);
            build.boxes[group].inline_row = Some(RowAlign::Left);
            if !earlier.is_empty() {
                // The piece belongs to its first text, not to the inline-block after it.
                let owner = match earlier[0] {
                    RunSource::Node(at) | RunSource::Before(at) | RunSource::After(at) => at,
                };
                let piece = build.row_text(group, owner);
                for run in &earlier {
                    if let RunSource::Node(at) = run {
                        slot[*at] = Some(piece);
                    }
                }
                build.runs[piece] = earlier;
            }
        }
        // The inline-table goes in the line, open there for the siblings after it; one that
        // another inline element opened ends.
        let parent_slot = if inline_table {
            parent_slot.map(|target| {
                let group = build.open[target]
                    .filter(|group| inline_rows.contains(group))
                    .unwrap_or(target);
                if let Some(open) = build.open[group]
                    && build.anonymous[open]
                    && build.boxes[open].table.is_some()
                    && source_parent[build.positions[open].0] != parent
                {
                    build.open[group] = None;
                }
                build.table_parent(group, index, table_role)
            })
        } else {
            parent_slot
        };
        if parent.is_some()
            && table_role.is_none()
            && !in_row
            && !matches!(resolution.display, Display::Block | Display::ListItem)
        {
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
        if let Some(role) = table_role {
            value.table = Some(role);
            if matches!(role, TableRole::Cell) {
                // 17.5: spans come from the document, here as source attributes.
                // Unparsable is 1; rowspan 0 means to the group end, as in HTML; too much
                // is clamped by the layout.
                let span = |name: &str, zero: usize| match source_node
                    .metadata()
                    .attribute(name)
                    .and_then(|text| text.trim().parse::<usize>().ok())
                {
                    Some(0) => zero,
                    Some(span) => span,
                    None => 1,
                };
                value.col_span = span("colspan", 1);
                value.row_span = span("rowspan", usize::MAX);
            }

            match role {
                TableRole::Table { .. } | TableRole::Cell | TableRole::Caption => {
                    value.establishes_bfc = true;
                    // 17.5: margins do not apply to cells; a table keeps its own.
                    if matches!(role, TableRole::Cell) {
                        value.margin = [Dim::Px(0.0); 4];
                    }
                }
                // 17.5.1 layers paint backgrounds only; nothing else applies to them, but
                // for their borders in a collapsed table (17.6.2), which its cells draw.
                _ => {
                    value.margin = [Dim::Px(0.0); 4];
                    value.padding = [Dim::Px(0.0); 4];
                    layer_borders.push((index, value.border));
                    value.border = [0.0; 4];
                }
            }
        }
        let arena = build.boxes.len();
        slot[index] = Some(arena);
        container[index] = Some(arena);
        build.anonymous.push(false);
        build
            .clipped
            .push(parent.is_some() && rows[index].overflow.value != BoxOverflow::Visible);
        // 11.1.1: the root's `overflow` applies to the viewport, where `visible` means `auto`.
        build.scrolls.push(match rows[index].overflow.value {
            BoxOverflow::Auto | BoxOverflow::Scroll => true,
            BoxOverflow::Visible => parent.is_none(),
            BoxOverflow::Hidden => false,
        });
        build.runs.push(Vec::new());
        build.open.push(None);
        let is_float = value.float.is_some();
        let in_flow = !is_float && !value.out_of_flow;
        build.boxes.push(value);
        if let Some(target) = parent_slot {
            if in_row
                && let Some(group) = build.open[target].filter(|open| inline_rows.contains(open))
            {
                // Inline-blocks establish a context; floats and absolutes stay as they are.
                build.boxes[arena].establishes_bfc = true;
                build.boxes[group].children.push(arena);
            } else {
                match build.open[target] {
                    Some(group) if is_float && build.boxes[group].table.is_none() => {
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
        }
        // A text leaf holds its runs directly, so it stays a leaf.
        text_block[index] = is_text;
        if let Some(value) = pseudo(index, 0) {
            if is_text {
                build.runs[arena].push(RunSource::Before(index));
            } else {
                let target = build.table_parent(arena, index, None);
                build.generated(&inline_rows, target, index, RunSource::Before(index), value);
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
        scrolls,
        runs,
        open: _,
        generated_boxes,
    } = build;
    layer_borders.retain(|entry| entry.1 != [0.0; 4]);
    layer_borders.sort_unstable_by_key(|entry| entry.0);
    Ok(PreparedTree {
        boxes,
        offsets,
        positions,
        slot,
        font_sizes,
        anonymous,
        runs,
        inline_parent,
        inline_boxes,
        table_columns,
        generated_boxes,
        border_cuts: Vec::new(),
        thin_sides: Vec::new(),
        collapsed_losers: Vec::new(),
        table_borders: Vec::new(),
        layer_borders,
        row_indents: Vec::new(),
        clipped,
        scrolls,
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
        Ok(self.by_projected(&self.layout_slots(
            viewport,
            limits,
            text_height,
            &mut |_, _, _, _| None,
        )?))
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
        text_split: &mut dyn FnMut(usize, f32, f32, f32) -> Option<f32>,
    ) -> Result<Vec<FlowGeometry>, TreeLayoutError> {
        self.layout_slots_baselines(viewport, limits, text_height, text_split, &mut |_, _, _| {
            None
        })
    }

    /// Lays the tree out as [`Self::layout_slots`], asking `text_baseline(slot, width, split)` for
    /// a leaf's last line's baseline, which rows of inline-blocks line up on.
    ///
    /// # Errors
    /// Returns flow and absolute placement errors.
    pub fn layout_slots_baselines(
        &self,
        viewport: FlowViewport,
        limits: FlowLimits,
        text_height: &mut dyn FnMut(usize, f32) -> Option<f32>,
        text_split: &mut dyn FnMut(usize, f32, f32, f32) -> Option<f32>,
        text_baseline: &mut dyn FnMut(usize, f32, Option<(f32, f32)>) -> Option<f32>,
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
            inline_boxes: _,
            table_columns: _,
            generated_boxes: _,
            border_cuts: _,
            thin_sides: _,
            collapsed_losers: _,
            table_borders: _,
            layer_borders: _,
            row_indents: _,
            clipped: _,
            scrolls: _,
        } = self;
        let _ = slot;
        if boxes.is_empty() {
            return Ok(Vec::new());
        }
        // `box-sizing: border-box` sizes as content sizes, for flow and absolute placement.
        let converted = content_boxes(boxes);
        let boxes = &*converted;
        let mut geometry = layout_block_flow_baselines(
            boxes,
            viewport,
            limits,
            text_height,
            text_split,
            text_baseline,
        )
        .map_err(E::Flow)?;
        // Arena order is preorder, so every box's ancestors have settled before it. A
        // relative offset moves a subtree after flow; floats and later flow do not react.
        // Parents and offsets by arena slot, so the pass stays linear in the tree.
        let mut parents = vec![None; boxes.len()];
        for (arena, value) in boxes.iter().enumerate() {
            for &child in &value.children {
                parents[child] = Some(arena);
            }
        }
        let mut offset_of = vec![[Dim::Auto; 4]; boxes.len()];
        for &(at, offset) in offsets {
            offset_of[at] = offset;
        }
        for arena in 0..boxes.len() {
            let (index, position) = positions[arena];
            let offset = offset_of[arena];
            match position {
                Position::Relative => {
                    let basis = parent_content(boxes, &parents, &geometry, arena, viewport);
                    let direction = parents[arena]
                        .map_or(boxes[arena].direction, |parent| boxes[parent].direction);
                    let (dx, dy) = relative_shift(direction, offset, basis);
                    shift(boxes, &mut geometry, arena, dx, dy);
                }
                Position::Absolute | Position::Fixed => {
                    place_absolute(
                        boxes,
                        &parents,
                        &mut geometry,
                        positions,
                        arena,
                        position,
                        offset,
                        viewport,
                        limits,
                        text_height,
                        text_split,
                        text_baseline,
                    )
                    .map_err(|error| error.at(index))?;
                }
                Position::Static => {}
            }
        }
        // A relative offset or an absolute box's percentages can overflow what flow kept
        // finite.
        if let Some(node) = geometry.iter().position(|item| !item.is_finite()) {
            return Err(E::Flow(FlowError::Nonfinite { node }));
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

/// 10.1: the padding box of the nearest positioned ancestor, or the initial block, and
/// whether it is the initial block.
fn containing_block(
    boxes: &[FlowBox],
    parents: &[Option<usize>],
    geometry: &[FlowGeometry],
    positions: &[(usize, Position)],
    arena: usize,
    position: Position,
    viewport: FlowViewport,
) -> (FlowRect, Direction, bool) {
    let initial = FlowRect {
        left: viewport.left,
        right: viewport.left + viewport.width,
        top: viewport.top,
        bottom: viewport.top + viewport.height.unwrap_or(0.0),
    };
    if position == Position::Fixed {
        return (initial, boxes[0].direction, true);
    }
    let mut at = parents[arena];
    while let Some(ancestor) = at {
        if positions[ancestor].1 != Position::Static {
            return (geometry[ancestor].padding, boxes[ancestor].direction, false);
        }
        at = parents[ancestor];
    }
    (initial, boxes[0].direction, true)
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
    parents: &[Option<usize>],
    geometry: &mut [FlowGeometry],
    positions: &[(usize, Position)],
    arena: usize,
    position: Position,
    offset: [Dim; 4],
    viewport: FlowViewport,
    limits: FlowLimits,
    text_height: &mut dyn FnMut(usize, f32) -> Option<f32>,
    text_split: &mut dyn FnMut(usize, f32, f32, f32) -> Option<f32>,
    text_baseline: &mut dyn FnMut(usize, f32, Option<(f32, f32)>) -> Option<f32>,
) -> Result<(), AbsoluteFailure> {
    let value = &boxes[arena];
    let (block, direction, initial) = containing_block(
        boxes, parents, geometry, positions, arena, position, viewport,
    );
    // An initial block of unknown height has no height to take a share of or to measure
    // `bottom` from: percentage heights and `bottom` are auto, as is a percentage `top`.
    let indefinite = initial && viewport.height.is_none();
    let indefinite_auto = |dim: Dim| match dim {
        Dim::Percent(_) | Dim::Mix(..) if indefinite => Dim::Auto,
        Dim::Clamped(_, percent) if indefinite && percent != 0.0 => Dim::Auto,
        other => other,
    };
    let offset = [
        indefinite_auto(offset[0]),
        offset[1],
        if indefinite { Dim::Auto } else { offset[2] },
        offset[3],
    ];
    let cb_width = block.right - block.left;
    let cb_height = block.bottom - block.top;
    let auto = |dim: Dim, basis: f32| match dim {
        Dim::Auto => AutoLength::Auto,
        Dim::Px(number) => AutoLength::Value(number),
        Dim::Percent(number) => AutoLength::Value(basis * number / 100.0),
        Dim::Mix(pixels, percent) => AutoLength::Value(pixels + basis * percent / 100.0),
        Dim::Clamped(pixels, percent) => {
            AutoLength::Value((pixels + basis * percent / 100.0).max(0.0))
        }
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
        static_direction: parents[arena].map_or(value.direction, |parent| boxes[parent].direction),
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
    // height and again at the used height. That height is a definite basis for
    // percentages only when it does not depend on content (10.5): a specified height, or
    // `top` and `bottom` both set; otherwise min and max pin it and percentages stay auto.
    let nodes = subtree(boxes, arena);
    let locals: std::collections::HashMap<usize, usize> = nodes
        .iter()
        .enumerate()
        .map(|(local, &node)| (node, local))
        .collect();
    let local = |node: usize| locals.get(&node).copied();
    let mut solve = |height: Dim, pinned: Dim| -> Result<Vec<FlowGeometry>, FlowError> {
        let mut arena_copy: Vec<FlowBox> = nodes.iter().map(|&node| boxes[node].clone()).collect();
        for item in &mut arena_copy {
            item.children = item.children.iter().filter_map(|&c| local(c)).collect();
        }
        let root = &mut arena_copy[0];
        root.out_of_flow = false;
        // 17.5.2.1: only a table whose own width is not auto lays out as fixed.
        root.fixed_table = root.fixed_table && root.width != Dim::Auto;
        root.width = Dim::Px(width.width);
        root.min_width = Dim::Auto;
        root.max_width = Dim::Auto;
        root.height = height;
        root.min_height = pinned;
        root.max_height = pinned;
        root.margin = [Dim::Px(0.0); 4];
        // The percentages' basis is the containing block, not this synthetic viewport.
        root.padding = padding.map(Dim::Px);
        layout_block_flow_baselines(
            &arena_copy,
            FlowViewport {
                left: 0.0,
                top: 0.0,
                width: padding[3] + padding[1] + value.border[3] + value.border[1] + width.width,
                height: Some(cb_height),
            },
            limits,
            &mut |local_node, used| text_height(nodes[local_node], used),
            &mut |local_node, narrow, until, wide| {
                text_split(nodes[local_node], narrow, until, wide)
            },
            &mut |local_node, used, split| text_baseline(nodes[local_node], used, split),
        )
    };
    let specified = indefinite_auto(value.height);
    let first = solve(Dim::Auto, Dim::Auto).map_err(AbsoluteFailure::Flow)?;
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
        min_height: zero(indefinite_auto(value.min_height), cb_height),
        max_height: match indefinite_auto(value.max_height) {
            Dim::Auto => None,
            other => Some(zero(other, cb_height)),
        },
    })
    .map_err(AbsoluteFailure::Height)?;
    let definite = specified != Dim::Auto || (offset[0] != Dim::Auto && offset[2] != Dim::Auto);
    let second = if definite {
        solve(Dim::Px(height.height), Dim::Auto)
    } else {
        solve(Dim::Auto, Dim::Px(height.height))
    }
    .map_err(AbsoluteFailure::Flow)?;
    let dx = block.left + width.left + width.margin_left;
    let dy = block.top + height.top + height.margin_top;
    for (index, &node) in nodes.iter().enumerate() {
        let mut placed = second[index];
        placed.move_by(dx, dy);
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

/// Inline generated text beside inline-blocks needs atomic inlines in a paragraph.
fn table_role_of(display: Display) -> Option<TableRole> {
    Some(match display {
        Display::Table | Display::InlineTable => TableRole::Table {
            spacing: [0.0, 0.0],
        },
        Display::TableHeaderGroup => TableRole::HeaderGroup,
        Display::TableRowGroup => TableRole::BodyGroup,
        Display::TableFooterGroup => TableRole::FooterGroup,
        Display::TableRow => TableRole::Row,
        Display::TableCell => TableRole::Cell,
        Display::TableCaption => TableRole::Caption,
        _ => return None,
    })
}

fn parent_content(
    boxes: &[FlowBox],
    parents: &[Option<usize>],
    geometry: &[FlowGeometry],
    arena: usize,
    viewport: FlowViewport,
) -> (f32, Option<f32>) {
    parents[arena].map_or((viewport.width, viewport.height), |parent| {
        let content: FlowRect = geometry[parent].content;
        // 9.3.2: a percentage of an auto-height containing block computes to auto.
        let height = containing_height(boxes, parents, geometry, arena, viewport);
        (content.right - content.left, height)
    })
}

/// The height of `arena`'s containing block when it is definite, as core decides it: a
/// line of inline-blocks hands its block's containing block through, a cell or caption
/// lays out against none, and a percentage of an indefinite basis is auto (10.5).
fn containing_height(
    boxes: &[FlowBox],
    parents: &[Option<usize>],
    geometry: &[FlowGeometry],
    arena: usize,
    viewport: FlowViewport,
) -> Option<f32> {
    let Some(parent) = parents[arena] else {
        return viewport.height;
    };
    let value = &boxes[parent];
    if value.inline_row.is_some() {
        return containing_height(boxes, parents, geometry, parent, viewport);
    }
    if value.out_of_flow {
        let content: FlowRect = geometry[parent].content;
        return (value.height != Dim::Auto).then_some(content.bottom - content.top);
    }
    let basis = if matches!(value.table, Some(TableRole::Cell | TableRole::Caption)) {
        None
    } else {
        containing_height(boxes, parents, geometry, parent, viewport)
    };
    let vertical = |dim: Dim| match dim {
        Dim::Auto => None,
        Dim::Px(number) => Some(number),
        Dim::Percent(number) => basis.map(|height| height * number / 100.0),
        Dim::Mix(pixels, percent) => basis.map(|height| pixels + height * percent / 100.0),
        Dim::Clamped(pixels, percent) if percent == 0.0 => Some(pixels.max(0.0)),
        Dim::Clamped(pixels, percent) => {
            basis.map(|height| (pixels + height * percent / 100.0).max(0.0))
        }
    };
    let height = vertical(value.height)?;
    let capped = vertical(value.max_height).map_or(height, |limit| height.min(limit));
    Some(capped.max(vertical(value.min_height).unwrap_or(0.0)))
}

fn relative_shift(direction: Direction, offset: [Dim; 4], basis: (f32, Option<f32>)) -> (f32, f32) {
    let horizontal = |dim: Dim| match dim {
        Dim::Auto => None,
        Dim::Px(number) => Some(number),
        Dim::Percent(number) => Some(basis.0 * number / 100.0),
        Dim::Mix(pixels, percent) => Some(pixels + basis.0 * percent / 100.0),
        Dim::Clamped(pixels, percent) => Some((pixels + basis.0 * percent / 100.0).max(0.0)),
    };
    let vertical = |dim: Dim| match dim {
        Dim::Auto => None,
        Dim::Px(number) => Some(number),
        Dim::Percent(number) => basis.1.map(|height| height * number / 100.0),
        Dim::Mix(pixels, percent) => basis.1.map(|height| pixels + height * percent / 100.0),
        Dim::Clamped(pixels, percent) => basis
            .1
            .map(|height| (pixels + height * percent / 100.0).max(0.0)),
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
    geometry[node].move_by(dx, dy);
    for &child in &boxes[node].children {
        shift(boxes, geometry, child, dx, dy);
    }
}

fn dim(fact: BoxLengthFact, numeric: NumericLimits) -> Result<Dim, ConversionError> {
    Ok(match fact {
        BoxLengthFact::Absolute(value) => Dim::Px(value.pixels),
        BoxLengthFact::Auto { .. } => Dim::Auto,
        BoxLengthFact::Percentage(value) => percent_dim(value, numeric)?,
    })
}

/// A percentage fact: with pixels beside it, a `calc()` sum as layout mixes them.
fn percent_dim(
    fact: crate::evaluation::clearance_box_facts::PercentageBoxFact,
    numeric: NumericLimits,
) -> Result<Dim, ConversionError> {
    let percent = fact.value.percentage_finite_f32(100.0, numeric)?.value;
    Ok(if fact.offset == 0.0 {
        Dim::Percent(percent)
    } else {
        Dim::Mix(fact.offset, percent)
    })
}

/// A `calc()` sum against a font size: pixels, or pixels with its percentage.
fn calc_dim(sum: iced_css_core::values::CalcLength, font_size: f32, root_font_size: f32) -> Dim {
    let pixels = sum.fallback_pixels(font_size, root_font_size);
    if sum.percent() == 0.0 {
        Dim::Px(pixels)
    } else {
        Dim::Mix(pixels, sum.percent())
    }
}

/// A size or padding that may not be negative: a `calc()` sum clips to nothing.
fn never_negative(dim: Dim) -> Dim {
    match dim {
        Dim::Mix(pixels, percent) => Dim::Clamped(pixels, percent),
        other => other,
    }
}

fn length_dim(
    fact: LengthPercentageBoxFact,
    numeric: NumericLimits,
) -> Result<Dim, ConversionError> {
    Ok(match fact {
        LengthPercentageBoxFact::Absolute(value) => Dim::Px(value.pixels),
        LengthPercentageBoxFact::Percentage(value) => percent_dim(value, numeric)?,
    })
}

fn max_dim(fact: BoxMaxFact, numeric: NumericLimits) -> Result<Dim, ConversionError> {
    Ok(match fact {
        BoxMaxFact::Absolute(value) => Dim::Px(value.pixels),
        BoxMaxFact::None { .. } => Dim::Auto,
        BoxMaxFact::Percentage(value) => percent_dim(value, numeric)?,
    })
}

fn flow_box(row: &ClearanceBoxFact, numeric: NumericLimits) -> Result<FlowBox, ConversionError> {
    let side = |index: usize| dim(row.margin[index], numeric);
    let pad = |index: usize| length_dim(row.padding[index], numeric).map(never_negative);
    Ok(FlowBox {
        clear: row.clear.value,
        establishes_bfc: row.overflow.value != BoxOverflow::Visible,
        width: never_negative(dim(row.width, numeric)?),
        min_width: never_negative(length_dim(row.min_width, numeric)?),
        max_width: never_negative(max_dim(row.max_width, numeric)?),
        height: never_negative(dim(row.height, numeric)?),
        min_height: never_negative(length_dim(row.min_height, numeric)?),
        max_height: never_negative(max_dim(row.max_height, numeric)?),
        margin: [side(0)?, side(1)?, side(2)?, side(3)?],
        padding: [pad(0)?, pad(1)?, pad(2)?, pad(3)?],
        border: row.border_widths(),
        ..FlowBox::default()
    })
}

/// One outer shadow of a box: offset right and down, blur radius, color and opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadow {
    pub offset: [f32; 2],
    pub blur: f32,
    pub color: [f32; 3],
    pub alpha: f32,
}

/// Which way a background gradient runs (CSS Images 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GradientAim {
    /// Radians, clockwise from up.
    Angle(f32),
    /// `to` a side or corner: across (-1 left, 1 right), down (-1 top, 1 bottom).
    Toward(i8, i8),
}

/// A `linear-gradient()` background, its colors computed.
#[derive(Debug, Clone, PartialEq)]
pub struct GradientPaint {
    pub aim: GradientAim,
    /// Color, opacity, and where it sits along the gradient line, when given.
    pub stops: Vec<([f32; 3], f32, Option<Dim>)>,
}

/// An easing function as the widget runs it.
pub fn css_easing(
    easing: &EasingValue,
    numeric: NumericLimits,
) -> crate::animation::easing::CssEasing {
    use crate::animation::easing::{CssEasing, StepPosition};
    let number = |value: &iced_css_core::values::Decimal| {
        value
            .scaled_finite_f32(1.0, numeric)
            .map_or(0.0, |converted| converted.value)
    };
    match easing {
        EasingValue::Linear => CssEasing::Linear,
        EasingValue::CubicBezier(points) => CssEasing::CubicBezier(points.each_ref().map(number)),
        EasingValue::Steps(count, position) => CssEasing::Steps(
            *count,
            match position {
                StepValue::JumpStart => StepPosition::JumpStart,
                StepValue::JumpEnd => StepPosition::JumpEnd,
                StepValue::JumpNone => StepPosition::JumpNone,
                StepValue::JumpBoth => StepPosition::JumpBoth,
            },
        ),
    }
}

/// Which way an animation's iterations run (`animation-direction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

/// One animation an element's `animation-*` lists ask for (CSS Animations 1).
#[derive(Debug, Clone, PartialEq)]
pub struct ElementAnimation {
    /// The `@keyframes` it runs.
    pub name: String,
    /// Milliseconds of one iteration, never negative.
    pub duration: f32,
    /// Milliseconds; a negative delay starts partway.
    pub delay: f32,
    pub easing: crate::animation::easing::CssEasing,
    /// How many iterations; infinite is `f32::INFINITY`.
    pub iterations: f32,
    pub direction: AnimationDirection,
    /// `animation-fill-mode`: shows the first keyframe during the delay.
    pub fill_backwards: bool,
    /// Holds the last keyframe once done.
    pub fill_forwards: bool,
    pub paused: bool,
}

/// One transition an element's `transition-*` lists ask for (CSS Transitions 1).
#[derive(Debug, Clone, PartialEq)]
pub struct ElementTransition {
    /// The longhands it moves; `None` is `all`.
    pub properties: Option<Vec<PropertyId>>,
    /// Milliseconds, never negative.
    pub duration: f32,
    /// Milliseconds; a negative delay starts partway.
    pub delay: f32,
    pub easing: crate::animation::easing::CssEasing,
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
    /// The background's opacity while an animation fades it; `None` is opaque.
    pub background_fade: Option<f32>,
    /// An inline element's background, which its text paints instead of a box.
    pub under_text: Option<[f32; 3]>,
    /// An inline element's border colors, which its line's sides paint instead of a box.
    pub under_text_border: [Option<[f32; 3]>; 4],
    /// The computed `cursor` keyword; a URL list uses its fallback.
    pub cursor: &'static str,
    /// `clip: rect()` offsets from the border box's top left: top, right, bottom, left, with
    /// `None` for auto. It only matters on absolutely positioned boxes.
    pub clip: Option<[Option<f32>; 4]>,
    /// The arena slot of the collapsed table this cell belongs to: it draws its borders after
    /// the table's cell backgrounds, which would otherwise cover the shared edges.
    pub collapsed_cell: Option<usize>,
    /// `empty-cells: hide`.
    pub hide_empty: bool,
    /// A cell `empty-cells: hide` keeps from drawing, unless it hosts a widget.
    pub hidden_empty: bool,
    /// `table-layout: fixed`.
    pub fixed_table: bool,
    /// `box-sizing: border-box`.
    pub border_box: bool,
    /// `background-origin`: what its gradient is placed against.
    pub background_origin: BoxArea,
    /// `background-clip`: where its background color and image show.
    pub background_clip: BoxArea,
    /// `display: flex` or `inline-flex`: how its children are laid out as flex items.
    pub flex: Option<FlexContainer>,
    /// What the box is as a flex item, should its parent be a flex container.
    pub flex_item: FlexItem,
    /// `box-shadow`: the first outer shadow of the list; `None` draws none.
    pub shadow: Option<BoxShadow>,
    /// `opacity` below 1, multiplied through the box's ancestors; `None` is opaque.
    pub opacity: Option<f32>,
    /// The box's own computed `opacity`, for a child's `inherit`.
    pub own_opacity: Option<f32>,
    /// `border-radius`: each corner's radius as pixels plus a percent of the border box's
    /// shorter side, from the top left clockwise. Only a corner's first radius is read.
    pub radius: [[f32; 2]; 4],
    /// Top, right, bottom, left.
    pub border: [Option<[f32; 3]>; 4],
    /// Each border color's opacity while an animation fades it; `None` is opaque.
    pub border_fade: [Option<f32>; 4],
    /// The `border-*-style` keywords, in that order.
    pub border_style: [&'static str; 4],
    /// The `outline-style` keyword.
    pub outline_style: &'static str,
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
        Color::SrgbA([r, g, b, _]) => Some([*r, *g, *b].map(|value| f32::from(value) / 255.0)),
        // A system palette and outline invert have no fixed value here; `currentColor` is
        // resolved before paint.
        Color::Transparent | Color::Invert | Color::System(_) | Color::CurrentColor => None,
    }
}

/// A color's opacity below 1, as a fade; `None` is opaque (or not a color that draws).
fn opacity(color: &Color) -> Option<f32> {
    match color {
        Color::SrgbA([_, _, _, alpha]) => Some(f32::from(*alpha) / 255.0),
        _ => None,
    }
}

static BLACK: Color = Color::Srgb([0, 0, 0]);

/// One `background-position` axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackgroundOffset {
    Px(f32),
    /// Of the room left after the image: 14.2.1's percentage rule.
    Percent(f32),
}

/// One of a box's three areas: `background-origin` and `background-clip` (CSS
/// Backgrounds 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxArea {
    #[default]
    Border,
    Padding,
    Content,
}

/// `background-size`: `None` is `auto`; a percentage is of the positioning area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackgroundSize {
    Cover,
    Contain,
    Sized(Option<BackgroundOffset>, Option<BackgroundOffset>),
}

/// A box's background image (14.2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct BackgroundImage {
    /// CSS Backgrounds 3: its size.
    pub size: BackgroundSize,
    /// What it is placed against.
    pub origin: BoxArea,
    /// Where it shows.
    pub clip: BoxArea,
    pub uri: String,
    pub repeat_x: bool,
    pub repeat_y: bool,
    pub position: [BackgroundOffset; 2],
    /// `background-attachment: fixed`: positioned against the viewport.
    pub fixed: bool,
}

/// The computed repeat, position and attachment, kept for `inherit`.
type BackgroundParts = (&'static str, [BackgroundOffset; 2], bool);

/// Everything `evaluate_tree_paint` computes per projected row.
pub struct TreePaint {
    pub paints: Vec<BoxPaint>,
    pub styles: Vec<TextStyle>,
    pub generated: Vec<[Option<GeneratedRun>; 2]>,
    /// The computed `list-style-image` per row; an inside marker stays text.
    pub marker_images: Vec<Option<String>>,
    /// Whether the row is a list item with an outside marker, whatever its text.
    pub outside_items: Vec<bool>,
    /// The outside list marker's text per row (12.5.1); it takes no room in the flow.
    pub markers: Vec<Option<String>>,
    /// The computed `border-spacing` per row, zero under `border-collapse: collapse`.
    pub spacings: Vec<[f32; 2]>,
    /// The computed background image per row.
    pub backgrounds: Vec<Option<BackgroundImage>>,
    /// Whether `border-collapse` is `collapse`, per row.
    pub collapses: Vec<bool>,
    /// The computed `vertical-align` keyword per row, for cells.
    pub vertical_aligns: Vec<Option<&'static str>>,
    /// Whether `caption-side` is `bottom`, per row.
    pub captions_below: Vec<bool>,
    /// The longhands whose winning declaration is `!important`, per projected index.
    pub important: Vec<Vec<PropertyId>>,
    /// Length longhands given in `em` or `ex`, or inherited from a parent's that were, per
    /// projected index: their pixels follow the font size.
    pub em_lengths: Vec<Vec<PropertyId>>,
    /// `display: grid` or `inline-grid`: how the row's children are laid out as grid items.
    pub grids: Vec<Option<Box<GridContainer>>>,
    /// Where the row goes as a grid item; `None` is placed automatically.
    pub grid_items: Vec<Option<Box<GridItem>>>,
    /// The transitions each row asks for; empty when none can run.
    pub transitions: Vec<Vec<ElementTransition>>,
    /// The animations each row asks for, by name; empty when none.
    pub animations: Vec<Vec<ElementAnimation>>,
    /// Each row's background gradient, if its `background-image` is one.
    pub gradients: Vec<Option<std::sync::Arc<GradientPaint>>>,
}

/// 12.5.1: the marker text of list item `ordinal` (from 1), as inside-marker text.
fn list_marker(kind: &str, ordinal: usize) -> Option<String> {
    let value = i32::try_from(ordinal).unwrap_or(i32::MAX);
    match kind {
        "none" => None,
        "disc" | "circle" | "square" => Some(format!("{}\u{a0}", counter_label(kind, value))),
        _ => Some(format!("{}.\u{a0}", counter_label(kind, value))),
    }
}

/// 12.4.1, 12.5.1: a counter value in a list style, as core's `format_counter` writes it.
/// Unknown styles, and values a style cannot represent, fall back to decimal.
fn counter_label(kind: &str, value: i32) -> String {
    use iced_css_core::values::{
        CounterLimits, CounterStyle, Integer, NumericLimits, format_counter,
    };
    let numeric = NumericLimits {
        digits: NonZeroUsize::new(16).expect("nonzero"),
        work: NonZeroUsize::new(4096).expect("nonzero"),
    };
    let limits = CounterLimits {
        numeric,
        retained_bytes: NonZeroUsize::new(1024).expect("nonzero"),
    };
    let style = CounterStyle::parse(kind).unwrap_or(CounterStyle::Decimal);
    Integer::parse(&value.to_string(), numeric)
        .ok()
        .and_then(|integer| format_counter(&integer, style, limits).ok())
        .unwrap_or_else(|| value.to_string())
}

/// What 12.4 needs to know about one element or pseudo-element.
#[derive(Default, Clone)]
struct CounterActions {
    reset: Vec<(String, i32)>,
    increment: Vec<(String, i32)>,
}

/// One pending `content` value, and the marker text that precedes it.
#[derive(Clone)]
struct PendingContent {
    marker: Option<String>,
    items: Vec<ContentItem>,
    actions: CounterActions,
    /// The computed `quotes` pairs; `None` is the initial value's curly marks.
    quotes: Option<Vec<[String; 2]>>,
}

/// Document-order generated text: 12.4's counter scopes and 12.3.2's quote depth. A
/// scope belongs to the parent of its resetting element, so it reaches later siblings and
/// their descendants; a pseudo-element's belongs to its element.
struct CounterWalk<'a> {
    children: &'a [Vec<usize>],
    parents: &'a [Option<usize>],
    /// Elements that generate boxes; `display: none` affects no counter (12.4.3).
    rendered: &'a [bool],
    actions: &'a [CounterActions],
    pending: &'a [[Option<PendingContent>; 2]],
    attribute: &'a dyn Fn(usize, &str) -> Option<String>,
    scopes: Vec<Vec<(String, i32)>>,
    quotes: usize,
    out: Vec<[Option<String>; 2]>,
}

impl CounterWalk<'_> {
    /// The scope holders in reach of `at`, innermost first. The last slot belongs to the
    /// root's own resets, whose scope encloses everything its children open.
    fn chain(&self, at: usize) -> Vec<usize> {
        let mut holders = Vec::new();
        let mut here = Some(at);
        while let Some(node) = here {
            holders.push(node);
            here = self.parents[node];
        }
        holders.push(self.parents.len());
        holders
    }

    /// The nearest scope with this name, from `at` outward.
    fn find(&mut self, at: usize, name: &str) -> Option<&mut i32> {
        let holder = self
            .chain(at)
            .into_iter()
            .find(|node| self.scopes[*node].iter().any(|(owned, _)| owned == name))?;
        self.scopes[holder]
            .iter_mut()
            .rev()
            .find(|(owned, _)| owned == name)
            .map(|(_, value)| value)
    }

    fn apply(&mut self, holder: usize, reader: usize, actions: &CounterActions) {
        for (name, value) in &actions.reset {
            let scope = &mut self.scopes[holder];
            match scope.iter_mut().find(|(owned, _)| owned == name) {
                Some(entry) => entry.1 = *value,
                None => scope.push((name.clone(), *value)),
            }
        }
        for (name, amount) in &actions.increment {
            match self.find(reader, name) {
                Some(value) => *value = value.saturating_add(*amount),
                // 12.4: an increment with no counter in scope resets it to 0 first.
                None => self.scopes[holder].push((name.clone(), *amount)),
            }
        }
    }

    fn content(&mut self, at: usize, pending: &PendingContent) -> String {
        self.apply(at, at, &pending.actions);
        let mut text = pending.marker.clone().unwrap_or_default();
        for item in &pending.items {
            match item {
                ContentItem::Text(value) => text.push_str(value),
                ContentItem::Attribute(name) => {
                    if let Some(value) = (self.attribute)(at, name) {
                        text.push_str(&value);
                    }
                }
                // 12.3.2: the depth picks the pair, its last for anything deeper. A close
                // with nothing open is ignored and prints nothing.
                ContentItem::Quote(keyword) => {
                    let default = [
                        ["\u{201c}".to_owned(), "\u{201d}".to_owned()],
                        ["\u{2018}".to_owned(), "\u{2019}".to_owned()],
                    ];
                    let pairs = pending.quotes.as_deref().unwrap_or(&default);
                    let mark = |depth: usize, side: usize| {
                        pairs
                            .get(depth)
                            .or(pairs.last())
                            .map(|pair| pair[side].clone())
                            .unwrap_or_default()
                    };
                    match keyword.as_str() {
                        "open-quote" => {
                            text.push_str(&mark(self.quotes, 0));
                            self.quotes += 1;
                        }
                        "close-quote" if self.quotes > 0 => {
                            self.quotes -= 1;
                            text.push_str(&mark(self.quotes, 1));
                        }
                        "no-open-quote" => self.quotes += 1,
                        "no-close-quote" => self.quotes = self.quotes.saturating_sub(1),
                        _ => {}
                    }
                }
                ContentItem::Uri(_) => {}
                ContentItem::Counter {
                    name,
                    separator,
                    style,
                } => {
                    let kind = style.as_str();
                    match separator {
                        None => {
                            // 12.4: reading a counter out of scope instantiates it here.
                            if self.find(at, name).is_none() {
                                self.scopes[at].push((name.clone(), 0));
                            }
                            let value = self.find(at, name).map_or(0, |value| *value);
                            text.push_str(&counter_label(kind, value));
                        }
                        Some(separator) => {
                            // counters(): every scope in reach, outermost first.
                            if self.find(at, name).is_none() {
                                self.scopes[at].push((name.clone(), 0));
                            }
                            let mut chain: Vec<i32> = self
                                .chain(at)
                                .into_iter()
                                .filter_map(|node| {
                                    self.scopes[node]
                                        .iter()
                                        .rev()
                                        .find(|(owned, _)| owned == name)
                                        .map(|entry| entry.1)
                                })
                                .collect();
                            if chain.is_empty() {
                                chain.push(0);
                            }
                            chain.reverse();
                            let labels: Vec<_> = chain
                                .iter()
                                .map(|value| counter_label(kind, *value))
                                .collect();
                            text.push_str(&labels.join(separator));
                        }
                    }
                }
            }
        }
        text
    }

    fn walk(&mut self, at: usize) {
        if !self.rendered[at] {
            return;
        }
        // The root's own resets sit in the slot past the last row.
        let holder = self.parents[at].unwrap_or(self.parents.len());
        let actions = self.actions;
        self.apply(holder, at, &actions[at]);
        let pending = self.pending;
        if let Some(before) = &pending[at][0] {
            self.out[at][0] = Some(self.content(at, before));
        }
        let children = self.children;
        for &child in &children[at] {
            self.walk(child);
        }
        if let Some(after) = &pending[at][1] {
            self.out[at][1] = Some(self.content(at, after));
        }
    }
}

/// The length longhands an animated `font-size` can move when given in `em` or `ex`.
const EM_LENGTHS: [&str; 24] = [
    "width",
    "min-width",
    "max-width",
    "height",
    "min-height",
    "max-height",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "top",
    "right",
    "bottom",
    "left",
    "outline-width",
    "text-indent",
];

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
    /// `font-variant: small-caps`.
    pub small_caps: bool,
    /// `word-spacing` in pixels past the normal space; negative values are ignored.
    pub word_spacing: f32,
    /// `text-indent` in pixels; a percentage is 0 here and in `text_indent_percent`.
    pub text_indent: f32,
    /// A percentage `text-indent`, of the width the block's text lays out in.
    pub text_indent_percent: f32,
    /// 100 to 900.
    pub weight: u16,
    pub italic: bool,
    pub line_height: TextLineHeight,
    /// The first listed family; a generic is its keyword.
    pub family: Option<String>,
    pub underline: bool,
    pub line_through: bool,
    /// 16.3.1: the row whose `text-decoration` draws the underline, when an ancestor's:
    /// its color and font size draw the line. `None` for a row's own or none.
    pub underline_from: Option<usize>,
    /// As `underline_from`, for the line through.
    pub line_through_from: Option<usize>,
    pub overline: bool,
    /// As `underline_from`, for the overline.
    pub overline_from: Option<usize>,
    /// `left`, `center` or `right`, after the initial value's direction.
    pub align: &'static str,
    pub white_space: &'static str,
    pub text_transform: &'static str,
}

fn clear_side(keyword: &str) -> ClearSide {
    match keyword {
        "left" => ClearSide::Left,
        "right" => ClearSide::Right,
        "both" => ClearSide::Both,
        _ => ClearSide::None,
    }
}

/// `left`, `center`, `right` or `justify` for a computed `text-align`.
fn align_keyword(align: ComputedTextAlign, rtl: bool) -> &'static str {
    match align {
        ComputedTextAlign::Right => "right",
        ComputedTextAlign::Center => "center",
        ComputedTextAlign::DirectionalInitial if rtl => "right",
        // An rtl paragraph stays start-aligned rather than justified.
        ComputedTextAlign::Justify if rtl => "right",
        ComputedTextAlign::Justify => "justify",
        _ => "left",
    }
}

fn line_height_of(
    value: &SpecifiedValue,
    font_size: f32,
    root_font_size: f32,
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
            let basis = unit.fallback_basis(font_size, root_font_size);
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

/// The box lengths generated content inherits from its element.
const BOX_LENGTHS: [&str; 10] = [
    "width",
    "height",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
];

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
    // The `medium` font size in pixels, as given to `prepare_captured_tree`; a generated
    // content's font keywords need it.
    font_medium: f32,
) -> Result<TreePaint, TreeLayoutError> {
    use TreeLayoutError as E;
    let id = |name: &'static str| PropertyId::lookup(name).ok_or(E::Paint { property: name });
    // Projected 0 is the source root, whose font size `rem` measures.
    let root_font_size = font_sizes.first().copied().unwrap_or(font_medium);
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
    let mut paints: Vec<BoxPaint> = Vec::with_capacity(projection.len());
    let mut grids: Vec<Option<Box<GridContainer>>> = vec![None; projection.len()];
    let mut grid_items: Vec<Option<Box<GridItem>>> = vec![None; projection.len()];
    let mut transitions: Vec<Vec<ElementTransition>> = vec![Vec::new(); projection.len()];
    let mut animations: Vec<Vec<ElementAnimation>> = vec![Vec::new(); projection.len()];
    let mut gradients: Vec<Option<std::sync::Arc<GradientPaint>>> = vec![None; projection.len()];
    let mut styles: Vec<TextStyle> = Vec::with_capacity(projection.len());
    let mut weights: Vec<FontWeight> = Vec::with_capacity(projection.len());
    let mut aligns: Vec<ComputedTextAlign> = Vec::with_capacity(projection.len());
    let mut visibility: Vec<bool> = Vec::with_capacity(projection.len());
    let mut z_indices: Vec<Option<i32>> = Vec::with_capacity(projection.len());
    let outline_color = id("outline-color")?;
    let mut generated: Vec<[Option<GeneratedRun>; 2]> = Vec::with_capacity(projection.len());
    let mut marker_types: Vec<Option<String>> = Vec::with_capacity(projection.len());
    let mut marker_inside: Vec<bool> = Vec::with_capacity(projection.len());
    let mut markers: Vec<Option<String>> = Vec::with_capacity(projection.len());
    let mut spacings: Vec<[f32; 2]> = Vec::with_capacity(projection.len());
    let mut backgrounds: Vec<Option<BackgroundImage>> = Vec::with_capacity(projection.len());
    let mut background_parts: Vec<BackgroundParts> = Vec::with_capacity(projection.len());
    let mut vertical_aligns: Vec<Option<&'static str>> = Vec::with_capacity(projection.len());
    let mut important: Vec<Vec<PropertyId>> = Vec::with_capacity(projection.len());
    let mut em_lengths: Vec<Vec<PropertyId>> = Vec::with_capacity(projection.len());
    let em_properties = EM_LENGTHS
        .iter()
        .map(|name| id(name))
        .collect::<Result<Vec<_>, _>>()?;
    let mut marker_images: Vec<Option<String>> = Vec::with_capacity(projection.len());
    // Each element's computed box lengths (`BOX_LENGTHS`) and border widths, which its
    // `:before` and `:after` inherit.
    let mut box_lengths: Vec<[Dim; 10]> = Vec::with_capacity(projection.len());
    let mut border_widths: Vec<[f32; 4]> = Vec::with_capacity(projection.len());
    let mut outline_colors: Vec<Option<[f32; 3]>> = Vec::with_capacity(projection.len());
    let mut outside_items: Vec<bool> = Vec::with_capacity(projection.len());
    let mut captions_below: Vec<bool> = Vec::with_capacity(projection.len());
    let mut collapses: Vec<bool> = Vec::with_capacity(projection.len());
    let mut raw_spacings: Vec<[f32; 2]> = Vec::with_capacity(projection.len());
    // Each element's computed `clear`, which its `:before` and `:after` inherit.
    let mut clears: Vec<ClearSide> = Vec::with_capacity(projection.len());
    // Each list parent's running item count.
    let mut items = vec![0_usize; projection.len()];
    let nodes = snapshot.construction().nodes();
    let mut counter_actions: Vec<CounterActions> = Vec::with_capacity(projection.len());
    let mut pending: Vec<[Option<PendingContent>; 2]> = Vec::with_capacity(projection.len());
    let mut rendered: Vec<bool> = Vec::with_capacity(projection.len());
    let mut quote_pairs: Vec<Option<Vec<[String; 2]>>> = Vec::with_capacity(projection.len());
    let directives = |value: DefaultedValueRef<'_>, inherited: &[(String, i32)]| {
        match value {
            DefaultedValueRef::Compute(SpecifiedValue::Counters(list)) => list
                .iter()
                .map(|directive| (directive.name.clone(), directive.value.saturating_i32()))
                .collect(),
            // These properties do not inherit by default, so this is an authored inherit.
            DefaultedValueRef::ParentComputed => inherited.to_vec(),
            _ => Vec::new(),
        }
    };
    let quotes_of = |value: DefaultedValueRef<'_>, inherited: Option<Vec<[String; 2]>>| match value
    {
        DefaultedValueRef::Compute(SpecifiedValue::Quotes(pairs)) => Some(pairs.clone()),
        DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword))
            if keyword.as_str() == "none" =>
        {
            Some(Vec::new())
        }
        DefaultedValueRef::ParentComputed => inherited,
        _ => None,
    };
    for (index, &font_size) in font_sizes.iter().enumerate() {
        let original = projection.original[index];
        let mut winners = PropertyWinners::new();
        cascade_positioned_rules(
            &mut winners,
            bundle.offers_for(&source, original),
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
        // Winners holding `var()`, as they parse for this target.
        bundle.resolve(&mut winners, original, None);
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
        important.push(
            PropertyId::all()
                .filter(|property| {
                    winners
                        .candidate(*property)
                        .ok()
                        .flatten()
                        .is_some_and(|winner| winner.value.important)
                })
                .collect(),
        );
        let defaulted = |name: &'static str| {
            let property = id(name)?;
            winners
                .defaulted(property, parent_index.is_some())
                .map_err(|_| E::Paint { property: name })
        };
        // Lengths given relative to the font, as an animated `font-size` moves them.
        // An inherited one follows when its parent's does: its computed pixels move with it.
        let mut relative = Vec::new();
        for (name, property) in EM_LENGTHS.iter().zip(&em_properties) {
            let em = match defaulted(name)? {
                // `rem` follows the root's font size, not this element's.
                DefaultedValueRef::Compute(SpecifiedValue::Length(LengthTarget::Relative(
                    _,
                    unit,
                ))) => *unit != iced_css_core::values::RelativeUnit::Rem,
                DefaultedValueRef::ParentComputed => {
                    parent_index.is_some_and(|parent| em_lengths[parent].contains(property))
                }
                _ => false,
            };
            if em {
                relative.push(*property);
            }
        }
        em_lengths.push(relative);
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
        let outlined = match defaulted("outline-style")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(style)) => {
                !matches!(style.as_str(), "none" | "hidden")
            }
            DefaultedValueRef::ParentComputed => parent_index.is_some_and(|parent: usize| {
                !matches!(paints[parent].outline_style, "none" | "hidden")
            }),
            _ => false,
        };
        let outline_width = match outlined {
            true => match defaulted("outline-width")? {
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
                // Of its own font size, as border widths are.
                DefaultedValueRef::Compute(SpecifiedValue::Length(LengthTarget::Relative(
                    value,
                    unit,
                ))) => value
                    .scaled_finite_f32(
                        unit.fallback_basis(font_size, root_font_size),
                        limits.numeric,
                    )
                    .ok()
                    .map(|c| c.value.max(0.0)),
                DefaultedValueRef::ParentComputed => parent_index
                    .and_then(|parent: usize| paints[parent].outline)
                    .map(|outline| outline.0),
                _ => Some(3.0),
            },
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
            DefaultedValueRef::Compute(value) => {
                line_height_of(value, font_size, root_font_size, limits.numeric)
            }
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
        // 16.3.1: decorations propagate down, except into floats, absolute boxes and the
        // contents of atomic inline-level boxes.
        let detached = resolution.is_some_and(|value| {
            value.float != Float::None
                || matches!(value.position, Position::Absolute | Position::Fixed)
                || matches!(value.display, Display::InlineBlock | Display::InlineTable)
        });
        let carried = inherited.filter(|_| !detached);
        let (mut underline, mut line_through, mut overline) = carried
            .map_or((false, false, false), |style| {
                (style.underline, style.line_through, style.overline)
            });
        // A carried line keeps the color and size of the box that set it.
        let from = |on: bool, from: Option<usize>| on.then(|| from.or(parent_index)).flatten();
        let (mut underline_from, mut line_through_from, mut overline_from) =
            carried.map_or((None, None, None), |style| {
                (
                    from(style.underline, style.underline_from),
                    from(style.line_through, style.line_through_from),
                    from(style.overline, style.overline_from),
                )
            });
        // Its own lines: an `inherit` takes the parent's own, not the ones it only carries.
        let own = match defaulted("text-decoration")? {
            DefaultedValueRef::Compute(SpecifiedValue::TextDecoration(own)) => {
                (own.underline(), own.line_through(), own.overline())
            }
            DefaultedValueRef::ParentComputed => inherited.map_or((false, false, false), |style| {
                (
                    style.underline && style.underline_from.is_none(),
                    style.line_through && style.line_through_from.is_none(),
                    style.overline && style.overline_from.is_none(),
                )
            }),
            _ => (false, false, false),
        };
        underline |= own.0;
        line_through |= own.1;
        overline |= own.2;
        if own.0 {
            underline_from = None;
        }
        if own.1 {
            line_through_from = None;
        }
        if own.2 {
            overline_from = None;
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
        let small_caps = match defaulted("font-variant")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                keyword.as_str() == "small-caps"
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.is_some_and(|parent: usize| styles[parent].small_caps)
            }
            _ => false,
        };
        styles.push(TextStyle {
            small_caps,
            word_spacing: 0.0,
            text_indent: 0.0,
            text_indent_percent: 0.0,
            weight: weight.number(),
            italic,
            line_height,
            family,
            underline,
            line_through,
            underline_from,
            line_through_from,
            overline,
            overline_from,
            align: align_keyword(align, rtl),
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
        // Computed apart from any drawn outline: it inherits even where none shows, and `None`
        // is `invert`, which follows each box's own `color` here.
        let own_outline_color = match defaulted("outline-color")? {
            DefaultedValueRef::ParentComputed => {
                parent_index.and_then(|parent: usize| outline_colors[parent])
            }
            _ => compute_color(
                &winners,
                outline_color,
                ColorComputationContext {
                    parent: None,
                    current_color: Some(current),
                    user_agent_color: Some(&BLACK),
                },
            )
            .ok()
            .and_then(|color| rgb(color.value(), limits.numeric)),
        };
        outline_colors.push(own_outline_color);
        let outline = outline_width.map(|width| {
            (
                width,
                own_outline_color.or_else(|| paint(0)).unwrap_or([0.0; 3]),
            )
        });
        let cursor = match defaulted("cursor")? {
            DefaultedValueRef::Compute(SpecifiedValue::Cursor(value)) => value.fallback.as_str(),
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => keyword.as_str(),
            DefaultedValueRef::ParentComputed => {
                parent_index.map_or("auto", |parent: usize| paints[parent].cursor)
            }
            _ => "auto",
        };
        let hide_empty = match defaulted("empty-cells")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                keyword.as_str() == "hide"
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.is_some_and(|parent: usize| paints[parent].hide_empty)
            }
            _ => false,
        };
        let fixed_table = match defaulted("table-layout")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                keyword.as_str() == "fixed"
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.is_some_and(|parent: usize| paints[parent].fixed_table)
            }
            _ => false,
        };
        // CSS Sizing 3: not inherited; `inherit` takes the parent's.
        let border_box = match defaulted("box-sizing")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                keyword.as_str() == "border-box"
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.is_some_and(|parent: usize| paints[parent].border_box)
            }
            _ => false,
        };
        // CSS Color 3: this box's own opacity, and with its ancestors' what it draws at.
        // There is no group to blend: each thing a box draws takes the product.
        let own_opacity = match defaulted("opacity")? {
            DefaultedValueRef::Compute(SpecifiedValue::Number(number)) => number
                .scaled_finite_f32(1.0, limits.numeric)
                .map_or(1.0, |converted| converted.value.clamp(0.0, 1.0)),
            DefaultedValueRef::ParentComputed => parent_index
                .and_then(|parent: usize| paints[parent].own_opacity)
                .unwrap_or(1.0),
            _ => 1.0,
        };
        let through = own_opacity
            * parent_index
                .and_then(|parent: usize| paints[parent].opacity)
                .unwrap_or(1.0);
        paints.push(BoxPaint {
            background_origin: BoxArea::Padding,
            background_clip: BoxArea::Border,
            flex: None,
            flex_item: FlexItem::default(),
            opacity: (through < 1.0).then_some(through),
            own_opacity: (own_opacity < 1.0).then_some(own_opacity),
            shadow: None,
            radius: [[0.0; 2]; 4],
            border_box,
            cursor,
            clip: None,
            collapsed_cell: None,
            hide_empty,
            hidden_empty: false,
            fixed_table,
            // A system color has no palette here: text stays visible, in black.
            // Text has no opacity of its own here: a see-through color draws opaque, and
            // one with none at all draws nothing.
            color: match row[0].value() {
                Color::System(_) | Color::Invert => Some([0.0; 3]),
                Color::SrgbA([_, _, _, 0]) => None,
                _ => paint(0),
            },
            background: paint(1),
            // CSS Color 3: an opacity below 1, as an animation's fade is drawn.
            background_fade: opacity(row[1].value()),
            border_fade: std::array::from_fn(|side| opacity(row[2 + side].value())),
            under_text: None,
            under_text_border: [None; 4],
            border_style: ["solid"; 4],
            outline_style: "solid",
            border: [paint(2), paint(3), paint(4), paint(5)],
            visible,
            z_index,
            outline,
        });
        visibility.push(visible);
        z_indices.push(z_index);
        let length = |target: &LengthTarget| match target {
            LengthTarget::Absolute(value) => value
                .to_nonnegative_f32(limits.numeric)
                .map_or(0.0, |c| c.value),
            LengthTarget::Relative(value, unit) => value
                .scaled_finite_f32(
                    unit.fallback_basis(font_size, root_font_size),
                    limits.numeric,
                )
                .map_or(0.0, |c| c.value),
            _ => 0.0,
        };
        let line = |name: &'static str, inherited: Option<&'static str>| {
            Ok::<_, E>(match defaulted(name)? {
                DefaultedValueRef::Compute(SpecifiedValue::Keyword(style)) => style.as_str(),
                DefaultedValueRef::ParentComputed => inherited.unwrap_or("none"),
                _ => "none",
            })
        };
        let parent_line =
            |side: usize| parent_index.map(|parent: usize| paints[parent].border_style[side]);
        let lines = [
            line("border-top-style", parent_line(0))?,
            line("border-right-style", parent_line(1))?,
            line("border-bottom-style", parent_line(2))?,
            line("border-left-style", parent_line(3))?,
        ];
        let outline_line = line(
            "outline-style",
            parent_index.map(|parent: usize| paints[parent].outline_style),
        )?;
        if let Some(paint) = paints.last_mut() {
            paint.border_style = lines;
            paint.outline_style = outline_line;
        }
        // 11.1.2: negative lengths are permitted here.
        let signed = |side: &LengthTarget| match side {
            LengthTarget::Absolute(value) => {
                value.to_finite_f32(limits.numeric).map_or(0.0, |c| c.value)
            }
            other => length(other),
        };
        let text_indent = match defaulted("text-indent")? {
            DefaultedValueRef::Compute(SpecifiedValue::Length(value)) => signed(value),
            DefaultedValueRef::ParentComputed => {
                parent_index.map_or(0.0, |parent: usize| styles[parent].text_indent)
            }
            _ => 0.0,
        };
        // A percentage keeps its computed value, a percentage, for the width at layout.
        let text_indent_percent = match defaulted("text-indent")? {
            DefaultedValueRef::Compute(SpecifiedValue::Length(LengthTarget::Percentage(value))) => {
                value
                    .percentage_finite_f32(100.0, limits.numeric)
                    .map_or(0.0, |c| c.value)
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.map_or(0.0, |parent: usize| styles[parent].text_indent_percent)
            }
            _ => 0.0,
        };
        let word_spacing = match defaulted("word-spacing")? {
            DefaultedValueRef::Compute(SpecifiedValue::Length(value)) => signed(value),
            DefaultedValueRef::ParentComputed => {
                parent_index.map_or(0.0, |parent: usize| styles[parent].word_spacing)
            }
            _ => 0.0,
        };
        if let Some(style) = styles.last_mut() {
            style.text_indent = text_indent;
            style.text_indent_percent = text_indent_percent;
            style.word_spacing = word_spacing;
        }
        let clip = match defaulted("clip")? {
            DefaultedValueRef::Compute(SpecifiedValue::ClipRect(sides)) => Some(
                sides
                    .each_ref()
                    .map(|side| (!matches!(side, LengthTarget::Auto)).then(|| signed(side))),
            ),
            DefaultedValueRef::ParentComputed => {
                parent_index.and_then(|parent: usize| paints[parent].clip)
            }
            _ => None,
        };
        if let Some(paint) = paints.last_mut() {
            paint.clip = clip;
        }
        let raw_spacing = match defaulted("border-spacing")? {
            DefaultedValueRef::Compute(SpecifiedValue::LengthPair(across, down)) => {
                [length(across), length(down)]
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.map_or([0.0, 0.0], |parent| raw_spacings[parent])
            }
            _ => [0.0, 0.0],
        };
        let collapse = match defaulted("border-collapse")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                keyword.as_str() == "collapse"
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.is_some_and(|parent| collapses[parent])
            }
            _ => false,
        };
        let uri = match defaulted("background-image")? {
            DefaultedValueRef::Compute(SpecifiedValue::Uri(value)) => Some(value.clone()),
            DefaultedValueRef::ParentComputed => parent_index
                .and_then(|parent| backgrounds[parent].as_ref())
                .map(|image| image.uri.clone()),
            _ => None,
        };
        // Inherit reads the parent's computed values, whether or not it has an image.
        let inherited_background = parent_index.map(|parent| background_parts[parent]);
        let repeat = match defaulted("background-repeat")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => keyword.as_str(),
            DefaultedValueRef::ParentComputed => {
                inherited_background.map_or("repeat", |parts: BackgroundParts| parts.0)
            }
            _ => "repeat",
        };
        let offset = |target: &LengthTarget| match target {
            LengthTarget::Percentage(value) => BackgroundOffset::Percent(
                value
                    .percentage_finite_f32(100.0, limits.numeric)
                    .map_or(0.0, |c| c.value),
            ),
            // 14.2.1: negative offsets are permitted.
            other => BackgroundOffset::Px(signed(other)),
        };
        let position = match defaulted("background-position")? {
            DefaultedValueRef::Compute(SpecifiedValue::LengthPair(x, y)) => [offset(x), offset(y)],
            DefaultedValueRef::ParentComputed => {
                inherited_background.map_or([BackgroundOffset::Percent(0.0); 2], |parts| parts.1)
            }
            _ => [BackgroundOffset::Percent(0.0); 2],
        };
        let fixed = match defaulted("background-attachment")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                keyword.as_str() == "fixed"
            }
            DefaultedValueRef::ParentComputed => inherited_background.is_some_and(|parts| parts.2),
            _ => false,
        };
        background_parts.push((repeat, position, fixed));
        // CSS Backgrounds 3: its size and boxes. `inherit` on the size reads as `auto`.
        let area = |name: &'static str, initial: BoxArea, parent: fn(&BoxPaint) -> BoxArea| {
            Ok::<_, E>(match defaulted(name)? {
                DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                    match keyword.as_str() {
                        "border-box" => BoxArea::Border,
                        "content-box" => BoxArea::Content,
                        _ => BoxArea::Padding,
                    }
                }
                DefaultedValueRef::ParentComputed => {
                    parent_index.map_or(initial, |at: usize| parent(&paints[at]))
                }
                _ => initial,
            })
        };
        let origin = area("background-origin", BoxArea::Padding, |paint| {
            paint.background_origin
        })?;
        let clip = area("background-clip", BoxArea::Border, |paint| {
            paint.background_clip
        })?;
        paints[index].background_origin = origin;
        paints[index].background_clip = clip;
        let size = match defaulted("background-size")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword))
                if keyword.as_str() == "cover" =>
            {
                BackgroundSize::Cover
            }
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword))
                if keyword.as_str() == "contain" =>
            {
                BackgroundSize::Contain
            }
            DefaultedValueRef::Compute(SpecifiedValue::LengthPair(across, down)) => {
                let side = |target: &LengthTarget| {
                    (!matches!(target, LengthTarget::Auto)).then(|| offset(target))
                };
                BackgroundSize::Sized(side(across), side(down))
            }
            _ => BackgroundSize::Sized(None, None),
        };
        backgrounds.push(uri.map(|uri| BackgroundImage {
            size,
            origin,
            clip,
            uri,
            repeat_x: matches!(repeat, "repeat" | "repeat-x"),
            repeat_y: matches!(repeat, "repeat" | "repeat-y"),
            position,
            fixed,
        }));
        vertical_aligns.push(match defaulted("vertical-align")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => Some(keyword.as_str()),
            DefaultedValueRef::ParentComputed => {
                parent_index.and_then(|parent| vertical_aligns[parent])
            }
            _ => None,
        });
        let caption_below = match defaulted("caption-side")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                keyword.as_str() == "bottom"
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.is_some_and(|parent| captions_below[parent])
            }
            _ => false,
        };
        captions_below.push(caption_below);
        raw_spacings.push(raw_spacing);
        collapses.push(collapse);
        spacings.push(if collapse { [0.0, 0.0] } else { raw_spacing });
        // Generated content: text in the element, inheriting from it.
        marker_images.push(match defaulted("list-style-image")? {
            DefaultedValueRef::Compute(SpecifiedValue::Uri(value)) => Some(value.clone()),
            DefaultedValueRef::ParentComputed => {
                parent_index.and_then(|parent| marker_images[parent].clone())
            }
            _ => None,
        });
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
        let inside = match defaulted("list-style-position")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                keyword.as_str() == "inside"
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.is_some_and(|parent| marker_inside[parent])
            }
            _ => false,
        };
        marker_inside.push(inside);
        let list_item = display
            .resolution(original)
            .is_some_and(|value| value.display == Display::ListItem);
        outside_items.push(!inside && list_item);
        let (marker, outside) = if inside {
            // An image takes the marker's place even with no marker text to show.
            let imaged = list_item && marker_images.last().is_some_and(Option::is_some);
            (marker.or_else(|| imaged.then(String::new)), None)
        } else {
            (None, marker)
        };
        markers.push(outside.map(|text| text.trim_end_matches('\u{a0}').to_owned()));
        let inherited_actions = parent_index
            .map(|parent| counter_actions[parent].clone())
            .unwrap_or_default();
        let actions = CounterActions {
            reset: directives(defaulted("counter-reset")?, &inherited_actions.reset),
            increment: directives(
                defaulted("counter-increment")?,
                &inherited_actions.increment,
            ),
        };
        let element_quotes = quotes_of(
            defaulted("quotes")?,
            parent_index.and_then(|parent| quote_pairs[parent].clone()),
        );
        rendered.push(
            display
                .resolution(original)
                .is_some_and(|value| value.display != Display::None),
        );
        // Its own lengths, as its generated content inherits them: an `em` in its font size,
        // its own `inherit` its parent's, and a border width none where its style is.
        let own_length = |target: &LengthTarget| match target {
            LengthTarget::Absolute(value) => {
                Dim::Px(value.to_finite_f32(limits.numeric).map_or(0.0, |c| c.value))
            }
            LengthTarget::Relative(value, unit) => Dim::Px(
                value
                    .scaled_finite_f32(
                        unit.fallback_basis(font_size, root_font_size),
                        limits.numeric,
                    )
                    .map_or(0.0, |c| c.value),
            ),
            LengthTarget::Percentage(value) => Dim::Percent(
                value
                    .percentage_finite_f32(100.0, limits.numeric)
                    .map_or(0.0, |c| c.value),
            ),
            LengthTarget::Auto => Dim::Auto,
            LengthTarget::Calc(sum) => calc_dim(*sum, font_size, root_font_size),
        };
        let mut lengths = [Dim::Px(0.0); 10];
        for (at, name) in BOX_LENGTHS.iter().enumerate() {
            let initial = if at < 2 { Dim::Auto } else { Dim::Px(0.0) };
            lengths[at] = match defaulted(name)? {
                DefaultedValueRef::Compute(SpecifiedValue::Length(target)) => own_length(target),
                DefaultedValueRef::ParentComputed => {
                    parent_index.map_or(initial, |parent| box_lengths[parent][at])
                }
                _ => initial,
            };
        }
        box_lengths.push(lengths);
        // CSS Flexbox 1 and Box Alignment 3. None of these inherits; `inherit` on one
        // reads as its initial value.
        let keyword_of_own = |name: &'static str| {
            Ok::<_, E>(match defaulted(name)? {
                DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                    Some(keyword.as_str())
                }
                _ => None,
            })
        };
        let factor = |name: &'static str, initial: f32| {
            Ok::<_, E>(match defaulted(name)? {
                DefaultedValueRef::Compute(SpecifiedValue::Number(number)) => number
                    .scaled_finite_f32(1.0, limits.numeric)
                    .map_or(initial, |converted| converted.value.max(0.0)),
                _ => initial,
            })
        };
        let gap = |name: &'static str| {
            Ok::<_, E>(match defaulted(name)? {
                DefaultedValueRef::Compute(SpecifiedValue::Length(target)) => {
                    match own_length(target) {
                        Dim::Px(pixels) => pixels.max(0.0),
                        // A percentage gap has no basis here.
                        _ => 0.0,
                    }
                }
                _ => 0.0,
            })
        };
        let space = |keyword: Option<&str>| match keyword {
            Some("flex-end" | "end" | "right") => FlexSpace::End,
            Some("center") => FlexSpace::Center,
            Some("space-between") => FlexSpace::SpaceBetween,
            Some("space-around") => FlexSpace::SpaceAround,
            Some("space-evenly") => FlexSpace::SpaceEvenly,
            Some("stretch") => FlexSpace::Stretch,
            _ => FlexSpace::Start,
        };
        // `baseline` aligns at the start.
        let align = |keyword: Option<&str>| match keyword {
            Some("flex-start" | "start" | "self-start" | "left" | "baseline") => {
                Some(FlexAlign::Start)
            }
            Some("flex-end" | "end" | "self-end" | "right") => Some(FlexAlign::End),
            Some("center") => Some(FlexAlign::Center),
            Some("stretch" | "normal") => Some(FlexAlign::Stretch),
            _ => None,
        };
        if matches!(keyword_of_own("display")?, Some("flex" | "inline-flex")) {
            let direction = keyword_of_own("flex-direction")?.unwrap_or("row");
            paints[index].flex = Some(FlexContainer {
                column: direction.starts_with("column"),
                reverse: direction.ends_with("-reverse"),
                wrap: match keyword_of_own("flex-wrap")? {
                    Some("wrap") => FlexWrap::Wrap,
                    Some("wrap-reverse") => FlexWrap::WrapReverse,
                    _ => FlexWrap::NoWrap,
                },
                justify: space(keyword_of_own("justify-content")?),
                align_items: align(keyword_of_own("align-items")?).unwrap_or(FlexAlign::Stretch),
                align_content: match keyword_of_own("align-content")? {
                    None | Some("normal" | "stretch") => FlexSpace::Stretch,
                    other => space(other),
                },
                gap: [gap("row-gap")?, gap("column-gap")?],
            });
        }
        paints[index].flex_item = FlexItem {
            grow: factor("flex-grow", 0.0)?,
            shrink: factor("flex-shrink", 1.0)?,
            basis: match defaulted("flex-basis")? {
                DefaultedValueRef::Compute(SpecifiedValue::Length(target)) => {
                    match own_length(target) {
                        Dim::Mix(pixels, percent) => Dim::Clamped(pixels, percent),
                        other => other,
                    }
                }
                _ => Dim::Auto,
            },
            align: align(keyword_of_own("align-self")?),
            order: match defaulted("order")? {
                DefaultedValueRef::Compute(SpecifiedValue::Integer(order)) => {
                    order.saturating_i32()
                }
                _ => 0,
            },
        };
        // CSS Grid 1. None of these inherits; `inherit` on one reads as its initial value.
        let grid_of = |name: &'static str| {
            Ok::<_, E>(match defaulted(name)? {
                DefaultedValueRef::Compute(SpecifiedValue::Grid(value)) => Some(value),
                _ => None,
            })
        };
        let breadth = |breadth: &GridBreadth| match breadth {
            GridBreadth::Length(target) => Breadth::Fixed(match own_length(target) {
                Dim::Mix(pixels, percent) => Dim::Clamped(pixels, percent),
                other => other,
            }),
            GridBreadth::Flex(number) => Breadth::Flex(
                number
                    .scaled_finite_f32(1.0, limits.numeric)
                    .map_or(0.0, |converted| converted.value.max(0.0)),
            ),
            GridBreadth::Auto => Breadth::Auto,
            GridBreadth::MinContent => Breadth::MinContent,
            GridBreadth::MaxContent => Breadth::MaxContent,
        };
        let track = |size: &GridTrackSize| match size {
            GridTrackSize::Breadth(one) => TrackSize::breadth(breadth(one)),
            GridTrackSize::MinMax(min, max) => TrackSize {
                min: breadth(min),
                max: breadth(max),
                fit: None,
            },
            GridTrackSize::FitContent(limit) => TrackSize {
                min: Breadth::Auto,
                max: Breadth::MaxContent,
                fit: Some(own_length(limit)),
            },
        };
        // Integer repetitions are written out; an automatic one is kept to count at layout.
        let track_list = |entries: &[GridTrackEntry]| {
            let mut list = TrackList {
                sizes: Vec::new(),
                names: vec![Vec::new()],
                repeat: None,
            };
            let limit = GRID_LIMIT as usize;
            for entry in entries {
                match entry {
                    GridTrackEntry::Names(names) => {
                        let at = list.sizes.len();
                        match list.repeat.as_mut().filter(|repeat| repeat.at == at) {
                            Some(repeat) => repeat.after.extend(names.iter().cloned()),
                            None => {
                                if let Some(line) = list.names.last_mut() {
                                    line.extend(names.iter().cloned());
                                }
                            }
                        }
                    }
                    GridTrackEntry::Size(size) if list.sizes.len() < limit => {
                        list.sizes.push(track(size));
                        list.names.push(Vec::new());
                    }
                    GridTrackEntry::Size(_) => {}
                    GridTrackEntry::Repeat(GridRepeat::Count(count), inner) => {
                        for _ in 0..*count {
                            for entry in inner {
                                match entry {
                                    GridTrackEntry::Names(names) => {
                                        if let Some(line) = list.names.last_mut() {
                                            line.extend(names.iter().cloned());
                                        }
                                    }
                                    GridTrackEntry::Size(size) if list.sizes.len() < limit => {
                                        list.sizes.push(track(size));
                                        list.names.push(Vec::new());
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    GridTrackEntry::Repeat(count, inner) => {
                        let mut repeat = AutoRepeat {
                            at: list.sizes.len(),
                            fit: *count == GridRepeat::AutoFit,
                            sizes: Vec::new(),
                            names: vec![Vec::new()],
                            after: Vec::new(),
                        };
                        for entry in inner {
                            match entry {
                                GridTrackEntry::Names(names) => {
                                    if let Some(line) = repeat.names.last_mut() {
                                        line.extend(names.iter().cloned());
                                    }
                                }
                                GridTrackEntry::Size(size) => {
                                    repeat.sizes.push(track(size));
                                    repeat.names.push(Vec::new());
                                }
                                GridTrackEntry::Repeat(..) => {}
                            }
                        }
                        list.repeat = Some(repeat);
                    }
                }
            }
            list
        };
        let auto_tracks = |name: &'static str| {
            Ok::<_, E>(match grid_of(name)? {
                Some(GridValue::Tracks(entries)) => {
                    let sizes: Vec<TrackSize> = entries
                        .iter()
                        .filter_map(|entry| match entry {
                            GridTrackEntry::Size(size) => Some(track(size)),
                            _ => None,
                        })
                        .collect();
                    if sizes.is_empty() {
                        vec![TrackSize::AUTO]
                    } else {
                        sizes
                    }
                }
                _ => vec![TrackSize::AUTO],
            })
        };
        if matches!(keyword_of_own("display")?, Some("grid" | "inline-grid")) {
            let template = |name: &'static str| {
                Ok::<_, E>(match grid_of(name)? {
                    Some(GridValue::Tracks(entries)) => track_list(entries),
                    _ => TrackList::default(),
                })
            };
            let (column_flow, dense) = match grid_of("grid-auto-flow")? {
                Some(GridValue::Flow { column, dense }) => (*column, *dense),
                _ => (false, false),
            };
            let content = |keyword: Option<&str>| match keyword {
                None | Some("normal" | "stretch") => FlexSpace::Stretch,
                other => space(other),
            };
            grids[index] = Some(Box::new(GridContainer {
                columns: template("grid-template-columns")?,
                rows: template("grid-template-rows")?,
                areas: match grid_of("grid-template-areas")? {
                    Some(GridValue::Areas(rows)) => grid_areas(rows),
                    _ => Vec::new(),
                },
                auto_columns: auto_tracks("grid-auto-columns")?,
                auto_rows: auto_tracks("grid-auto-rows")?,
                column_flow,
                dense,
                justify_content: content(keyword_of_own("justify-content")?),
                align_content: content(keyword_of_own("align-content")?),
                justify_items: align(keyword_of_own("justify-items")?)
                    .unwrap_or(FlexAlign::Stretch),
                align_items: align(keyword_of_own("align-items")?).unwrap_or(FlexAlign::Stretch),
                gap: [gap("row-gap")?, gap("column-gap")?],
            }));
        }
        let line = |name: &'static str| {
            Ok::<_, E>(match grid_of(name)? {
                Some(GridValue::Line(line)) => GridLine {
                    span: line.span,
                    number: line.number,
                    name: line.name.clone(),
                },
                _ => GridLine::default(),
            })
        };
        let item = GridItem {
            row: [line("grid-row-start")?, line("grid-row-end")?],
            column: [line("grid-column-start")?, line("grid-column-end")?],
            justify: align(keyword_of_own("justify-self")?),
        };
        if item != GridItem::default() {
            grid_items[index] = Some(Box::new(item));
        }
        // CSS Transitions 1. None of these inherits; `inherit` on one reads as its initial
        // value. The list of properties sets how many transitions there are, and the other
        // lists repeat to match it.
        let transition_of = |name: &'static str| {
            Ok::<_, E>(match defaulted(name)? {
                DefaultedValueRef::Compute(SpecifiedValue::Transition(value)) => Some(value),
                _ => None,
            })
        };
        let times = |value: Option<&TransitionValue>| match value {
            Some(TransitionValue::Times(times)) => times
                .iter()
                .map(|time| {
                    time.value
                        .scaled_finite_f32(1.0, limits.numeric)
                        .map_or(0.0, |converted| {
                            converted.value * if time.milliseconds { 1.0 } else { 1000.0 }
                        })
                })
                .collect(),
            _ => vec![0.0],
        };
        let durations: Vec<f32> = times(transition_of("transition-duration")?);
        let delays: Vec<f32> = times(transition_of("transition-delay")?);
        if durations.iter().any(|duration| *duration > 0.0)
            || delays.iter().any(|delay| *delay > 0.0)
        {
            use crate::animation::easing::CssEasing;
            let easings: Vec<CssEasing> = match transition_of("transition-timing-function")? {
                Some(TransitionValue::Easings(easings)) => easings
                    .iter()
                    .map(|easing| css_easing(easing, limits.numeric))
                    .collect(),
                _ => vec![CssEasing::EASE],
            };
            let properties = match transition_of("transition-property")? {
                Some(TransitionValue::Properties(properties)) => properties.clone(),
                _ => vec![TransitionProperty::All],
            };
            for (at, property) in properties.iter().enumerate() {
                let properties = match property {
                    TransitionProperty::All => None,
                    TransitionProperty::None => continue,
                    // An unknown name holds its place in the lists and moves nothing.
                    TransitionProperty::Named(name) => match PropertyId::lookup(name) {
                        Some(property) => Some(property.longhands().to_vec()),
                        None => continue,
                    },
                };
                let duration = durations[at % durations.len()].max(0.0);
                let delay = delays[at % delays.len()];
                // A transition taking no time at all is none (2.1).
                if duration + delay.max(0.0) <= 0.0 {
                    continue;
                }
                transitions[index].push(ElementTransition {
                    properties,
                    duration,
                    delay,
                    easing: easings[at % easings.len()],
                });
            }
        }
        // CSS Animations 1, likewise: the list of names sets how many there are.
        if let Some(TransitionValue::Names(names)) = transition_of("animation-name")? {
            let durations: Vec<f32> = times(transition_of("animation-duration")?);
            let delays: Vec<f32> = times(transition_of("animation-delay")?);
            let easings: Vec<crate::animation::easing::CssEasing> =
                match transition_of("animation-timing-function")? {
                    Some(TransitionValue::Easings(easings)) => easings
                        .iter()
                        .map(|easing| css_easing(easing, limits.numeric))
                        .collect(),
                    _ => vec![crate::animation::easing::CssEasing::EASE],
                };
            let counts: Vec<f32> = match transition_of("animation-iteration-count")? {
                Some(TransitionValue::Counts(counts)) => counts
                    .iter()
                    .map(|count| match count {
                        None => f32::INFINITY,
                        Some(count) => count
                            .scaled_finite_f32(1.0, limits.numeric)
                            .map_or(1.0, |converted| converted.value.max(0.0)),
                    })
                    .collect(),
                _ => vec![1.0],
            };
            let words = |name: &'static str, initial: &'static str| {
                Ok::<_, E>(match transition_of(name)? {
                    Some(TransitionValue::Keywords(words)) => words.clone(),
                    _ => vec![initial],
                })
            };
            let directions = words("animation-direction", "normal")?;
            let fills = words("animation-fill-mode", "none")?;
            let states = words("animation-play-state", "running")?;
            for (at, name) in names.iter().enumerate() {
                let Some(name) = name else { continue };
                let fill = fills[at % fills.len()];
                animations[index].push(ElementAnimation {
                    name: name.clone(),
                    duration: durations[at % durations.len()].max(0.0),
                    delay: delays[at % delays.len()],
                    easing: easings[at % easings.len()],
                    iterations: counts[at % counts.len()],
                    direction: match directions[at % directions.len()] {
                        "reverse" => AnimationDirection::Reverse,
                        "alternate" => AnimationDirection::Alternate,
                        "alternate-reverse" => AnimationDirection::AlternateReverse,
                        _ => AnimationDirection::Normal,
                    },
                    fill_backwards: matches!(fill, "backwards" | "both"),
                    fill_forwards: matches!(fill, "forwards" | "both"),
                    paused: states[at % states.len()] == "paused",
                });
            }
        }
        // CSS Images 3: a linear gradient as the background image. `inherit` takes the
        // parent's.
        gradients[index] = match defaulted("background-image")? {
            DefaultedValueRef::Compute(SpecifiedValue::Gradient(gradient)) => {
                let number = |value: &iced_css_core::values::Decimal| {
                    value
                        .scaled_finite_f32(1.0, limits.numeric)
                        .map_or(0.0, |converted| converted.value)
                };
                let aim = match &gradient.direction {
                    GradientDirection::Angle(value, unit) => {
                        let turns = match unit {
                            AngleUnit::Deg => number(value) / 360.0,
                            AngleUnit::Rad => number(value) / std::f32::consts::TAU,
                            AngleUnit::Grad => number(value) / 400.0,
                            AngleUnit::Turn => number(value),
                        };
                        GradientAim::Angle(turns * std::f32::consts::TAU)
                    }
                    GradientDirection::Toward(across, down) => GradientAim::Toward(*across, *down),
                };
                let mut stops = Vec::with_capacity(gradient.stops.len());
                for stop in &gradient.stops {
                    // No color of its own: the element's `color`.
                    let (color, alpha) = match &stop.color {
                        Some(color) => (
                            rgb(color, limits.numeric).unwrap_or([0.0; 3]),
                            opacity(color).unwrap_or(if rgb(color, limits.numeric).is_some() {
                                1.0
                            } else {
                                0.0
                            }),
                        ),
                        None => (paints[index].color.unwrap_or([0.0; 3]), 1.0),
                    };
                    stops.push((color, alpha, stop.position.as_ref().map(own_length)));
                }
                Some(std::sync::Arc::new(GradientPaint { aim, stops }))
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.and_then(|parent: usize| gradients[parent].clone())
            }
            _ => None,
        };
        // CSS Backgrounds 3: the first outer shadow; iced draws one, without a spread.
        // Not inherited; `inherit` takes the parent's.
        paints[index].shadow = match defaulted("box-shadow")? {
            DefaultedValueRef::Compute(SpecifiedValue::Shadows(shadows)) => shadows
                .iter()
                .find(|shadow| !shadow.inset)
                .and_then(|shadow| {
                    let pixels = |length: &LengthTarget| match own_length(length) {
                        Dim::Px(pixels) => pixels,
                        _ => 0.0,
                    };
                    // No color of its own: the element's `color`.
                    let (color, alpha) = match &shadow.color {
                        Some(color) => (rgb(color, limits.numeric)?, opacity(color).unwrap_or(1.0)),
                        None => (paints[index].color?, 1.0),
                    };
                    Some(BoxShadow {
                        offset: [pixels(&shadow.offset[0]), pixels(&shadow.offset[1])],
                        blur: pixels(&shadow.blur).max(0.0),
                        color,
                        alpha,
                    })
                }),
            DefaultedValueRef::ParentComputed => {
                parent_index.and_then(|parent: usize| paints[parent].shadow)
            }
            _ => None,
        };
        // CSS Backgrounds 3: the corner radii. Not inherited; `inherit` takes the parent's.
        for (corner, name) in [
            "border-top-left-radius",
            "border-top-right-radius",
            "border-bottom-right-radius",
            "border-bottom-left-radius",
        ]
        .into_iter()
        .enumerate()
        {
            let radius = match defaulted(name)? {
                DefaultedValueRef::Compute(SpecifiedValue::LengthPair(across, _)) => {
                    match own_length(across) {
                        Dim::Px(pixels) => [pixels.max(0.0), 0.0],
                        Dim::Percent(percent) => [0.0, percent.max(0.0)],
                        Dim::Mix(pixels, percent) | Dim::Clamped(pixels, percent) => {
                            [pixels, percent]
                        }
                        Dim::Auto => [0.0; 2],
                    }
                }
                DefaultedValueRef::ParentComputed => {
                    parent_index.map_or([0.0; 2], |parent: usize| paints[parent].radius[corner])
                }
                _ => [0.0; 2],
            };
            paints[index].radius[corner] = radius;
        }
        let mut widths = [0.0_f32; 4];
        for (at, name) in [
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
        ]
        .into_iter()
        .enumerate()
        {
            widths[at] = if matches!(paints[index].border_style[at], "none" | "hidden") {
                0.0
            } else {
                match defaulted(name)? {
                    DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                        match keyword.as_str() {
                            "thin" => 1.0,
                            "thick" => 5.0,
                            _ => 3.0,
                        }
                    }
                    DefaultedValueRef::Compute(SpecifiedValue::Length(target)) => {
                        match own_length(target) {
                            Dim::Px(pixels) => pixels.max(0.0),
                            _ => 3.0,
                        }
                    }
                    DefaultedValueRef::ParentComputed => {
                        parent_index.map_or(3.0, |parent| border_widths[parent][at])
                    }
                    _ => 3.0,
                }
            };
        }
        border_widths.push(widths);
        let element_display = display.resolution(original).map(|value| value.display);
        let element_clear = match defaulted("clear")? {
            DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                clear_side(keyword.as_str())
            }
            DefaultedValueRef::ParentComputed => {
                parent_index.map_or(ClearSide::None, |parent| clears[parent])
            }
            _ => ClearSide::None,
        };
        clears.push(element_clear);
        let mut pending_pair = [None, None];
        let mut pair = [None, None];
        for (side, pseudo) in [PseudoElement::Before, PseudoElement::After]
            .into_iter()
            .enumerate()
        {
            let mut winners = PropertyWinners::new();
            cascade_positioned_rules(
                &mut winners,
                bundle.offers_for(&source, original),
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
            // Winners holding `var()`, as they parse for this target.
            bundle.resolve(&mut winners, original, Some(pseudo));
            let cascaded = |name: &'static str| {
                let property = id(name)?;
                winners
                    .defaulted(property, true)
                    .map_err(|_| E::Paint { property: name })
            };
            let mut content = None;
            if let DefaultedValueRef::Compute(SpecifiedValue::Content(list)) = cascaded("content")?
            {
                content = Some(list.clone());
            }
            // `inherit` takes the element's computed display and clear.
            let (hide_display, inline) = match cascaded("display")? {
                DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                    (keyword.as_str() == "none", keyword.as_str() == "inline")
                }
                DefaultedValueRef::ParentComputed => (
                    element_display == Some(Display::None),
                    element_display.is_none_or(|value| value == Display::Inline),
                ),
                _ => (false, true),
            };
            if hide_display {
                content = None;
            }
            // Only a generated box is block-level: an inside marker alone stays inline.
            let block = content.is_some() && !inline;
            let clear = match cascaded("clear")? {
                DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                    clear_side(keyword.as_str())
                }
                DefaultedValueRef::ParentComputed => element_clear,
                _ => ClearSide::None,
            };
            let marker_text = if side == 0 { marker.clone() } else { None };
            if content.is_none() && marker_text.is_none() {
                continue;
            }
            // With no content of its own there is no box, only an inside marker.
            let has_content = content.is_some();
            // A pseudo that generates nothing (`display: none`, or no `content`) affects no
            // counter (12.4.3); an inside marker stays.
            pending_pair[side] = Some(PendingContent {
                marker: marker_text,
                items: content.unwrap_or_default(),
                actions: if hide_display || !has_content {
                    CounterActions::default()
                } else {
                    CounterActions {
                        reset: directives(cascaded("counter-reset")?, &actions.reset),
                        increment: directives(cascaded("counter-increment")?, &actions.increment),
                    }
                },
                quotes: quotes_of(cascaded("quotes")?, element_quotes.clone()),
            });
            // Filled in document order below, once every counter has been seen.
            let text = String::new();
            let owner = styles[index].clone();
            let size = match cascaded("font-size")? {
                DefaultedValueRef::Compute(SpecifiedValue::Length(target)) => match target {
                    LengthTarget::Absolute(length) => length
                        .to_nonnegative_f32(limits.numeric)
                        .map_or(font_size, |c| c.value),
                    LengthTarget::Relative(number, unit) => number
                        .scaled_finite_f32(
                            unit.fallback_basis(font_size, root_font_size),
                            limits.numeric,
                        )
                        .map_or(font_size, |c| c.value),
                    LengthTarget::Percentage(number) => number
                        .percentage_finite_f32(font_size, limits.numeric)
                        .map_or(font_size, |c| c.value),
                    // Its percentage is of the element's font size, as `em` is.
                    LengthTarget::Calc(sum) => (sum.fallback_pixels(font_size, root_font_size)
                        + sum.percent() / 100.0 * font_size)
                        .max(0.0),
                    LengthTarget::Auto => font_size,
                },
                DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                    let table = FontSizeTable::new(font_medium).map_err(E::FontMedium)?;
                    match keyword.as_str() {
                        "xx-small" => table.entries()[0],
                        "x-small" => table.entries()[1],
                        "small" => table.entries()[2],
                        "medium" => table.entries()[3],
                        "large" => table.entries()[4],
                        "x-large" => table.entries()[5],
                        "xx-large" => table.entries()[6],
                        "larger" => table.larger(font_size).map_or(font_size, |c| c.value),
                        "smaller" => table.smaller(font_size).map_or(font_size, |c| c.value),
                        _ => font_size,
                    }
                }
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
            let (mut underline, mut line_through, mut overline) =
                (owner.underline, owner.line_through, owner.overline);
            // The element's lines run through its generated content in its color and size.
            let from = |on: bool, from: Option<usize>| on.then(|| from.unwrap_or(index));
            let (mut underline_from, mut line_through_from, mut overline_from) = (
                from(owner.underline, owner.underline_from),
                from(owner.line_through, owner.line_through_from),
                from(owner.overline, owner.overline_from),
            );
            if let DefaultedValueRef::Compute(SpecifiedValue::TextDecoration(own)) =
                cascaded("text-decoration")?
            {
                underline |= own.underline();
                line_through |= own.line_through();
                overline |= own.overline();
                if own.underline() {
                    underline_from = None;
                }
                if own.line_through() {
                    line_through_from = None;
                }
                if own.overline() {
                    overline_from = None;
                }
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
            let keyword_of = |name: &'static str, inherited: &'static str| {
                Ok::<_, TreeLayoutError>(match cascaded(name)? {
                    DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                        keyword.as_str()
                    }
                    _ => inherited,
                })
            };
            let family = match cascaded("font-family")? {
                DefaultedValueRef::Compute(SpecifiedValue::FontFamily(list)) => family_of(list),
                _ => owner.family.clone(),
            };
            let white_space = keyword_of("white-space", owner.white_space)?;
            let text_transform = keyword_of("text-transform", owner.text_transform)?;
            let shown =
                keyword_of("visibility", if visible { "visible" } else { "hidden" })? == "visible";
            // A block-level one is a box of its own: lengths in its own font size.
            let length = |target: &LengthTarget, size: f32| match target {
                LengthTarget::Absolute(value) => {
                    Dim::Px(value.to_finite_f32(limits.numeric).map_or(0.0, |c| c.value))
                }
                LengthTarget::Relative(value, unit) => Dim::Px(
                    value
                        .scaled_finite_f32(
                            unit.fallback_basis(size, root_font_size),
                            limits.numeric,
                        )
                        .map_or(0.0, |c| c.value),
                ),
                LengthTarget::Percentage(value) => Dim::Percent(
                    value
                        .percentage_finite_f32(100.0, limits.numeric)
                        .map_or(0.0, |c| c.value),
                ),
                LengthTarget::Auto => Dim::Auto,
                LengthTarget::Calc(sum) => calc_dim(*sum, size, root_font_size),
            };
            // `inherit` takes the element's computed value (`box_lengths`).
            let dim = |name: &'static str, initial: Dim| {
                Ok::<_, TreeLayoutError>(match cascaded(name)? {
                    DefaultedValueRef::Compute(SpecifiedValue::Length(target)) => {
                        length(target, size)
                    }
                    DefaultedValueRef::ParentComputed => BOX_LENGTHS
                        .iter()
                        .position(|own| *own == name)
                        .map_or(initial, |at| box_lengths[index][at]),
                    _ => initial,
                })
            };
            // An inline one's sides wrap its text as an inline element's do (10.8).
            let frame = if block || has_content {
                let mut border_style = ["none"; 4];
                let mut border_width = [0.0_f32; 4];
                let mut border_color = [None; 4];
                for at in 0..4 {
                    // `inherit` takes the element's style; unset, it is none.
                    let name = [
                        "border-top-style",
                        "border-right-style",
                        "border-bottom-style",
                        "border-left-style",
                    ][at];
                    let style = match cascaded(name)? {
                        DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                            keyword.as_str()
                        }
                        DefaultedValueRef::ParentComputed => paints[index].border_style[at],
                        _ => "none",
                    };
                    border_style[at] = style;
                    let width = match dim(
                        [
                            "border-top-width",
                            "border-right-width",
                            "border-bottom-width",
                            "border-left-width",
                        ][at],
                        Dim::Px(3.0),
                    )? {
                        Dim::Px(width) => width.max(0.0),
                        _ => 3.0,
                    };
                    let width_name = [
                        "border-top-width",
                        "border-right-width",
                        "border-bottom-width",
                        "border-left-width",
                    ][at];
                    let width = match cascaded(width_name)? {
                        DefaultedValueRef::Compute(SpecifiedValue::Keyword(keyword)) => {
                            match keyword.as_str() {
                                "thin" => 1.0,
                                "medium" => 3.0,
                                "thick" => 5.0,
                                _ => width,
                            }
                        }
                        // Inherited, the element's computed width (`border_widths`).
                        DefaultedValueRef::ParentComputed => border_widths[index][at],
                        _ => width,
                    };
                    border_width[at] = if matches!(style, "none" | "hidden") {
                        0.0
                    } else {
                        width
                    };
                    let color = compute_color(
                        &winners,
                        border[at],
                        ColorComputationContext {
                            parent: Some(row[2 + at]),
                            current_color: Some(pseudo_color),
                            user_agent_color: Some(&BLACK),
                        },
                    )
                    .map_err(|error| E::Color {
                        projected: index,
                        error,
                    })?;
                    border_color[at] = rgb(color.value(), limits.numeric);
                }
                let four = |names: [&'static str; 4]| -> Result<[Dim; 4], TreeLayoutError> {
                    Ok([
                        dim(names[0], Dim::Px(0.0))?,
                        dim(names[1], Dim::Px(0.0))?,
                        dim(names[2], Dim::Px(0.0))?,
                        dim(names[3], Dim::Px(0.0))?,
                    ])
                };
                Some(GeneratedFrame {
                    width: dim("width", Dim::Auto)?,
                    height: dim("height", Dim::Auto)?,
                    margin: four(["margin-top", "margin-right", "margin-bottom", "margin-left"])?,
                    padding: four([
                        "padding-top",
                        "padding-right",
                        "padding-bottom",
                        "padding-left",
                    ])?,
                    border: border_width,
                    paint: BoxPaint {
                        visible: shown,
                        background: rgb(pseudo_background.value(), limits.numeric),
                        border: border_color,
                        border_style,
                        ..BoxPaint::default()
                    },
                })
            } else {
                None
            };
            // Its own line height, in its own font size; `inherit` the element's.
            let line_height = match cascaded("line-height")? {
                DefaultedValueRef::Compute(value) => {
                    line_height_of(value, size, root_font_size, limits.numeric)
                }
                _ => owner.line_height,
            };
            // Alignment and indent belong to a block container: only a block-level one has
            // its own; an inline one's lines are the element's.
            let (align, text_indent, text_indent_percent) = if block {
                let align =
                    compute_text_align(&winners, Some(aligns[index])).map_err(|_| E::Paint {
                        property: "text-align",
                    })?;
                let (indent, percent) = match cascaded("text-indent")? {
                    DefaultedValueRef::Compute(SpecifiedValue::Length(target)) => {
                        match length(target, size) {
                            Dim::Px(pixels) => (pixels, 0.0),
                            Dim::Percent(percent) => (0.0, percent),
                            Dim::Mix(pixels, percent) | Dim::Clamped(pixels, percent) => {
                                (pixels, percent)
                            }
                            Dim::Auto => (0.0, 0.0),
                        }
                    }
                    DefaultedValueRef::ParentComputed => {
                        (owner.text_indent, owner.text_indent_percent)
                    }
                    _ => (0.0, 0.0),
                };
                (align_keyword(align, rtl), indent, percent)
            } else {
                (owner.align, owner.text_indent, owner.text_indent_percent)
            };
            // An inside marker alone is its list item's text: the `:before` generates nothing.
            let (size, color, shown, style) = if has_content {
                (
                    size,
                    rgb(pseudo_color.value(), limits.numeric),
                    shown,
                    TextStyle {
                        weight,
                        italic,
                        underline,
                        line_through,
                        underline_from,
                        line_through_from,
                        overline,
                        overline_from,
                        family,
                        white_space,
                        text_transform,
                        line_height,
                        align,
                        text_indent,
                        text_indent_percent,
                        ..owner
                    },
                )
            } else {
                (
                    font_size,
                    rgb(current.value(), limits.numeric),
                    visible,
                    owner,
                )
            };
            pair[side] = Some(GeneratedRun {
                text,
                size,
                color,
                // Only a box paints a background; an inside marker alone has none.
                background: rgb(pseudo_background.value(), limits.numeric).filter(|_| has_content),
                visible: shown,
                style,
                block,
                clear,
                frame,
                marker: None,
            });
        }
        generated.push(pair);
        pending.push(pending_pair);
        counter_actions.push(actions);
        quote_pairs.push(element_quotes);
    }
    let parents: Vec<Option<usize>> = projection
        .original
        .iter()
        .map(|&original| {
            nodes[original]
                .parent()
                .map(|parent| projection.projected[parent])
        })
        .collect();
    let mut children = vec![Vec::new(); parents.len()];
    for (index, parent) in parents.iter().enumerate() {
        if let Some(parent) = parent {
            children[*parent].push(index);
        }
    }
    let attribute = |at: usize, name: &str| {
        nodes[projection.original[at]]
            .metadata()
            .attribute(name)
            .map(str::to_owned)
    };
    let mut walk = CounterWalk {
        children: &children,
        parents: &parents,
        rendered: &rendered,
        actions: &counter_actions,
        pending: &pending,
        attribute: &attribute,
        scopes: vec![Vec::new(); parents.len() + 1],
        quotes: 0,
        out: vec![[None, None]; parents.len()],
    };
    walk.walk(0);
    for (index, texts) in walk.out.into_iter().enumerate() {
        for (side, text) in texts.into_iter().enumerate() {
            if let (Some(made), Some(text)) = (&mut generated[index][side], text) {
                made.text = text;
                made.marker = pending[index][side]
                    .as_ref()
                    .and_then(|content| content.marker.as_ref())
                    .map(String::len);
            }
        }
    }
    Ok(TreePaint {
        paints,
        styles,
        generated,
        markers,
        spacings,
        backgrounds,
        collapses,
        vertical_aligns,
        captions_below,
        important,
        em_lengths,
        marker_images,
        outside_items,
        grids,
        grid_items,
        transitions,
        animations,
        gradients,
    })
}
