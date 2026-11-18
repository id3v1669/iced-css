//! Grid layout (CSS Grid Layout Level 1, sections 7, 8, 10 and 11), as one more way a box
//! that establishes a formatting context lays out its children.
//!
//! CSS Grid 2 subgrids: a grid item whose `grid-template-columns` or `-rows` is `subgrid`
//! takes the tracks it spans in that axis from its parent. Its items size those tracks
//! with the parent's, its margin, border and padding added at its edges, and are placed in
//! them; they are clamped to its span, with no implicit tracks there, and its own gap in
//! that axis is not used.
//!
//! Left out: baseline alignment (it aligns at the start), absolutely positioned
//! items placed by grid lines (they take the content box's start as their static
//! position), and fragmentation. Space beyond the items spanning several tracks is shared
//! equally among the tracks it may grow, and an item's percentage height has no basis.
use super::{
    AutoLength, Containing, Dim, Direction, FlowBox, FlowError, Solver, clamp,
    flex_layout::{FlexAlign, FlexSpace, spread},
};

/// The most tracks an axis may hold, and the largest line number or span (5.3).
const LIMIT: usize = 1000;

/// One side of a track's sizing function, its percentages unresolved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Breadth {
    Fixed(Dim),
    /// `fr`.
    Flex(f32),
    Auto,
    MinContent,
    MaxContent,
}

/// A track's sizing function: `minmax(min, max)`, with `fit-content()` as an automatic
/// minimum, a max-content maximum and a limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackSize {
    pub min: Breadth,
    pub max: Breadth,
    pub fit: Option<Dim>,
}

impl TrackSize {
    pub const AUTO: Self = Self::breadth(Breadth::Auto);

    /// One breadth for both sides; a `<flex>` alone has an automatic minimum.
    pub const fn breadth(breadth: Breadth) -> Self {
        Self {
            min: match breadth {
                Breadth::Flex(_) => Breadth::Auto,
                other => other,
            },
            max: breadth,
            fit: None,
        }
    }
}

/// `repeat(auto-fill | auto-fit, ...)` within a track list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AutoRepeat {
    /// Where it sits: before the track of this index in the list's sizes.
    pub at: usize,
    /// `auto-fit`: repeated tracks no item is placed in collapse.
    pub fit: bool,
    pub sizes: Vec<TrackSize>,
    /// The names of the lines around its tracks, one more than there are tracks.
    pub names: Vec<Vec<String>>,
    /// The names of the line just after it.
    pub after: Vec<String>,
}

/// `grid-template-columns` or `grid-template-rows` with integer repetitions expanded.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrackList {
    pub sizes: Vec<TrackSize>,
    /// The names of each line, one more than there are sizes; empty is no names.
    pub names: Vec<Vec<String>>,
    pub repeat: Option<AutoRepeat>,
    /// CSS Grid 2 `subgrid`: in a grid item, the axis takes the tracks it spans in its
    /// parent grid; `names` name its lines from the first. Elsewhere it is `none`.
    pub subgrid: bool,
}

/// A named area of `grid-template-areas`: its start and end lines, from 0.
#[derive(Debug, Clone, PartialEq)]
pub struct GridArea {
    pub name: String,
    pub rows: [usize; 2],
    pub columns: [usize; 2],
}

/// One line placement: `auto` is the default.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GridLine {
    pub span: bool,
    /// Zero is none given.
    pub number: i32,
    pub name: Option<String>,
}

/// What a box is as a grid item; it only matters in a grid container. `align-self` and
/// `order` are those of its [`super::FlexItem`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GridItem {
    /// `grid-row-start`, `grid-row-end`.
    pub row: [GridLine; 2],
    /// `grid-column-start`, `grid-column-end`.
    pub column: [GridLine; 2],
    /// `justify-self`; `None` is `auto`, the container's `justify-items`.
    pub justify: Option<FlexAlign>,
}

/// A grid container's own properties.
#[derive(Debug, Clone, PartialEq)]
pub struct GridContainer {
    pub columns: TrackList,
    pub rows: TrackList,
    pub areas: Vec<GridArea>,
    /// `grid-auto-columns`, never empty.
    pub auto_columns: Vec<TrackSize>,
    /// `grid-auto-rows`, never empty.
    pub auto_rows: Vec<TrackSize>,
    /// `grid-auto-flow: column`.
    pub column_flow: bool,
    pub dense: bool,
    /// `Stretch` is the initial `normal`.
    pub justify_content: FlexSpace,
    pub align_content: FlexSpace,
    pub justify_items: FlexAlign,
    pub align_items: FlexAlign,
    /// `row-gap`, then `column-gap`.
    pub gap: [f32; 2],
}

impl Default for GridContainer {
    fn default() -> Self {
        Self {
            columns: TrackList::default(),
            rows: TrackList::default(),
            areas: Vec::new(),
            auto_columns: vec![TrackSize::AUTO],
            auto_rows: vec![TrackSize::AUTO],
            column_flow: false,
            dense: false,
            justify_content: FlexSpace::Stretch,
            align_content: FlexSpace::Stretch,
            justify_items: FlexAlign::Stretch,
            align_items: FlexAlign::Stretch,
            gap: [0.0; 2],
        }
    }
}

/// The named areas of `grid-template-areas` rows (each a row of cells, `None` a `.`),
/// which the parser has checked are rectangles.
pub fn grid_areas(rows: &[Vec<Option<String>>]) -> Vec<GridArea> {
    let mut areas: Vec<GridArea> = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            let Some(name) = cell else { continue };
            match areas.iter_mut().find(|area| area.name == *name) {
                Some(area) => {
                    area.rows[1] = area.rows[1].max(y + 1);
                    area.columns[1] = area.columns[1].max(x + 1);
                }
                None => areas.push(GridArea {
                    name: name.clone(),
                    rows: [y, y + 1],
                    columns: [x, x + 1],
                }),
            }
        }
    }
    areas
}

/// The explicit grid along one axis.
struct Axis {
    sizes: Vec<TrackSize>,
    names: Vec<Vec<String>>,
    /// The tracks an `auto-fit` repeated, which collapse when empty.
    fit: Option<[usize; 2]>,
}

impl Axis {
    fn explicit(&self) -> i32 {
        self.sizes.len() as i32
    }
    /// The lines carrying `name`, in order.
    fn named(&self, name: &str) -> Vec<i32> {
        self.names
            .iter()
            .enumerate()
            .filter(|(_, names)| names.iter().any(|candidate| candidate == name))
            .map(|(line, _)| line as i32)
            .collect()
    }
}

/// A fixed size, for counting automatic repetitions: the maximum if definite, else the
/// minimum.
fn fixed_size(size: &TrackSize, basis: Option<f32>) -> f32 {
    let definite = |breadth: Breadth| match breadth {
        Breadth::Fixed(dim) => dim.vertical(basis),
        _ => None,
    };
    definite(size.max)
        .or_else(|| definite(size.min))
        .unwrap_or(0.0)
        .max(0.0)
}

