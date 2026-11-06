//! General CSS 2.2 block flow over an arbitrary box tree: nested normal blocks, floats
//! and block formatting contexts. Covers 8.3.1 margin collapsing (siblings, parent and
//! child, collapse-through), 9.5.1 float placement, 9.5.2 clearance, 9.5 new-BFC float
//! avoidance, 10.3.3/10.3.5 widths and 10.6.3/10.6.7 heights.
//!
//! Leaf content is an opaque measured rectangle. Inline layout, relative and absolute
//! positioning and paint are the caller's. Policy: a clearance equal to zero is no clearance (9.5.2 "past"),
//! and a float whose margin box would be negative occupies zero extent (user U7=A by
//! delegated decision), keeping its border box where its margins put it.
use super::{
    block::{AutoLength, BlockWidthError, BlockWidthInput, Direction, solve_block_width},
    clearance::{ClearSide, ClearanceFloatError, lowest_clearance_float_bottom},
    floats::{
        Band, FloatError, FloatRequest, FloatSide, PlacedFloat, exclusion_band, next_band_edge,
        place_float,
    },
    margins::{MarginError, MarginExtrema},
    shrink::{ShrinkWidthError, ShrinkWidthInput, solve_shrink_width},
};
use std::num::NonZeroUsize;

/// A specified length before its containing block is known.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dim {
    Auto,
    Px(f32),
    /// Percent of the containing block's width, or its height for vertical sizes.
    Percent(f32),
}

impl Dim {
    fn horizontal(self, basis: f32) -> AutoLength {
        match self {
            Self::Auto => AutoLength::Auto,
            Self::Px(value) => AutoLength::Value(value),
            Self::Percent(value) => AutoLength::Value(basis * value / 100.0),
        }
    }
    fn zero_auto(self, basis: f32) -> f32 {
        match self.horizontal(basis) {
            AutoLength::Auto => 0.0,
            AutoLength::Value(value) => value,
        }
    }
    /// A percentage of an indefinite basis computes to auto (10.5, 10.7).
    fn vertical(self, basis: Option<f32>) -> Option<f32> {
        match (self, basis) {
            (Self::Auto, _) | (Self::Percent(_), None) => None,
            (Self::Px(value), _) => Some(value),
            (Self::Percent(value), Some(basis)) => Some(basis * value / 100.0),
        }
    }
}

/// Alignment along an axis: start, middle, end. For a line of inline-blocks (16.2) and a
/// cell's content in its row (17.5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAlign {
    Left,
    Center,
    Right,
}

/// A box's part in a table (17.2). `border-collapse` is treated as separate borders with
/// no spacing, and extra specified table height is not distributed to rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TableRole {
    /// Horizontal and vertical `border-spacing`.
    Table {
        spacing: [f32; 2],
    },
    HeaderGroup,
    BodyGroup,
    FooterGroup,
    Row,
    Cell,
    Caption,
}

/// One box. `children` index the same arena; the arena must form one tree rooted at 0.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowBox {
    pub children: Vec<usize>,
    pub direction: Direction,
    pub float: Option<FloatSide>,
    pub clear: ClearSide,
    pub table: Option<TableRole>,
    /// A cell's columns and rows, at least 1 (17.5: spans come from the document language).
    pub col_span: usize,
    pub row_span: usize,
    /// Where a cell's content sits in a taller row (17.5.3). A baseline is treated as top.
    pub cell_align: RowAlign,
    /// A caption placed below the table (`caption-side: bottom`).
    pub caption_below: bool,
    /// An anonymous block whose children are inline-blocks, placed in wrapping lines
    /// with this alignment. Each line aligns its boxes' bottom margin edges: the baseline
    /// of an empty inline-block, and an approximation for one holding text.
    pub inline_row: Option<RowAlign>,
    /// Absolutely positioned: takes no part in flow; the result holds only its static
    /// position, in `border.left` and `border.top`, for the caller's 10.3.7/10.6.4 pass.
    pub out_of_flow: bool,
    /// `overflow` other than visible, or any other 9.4.1 establishing condition.
    pub establishes_bfc: bool,
    pub width: Dim,
    pub min_width: Dim,
    pub max_width: Dim,
    pub height: Dim,
    pub min_height: Dim,
    pub max_height: Dim,
    /// Top, right, bottom, left.
    pub margin: [Dim; 4],
    pub padding: [Dim; 4],
    pub border: [f32; 4],
    /// Measured content of a leaf; ignored when the box has children.
    pub content: Option<FlowContent>,
}

impl Default for FlowBox {
    fn default() -> Self {
        Self {
            children: Vec::new(),
            direction: Direction::Ltr,
            float: None,
            clear: ClearSide::None,
            table: None,
            col_span: 1,
            row_span: 1,
            cell_align: RowAlign::Left,
            caption_below: false,
            inline_row: None,
            out_of_flow: false,
            establishes_bfc: false,
            width: Dim::Auto,
            min_width: Dim::Auto,
            max_width: Dim::Auto,
            height: Dim::Auto,
            min_height: Dim::Auto,
            max_height: Dim::Auto,
            margin: [Dim::Px(0.0); 4],
            padding: [Dim::Px(0.0); 4],
            border: [0.0; 4],
            content: None,
        }
    }
}

/// Leaf measurements; the caller shapes text at the used width beforehand.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FlowContent {
    pub height: f32,
    pub preferred_width: f32,
    pub preferred_min_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FlowRect {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FlowGeometry {
    pub border: FlowRect,
    pub padding: FlowRect,
    pub content: FlowRect,
    /// Used top, right, bottom, left.
    pub margin: [f32; 4],
    pub clearance: Option<f32>,
    /// Adjoining top and bottom margins (8.3.1).
    pub collapsed_through: bool,
    /// Where a leaf's content goes when floats displace it from its content box: the
    /// band clear of every float its lines overlap, starting below them if none has
    /// room. Lines past a shorter float stay this narrow: real line boxes would widen
    /// them, so the result is conservative and never overlaps a float.
    pub leaf_band: Option<FlowRect>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowViewport {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    /// The initial containing block height, when the host knows it.
    pub height: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowLimits {
    pub boxes: NonZeroUsize,
    pub depth: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlowError {
    Boxes,
    Depth,
    /// A child index is out of range, repeated, or the root.
    Tree {
        node: usize,
    },
    Nonfinite {
        node: usize,
    },
    Width {
        node: usize,
        error: BlockWidthError,
    },
    Shrink {
        node: usize,
        error: ShrinkWidthError,
    },
    Float {
        node: usize,
        error: FloatError,
    },
    Clear {
        node: usize,
        error: ClearanceFloatError,
    },
    Margin {
        node: usize,
        error: MarginError,
    },
}

impl std::fmt::Display for FlowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "block flow: {self:?}")
    }
}
impl std::error::Error for FlowError {}

#[derive(Clone, Copy)]
struct Containing {
    left: f32,
    width: f32,
    height: Option<f32>,
    /// Known once the containing block's own top edge has settled.
    top: Option<f32>,
    direction: Direction,
}

/// The vertical cursor of one block formatting context.
struct Flow {
    y: f32,
    pending: MarginExtrema,
    /// Open boxes whose top border edge waits on an adjoining margin's resolution.
    unsettled: Vec<usize>,
    floats: Vec<PlacedFloat>,
    /// The lowest earlier block outer top, for 9.5.1 rule 5.
    block_limit: f32,
    /// Floats and static positions inside an open parent: 9.5.1 rule 4 needs that
    /// parent's top, which its adjoining margins have not fixed yet.
    deferred: Vec<(usize, Containing)>,
    /// Collapsed-through boxes whose margins joined an open parent's (8.3.1): their
    /// top border edge is that parent's.
    open_through: Vec<usize>,
    /// The open run ends in an empty box with clearance, whose bottom margin must not
    /// collapse with its parent's (8.3.1).
    clearance_through: bool,
}

impl Flow {
    fn new(y: f32) -> Self {
        Self {
            y,
            pending: MarginExtrema::default(),
            unsettled: Vec::new(),
            floats: Vec::new(),
            block_limit: y,
            deferred: Vec::new(),
            open_through: Vec::new(),
            clearance_through: false,
        }
    }
    fn position(&self) -> f32 {
        self.y + self.pending.collapsed()
    }
    fn adjoin(&mut self, value: f32, node: usize) -> Result<(), FlowError> {
        self.pending
            .extend(&[value], NonZeroUsize::MIN)
            .map_err(|error| FlowError::Margin { node, error })
    }
    fn lowest(&self, clear: ClearSide, node: usize) -> Result<Option<f32>, FlowError> {
        let Some(limit) = NonZeroUsize::new(self.floats.len()) else {
            return Ok(None);
        };
        lowest_clearance_float_bottom(&self.floats, clear, limit)
            .map_err(|error| FlowError::Clear { node, error })
    }
}

/// One cell's place in the table grid.
#[derive(Debug, Clone, Copy)]
struct GridCell {
    cell: usize,
    row: usize,
    column: usize,
    col_span: usize,
    row_span: usize,
}

struct Solver<'a> {
    boxes: &'a [FlowBox],
    geometry: Vec<FlowGeometry>,
    /// The height the last finished box's contents flowed to, whatever its `height` says.
    last_flowed: f32,
    /// A leaf's content height at its used width, overriding the static measurement.
    text_height: &'a mut dyn FnMut(usize, f32) -> Option<f32>,
}

/// Lays out the tree rooted at index 0 as one block formatting context root.
///
/// # Errors
/// Rejects a malformed tree, nonfinite measurements, and exceeded limits, and returns
/// the failing node for a solver error. Nothing is published on failure.
pub fn layout_block_flow(
    boxes: &[FlowBox],
    viewport: FlowViewport,
    limits: FlowLimits,
) -> Result<Vec<FlowGeometry>, FlowError> {
    layout_block_flow_measured(boxes, viewport, limits, &mut |_, _| None)
}

/// Lays out as [`layout_block_flow`], asking `text_height(node, width)` for each leaf's
/// content height at its used width: wrapped text depends on the width it receives.
/// `None` keeps the leaf's static [`FlowContent`] height.
///
/// # Errors
/// As [`layout_block_flow`].
pub fn layout_block_flow_measured(
    boxes: &[FlowBox],
    viewport: FlowViewport,
    limits: FlowLimits,
    text_height: &mut dyn FnMut(usize, f32) -> Option<f32>,
) -> Result<Vec<FlowGeometry>, FlowError> {
    if boxes.is_empty() || boxes.len() > limits.boxes.get() {
        return Err(FlowError::Boxes);
    }
    validate(boxes)?;
    if tree_depth(boxes) > limits.depth.get() {
        return Err(FlowError::Depth);
    }
    let mut solver = Solver {
        boxes,
        geometry: vec![FlowGeometry::default(); boxes.len()],
        last_flowed: 0.0,
        text_height,
    };
    let containing = Containing {
        left: viewport.left,
        width: viewport.width,
        height: viewport.height,
        top: Some(viewport.top),
        direction: boxes[0].direction,
    };
    solver.layout_root(containing, viewport.top)?;
    Ok(solver.geometry)
}

/// The (preferred minimum, preferred) content widths of a box, for shrink-to-fit.
#[must_use]
pub fn preferred_content_widths(boxes: &[FlowBox], node: usize) -> (f32, f32) {
    Solver {
        boxes,
        geometry: Vec::new(),
        last_flowed: 0.0,
        text_height: &mut |_, _| None,
    }
    .measure(node)
}

fn validate(boxes: &[FlowBox]) -> Result<(), FlowError> {
    let mut seen = vec![false; boxes.len()];
    seen[0] = true;
    for (node, value) in boxes.iter().enumerate() {
        for &child in &value.children {
            if child >= boxes.len() || seen[child] {
                return Err(FlowError::Tree { node });
            }
            seen[child] = true;
        }
        let dims = [
            value.width,
            value.min_width,
            value.max_width,
            value.height,
            value.min_height,
            value.max_height,
        ];
        let finite_dims =
            dims.iter()
                .chain(&value.margin)
                .chain(&value.padding)
                .all(|dim| match dim {
                    Dim::Auto => true,
                    Dim::Px(number) | Dim::Percent(number) => number.is_finite(),
                });
        let content = value.content.unwrap_or_default();
        if !finite_dims
            || !value.border.iter().all(|edge| edge.is_finite())
            || !content.height.is_finite()
            || !content.preferred_width.is_finite()
            || !content.preferred_min_width.is_finite()
        {
            return Err(FlowError::Nonfinite { node });
        }
    }
    if let Some(node) = seen.iter().position(|reached| !reached) {
        return Err(FlowError::Tree { node });
    }
    Ok(())
}

fn tree_depth(boxes: &[FlowBox]) -> usize {
    let mut deepest = 0;
    let mut stack = vec![(0, 1)];
    while let Some((node, depth)) = stack.pop() {
        deepest = deepest.max(depth);
        stack.extend(boxes[node].children.iter().map(|&child| (child, depth + 1)));
    }
    deepest
}

fn rect(left: f32, right: f32, top: f32, bottom: f32) -> FlowRect {
    FlowRect {
        left,
        right,
        top,
        bottom,
    }
}