/// 7.2.3.2: how many times an automatic repetition repeats in `available`.
fn repeat_count(list: &TrackList, repeat: &AutoRepeat, available: Option<f32>, gap: f32) -> usize {
    let most = (LIMIT.saturating_sub(list.sizes.len()) / repeat.sizes.len().max(1)).max(1);
    let Some(available) = available else {
        return 1;
    };
    let each: f32 = repeat
        .sizes
        .iter()
        .map(|size| fixed_size(size, Some(available)))
        .sum::<f32>()
        + gap * repeat.sizes.len() as f32;
    let others: f32 = list
        .sizes
        .iter()
        .map(|size| fixed_size(size, Some(available)))
        .sum();
    let room = available - others - gap * (list.sizes.len() as f32 - 1.0);
    if each <= 0.0 || !room.is_finite() {
        return 1;
    }
    ((room / each).floor().max(1.0) as usize).min(most)
}

/// The explicit grid of one axis: its list with any automatic repetition expanded, as
/// many tracks as `extent` (the areas) needs, sized by `auto` beyond the list.
fn explicit_axis(
    list: &TrackList,
    auto: &[TrackSize],
    extent: usize,
    available: Option<f32>,
    gap: f32,
) -> Axis {
    let mut sizes = Vec::new();
    let mut names: Vec<Vec<String>> = vec![list.names.first().cloned().unwrap_or_default()];
    let mut fit = None;
    let push =
        |sizes: &mut Vec<TrackSize>, names: &mut Vec<Vec<String>>, size, after: &[String]| {
            sizes.push(size);
            names.push(after.to_vec());
        };
    for at in 0..=list.sizes.len() {
        if let Some(repeat) = list.repeat.as_ref().filter(|repeat| repeat.at == at) {
            let count = repeat_count(list, repeat, available, gap);
            let first = sizes.len();
            for _ in 0..count {
                if let (Some(line), Some(own)) = (names.last_mut(), repeat.names.first()) {
                    line.extend(own.iter().cloned());
                }
                for (index, size) in repeat.sizes.iter().enumerate() {
                    let after = repeat.names.get(index + 1).map_or(&[][..], Vec::as_slice);
                    push(&mut sizes, &mut names, *size, after);
                }
            }
            if let Some(line) = names.last_mut() {
                line.extend(repeat.after.iter().cloned());
            }
            if repeat.fit {
                fit = Some([first, sizes.len()]);
            }
        }
        if let Some(size) = list.sizes.get(at) {
            let after = list.names.get(at + 1).map_or(&[][..], Vec::as_slice);
            push(&mut sizes, &mut names, *size, after);
        }
    }
    let mut implicit = 0;
    while sizes.len() < extent.min(LIMIT) {
        push(&mut sizes, &mut names, auto[implicit % auto.len()], &[]);
        implicit += 1;
    }
    sizes.truncate(LIMIT);
    names.truncate(LIMIT + 1);
    Axis { sizes, names, fit }
}

/// One axis of an item's placement: lines from the explicit grid's start (possibly
/// before it), or a span still to be placed.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Span {
    Lines(i32, i32),
    Auto(usize),
}

/// The `n`th line named so (from the end when negative), where every implicit line past
/// the explicit grid carries every name.
fn nth_named(found: &[i32], n: i32, explicit: i32) -> i32 {
    let count = found.len() as i32;
    if n > 0 {
        found
            .get(n as usize - 1)
            .copied()
            .unwrap_or(explicit + n - count)
    } else {
        let back = -n;
        if back <= count {
            found[(count - back) as usize]
        } else {
            -(back - count)
        }
    }
}

/// A definite line placement, or `None` for `auto` and spans.
fn definite(line: &GridLine, axis: &Axis, end: bool) -> Option<i32> {
    if line.span || (line.number == 0 && line.name.is_none()) {
        return None;
    }
    let explicit = axis.explicit();
    Some(match (&line.name, line.number) {
        (None, n) if n > 0 => n - 1,
        (None, n) => explicit + 1 + n,
        // 8.3.1: a lone name is first an area's edge.
        (Some(name), 0) => {
            let edge = axis.named(&format!("{name}-{}", if end { "end" } else { "start" }));
            match edge.first() {
                Some(line) => *line,
                None => nth_named(&axis.named(name), 1, explicit),
            }
        }
        (Some(name), n) => nth_named(&axis.named(name), n, explicit),
    })
}

/// The line a span reaches from `from`, forwards or backwards.
fn span_from(line: &GridLine, axis: &Axis, from: i32, forward: bool) -> i32 {
    let n = line.number.max(1);
    let Some(name) = &line.name else {
        return if forward { from + n } else { from - n };
    };
    let found = axis.named(name);
    if forward {
        let beyond: Vec<i32> = found.into_iter().filter(|line| *line > from).collect();
        beyond
            .get(n as usize - 1)
            .copied()
            .unwrap_or_else(|| from.max(axis.explicit()) + n - beyond.len() as i32)
    } else {
        let before: Vec<i32> = found
            .into_iter()
            .filter(|line| *line < from)
            .rev()
            .collect();
        before
            .get(n as usize - 1)
            .copied()
            .unwrap_or_else(|| from.min(0) - (n - before.len() as i32))
    }
}

/// 8.3: one axis of an item's placement, with 8.3.1's conflicts handled.
fn resolve_span(lines: &[GridLine; 2], axis: &Axis) -> Span {
    let limit = LIMIT as i32;
    let bounded = |line: i32| line.clamp(-limit, limit);
    let [start, end] = lines;
    match (definite(start, axis, false), definite(end, axis, true)) {
        (Some(a), Some(b)) => {
            let (a, b) = (bounded(a.min(b)), bounded(a.max(b)));
            Span::Lines(a, if a == b { a + 1 } else { b })
        }
        (Some(a), None) if end.span => {
            let a = bounded(a);
            Span::Lines(a, bounded(span_from(end, axis, a, true)).max(a + 1))
        }
        (Some(a), None) => Span::Lines(bounded(a), bounded(a) + 1),
        (None, Some(b)) if start.span => {
            let b = bounded(b);
            Span::Lines(bounded(span_from(start, axis, b, false)).min(b - 1), b)
        }
        (None, Some(b)) => Span::Lines(bounded(b) - 1, bounded(b)),
        (None, None) => {
            // Of two spans the start's counts; a span of a name alone is one.
            let span = if start.span { start } else { end };
            let count = if span.span && span.number > 0 {
                span.number as usize
            } else {
                1
            };
            Span::Auto(count.min(LIMIT))
        }
    }
}