impl Solver<'_> {
    fn leaf_height(&mut self, node: usize, width: f32) -> Option<f32> {
        let value = &self.boxes[node];
        if !value.children.is_empty() {
            return None;
        }
        (self.text_height)(node, width).or(value.content.map(|content| content.height))
    }

    fn paddings(&self, node: usize, basis: f32) -> [f32; 4] {
        self.boxes[node].padding.map(|dim| dim.zero_auto(basis))
    }

    /// Longest-line and narrowest-line content widths (10.3.5's preferred widths).
    fn measure(&self, node: usize) -> (f32, f32) {
        let value = &self.boxes[node];
        if value.inline_row.is_some() {
            // A row wraps only as it must: its longest line holds every box.
            let (mut minimum, mut preferred) = (0.0_f32, 0.0_f32);
            for &child in &value.children {
                if self.boxes[child].float.is_none() && !self.boxes[child].out_of_flow {
                    let (child_min, child_preferred) = self.outer_measure(child);
                    minimum = minimum.max(child_min);
                    preferred += child_preferred;
                }
            }
            return (minimum, preferred.max(minimum));
        }
        if let Some(TableRole::Table { spacing }) = value.table {
            let (rows, captions) = self.table_parts(node);
            let (mins, prefs) = self.table_columns(&rows);
            let gaps = spacing[0] * (mins.len() + 1) as f32;
            let mut minimum = mins.iter().sum::<f32>() + gaps;
            let mut preferred = prefs.iter().sum::<f32>() + gaps;
            for caption in captions {
                let (caption_min, caption_preferred) = self.outer_measure(caption);
                minimum = minimum.max(caption_min);
                preferred = preferred.max(caption_preferred);
            }
            return (minimum, preferred);
        }
        if value.children.is_empty() {
            let content = value.content.unwrap_or_default();
            return (content.preferred_min_width, content.preferred_width);
        }
        let (mut minimum, mut preferred, mut row) = (0.0_f32, 0.0_f32, 0.0_f32);
        for &child in &value.children {
            if self.boxes[child].out_of_flow {
                continue;
            }
            let (child_min, child_preferred) = self.outer_measure(child);
            minimum = minimum.max(child_min);
            if self.boxes[child].float.is_some() {
                row += child_preferred;
            } else {
                preferred = preferred.max(row).max(child_preferred);
                row = 0.0;
            }
        }
        // Negative margins can shrink a float row below the widest single child.
        (minimum, preferred.max(row).max(minimum))
    }

    /// The table's rows in display order (17.2: header, body, footer), and captions.
    fn table_parts(&self, table: usize) -> (Vec<usize>, Vec<usize>) {
        let (mut head, mut body, mut foot, mut captions) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for &child in &self.boxes[table].children {
            let inner = || self.boxes[child].children.iter().copied();
            match self.boxes[child].table {
                Some(TableRole::Row) => body.push(child),
                Some(TableRole::HeaderGroup) => head.extend(inner()),
                Some(TableRole::BodyGroup) => body.extend(inner()),
                Some(TableRole::FooterGroup) => foot.extend(inner()),
                Some(TableRole::Caption) => captions.push(child),
                _ => {}
            }
        }
        head.append(&mut body);
        head.append(&mut foot);
        (head, captions)
    }

    /// 17.5: cells claim grid slots row by row, skipping slots a rowspan from above holds.
    fn table_grid(&self, rows: &[usize]) -> (Vec<GridCell>, usize) {
        let mut held: Vec<usize> = Vec::new();
        let mut placed = Vec::new();
        let mut columns = 0;
        for (row_index, &row) in rows.iter().enumerate() {
            let mut column = 0;
            for &cell in &self.boxes[row].children {
                while held.get(column).is_some_and(|rest| *rest > 0) {
                    column += 1;
                }
                let value = &self.boxes[cell];
                let col_span = value.col_span.max(1);
                // A rowspan stops at the last row (17.5).
                let row_span = value.row_span.max(1).min(rows.len() - row_index);
                if held.len() < column + col_span {
                    held.resize(column + col_span, 0);
                }
                for rest in &mut held[column..column + col_span] {
                    *rest = (*rest).max(row_span);
                }
                placed.push(GridCell {
                    cell,
                    row: row_index,
                    column,
                    col_span,
                    row_span,
                });
                column += col_span;
                columns = columns.max(column);
            }
            for rest in &mut held {
                *rest = rest.saturating_sub(1);
            }
        }
        (placed, columns)
    }

    /// Each column's narrowest and preferred width (17.5.2.2): single-column cells first,
    /// then each spanning cell's shortfall shared evenly over its columns.
    fn table_columns(&self, rows: &[usize]) -> (Vec<f32>, Vec<f32>) {
        let (placed, columns) = self.table_grid(rows);
        let (mut mins, mut prefs) = (vec![0.0_f32; columns], vec![0.0_f32; columns]);
        for item in placed.iter().filter(|item| item.col_span == 1) {
            let (minimum, preferred) = self.outer_measure(item.cell);
            mins[item.column] = mins[item.column].max(minimum);
            prefs[item.column] = prefs[item.column].max(preferred);
        }
        let mut spanning: Vec<_> = placed.iter().filter(|item| item.col_span > 1).collect();
        spanning.sort_by_key(|item| item.col_span);
        for item in spanning {
            let range = item.column..item.column + item.col_span;
            // The gaps inside the span count toward the cell's room.
            let gaps = self.table_spacing_of(rows) * (item.col_span - 1) as f32;
            let (minimum, preferred) = self.outer_measure(item.cell);
            for (widths, wanted) in [(&mut mins, minimum), (&mut prefs, preferred)] {
                let shortfall = wanted - gaps - widths[range.clone()].iter().sum::<f32>();
                if shortfall > 0.0 {
                    for width in &mut widths[range.clone()] {
                        *width += shortfall / item.col_span as f32;
                    }
                }
            }
        }
        for column in 0..columns {
            prefs[column] = prefs[column].max(mins[column]);
        }
        (mins, prefs)
    }

    /// The horizontal spacing of the table a row list belongs to, found through a row.
    fn table_spacing_of(&self, rows: &[usize]) -> f32 {
        let Some(&first) = rows.first() else {
            return 0.0;
        };
        self.boxes
            .iter()
            .find_map(|candidate| match candidate.table {
                Some(TableRole::Table { spacing })
                    if candidate.children.contains(&first)
                        || candidate
                            .children
                            .iter()
                            .any(|child| self.boxes[*child].children.contains(&first)) =>
                {
                    Some(spacing[0])
                }
                _ => None,
            })
            .unwrap_or(0.0)
    }

    /// Lays out rows and captions in `inner`; returns the content bottom.
    fn layout_table(
        &mut self,
        node: usize,
        spacing: [f32; 2],
        inner: Containing,
        content_top: f32,
    ) -> Result<f32, FlowError> {
        let (rows, captions) = self.table_parts(node);
        let (mins, prefs) = self.table_columns(&rows);
        let count = mins.len();
        let available = (inner.width - spacing[0] * (count + 1) as f32).max(0.0);
        let (sum_min, sum_pref): (f32, f32) = (mins.iter().sum(), prefs.iter().sum());
        let widths: Vec<f32> = (0..count)
            .map(|column| {
                if available >= sum_pref {
                    // Extra room goes by preferred width, or evenly with none to go by.
                    let share = if sum_pref > 0.0 {
                        prefs[column] / sum_pref
                    } else {
                        1.0 / count as f32
                    };
                    prefs[column] + (available - sum_pref) * share
                } else if available > sum_min {
                    mins[column]
                        + (prefs[column] - mins[column]) * (available - sum_min)
                            / (sum_pref - sum_min)
                } else {
                    mins[column]
                }
            })
            .collect();
        let mut y = content_top;
        let cell_box = |solver: &mut Self, cell: usize, width: f32, basis: f32| {
            let padding = solver.paddings(cell, basis);
            let border = solver.boxes[cell].border;
            let content = (width - border[3] - padding[3] - padding[1] - border[1]).max(0.0);
            let bottom = solver.finish_bfc_box(
                cell,
                Containing {
                    left: 0.0,
                    width: content,
                    height: None,
                    top: Some(0.0),
                    direction: inner.direction,
                },
                (content, 0.0, 0.0),
                0.0,
                0.0,
                padding,
            )?;
            Ok::<f32, FlowError>(bottom)
        };
        // 17.4: a caption is a block above the table's grid, as wide as the table.
        let (below, above): (Vec<usize>, Vec<usize>) = captions
            .into_iter()
            .partition(|caption| self.boxes[*caption].caption_below);
        for caption in above {
            let height = cell_box(self, caption, inner.width, inner.width)?;
            self.translate(caption, inner.left, y);
            y += height;
        }
        let grid_top = y;
        y += spacing[1];
        let (placed, _) = self.table_grid(&rows);
        // First every cell at its width, which gives the rows their heights.
        let mut heights: Vec<f32> = rows
            .iter()
            .map(|row| self.boxes[*row].height.vertical(None).unwrap_or(0.0))
            .collect();
        let mut measures = Vec::with_capacity(placed.len());
        for item in &placed {
            let width = widths[item.column..item.column + item.col_span]
                .iter()
                .sum::<f32>()
                + spacing[0] * (item.col_span - 1) as f32;
            let bottom = cell_box(self, item.cell, width, inner.width)?;
            measures.push((width, bottom, self.last_flowed));
            if item.row_span == 1 {
                heights[item.row] = heights[item.row].max(bottom);
            }
        }
        // 17.5.3: a rowspan's extra height goes to its last row.
        let mut order: Vec<usize> = (0..placed.len()).collect();
        order.sort_by_key(|index| placed[*index].row_span);
        for index in order {
            let item = placed[index];
            if item.row_span > 1 {
                let range = item.row..item.row + item.row_span;
                let span = heights[range.clone()].iter().sum::<f32>()
                    + spacing[1] * (item.row_span - 1) as f32;
                let shortfall = measures[index].1 - span;
                if shortfall > 0.0 {
                    heights[range.end - 1] += shortfall;
                }
            }
        }
        let mut tops = Vec::with_capacity(rows.len());
        for &height in &heights {
            tops.push(y);
            y += height + spacing[1];
        }
        for (item, (width, _, cell_natural)) in placed.iter().zip(measures) {
            let before: f32 =
                widths[..item.column].iter().sum::<f32>() + spacing[0] * (item.column + 1) as f32;
            let left = match inner.direction {
                Direction::Ltr => inner.left + before,
                Direction::Rtl => inner.left + inner.width - before - width,
            };
            let top = tops[item.row];
            let height = heights[item.row..item.row + item.row_span]
                .iter()
                .sum::<f32>()
                + spacing[1] * (item.row_span - 1) as f32;
            let cell = item.cell;
            self.translate(cell, left, top);
            // 17.5.3: every cell fills its rows; its content sits as `vertical-align` says.
            let geometry = &mut self.geometry[cell];
            let extra = top + height - geometry.border.bottom;
            geometry.border.bottom += extra;
            geometry.padding.bottom += extra;
            // The room the content can move in is the box less its frame.
            let top_frame = geometry.content.top - geometry.border.top;
            let bottom_frame =
                self.boxes[cell].border[2] + self.boxes[cell].padding[2].zero_auto(inner.width);
            let room = height - top_frame - bottom_frame - cell_natural;
            let offset = match self.boxes[cell].cell_align {
                RowAlign::Left => 0.0,
                RowAlign::Center => (room / 2.0).max(0.0),
                RowAlign::Right => room.max(0.0),
            };
            if offset != 0.0 {
                let geometry = &mut self.geometry[cell];
                geometry.content.top += offset;
                geometry.content.bottom = geometry.content.top + cell_natural;
                if let Some(band) = &mut geometry.leaf_band {
                    band.top += offset;
                    band.bottom += offset;
                }
                for child in self.boxes[cell].children.clone() {
                    self.translate(child, 0.0, offset);
                }
            }
        }
        for (index, &row) in rows.iter().enumerate() {
            let area = rect(
                inner.left,
                inner.left + inner.width,
                tops[index],
                tops[index] + heights[index],
            );
            self.geometry[row] = FlowGeometry {
                border: area,
                padding: area,
                content: area,
                ..FlowGeometry::default()
            };
        }
        for caption in below {
            let height = cell_box(self, caption, inner.width, inner.width)?;
            self.translate(caption, inner.left, y);
            y += height;
        }
        for &child in &self.boxes[node].children.clone() {
            if matches!(
                self.boxes[child].table,
                Some(TableRole::HeaderGroup | TableRole::BodyGroup | TableRole::FooterGroup)
            ) {
                let group = &self.boxes[child].children;
                let span = group.iter().fold(None, |span: Option<(f32, f32)>, &row| {
                    let edge = self.geometry[row].border;
                    Some(span.map_or((edge.top, edge.bottom), |(top, bottom)| {
                        (top.min(edge.top), bottom.max(edge.bottom))
                    }))
                });
                let (top, bottom) = span.unwrap_or((grid_top, grid_top));
                let area = rect(inner.left, inner.left + inner.width, top, bottom);
                self.geometry[child] = FlowGeometry {
                    border: area,
                    padding: area,
                    content: area,
                    ..FlowGeometry::default()
                };
            }
        }
        Ok(y)
    }

    fn outer_measure(&self, node: usize) -> (f32, f32) {
        let value = &self.boxes[node];
        let side = |dims: [Dim; 4]| dims[1].zero_auto(0.0) + dims[3].zero_auto(0.0);
        let frame = side(value.margin) + side(value.padding) + value.border[1] + value.border[3];
        let lower = match value.min_width {
            Dim::Px(limit) => limit,
            _ => 0.0,
        };
        let upper = match value.max_width {
            Dim::Px(limit) => Some(limit),
            _ => None,
        };
        let (minimum, preferred) = match value.width {
            Dim::Px(width) => (width, width),
            _ => self.measure(node),
        };
        (
            frame + clamp(minimum, lower, upper),
            frame + clamp(preferred, lower, upper),
        )
    }

    fn layout_root(&mut self, containing: Containing, top: f32) -> Result<(), FlowError> {
        let node = 0;
        let value = &self.boxes[node];
        let width = self.block_width(node, containing)?;
        let basis = containing.width;
        let padding = self.paddings(node, basis);
        let mt = value.margin[0].zero_auto(basis);
        let mb = value.margin[2].zero_auto(basis);
        let border_top = top + mt;
        self.finish_bfc_box(
            node,
            containing,
            width,
            containing.left + width.1,
            border_top,
            padding,
        )?;
        self.geometry[node].margin = [mt, width.2, mb, width.1];
        Ok(())
    }

    /// Returns the used (width, margin left, margin right). 17.5.2.2: a table is never
    /// narrower than its columns' minimum, and an auto one only as wide as they prefer.
    fn block_width(&self, node: usize, cb: Containing) -> Result<(f32, f32, f32), FlowError> {
        let value = &self.boxes[node];
        let padding = self.paddings(node, cb.width);
        let first = self.block_width_at(node, cb, value.width.horizontal(cb.width), padding)?;
        if !matches!(value.table, Some(TableRole::Table { .. })) {
            return Ok(first);
        }
        let (minimum, preferred) = self.measure(node);
        let wanted = if value.width == Dim::Auto {
            first.0.min(preferred)
        } else {
            first.0
        };
        // Margins are solved for the final width, so auto margins still center it.
        self.block_width_at(node, cb, AutoLength::Value(wanted.max(minimum)), padding)
    }

    fn block_width_at(
        &self,
        node: usize,
        cb: Containing,
        width: AutoLength,
        padding: [f32; 4],
    ) -> Result<(f32, f32, f32), FlowError> {
        let value = &self.boxes[node];
        let result = solve_block_width(BlockWidthInput {
            containing_width: cb.width,
            direction: cb.direction,
            width,
            margin_left: value.margin[3].horizontal(cb.width),
            margin_right: value.margin[1].horizontal(cb.width),
            border_left: value.border[3],
            border_right: value.border[1],
            padding_left: padding[3],
            padding_right: padding[1],
            min_width: value.min_width.zero_auto(cb.width),
            max_width: match value.max_width {
                Dim::Auto => None,
                other => Some(other.zero_auto(cb.width)),
            },
        })
        .map_err(|error| FlowError::Width { node, error })?;
        Ok((result.width, result.margin_left, result.margin_right))
    }

    fn lay_children(
        &mut self,
        node: usize,
        flow: &mut Flow,
        cb: Containing,
    ) -> Result<(), FlowError> {
        for &child in &self.boxes[node].children.clone() {
            if let Some(align) = self.boxes[child].inline_row {
                let top = flow.position();
                self.layout_inline_row(child, align, flow, cb)?;
                let geometry = &mut self.geometry[child];
                geometry.border.top = top;
                geometry.padding.top = top;
                geometry.content.top = top;
                flow.block_limit = flow.block_limit.max(top);
            } else if self.boxes[child].out_of_flow {
                if flow.unsettled.is_empty() {
                    self.record_static(child, flow.position(), cb);
                } else {
                    flow.deferred.push((child, cb));
                }
            } else if self.boxes[child].float.is_some() {
                self.layout_float(child, flow, cb)?;
            } else if self.boxes[child].establishes_bfc {
                self.layout_bfc_block(child, flow, cb)?;
            } else {
                self.layout_normal(child, flow, cb)?;
            }
        }
        Ok(())
    }

    fn set_top(&mut self, node: usize, top: f32) {
        self.geometry[node].border.top = top;
    }

    /// 10.3.7/10.6.4: the static margin edge, with the run collapsed so far.
    fn record_static(&mut self, node: usize, top: f32, cb: Containing) {
        let border = &mut self.geometry[node].border;
        border.left = cb.left;
        border.right = cb.left + cb.width;
        border.top = top;
    }

    /// The open parents' top is now known: place what was waiting for it.
    fn flush(&mut self, flow: &mut Flow, top: f32) -> Result<(), FlowError> {
        for node in std::mem::take(&mut flow.open_through) {
            let geometry = &mut self.geometry[node];
            for edges in [
                &mut geometry.border,
                &mut geometry.padding,
                &mut geometry.content,
            ] {
                edges.top = top;
                edges.bottom = top;
            }
        }
        for (node, cb) in std::mem::take(&mut flow.deferred) {
            if self.boxes[node].out_of_flow {
                self.record_static(node, top, cb);
            } else {
                let cb = Containing {
                    top: Some(top),
                    ..cb
                };
                self.place_float_box(node, flow, cb, top)?;
            }
        }
        Ok(())
    }

    fn settle(&mut self, flow: &mut Flow) -> Result<f32, FlowError> {
        let top = flow.position();
        for node in std::mem::take(&mut flow.unsettled) {
            self.set_top(node, top);
        }
        flow.y = top;
        flow.pending = MarginExtrema::default();
        flow.clearance_through = false;
        self.flush(flow, top)?;
        Ok(top)
    }

    /// Applies clear to a box whose top margin already adjoins `flow.pending`. Returns
    /// the clearance. Open ancestors settle above it, without the box's own margin.
    fn apply_clear(
        &mut self,
        node: usize,
        flow: &mut Flow,
        top_margin: f32,
    ) -> Result<Option<f32>, FlowError> {
        let clear = self.boxes[node].clear;
        // Earlier floats waiting on an open parent must exist before clear reads them.
        // They take the parent's position so far; a parent later pushed down by this
        // box's own margin (only when no clearance results) is not re-placed.
        if clear != ClearSide::None && !flow.deferred.is_empty() {
            let top = flow.position();
            self.flush(flow, top)?;
        }
        let Some(bottom) = flow.lowest(clear, node)? else {
            flow.adjoin(top_margin, node)?;
            return Ok(None);
        };
        let mut joined = flow.pending;
        joined
            .extend(&[top_margin], NonZeroUsize::MIN)
            .map_err(|error| FlowError::Margin { node, error })?;
        let hypothetical = flow.y + joined.collapsed();
        if bottom <= hypothetical {
            flow.pending = joined;
            return Ok(None);
        }
        self.settle(flow)?;
        let clearance = bottom - (flow.y + top_margin);
        flow.y = bottom;
        Ok(Some(clearance))
    }

    fn layout_normal(
        &mut self,
        node: usize,
        flow: &mut Flow,
        cb: Containing,
    ) -> Result<(), FlowError> {
        let value = &self.boxes[node];
        let (width, ml, mr) = self.block_width(node, cb)?;
        let basis = cb.width;
        let padding = self.paddings(node, basis);
        let border = value.border;
        let mt = value.margin[0].zero_auto(basis);
        let mb = value.margin[2].zero_auto(basis);
        flow.clearance_through = false;
        let clearance = self.apply_clear(node, flow, mt)?;
        let mut settled = clearance.is_some();
        if settled {
            self.set_top(node, flow.y);
        }
        if border[0] + padding[0] != 0.0 {
            if !settled {
                let top = flow.position();
                self.set_top(node, top);
                self.settle(flow)?;
                settled = true;
            }
            flow.y += border[0] + padding[0];
        } else if !settled {
            flow.unsettled.push(node);
        }
        let border_left = cb.left + ml;
        let content_left = border_left + border[3] + padding[3];
        let height = value.height.vertical(cb.height);
        let minimum = value.min_height.vertical(cb.height).unwrap_or(0.0);
        let maximum = value.max_height.vertical(cb.height);
        let inner = Containing {
            left: content_left,
            width,
            height: height.map(|h| clamp(h, minimum, maximum)),
            top: settled.then_some(flow.y),
            direction: value.direction,
        };
        let mut leaf_band = None;
        // 9.5: lines shorten beside earlier floats; with no room they move below.
        let measured = self.leaf_height(node, width);
        if measured.is_some_and(|height| height != 0.0) {
            // Content fixes open parents' tops, which places floats deferred on them.
            self.settle(flow)?;
        }
        let leaf_height = if flow.floats.is_empty() || measured.is_none_or(|height| height == 0.0) {
            measured
        } else {
            let (band, height) = self.place_leaf(
                node,
                flow,
                content_left,
                width,
                measured.unwrap_or_default(),
            )?;
            leaf_band = band;
            Some(height)
        };
        if let Some(height) = leaf_height.filter(|height| *height != 0.0) {
            self.settle(flow)?;
            flow.y += height;
        }
        self.lay_children(node, flow, inner)?;
        let still_open = flow.unsettled.contains(&node);
        let separated = border[2] + padding[2] != 0.0;
        let zero_or_auto = height.is_none_or(|h| h == 0.0);
        if still_open && !separated && zero_or_auto && minimum == 0.0 {
            // 8.3.1: as with a nonzero bottom border, then the margins join the run.
            let top = flow.position();
            flow.unsettled.retain(|open| *open != node);
            flow.adjoin(mb, node)?;
            self.geometry[node].border = rect(
                border_left,
                border_left + border[3] + padding[3] + width + padding[1] + border[1],
                top,
                top,
            );
            self.geometry[node].padding = rect(
                border_left + border[3],
                content_left + width + padding[1],
                top,
                top,
            );
            self.geometry[node].content = rect(content_left, content_left + width, top, top);
            self.geometry[node].margin = [mt, mr, mb, ml];
            self.geometry[node].clearance = None;
            self.geometry[node].collapsed_through = true;
            flow.block_limit = flow.block_limit.max(top - mt);
            if flow.unsettled.is_empty() {
                self.flush(flow, top)?;
            } else {
                flow.open_through.push(node);
            }
            return Ok(());
        }
        if still_open {
            self.settle(flow)?;
        }
        let top = self.geometry[node].border.top;
        let content_top = top + border[0] + padding[0];
        // 8.3.1 reads only `height` and `min-height`; `max-height` still caps the box.
        let bottom_adjoins = height.is_none() && !separated && minimum == 0.0;
        let content_bottom = match height {
            Some(specified) => content_top + clamp(specified, minimum, maximum),
            None if bottom_adjoins => {
                // 10.6.3: a last-child margin that cannot collapse through this box
                // counts toward its height.
                let end = if flow.clearance_through {
                    flow.position()
                } else {
                    flow.y
                };
                content_top + clamp((end - content_top).max(0.0), 0.0, maximum)
            }
            None => {
                let flowed = (flow.position() - content_top).max(0.0);
                content_top + clamp(flowed, minimum, maximum)
            }
        };
        let border_bottom = content_bottom + padding[2] + border[2];
        flow.block_limit = flow.block_limit.max(top - mt);
        if bottom_adjoins && !flow.clearance_through {
            flow.adjoin(mb, node)?;
            flow.y = border_bottom;
        } else {
            flow.y = border_bottom;
            flow.pending = MarginExtrema::default();
            flow.adjoin(mb, node)?;
        }
        flow.clearance_through = clearance.is_some()
            && border[0] + padding[0] == 0.0
            && !separated
            && zero_or_auto
            && minimum == 0.0
            && leaf_height.is_none_or(|height| height == 0.0)
            && content_bottom == content_top;
        self.publish(
            node,
            border_left,
            width,
            padding,
            top,
            content_bottom,
            [mt, mr, mb, ml],
            clearance,
        );
        self.geometry[node].leaf_band = leaf_band;
        Ok(())
    }

    /// Places a leaf clear of floats. Narrowing makes text taller, which can reach
    /// another float, so this repeats until the span and the band agree. Returns the
    /// band when displaced, and the height including any gap above the first line.
    fn place_leaf(
        &mut self,
        node: usize,
        flow: &Flow,
        left: f32,
        width: f32,
        measured: f32,
    ) -> Result<(Option<FlowRect>, f32), FlowError> {
        let float_error = |error| FlowError::Float { node, error };
        let right = left + width;
        let least = self.boxes[node]
            .content
            .map_or(0.0, |content| content.preferred_min_width);
        let probe = flow.position();
        let mut y = probe;
        let mut height = measured;
        let mut bound = (flow.floats.len() + 1) * (flow.floats.len() + 1);
        loop {
            let (mut near, mut far) = (left, right);
            for float in flow
                .floats
                .iter()
                .filter(|float| float.top < y + height && float.bottom > y)
            {
                match float.side {
                    FloatSide::Left => near = near.max(float.right),
                    FloatSide::Right => far = far.min(float.left),
                }
            }
            let used = far - near;
            let next = next_band_edge(&flow.floats, y).map_err(float_error)?;
            if (used <= 0.0 || used < least)
                && let Some(edge) = next
            {
                y = edge;
                height = self.leaf_height(node, width).unwrap_or(measured);
                continue;
            }
            let displaced = near != left || far != right || y != probe;
            let at_used = self.leaf_height(node, used.max(0.0)).unwrap_or(measured);
            bound = bound.saturating_sub(1);
            if at_used == height || bound == 0 {
                let band = displaced.then(|| rect(near, far.max(near), y, y));
                return Ok((band, y - probe + at_used));
            }
            height = at_used;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn publish(
        &mut self,
        node: usize,
        border_left: f32,
        width: f32,
        padding: [f32; 4],
        top: f32,
        content_bottom: f32,
        margin: [f32; 4],
        clearance: Option<f32>,
    ) {
        let border = self.boxes[node].border;
        let padding_left = border_left + border[3];
        let content_left = padding_left + padding[3];
        let content_right = content_left + width;
        let padding_right = content_right + padding[1];
        let padding_top = top + border[0];
        let content_top = padding_top + padding[0];
        let padding_bottom = content_bottom + padding[2];
        self.geometry[node] = FlowGeometry {
            border: rect(
                border_left,
                padding_right + border[1],
                top,
                padding_bottom + border[2],
            ),
            padding: rect(padding_left, padding_right, padding_top, padding_bottom),
            content: rect(content_left, content_right, content_top, content_bottom),
            margin,
            clearance,
            collapsed_through: false,
            leaf_band: None,
        };
    }

    /// Lays out a BFC root's contents with its border top fixed, publishing the box.
    /// Returns the used border-box bottom. Floats do not escape.
    fn finish_bfc_box(
        &mut self,
        node: usize,
        cb: Containing,
        width: (f32, f32, f32),
        border_left: f32,
        top: f32,
        padding: [f32; 4],
    ) -> Result<f32, FlowError> {
        let value = &self.boxes[node];
        let border = value.border;
        let content_left = border_left + border[3] + padding[3];
        let content_top = top + border[0] + padding[0];
        let height = value.height.vertical(cb.height);
        let minimum = value.min_height.vertical(cb.height).unwrap_or(0.0);
        let maximum = value.max_height.vertical(cb.height);
        let inner = Containing {
            left: content_left,
            width: width.0,
            height: height.map(|h| clamp(h, minimum, maximum)),
            top: Some(content_top),
            direction: value.direction,
        };
        let mut flow = Flow::new(content_top);
        if let Some(TableRole::Table { spacing }) = value.table {
            flow.y = self.layout_table(node, spacing, inner, content_top)?;
        } else {
            if let Some(height) = self.leaf_height(node, width.0) {
                flow.y += height;
            }
            self.lay_children(node, &mut flow, inner)?;
        }
        // 10.6.7: the bottom margin edge of the last in-flow child and every float.
        let flowed = flow
            .floats
            .iter()
            .fold(flow.position(), |bottom, float| bottom.max(float.bottom));
        self.last_flowed = (flowed - content_top).max(0.0);
        let used = match height {
            // 17.5.3: on a table or cell a height is a minimum.
            Some(specified)
                if matches!(value.table, Some(TableRole::Table { .. } | TableRole::Cell)) =>
            {
                clamp(specified, minimum, maximum).max(flowed - content_top)
            }
            Some(specified) => clamp(specified, minimum, maximum),
            None => clamp((flowed - content_top).max(0.0), minimum, maximum),
        };
        let content_bottom = content_top + used;
        self.publish(
            node,
            border_left,
            width.0,
            padding,
            top,
            content_bottom,
            [0.0; 4],
            None,
        );
        Ok(self.geometry[node].border.bottom)
    }

    fn translate(&mut self, node: usize, dx: f32, dy: f32) {
        let geometry = &mut self.geometry[node];
        for edges in [
            &mut geometry.border,
            &mut geometry.padding,
            &mut geometry.content,
        ] {
            edges.left += dx;
            edges.right += dx;
            edges.top += dy;
            edges.bottom += dy;
        }
        if let Some(band) = &mut geometry.leaf_band {
            band.left += dx;
            band.right += dx;
            band.top += dy;
            band.bottom += dy;
        }
        for &child in &self.boxes[node].children {
            if self.boxes[child].out_of_flow {
                let moved = &mut self.geometry[child].border;
                moved.left += dx;
                moved.right += dx;
                moved.top += dy;
            } else {
                self.translate(child, dx, dy);
            }
        }
    }

    fn layout_float(
        &mut self,
        node: usize,
        flow: &mut Flow,
        cb: Containing,
    ) -> Result<(), FlowError> {
        if !flow.unsettled.is_empty() {
            flow.deferred.push((node, cb));
            return Ok(());
        }
        let top = flow.position();
        self.place_float_box(node, flow, cb, top)
    }

    /// Lays out a shrink-to-fit box (a float or inline-block, 10.3.5, 10.3.9) at the
    /// origin. Returns its used margins, left, right, top, bottom, and border-box bottom.
    fn shrink_box(
        &mut self,
        node: usize,
        cb: Containing,
    ) -> Result<(f32, f32, f32, f32, f32), FlowError> {
        let value = &self.boxes[node];
        let basis = cb.width;
        let padding = self.paddings(node, basis);
        let (preferred_min_width, preferred_width) = self.measure(node);
        let used = solve_shrink_width(ShrinkWidthInput {
            containing_width: cb.width,
            width: value.width.horizontal(basis),
            margin_left: value.margin[3].horizontal(basis),
            margin_right: value.margin[1].horizontal(basis),
            border_left: value.border[3],
            border_right: value.border[1],
            padding_left: padding[3],
            padding_right: padding[1],
            scrollbar_width: 0.0,
            preferred_min_width,
            preferred_width,
            min_width: value.min_width.zero_auto(basis),
            max_width: match value.max_width {
                Dim::Auto => None,
                other => Some(other.zero_auto(basis)),
            },
        })
        .map_err(|error| FlowError::Shrink { node, error })?;
        // 17.5.2.2 holds for a floating or inline table too.
        let used_width = if matches!(value.table, Some(TableRole::Table { .. })) {
            used.width.max(preferred_min_width)
        } else {
            used.width
        };
        let mt = value.margin[0].zero_auto(basis);
        let mb = value.margin[2].zero_auto(basis);
        // The contents are laid out at a provisional origin, then moved.
        let bottom = self.finish_bfc_box(
            node,
            Containing {
                left: 0.0,
                top: Some(0.0),
                ..cb
            },
            (used_width, 0.0, 0.0),
            0.0,
            0.0,
            padding,
        )?;
        Ok((used.margin_left, used.margin_right, mt, mb, bottom))
    }

    /// Lines of inline-blocks (9.4.2): they fill a line, then wrap, and an overlong box
    /// keeps a line to itself.
    fn layout_inline_row(
        &mut self,
        node: usize,
        align: RowAlign,
        flow: &mut Flow,
        cb: Containing,
    ) -> Result<(), FlowError> {
        self.settle(flow)?;
        struct Item {
            node: usize,
            width: f32,
            ml: f32,
            mt: f32,
            height: f32,
            margin: [f32; 4],
        }
        let mut items = Vec::new();
        for &child in &self.boxes[node].children.clone() {
            if self.boxes[child].float.is_some() || self.boxes[child].out_of_flow {
                continue;
            }
            let (ml, mr, mt, mb, bottom) = self.shrink_box(child, cb)?;
            let border_width = self.geometry[child].border.right;
            items.push(Item {
                node: child,
                width: ml + border_width + mr,
                ml,
                mt,
                height: mt + bottom + mb,
                margin: [mt, mr, mb, ml],
            });
        }
        // 9.5: lines stay clear of floats. Every line is checked as tall as the tallest
        // box in the row, so a line can only be shorter than it had to be, never overlap.
        let tallest = items
            .iter()
            .fold(0.0_f32, |most, item| most.max(item.height));
        let float_error = |error| FlowError::Float { node, error };
        let (full_left, full_right) = (cb.left, cb.left + cb.width);
        let mut y = flow.y;
        let mut start = 0;
        while start < items.len() {
            let (mut near, mut far) = (full_left, full_right);
            for float in flow
                .floats
                .iter()
                .filter(|float| float.top < y + tallest && float.bottom > y)
            {
                match float.side {
                    FloatSide::Left => near = near.max(float.right),
                    FloatSide::Right => far = far.min(float.left),
                }
            }
            let room = far - near;
            if items[start].width > room
                && let Some(edge) = next_band_edge(&flow.floats, y).map_err(float_error)?
            {
                y = edge;
                continue;
            }
            let mut end = start + 1;
            let mut used = items[start].width;
            while end < items.len() && used + items[end].width <= room {
                used += items[end].width;
                end += 1;
            }
            let line = items[start..end]
                .iter()
                .fold(0.0_f32, |tallest, item| tallest.max(item.height));
            let mut x = near
                + match align {
                    RowAlign::Left => 0.0,
                    RowAlign::Center => ((room - used) / 2.0).max(0.0),
                    RowAlign::Right => (room - used).max(0.0),
                };
            // 9.2.4: an rtl container lays its inline boxes out from the right.
            let order: Vec<usize> = if cb.direction == Direction::Rtl {
                (start..end).rev().collect()
            } else {
                (start..end).collect()
            };
            for index in order {
                let item = &items[index];
                self.translate(item.node, x + item.ml, y + line - item.height + item.mt);
                self.geometry[item.node].margin = item.margin;
                x += item.width;
            }
            y += line;
            start = end;
        }
        flow.y = y;
        // The anonymous block itself holds the whole run.
        let area = rect(cb.left, cb.left + cb.width, flow.y, flow.y);
        self.geometry[node] = FlowGeometry {
            border: area,
            padding: area,
            content: area,
            ..FlowGeometry::default()
        };
        Ok(())
    }

    fn place_float_box(
        &mut self,
        node: usize,
        flow: &mut Flow,
        cb: Containing,
        flow_top: f32,
    ) -> Result<(), FlowError> {
        let side = self.boxes[node].float.ok_or(FlowError::Tree { node })?;
        let value = &self.boxes[node];
        let (ml, mr, mt, mb, border_box_bottom) = self.shrink_box(node, cb)?;
        let border_width = self.geometry[node].border.right;
        let mut hypothetical = flow_top.max(flow.block_limit);
        if let Some(bottom) = flow.lowest(value.clear, node)? {
            hypothetical = hypothetical.max(bottom);
        }
        // 9.5.1 rule 4 cannot apply above a parent whose own top edge is still open.
        let containing_top = cb.top.unwrap_or(hypothetical);
        let band = NonZeroUsize::new(flow.floats.len() + 1).ok_or(FlowError::Boxes)?;
        let placement = place_float(
            &flow.floats,
            Band {
                left: cb.left,
                right: cb.left + cb.width,
            },
            FloatRequest {
                side,
                width: (ml + border_width + mr).max(0.0),
                height: (mt + border_box_bottom + mb).max(0.0),
                containing_top,
                hypothetical_top: hypothetical,
            },
            band,
        )
        .map_err(|error| FlowError::Float { node, error })?;
        flow.floats.push(placement.placed);
        self.translate(node, placement.placed.left + ml, placement.placed.top + mt);
        self.geometry[node].margin = [mt, mr, mb, ml];
        Ok(())
    }

    fn layout_bfc_block(
        &mut self,
        node: usize,
        flow: &mut Flow,
        cb: Containing,
    ) -> Result<(), FlowError> {
        flow.clearance_through = false;
        let value = &self.boxes[node];
        let (solved, ml, mr) = self.block_width(node, cb)?;
        let basis = cb.width;
        let padding = self.paddings(node, basis);
        let border = value.border;
        let mt = value.margin[0].zero_auto(basis);
        let mb = value.margin[2].zero_auto(basis);
        let clearance = self.apply_clear(node, flow, mt)?;
        let hypothetical = if clearance.is_some() {
            flow.y
        } else {
            self.settle(flow)?
        };
        // A solved margin may hold an auto or over-constrained share; fitting beside a
        // float uses only the specified margins.
        let box_frame = border[3] + padding[3] + padding[1] + border[1];
        let specified_left = value.margin[3].zero_auto(basis);
        let frame = specified_left + box_frame + value.margin[1].zero_auto(basis);
        let least = value.min_width.zero_auto(basis);
        // A table's width is already settled; it moves down rather than narrow.
        let auto_width = value.width == Dim::Auto && value.table.is_none();
        let containing = Band {
            left: cb.left,
            right: cb.left + cb.width,
        };
        let float_error = |error| FlowError::Float { node, error };
        // 9.5: the border box must not overlap a float. An auto width narrows to the
        // band; otherwise and below the box's minimum, the search moves down.
        let (minimum, _) = self.measure(node);
        let mut y = hypothetical;
        let (left, width, bottom) = loop {
            let first = exclusion_band(&flow.floats, containing, y).map_err(float_error)?;
            let width = if auto_width {
                // 10.4: min-width still holds; too narrow a band then moves the box down.
                solved.min(first.width() - frame).max(least)
            } else {
                solved
            };
            let fitted = (width + frame) <= first.width() && (!auto_width || width >= minimum);
            let next = next_band_edge(&flow.floats, y).map_err(float_error)?;
            let Some(edge) = next.filter(|_| !fitted && !flow.floats.is_empty()) else {
                let unnarrowed = first.left == containing.left && first.right == containing.right;
                let left = if unnarrowed {
                    cb.left + ml
                } else {
                    first.left + specified_left
                };
                // Place at origin to learn the height, then check the whole span.
                let bottom = self.finish_bfc_box(
                    node,
                    Containing {
                        left: 0.0,
                        top: Some(0.0),
                        ..cb
                    },
                    (width.max(0.0), 0.0, 0.0),
                    0.0,
                    0.0,
                    padding,
                )?;
                match first_conflict(&flow.floats, left, width + box_frame, y, bottom)
                    .map_err(float_error)?
                {
                    Some(lower) => {
                        y = lower;
                        continue;
                    }
                    None => break (left, width.max(0.0), bottom),
                }
            };
            y = edge;
        };
        self.translate(node, left, y);
        let geometry = &mut self.geometry[node];
        geometry.margin = [mt, mr, mb, ml];
        geometry.clearance = clearance;
        let _ = width;
        flow.y = y + bottom;
        flow.block_limit = flow.block_limit.max(y - mt);
        flow.pending = MarginExtrema::default();
        flow.adjoin(mb, node)?;
        Ok(())
    }
}

/// The next band edge if a box of this span overlaps a float below its first band.
fn first_conflict(
    floats: &[PlacedFloat],
    left: f32,
    width: f32,
    y: f32,
    height: f32,
) -> Result<Option<f32>, FloatError> {
    let bottom = y + height;
    for float in floats
        .iter()
        .filter(|float| float.top < bottom && float.bottom > y)
    {
        let overlaps = float.left < left + width && float.right > left;
        if overlaps && width > 0.0 {
            return next_band_edge(floats, y).map(|edge| edge.filter(|edge| *edge > y));
        }
    }
    Ok(None)
}

fn clamp(value: f32, minimum: f32, maximum: Option<f32>) -> f32 {
    let capped = maximum.map_or(value, |limit| value.min(limit));
    capped.max(minimum)
}

#[cfg(test)]
#[path = "block_flow_tests.rs"]
mod tests;