/// Where each item went: its rows, then its columns, as track indices from 0.
struct Placement {
    areas: Vec<[[usize; 2]; 2]>,
    /// How many implicit tracks precede the explicit grid, rows then columns.
    shift: [usize; 2],
    /// How many tracks each axis has.
    count: [usize; 2],
}

/// Which cells hold an item, by major then minor track.
struct Cells {
    rows: Vec<Vec<bool>>,
    minor: usize,
}

impl Cells {
    fn fits(
        &self,
        major: [usize; 2],
        minor: [usize; 2],
        work: &mut usize,
    ) -> Result<bool, FlowError> {
        for row in self.rows.iter().take(major[1]).skip(major[0]) {
            *work = work.checked_sub(1).ok_or(FlowError::Work)?;
            if row[minor[0]..minor[1].min(self.minor)]
                .iter()
                .any(|cell| *cell)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn fill(&mut self, major: [usize; 2], minor: [usize; 2]) {
        while self.rows.len() < major[1] {
            self.rows.push(vec![false; self.minor]);
        }
        for row in &mut self.rows[major[0]..major[1]] {
            for cell in &mut row[minor[0]..minor[1].min(self.minor)] {
                *cell = true;
            }
        }
    }
}

/// 8.5: places `children` (in order-modified document order).
fn place(
    boxes: &[FlowBox],
    children: &[usize],
    grid: &GridContainer,
    axes: [&Axis; 2],
    work: &mut usize,
) -> Result<Placement, FlowError> {
    let empty = GridItem::default();
    let spans: Vec<[Span; 2]> = children
        .iter()
        .map(|child| {
            let item = boxes[*child].grid_item.as_deref().unwrap_or(&empty);
            [
                resolve_span(&item.row, axes[0]),
                resolve_span(&item.column, axes[1]),
            ]
        })
        .collect();
    // Implicit tracks before the explicit grid.
    let mut shift = [0usize; 2];
    for span in &spans {
        for axis in 0..2 {
            if let Span::Lines(start, _) = span[axis] {
                shift[axis] = shift[axis].max((-start).max(0) as usize);
            }
        }
    }
    let shifted = |span: Span, axis: usize| match span {
        Span::Lines(start, end) => Some([
            (start + shift[axis] as i32) as usize,
            (end + shift[axis] as i32) as usize,
        ]),
        Span::Auto(_) => None,
    };
    let length = |span: Span| match span {
        Span::Lines(start, end) => (end - start) as usize,
        Span::Auto(count) => count,
    };
    let major = usize::from(grid.column_flow);
    let minor = 1 - major;
    // 8.5 step 3: the minor axis's extent is settled before auto-placement.
    let mut minor_count = axes[minor].sizes.len() + shift[minor];
    for span in &spans {
        minor_count = minor_count.max(match shifted(span[minor], minor) {
            Some(lines) => lines[1],
            None => length(span[minor]),
        });
    }
    let mut cells = Cells {
        rows: Vec::new(),
        minor: minor_count,
    };
    let mut areas: Vec<Option<[[usize; 2]; 2]>> = vec![None; children.len()];
    let area = |major_lines: [usize; 2], minor_lines: [usize; 2]| {
        let mut both = [[0; 2]; 2];
        both[major] = major_lines;
        both[minor] = minor_lines;
        both
    };
    // Step 1: items placed in both axes.
    for (at, span) in spans.iter().enumerate() {
        if let (Some(major_lines), Some(minor_lines)) =
            (shifted(span[major], major), shifted(span[minor], minor))
        {
            cells.fill(major_lines, minor_lines);
            areas[at] = Some(area(major_lines, minor_lines));
        }
    }
    // Step 2: items locked to a major track.
    let mut cursors: Vec<(usize, usize)> = Vec::new();
    for (at, span) in spans.iter().enumerate() {
        let (Some(major_lines), None) = (shifted(span[major], major), shifted(span[minor], minor))
        else {
            continue;
        };
        let wide = length(span[minor]);
        let from = if grid.dense {
            0
        } else {
            cursors
                .iter()
                .find(|(line, _)| *line == major_lines[0])
                .map_or(0, |(_, cursor)| *cursor)
        };
        let mut found = None;
        for start in (from..=minor_count.saturating_sub(wide)).chain(0..from) {
            if cells.fits(major_lines, [start, start + wide], work)? {
                found = Some(start);
                break;
            }
        }
        // Nowhere free: it overlaps at the cursor.
        let start = found.unwrap_or(from.min(minor_count.saturating_sub(wide)));
        let minor_lines = [start, start + wide];
        cells.fill(major_lines, minor_lines);
        areas[at] = Some(area(major_lines, minor_lines));
        match cursors.iter_mut().find(|(line, _)| *line == major_lines[0]) {
            Some((_, cursor)) => *cursor = minor_lines[1],
            None => cursors.push((major_lines[0], minor_lines[1])),
        }
    }
    // Step 4: the rest, by the auto-placement cursor.
    let (mut cursor_major, mut cursor_minor) = (0usize, 0usize);
    for (at, span) in spans.iter().enumerate() {
        if areas[at].is_some() {
            continue;
        }
        let tall = length(span[major]);
        if grid.dense {
            (cursor_major, cursor_minor) = (0, 0);
        }
        let minor_lines = if let Some(minor_lines) = shifted(span[minor], minor) {
            if grid.dense {
                cursor_major = 0;
            } else if minor_lines[0] < cursor_minor {
                cursor_major += 1;
            }
            cursor_minor = minor_lines[0];
            while !cells.fits([cursor_major, cursor_major + tall], minor_lines, work)? {
                cursor_major += 1;
            }
            minor_lines
        } else {
            let wide = length(span[minor]);
            loop {
                *work = work.checked_sub(1).ok_or(FlowError::Work)?;
                if cursor_minor + wide > minor_count {
                    cursor_major += 1;
                    cursor_minor = 0;
                    continue;
                }
                let lines = [cursor_minor, cursor_minor + wide];
                if cells.fits([cursor_major, cursor_major + tall], lines, work)? {
                    break lines;
                }
                cursor_minor += 1;
            }
        };
        let major_lines = [cursor_major, cursor_major + tall];
        cells.fill(major_lines, minor_lines);
        areas[at] = Some(area(major_lines, minor_lines));
        if shifted(span[minor], minor).is_none() {
            cursor_minor = minor_lines[1];
        }
    }
    let areas: Vec<[[usize; 2]; 2]> = areas.into_iter().map(Option::unwrap_or_default).collect();
    let mut count = [0usize; 2];
    for axis in 0..2 {
        count[axis] = areas
            .iter()
            .map(|area| area[axis][1])
            .fold(axes[axis].sizes.len() + shift[axis], usize::max);
    }
    Ok(Placement {
        areas,
        shift,
        count,
    })
}

/// The sizing function of every track of an axis, explicit and implicit, and whether
/// it collapsed.
fn all_tracks(
    axis: &Axis,
    auto: &[TrackSize],
    shift: usize,
    count: usize,
    used: &[bool],
) -> (Vec<TrackSize>, Vec<bool>) {
    let explicit = axis.sizes.len();
    let sizes = (0..count)
        .map(|track| {
            if track < shift {
                // The track just before the explicit grid takes the last auto size.
                let back = (shift - 1 - track) % auto.len();
                auto[auto.len() - 1 - back]
            } else if track - shift < explicit {
                axis.sizes[track - shift]
            } else {
                auto[(track - shift - explicit) % auto.len()]
            }
        })
        .collect();
    let collapsed = (0..count)
        .map(|track| {
            axis.fit.is_some_and(|[from, to]| {
                (from + shift..to + shift).contains(&track) && !used[track]
            })
        })
        .collect();
    (sizes, collapsed)
}

/// How a subgrid's axis takes its parent's tracks.
#[derive(Debug, Clone)]
pub(super) enum SubgridAxis {
    /// Laid out to learn what its items ask: so many tracks, sized by itself.
    Measure(usize),
    /// The parent's tracks it spans, as their edges in its content box.
    Edges(Vec<[f32; 2]>),
}

impl SubgridAxis {
    fn span(&self) -> usize {
        match self {
            Self::Measure(span) => *span,
            Self::Edges(edges) => edges.len(),
        }
    }
}

/// What one item asks of the tracks it spans.
#[derive(Debug, Clone)]
pub(super) struct Contribution {
    start: usize,
    span: usize,
    /// Its minimum contribution: how small its automatic minimum lets it be.
    minimum: f32,
    min_content: f32,
    max_content: f32,
}

/// The space an axis is sized in.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Definite(f32),
    MinContent,
    MaxContent,
}

struct Track {
    base: f32,
    limit: f32,
    min: Breadth,
    max: Breadth,
    flex: Option<f32>,
    fit: Option<f32>,
}

fn intrinsic(breadth: Breadth) -> bool {
    matches!(
        breadth,
        Breadth::Auto | Breadth::MinContent | Breadth::MaxContent
    )
}

/// 11.7.1: the size of one `fr` sharing `space` among `indices`.
fn fr_size(tracks: &[Track], indices: std::ops::Range<usize>, space: f32) -> f32 {
    let mut rigid: Vec<bool> = tracks[indices.clone()]
        .iter()
        .map(|track| track.flex.is_none())
        .collect();
    let mut size = 0.0;
    for _ in 0..=rigid.len() {
        let parts = tracks[indices.clone()].iter().zip(&rigid);
        let leftover = space
            - parts
                .clone()
                .filter(|(_, rigid)| **rigid)
                .map(|(track, _)| track.base)
                .sum::<f32>();
        let factors: f32 = parts
            .filter(|(_, rigid)| !**rigid)
            .filter_map(|(track, _)| track.flex)
            .sum();
        size = leftover / factors.max(1.0);
        let mut changed = false;
        for (track, rigid) in tracks[indices.clone()].iter().zip(&mut rigid) {
            if let Some(flex) = track.flex.filter(|_| !*rigid) {
                if size * flex < track.base {
                    *rigid = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    size.max(0.0)
}

/// 11.3 to 11.8: the used size of each track.
fn size_tracks(
    sizes: &[TrackSize],
    collapsed: &[bool],
    items: &[Contribution],
    gap: f32,
    mode: Mode,
    basis: Option<f32>,
    stretch: bool,
) -> Vec<f32> {
    // 11.4: a percentage of an indefinite basis is `auto`.
    let resolve = |breadth: Breadth| match breadth {
        Breadth::Fixed(dim) => dim.vertical(basis).map_or(Breadth::Auto, |pixels| {
            Breadth::Fixed(Dim::Px(pixels.max(0.0)))
        }),
        other => other,
    };
    let pixels = |breadth: Breadth| match breadth {
        Breadth::Fixed(Dim::Px(pixels)) => Some(pixels),
        _ => None,
    };
    let mut tracks: Vec<Track> = sizes
        .iter()
        .zip(collapsed)
        .map(|(size, collapsed)| {
            if *collapsed {
                let zero = Breadth::Fixed(Dim::Px(0.0));
                return Track {
                    base: 0.0,
                    limit: 0.0,
                    min: zero,
                    max: zero,
                    flex: None,
                    fit: None,
                };
            }
            let (min, max) = (resolve(size.min), resolve(size.max));
            let base = pixels(min).unwrap_or(0.0);
            Track {
                base,
                limit: pixels(max).map_or(f32::INFINITY, |limit| limit.max(base)),
                min,
                max,
                flex: match max {
                    Breadth::Flex(flex) => Some(flex.max(0.0)),
                    _ => None,
                },
                fit: size.fit.and_then(|fit| fit.vertical(basis)),
            }
        })
        .collect();
    let constrained = !matches!(mode, Mode::Definite(_));
    let automatic = |item: &Contribution| {
        if constrained {
            item.min_content
        } else {
            item.minimum
        }
    };
    let grow = |limit: f32, value: f32| {
        if limit.is_infinite() {
            value
        } else {
            limit.max(value)
        }
    };
    let flexible = |tracks: &[Track], item: &Contribution| {
        tracks[item.start..item.start + item.span]
            .iter()
            .any(|track| track.flex.is_some())
    };
    // 11.5 step 2: items spanning one track.
    for item in items.iter().filter(|item| item.span == 1) {
        if flexible(&tracks, item) {
            continue;
        }
        let track = &mut tracks[item.start];
        let wanted = match track.min {
            Breadth::MinContent => item.min_content,
            Breadth::MaxContent => item.max_content,
            // Limited by a fixed maximum.
            Breadth::Auto => match pixels(track.max) {
                Some(limit) => automatic(item).min(limit),
                None => automatic(item),
            },
            _ => 0.0,
        };
        track.base = track.base.max(wanted);
        match track.max {
            Breadth::MinContent => track.limit = grow(track.limit, item.min_content),
            Breadth::MaxContent | Breadth::Auto => {
                let content = match track.fit {
                    Some(fit) => item.max_content.min(fit.max(item.min_content)),
                    None => item.max_content,
                };
                track.limit = grow(track.limit, content);
            }
            _ => {}
        }
        if track.limit < track.base {
            track.limit = track.base;
        }
    }
    // Step 3: items spanning several tracks, the narrower spans first.
    let widest = items.iter().map(|item| item.span).max().unwrap_or(0);
    for span in 2..=widest {
        for item in items.iter().filter(|item| item.span == span) {
            if flexible(&tracks, item) {
                continue;
            }
            let range = item.start..item.start + span;
            let gaps = gap * (span - 1) as f32;
            let growable: Vec<usize> = range
                .clone()
                .filter(|at| intrinsic(tracks[*at].min))
                .collect();
            let extra = automatic(item)
                - tracks[range.clone()]
                    .iter()
                    .map(|track| track.base)
                    .sum::<f32>()
                - gaps;
            if extra > 0.0 && !growable.is_empty() {
                for at in &growable {
                    tracks[*at].base += extra / growable.len() as f32;
                }
            }
            let growable: Vec<usize> = range
                .clone()
                .filter(|at| intrinsic(tracks[*at].max))
                .collect();
            let limits: f32 = tracks[range.clone()]
                .iter()
                .map(|track| {
                    if track.limit.is_infinite() {
                        track.base
                    } else {
                        track.limit
                    }
                })
                .sum();
            let extra = item.max_content - limits - gaps;
            if extra > 0.0 && !growable.is_empty() {
                for at in &growable {
                    let track = &mut tracks[*at];
                    let from = if track.limit.is_infinite() {
                        track.base
                    } else {
                        track.limit
                    };
                    track.limit = from + extra / growable.len() as f32;
                }
            }
            for track in &mut tracks[range] {
                if track.limit < track.base {
                    track.limit = track.base;
                }
            }
        }
    }
    // Step 4: items crossing flexible tracks grow those with an automatic minimum, by
    // their factors.
    let crossing: Vec<&Contribution> = items
        .iter()
        .filter(|item| flexible(&tracks, item))
        .collect();
    for item in crossing {
        let range = item.start..item.start + item.span;
        let growable: Vec<usize> = range
            .clone()
            .filter(|at| tracks[*at].flex.is_some() && intrinsic(tracks[*at].min))
            .collect();
        let extra = automatic(item)
            - tracks[range].iter().map(|track| track.base).sum::<f32>()
            - gap * (item.span - 1) as f32;
        let factors: f32 = growable.iter().filter_map(|at| tracks[*at].flex).sum();
        if extra > 0.0 && !growable.is_empty() {
            for at in &growable {
                let share = if factors > 0.0 {
                    tracks[*at].flex.unwrap_or(0.0) / factors
                } else {
                    1.0 / growable.len() as f32
                };
                tracks[*at].base += extra * share;
            }
        }
    }
    for track in &mut tracks {
        if track.limit.is_infinite() || track.limit < track.base {
            track.limit = track.base;
        }
    }
    let live = collapsed.iter().filter(|collapsed| !**collapsed).count();
    let gaps = gap * live.saturating_sub(1) as f32;
    let total = |tracks: &[Track]| tracks.iter().map(|track| track.base).sum::<f32>();
    // 11.6: grow the tracks to their limits.
    match mode {
        Mode::Definite(available) => {
            let mut free = available - total(&tracks) - gaps;
            for _ in 0..=tracks.len() {
                let open: Vec<usize> = (0..tracks.len())
                    .filter(|at| tracks[*at].limit - tracks[*at].base > 1e-4)
                    .collect();
                if open.is_empty() || free <= 1e-4 {
                    break;
                }
                let share = free / open.len() as f32;
                for at in open {
                    let track = &mut tracks[at];
                    let grown = share.min(track.limit - track.base);
                    track.base += grown;
                    free -= grown;
                }
            }
        }
        Mode::MaxContent => {
            for track in &mut tracks {
                track.base = track.limit;
            }
        }
        Mode::MinContent => {}
    }
    // 11.7: flexible tracks.
    if tracks.iter().any(|track| track.flex.is_some()) {
        let fr = match mode {
            Mode::MinContent => 0.0,
            Mode::Definite(available) => fr_size(&tracks, 0..tracks.len(), available - gaps),
            Mode::MaxContent => {
                let mut fr = 0.0_f32;
                for track in &tracks {
                    if let Some(flex) = track.flex {
                        fr = fr.max(if flex > 1.0 {
                            track.base / flex
                        } else {
                            track.base
                        });
                    }
                }
                for item in items.iter().filter(|item| flexible(&tracks, item)) {
                    let range = item.start..item.start + item.span;
                    let space = item.max_content - gap * (item.span - 1) as f32;
                    fr = fr.max(fr_size(&tracks, range, space));
                }
                fr
            }
        };
        for track in &mut tracks {
            if let Some(flex) = track.flex {
                track.base = track.base.max(fr * flex);
            }
        }
    }
    // 11.8: `normal` and `stretch` content alignment grow the `auto` tracks.
    if let (true, Mode::Definite(available)) = (stretch, mode) {
        let free = available - total(&tracks) - gaps;
        let autos: Vec<usize> = (0..tracks.len())
            .filter(|at| tracks[*at].max == Breadth::Auto && !collapsed[*at])
            .collect();
        if free > 0.0 && !autos.is_empty() {
            for at in &autos {
                tracks[*at].base += free / autos.len() as f32;
            }
        }
    }
    tracks.into_iter().map(|track| track.base).collect()
}

/// Where each track starts and ends along an axis, from `start`: a gap, and `between`,
/// follow every live track but the last.
fn track_edges(
    sizes: &[f32],
    collapsed: &[bool],
    gap: f32,
    start: f32,
    between: f32,
) -> Vec<[f32; 2]> {
    let last_live = collapsed.iter().rposition(|collapsed| !*collapsed);
    let mut at = start;
    let mut edges = Vec::with_capacity(sizes.len());
    for (track, size) in sizes.iter().enumerate() {
        edges.push([at, at + size]);
        at += size;
        if !collapsed[track] && Some(track) != last_live {
            at += gap + between;
        }
    }
    edges
}

/// The span of `area` along an axis, gaps included.
fn area_extent(edges: &[[f32; 2]], lines: [usize; 2]) -> f32 {
    match (
        edges.get(lines[0]),
        lines[1].checked_sub(1).and_then(|last| edges.get(last)),
    ) {
        (Some(first), Some(last)) => (last[1] - first[0]).max(0.0),
        _ => 0.0,
    }
}

/// What `content alignment` leaves before the first track and between two, for the
/// tracks of an axis whose sizes and gaps total `used`.
fn distribute(space: FlexSpace, free: f32, live: usize) -> (f32, f32) {
    match space {
        // Its tracks have grown already; what they could not take sits at the end.
        FlexSpace::Stretch => (0.0, 0.0),
        // Overflowing tracks spill both ways when centered or at the end.
        FlexSpace::End => (free, 0.0),
        FlexSpace::Center => (free / 2.0, 0.0),
        other => spread(other, free, live),
    }
}

impl Solver<'_> {
    /// The in-flow children of a grid container, in order-modified document order.
    fn grid_children(&self, node: usize) -> Vec<usize> {
        let mut children: Vec<usize> = self.boxes[node]
            .children
            .iter()
            .copied()
            .filter(|child| !self.boxes[*child].out_of_flow)
            .collect();
        children.sort_by_key(|child| self.boxes[*child].flex_item.order);
        children
    }

    /// Both axes' explicit grids.
    fn grid_axes(grid: &GridContainer, width: Option<f32>, height: Option<f32>) -> [Axis; 2] {
        let extent = |pick: fn(&GridArea) -> usize| grid.areas.iter().map(pick).max().unwrap_or(0);
        let mut rows = explicit_axis(
            &grid.rows,
            &grid.auto_rows,
            extent(|area| area.rows[1]),
            height,
            grid.gap[0],
        );
        let mut columns = explicit_axis(
            &grid.columns,
            &grid.auto_columns,
            extent(|area| area.columns[1]),
            width,
            grid.gap[1],
        );
        // 7.3.2: an area names the lines at its edges.
        for area in &grid.areas {
            for (axis, lines) in [(&mut rows, area.rows), (&mut columns, area.columns)] {
                while axis.names.len() <= lines[1] {
                    axis.names.push(Vec::new());
                }
                axis.names[lines[0]].push(format!("{}-start", area.name));
                axis.names[lines[1]].push(format!("{}-end", area.name));
            }
        }
        [rows, columns]
    }

    /// Padding, border and margins along each axis: down, then across.
    fn grid_frame(&self, node: usize, basis: f32) -> [f32; 2] {
        let value = &self.boxes[node];
        let padding = self.paddings(node, basis);
        let margin = value.margin.map(|dim| dim.zero_auto(basis));
        [
            padding[0] + padding[2] + value.border[0] + value.border[2] + margin[0] + margin[2],
            padding[1] + padding[3] + value.border[1] + value.border[3] + margin[1] + margin[3],
        ]
    }

    /// Whether `child` is a subgrid in `axis` (0 rows, 1 columns).
    fn subgridded(&self, child: usize, axis: usize) -> bool {
        self.boxes[child].grid.as_deref().is_some_and(|grid| {
            if axis == 0 {
                grid.rows.subgrid
            } else {
                grid.columns.subgrid
            }
        })
    }

    /// A subgrid's margin, border and padding at the start and the end of `axis`.
    fn subgrid_frame(&self, child: usize, axis: usize, basis: f32) -> [f32; 2] {
        let value = &self.boxes[child];
        let padding = self.paddings(child, basis);
        let margin = value.margin.map(|dim| dim.zero_auto(basis));
        let [start, end] = if axis == 0 { [0, 2] } else { [3, 1] };
        [
            margin[start] + value.border[start] + padding[start],
            margin[end] + value.border[end] + padding[end],
        ]
    }

    /// The explicit grid of a subgrid's own axes: `spans` (by axis) are those its parent
    /// gives it, so many tracks named by its own line names.
    fn subgrid_axes(axes: &mut [Axis; 2], grid: &GridContainer, spans: [Option<usize>; 2]) {
        for axis in 0..2 {
            let Some(span) = spans[axis] else { continue };
            let list = if axis == 0 { &grid.rows } else { &grid.columns };
            let mut names = list.names.clone();
            names.resize(span + 1, Vec::new());
            axes[axis] = Axis {
                sizes: vec![TrackSize::AUTO; span],
                names,
                fit: None,
            };
        }
    }

    /// A subgrid's items stay inside its span, which has no implicit tracks.
    fn clamp_to_spans(placement: &mut Placement, spans: [Option<usize>; 2]) {
        for axis in 0..2 {
            let Some(span) = spans[axis] else { continue };
            let span = span.max(1);
            let shift = placement.shift[axis];
            for area in &mut placement.areas {
                let start = area[axis][0].saturating_sub(shift).min(span - 1);
                let end = area[axis][1].saturating_sub(shift).clamp(start + 1, span);
                area[axis] = [start, end];
            }
            placement.shift[axis] = 0;
            placement.count[axis] = span;
        }
    }

    /// A subgrid's items and where they go in it, for spans given by its parent.
    fn subgrid_placement(
        &self,
        child: usize,
        spans: [Option<usize>; 2],
    ) -> Option<(Vec<usize>, Placement)> {
        let grid = self.boxes[child].grid.as_deref()?;
        let children = self.grid_children(child);
        let mut axes = Self::grid_axes(grid, None, None);
        Self::subgrid_axes(&mut axes, grid, spans);
        // Measuring spends nothing; layout pays for placement.
        let mut work = self.work;
        let mut placement =
            place(self.boxes, &children, grid, [&axes[0], &axes[1]], &mut work).ok()?;
        Self::clamp_to_spans(&mut placement, spans);
        Some((children, placement))
    }

    /// What each item asks of the columns it spans. A subgrid in columns asks for its
    /// items, at their places in it, its frame added at its edges.
    fn column_contributions(
        &self,
        children: &[usize],
        placement: &Placement,
        basis: f32,
    ) -> Vec<Contribution> {
        let mut found = Vec::new();
        for (&child, area) in children.iter().zip(&placement.areas) {
            if !self.subgridded(child, 1) {
                continue;
            }
            let span = area[1][1] - area[1][0];
            let rows = self.subgridded(child, 0).then_some(area[0][1] - area[0][0]);
            let frame = self.subgrid_frame(child, 1, basis);
            if let Some((inner, inner_placement)) =
                self.subgrid_placement(child, [rows, Some(span)])
            {
                for mut wanted in self.column_contributions(&inner, &inner_placement, basis) {
                    let mut extra = 0.0;
                    if wanted.start == 0 {
                        extra += frame[0];
                    }
                    if wanted.start + wanted.span == span {
                        extra += frame[1];
                    }
                    wanted.start += area[1][0];
                    wanted.minimum += extra;
                    wanted.min_content += extra;
                    wanted.max_content += extra;
                    found.push(wanted);
                }
            }
            let frame = frame[0] + frame[1];
            found.push(Contribution {
                start: area[1][0],
                span,
                minimum: frame,
                min_content: frame,
                max_content: frame,
            });
        }
        found.extend(
            children
                .iter()
                .zip(&placement.areas)
                .filter(|(child, _)| !self.subgridded(**child, 1))
                .map(|(&child, area)| {
                    let value = &self.boxes[child];
                    let (min_content, max_content) = self.outer_measure(child);
                    // 6.6: no narrower than its content, unless it clips.
                    let minimum = if value.clips {
                        self.grid_frame(child, basis)[1] + value.min_width.zero_auto(basis)
                    } else {
                        min_content
                    };
                    Contribution {
                        start: area[1][0],
                        span: area[1][1] - area[1][0],
                        minimum,
                        min_content,
                        max_content,
                    }
                }),
        );
        found
    }

    /// Narrowest and widest content widths of a grid container.
    pub(super) fn measure_grid(&self, node: usize, grid: &GridContainer) -> (f32, f32) {
        let children = self.grid_children(node);
        let axes = Self::grid_axes(grid, None, None);
        // Measuring spends nothing; layout pays for placement.
        let mut work = self.work;
        let Ok(placement) = place(self.boxes, &children, grid, [&axes[0], &axes[1]], &mut work)
        else {
            return (0.0, 0.0);
        };
        let mut used = vec![false; placement.count[1]];
        for area in &placement.areas {
            for track in area[1][0]..area[1][1] {
                used[track] = true;
            }
        }
        let (sizes, collapsed) = all_tracks(
            &axes[1],
            &grid.auto_columns,
            placement.shift[1],
            placement.count[1],
            &used,
        );
        let items = self.column_contributions(&children, &placement, 0.0);
        let live = collapsed.iter().filter(|collapsed| !**collapsed).count();
        let gaps = grid.gap[1] * live.saturating_sub(1) as f32;
        let sum = |mode| {
            size_tracks(&sizes, &collapsed, &items, grid.gap[1], mode, None, false)
                .iter()
                .sum::<f32>()
                + gaps
        };
        let minimum = sum(Mode::MinContent);
        let preferred = sum(Mode::MaxContent);
        (minimum, preferred.max(minimum))
    }

    /// Lays out a grid container's items in its content box `inner`, whose top is
    /// `content_top`. Returns the bottom of the content: what an auto height takes.
    pub(super) fn layout_grid(
        &mut self,
        node: usize,
        grid: &GridContainer,
        inner: Containing,
        content_top: f32,
    ) -> Result<f32, FlowError> {
        for &child in &self.boxes[node].children {
            self.work = self.work.checked_sub(1).ok_or(FlowError::Work)?;
            if self.boxes[child].out_of_flow {
                self.record_static(child, content_top, inner);
            }
        }
        let children = self.grid_children(node);
        // As a subgrid, what its parent gives each axis.
        let own = self.subgrids.get(&node).cloned().unwrap_or([None, None]);
        let spans = [0, 1].map(|axis| {
            let subgrid = if axis == 0 {
                grid.rows.subgrid
            } else {
                grid.columns.subgrid
            };
            own[axis]
                .as_ref()
                .filter(|_| subgrid)
                .map(SubgridAxis::span)
        });
        let given = |axis: usize| match &own[axis] {
            Some(SubgridAxis::Edges(edges)) if spans[axis].is_some() => Some(edges.clone()),
            _ => None,
        };
        let (row_given, column_given) = (given(0), given(1));
        let measuring_rows = spans[0].is_some() && matches!(own[0], Some(SubgridAxis::Measure(_)));
        let mut axes = Self::grid_axes(grid, Some(inner.width), inner.height);
        Self::subgrid_axes(&mut axes, grid, spans);
        let mut placement = place(
            self.boxes,
            &children,
            grid,
            [&axes[0], &axes[1]],
            &mut self.work,
        )?;
        Self::clamp_to_spans(&mut placement, spans);
        let sizes_of = |edges: &[[f32; 2]]| -> Vec<f32> {
            edges
                .iter()
                .map(|edge| (edge[1] - edge[0]).max(0.0))
                .collect()
        };
        let end_of = |edges: &[[f32; 2]]| edges.last().map_or(0.0, |edge| edge[1]);
        let mut tracks = Vec::with_capacity(2);
        for axis in 0..2 {
            let mut used = vec![false; placement.count[axis]];
            for area in &placement.areas {
                for track in area[axis][0]..area[axis][1] {
                    used[track] = true;
                }
            }
            let auto = if axis == 0 {
                &grid.auto_rows
            } else {
                &grid.auto_columns
            };
            tracks.push(all_tracks(
                &axes[axis],
                auto,
                placement.shift[axis],
                placement.count[axis],
                &used,
            ));
        }
        let [(row_sizes, row_collapsed), (column_sizes, column_collapsed)] =
            [tracks[0].clone(), tracks[1].clone()];
        // 11.1: columns first, against the content box's width.
        let columns = match &column_given {
            Some(edges) => sizes_of(edges),
            None => {
                let items = self.column_contributions(&children, &placement, inner.width);
                size_tracks(
                    &column_sizes,
                    &column_collapsed,
                    &items,
                    grid.gap[1],
                    Mode::Definite(inner.width),
                    Some(inner.width),
                    grid.justify_content == FlexSpace::Stretch,
                )
            }
        };
        let column_edges = match &column_given {
            Some(edges) => edges.clone(),
            None => track_edges(&columns, &column_collapsed, grid.gap[1], 0.0, 0.0),
        };
        // Each item at its width in its area, for its height.
        let mut widths = Vec::with_capacity(children.len());
        let mut laid = Vec::with_capacity(children.len());
        let mut rows_wanted = Vec::with_capacity(children.len());
        for (&child, area) in children.iter().zip(&placement.areas) {
            let area_width = area_extent(&column_edges, area[1]);
            let value = &self.boxes[child];
            let padding = self.paddings(child, area_width);
            let margin = value.margin.map(|dim| dim.zero_auto(area_width));
            let frame = padding[1] + padding[3] + value.border[1] + value.border[3];
            let room = (area_width - frame - margin[1] - margin[3]).max(0.0);
            let justify = value
                .grid_item
                .as_deref()
                .and_then(|item| item.justify)
                .unwrap_or(grid.justify_items);
            let auto_margins = value.margin[1] == Dim::Auto || value.margin[3] == Dim::Auto;
            let width = match value.width.horizontal(area_width) {
                AutoLength::Value(width) => width,
                AutoLength::Auto if justify == FlexAlign::Stretch && !auto_margins => room,
                AutoLength::Auto => {
                    let (narrowest, widest) = self.measure(child);
                    widest.min(room.max(narrowest))
                }
            };
            let width = clamp(
                width,
                value.min_width.zero_auto(area_width),
                match value.max_width {
                    Dim::Auto => None,
                    other => Some(other.zero_auto(area_width)),
                },
            )
            .max(0.0);
            let cb = Containing {
                width: area_width,
                height: None,
                top: None,
                ..inner
            };
            // A subgrid takes the columns it spans as they are, and is measured for rows.
            let mut taken = [None, None];
            if self.subgridded(child, 1) {
                let frame = self.subgrid_frame(child, 1, area_width)[0];
                let left = column_edges.get(area[1][0]).map_or(0.0, |edge| edge[0]) + frame;
                taken[1] = Some(SubgridAxis::Edges(
                    column_edges
                        .get(area[1][0]..area[1][1])
                        .unwrap_or_default()
                        .iter()
                        .map(|edge| [edge[0] - left, edge[1] - left])
                        .collect(),
                ));
            }
            let row_subgrid = self.subgridded(child, 0);
            if row_subgrid {
                taken[0] = Some(SubgridAxis::Measure(area[0][1] - area[0][0]));
            }
            if taken.iter().any(Option::is_some) {
                self.subgrids.insert(child, taken);
            }
            let border_height = self.lay_item(child, cb, width)?;
            let outer = border_height + margin[0] + margin[2];
            widths.push(area_width);
            laid.push((cb, width));
            let wanted = if row_subgrid {
                self.subgrid_wanted.remove(&child)
            } else {
                None
            };
            match wanted {
                // Its items ask for its rows, its frame added at its edges.
                Some(wanted) => {
                    let span = area[0][1] - area[0][0];
                    let frame = self.subgrid_frame(child, 0, area_width);
                    for mut one in wanted {
                        let mut extra = 0.0;
                        if one.start == 0 {
                            extra += frame[0];
                        }
                        if one.start + one.span == span {
                            extra += frame[1];
                        }
                        one.start += area[0][0];
                        one.minimum += extra;
                        one.min_content += extra;
                        one.max_content += extra;
                        rows_wanted.push(one);
                    }
                    let frame = frame[0] + frame[1];
                    rows_wanted.push(Contribution {
                        start: area[0][0],
                        span,
                        minimum: frame,
                        min_content: frame,
                        max_content: frame,
                    });
                }
                None => rows_wanted.push(Contribution {
                    start: area[0][0],
                    span: area[0][1] - area[0][0],
                    minimum: outer,
                    min_content: outer,
                    max_content: outer,
                }),
            }
        }
        // Measured as a subgrid: its parent sizes its rows with what its items ask.
        if measuring_rows {
            self.subgrid_wanted.insert(node, rows_wanted.clone());
        }
        let row_mode = inner.height.map_or(Mode::MaxContent, Mode::Definite);
        let rows = match &row_given {
            Some(edges) => sizes_of(edges),
            None => size_tracks(
                &row_sizes,
                &row_collapsed,
                &rows_wanted,
                grid.gap[0],
                row_mode,
                inner.height,
                grid.align_content == FlexSpace::Stretch,
            ),
        };
        // 10.5: content alignment shares what the tracks leave.
        let live = |collapsed: &[bool]| collapsed.iter().filter(|collapsed| !**collapsed).count();
        let total = |sizes: &[f32], collapsed: &[bool], gap: f32| {
            sizes.iter().sum::<f32>() + gap * live(collapsed).saturating_sub(1) as f32
        };
        let rows_total = match &row_given {
            Some(edges) => end_of(edges),
            None => total(&rows, &row_collapsed, grid.gap[0]),
        };
        let columns_total = match &column_given {
            Some(edges) => end_of(edges),
            None => total(&columns, &column_collapsed, grid.gap[1]),
        };
        let (column_start, column_between) = distribute(
            grid.justify_content,
            inner.width - columns_total,
            live(&column_collapsed),
        );
        let (row_start, row_between) = distribute(
            grid.align_content,
            inner.height.map_or(0.0, |height| height - rows_total),
            live(&row_collapsed),
        );
        // What a parent gave is placed as given.
        let column_edges = match column_given {
            Some(edges) => edges,
            None => track_edges(
                &columns,
                &column_collapsed,
                grid.gap[1],
                column_start,
                column_between,
            ),
        };
        let row_edges = match row_given {
            Some(edges) => edges,
            None => track_edges(&rows, &row_collapsed, grid.gap[0], row_start, row_between),
        };
        let rtl = inner.direction == Direction::Rtl;
        for (((&child, area), area_width), (cb, width)) in
            children.iter().zip(&placement.areas).zip(widths).zip(laid)
        {
            // A subgrid in rows is laid again in the rows it spans, as sized.
            if self.subgridded(child, 0) {
                let frame = self.subgrid_frame(child, 0, area_width)[0];
                let top = row_edges.get(area[0][0]).map_or(0.0, |edge| edge[0]) + frame;
                let edges = row_edges
                    .get(area[0][0]..area[0][1])
                    .unwrap_or_default()
                    .iter()
                    .map(|edge| [edge[0] - top, edge[1] - top])
                    .collect();
                self.subgrids.entry(child).or_insert([None, None])[0] =
                    Some(SubgridAxis::Edges(edges));
                self.lay_item(child, cb, width)?;
            }
            let value = &self.boxes[child];
            let left = column_edges.get(area[1][0]).map_or(0.0, |edge| edge[0]);
            let top = row_edges.get(area[0][0]).map_or(0.0, |edge| edge[0]);
            let area_height = area_extent(&row_edges, area[0]);
            let margin = value.margin.map(|dim| dim.zero_auto(area_width));
            let geometry = self.geometry[child];
            let border_width = geometry.border.right - geometry.border.left;
            let mut border_height = geometry.border.bottom - geometry.border.top;
            let content_height = geometry.content.bottom - geometry.content.top;
            let justify = value
                .grid_item
                .as_deref()
                .and_then(|item| item.justify)
                .unwrap_or(grid.justify_items);
            let align = value.flex_item.align.unwrap_or(grid.align_items);
            // 11.2: auto margins take the space first, then self-alignment places it.
            let place_in = |room: f32,
                            size: f32,
                            before: f32,
                            after: f32,
                            autos: (bool, bool),
                            align: FlexAlign| {
                let free = room - size - before - after;
                match autos {
                    (true, true) => before + free.max(0.0) / 2.0,
                    (true, false) => before + free.max(0.0),
                    (false, true) => before,
                    (false, false) => {
                        before
                            + match align {
                                FlexAlign::Stretch | FlexAlign::Start => 0.0,
                                FlexAlign::End => free,
                                FlexAlign::Center => free / 2.0,
                            }
                    }
                }
            };
            let vertical_autos = (value.margin[0] == Dim::Auto, value.margin[2] == Dim::Auto);
            if align == FlexAlign::Stretch
                && value.height == Dim::Auto
                && vertical_autos == (false, false)
            {
                let frame = border_height - content_height;
                let stretched = clamp(
                    area_height - margin[0] - margin[2] - frame,
                    value.min_height.vertical(None).unwrap_or(0.0),
                    value.max_height.vertical(None),
                )
                .max(0.0);
                self.set_content_height(child, stretched);
                border_height = stretched + frame;
            }
            let value = &self.boxes[child];
            let x = left
                + place_in(
                    area_width,
                    border_width,
                    margin[3],
                    margin[1],
                    (value.margin[3] == Dim::Auto, value.margin[1] == Dim::Auto),
                    justify,
                );
            let y = top
                + place_in(
                    area_height,
                    border_height,
                    margin[0],
                    margin[2],
                    vertical_autos,
                    align,
                );
            let x = if rtl {
                inner.width - x - border_width
            } else {
                x
            };
            let now = self.geometry[child].border;
            self.translate(child, inner.left + x - now.left, content_top + y - now.top);
            self.geometry[child].margin = margin;
        }
        Ok(content_top + rows_total)
    }
}
