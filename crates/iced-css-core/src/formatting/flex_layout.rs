//! Flex layout (CSS Flexible Box Layout Level 1, section 9), as one more way a box that
//! establishes a formatting context lays out its children.
//!
//! Left out: baseline alignment (it aligns at the start), `visibility: collapse`, and
//! fragmentation. An item's automatic minimum size is its narrowest content width in a row,
//! unless it clips; a column has no automatic minimum.
use super::{AutoLength, Containing, Dim, Direction, FlowError, Solver, clamp};

/// `flex-wrap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// `justify-content` and, with `Stretch`, `align-content`: how free space is shared out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexSpace {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    /// `align-content` only: the lines grow to take the space.
    Stretch,
}

/// `align-items` and `align-self`. `baseline` is `Start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexAlign {
    #[default]
    Stretch,
    Start,
    End,
    Center,
}

/// A flex container's own properties.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FlexContainer {
    /// `flex-direction: column` or `column-reverse`: the main axis runs down.
    pub column: bool,
    /// A `-reverse` direction.
    pub reverse: bool,
    pub wrap: FlexWrap,
    pub justify: FlexSpace,
    pub align_items: FlexAlign,
    /// `Stretch` is the initial `normal`.
    pub align_content: FlexSpace,
    /// `row-gap`, between lines of a row container and items of a column one, then
    /// `column-gap`.
    pub gap: [f32; 2],
}

/// What a box is as a flex item; it only matters in a flex container.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexItem {
    pub grow: f32,
    pub shrink: f32,
    /// `Dim::Auto` is `auto`: the item's `width` or `height`, or its content.
    pub basis: Dim,
    /// `None` is `auto`: the container's `align-items`.
    pub align: Option<FlexAlign>,
    pub order: i32,
}

impl Default for FlexItem {
    fn default() -> Self {
        Self {
            grow: 0.0,
            shrink: 1.0,
            basis: Dim::Auto,
            align: None,
            order: 0,
        }
    }
}

/// One item while its line is resolved: sizes along the main axis, of its content box.
struct Item {
    node: usize,
    grow: f32,
    shrink: f32,
    base: f32,
    /// `base` within its minimum and maximum.
    hypothetical: f32,
    minimum: f32,
    maximum: Option<f32>,
    /// Padding, border and margins along the main axis.
    frame: f32,
    /// Its main size once flexed.
    used: f32,
    /// Margins: top, right, bottom, left, with `auto` as none.
    margin: [f32; 4],
    /// How many of its two main-axis margins are `auto`.
    auto_margins: usize,
    /// Its outer size across the main axis, once laid out.
    cross: f32,
}

/// Where each of `count` things starts along an axis with `free` space to share, and the
/// space between two of them beyond the gap.
pub(super) fn spread(space: FlexSpace, free: f32, count: usize) -> (f32, f32) {
    let free = free.max(0.0);
    let n = count as f32;
    match space {
        FlexSpace::Start | FlexSpace::Stretch => (0.0, 0.0),
        FlexSpace::End => (free, 0.0),
        FlexSpace::Center => (free / 2.0, 0.0),
        FlexSpace::SpaceBetween if count > 1 => (0.0, free / (n - 1.0)),
        FlexSpace::SpaceBetween => (0.0, 0.0),
        FlexSpace::SpaceAround if count > 0 => (free / n / 2.0, free / n),
        FlexSpace::SpaceEvenly if count > 0 => (free / (n + 1.0), free / (n + 1.0)),
        FlexSpace::SpaceAround | FlexSpace::SpaceEvenly => (0.0, 0.0),
    }
}

/// 9.7: the used main sizes of one line's items, given the space they share.
fn resolve_flexible(items: &mut [Item], available: f32, gap: f32) {
    let gaps = gap * items.len().saturating_sub(1) as f32;
    let outer: f32 = items
        .iter()
        .map(|item| item.hypothetical + item.frame)
        .sum();
    let growing = outer + gaps < available;
    let mut frozen: Vec<bool> = items
        .iter()
        .map(|item| {
            let factor = if growing { item.grow } else { item.shrink };
            factor == 0.0
                || (growing && item.base > item.hypothetical)
                || (!growing && item.base < item.hypothetical)
        })
        .collect();
    for item in items.iter_mut() {
        item.used = item.hypothetical;
    }
    let initial_free = available
        - gaps
        - items
            .iter()
            .zip(&frozen)
            .map(|(item, frozen)| item.frame + if *frozen { item.used } else { item.base })
            .sum::<f32>();
    // Each pass freezes at least one item, so this ends within the item count.
    for _ in 0..=items.len() {
        if frozen.iter().all(|frozen| *frozen) {
            break;
        }
        let mut free = available
            - gaps
            - items
                .iter()
                .zip(&frozen)
                .map(|(item, frozen)| item.frame + if *frozen { item.used } else { item.base })
                .sum::<f32>();
        let factors: f32 = items
            .iter()
            .zip(&frozen)
            .filter(|(_, frozen)| !**frozen)
            .map(|(item, _)| if growing { item.grow } else { item.shrink })
            .sum();
        // Factors summing to less than one take only that share of the space.
        if factors < 1.0 && (initial_free * factors).abs() < free.abs() {
            free = initial_free * factors;
        }
        let scaled: f32 = items
            .iter()
            .zip(&frozen)
            .filter(|(_, frozen)| !**frozen)
            .map(|(item, _)| item.shrink * item.base)
            .sum();
        let mut violation = 0.0_f32;
        let mut clamped = vec![0.0_f32; items.len()];
        for (at, item) in items.iter_mut().enumerate() {
            if frozen[at] {
                continue;
            }
            let target = if growing {
                item.base + free * item.grow / factors.max(f32::MIN_POSITIVE)
            } else if scaled > 0.0 {
                item.base - free.abs() * (item.shrink * item.base) / scaled
            } else {
                item.base
            };
            let within = clamp(target, item.minimum, item.maximum).max(0.0);
            clamped[at] = within - target;
            violation += within - target;
            item.used = within;
        }
        // No violation freezes everything; otherwise those clamped the way the total went.
        for at in 0..items.len() {
            if !frozen[at]
                && (violation == 0.0
                    || (violation > 0.0 && clamped[at] > 0.0)
                    || (violation < 0.0 && clamped[at] < 0.0))
            {
                frozen[at] = true;
            }
        }
    }
}

impl Solver<'_> {
    /// Narrowest and widest content widths of a flex container.
    pub(super) fn measure_flex(&self, node: usize, flex: FlexContainer) -> (f32, f32) {
        let children: Vec<usize> = self.boxes[node]
            .children
            .iter()
            .copied()
            .filter(|child| !self.boxes[*child].out_of_flow)
            .collect();
        let gaps = flex.gap[1] * children.len().saturating_sub(1) as f32;
        let (mut minimum, mut preferred) = (0.0_f32, 0.0_f32);
        for child in children {
            let (child_min, child_preferred) = self.outer_measure(child);
            if flex.column {
                minimum = minimum.max(child_min);
                preferred = preferred.max(child_preferred);
            } else {
                // A row that may wrap is as narrow as its widest item; one that may not
                // holds them all.
                minimum = if flex.wrap == FlexWrap::NoWrap {
                    minimum + child_min
                } else {
                    minimum.max(child_min)
                };
                preferred += child_preferred;
            }
        }
        if !flex.column {
            preferred += gaps;
            if flex.wrap == FlexWrap::NoWrap {
                minimum += gaps;
            }
        }
        (minimum, preferred.max(minimum))
    }

    /// Sets a laid-out box's content height, moving its lower edges.
    pub(super) fn set_content_height(&mut self, node: usize, height: f32) {
        let geometry = &mut self.geometry[node];
        let delta = height - (geometry.content.bottom - geometry.content.top);
        geometry.content.bottom += delta;
        geometry.padding.bottom += delta;
        geometry.border.bottom += delta;
    }

    /// Lays `node` out again at content height `height`, unless it has it already: what it
    /// holds moves with the size it was given (grown, shrunk or stretched).
    pub(super) fn relay_at_height(
        &mut self,
        node: usize,
        cb: Containing,
        height: f32,
    ) -> Result<(), FlowError> {
        let content = self.geometry[node].content;
        // The same size as laid is enough only where it was definite already: an auto
        // height made definite gives percentage heights inside it a basis.
        if ((content.bottom - content.top) - height).abs() <= 0.01
            && self.boxes[node].height != Dim::Auto
        {
            return Ok(());
        }
        let width = content.right - content.left;
        self.forced_heights.insert(node, height);
        self.lay_item(node, cb, width)?;
        // Its own limits may still have it otherwise; the container's size stands.
        self.set_content_height(node, height);
        Ok(())
    }

    /// Lays `node` out at the origin with content width `width`, as a formatting context of
    /// its own, and returns its border box height.
    pub(super) fn lay_item(
        &mut self,
        node: usize,
        cb: Containing,
        width: f32,
    ) -> Result<f32, FlowError> {
        let forced = self.forced_heights.get(&node).copied();
        // Everything its layout depends on from outside, a subgrid's parent tracks too.
        let tracks = self.subgrids.get(&node).map_or(0, |given| {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            for axis in given {
                match axis {
                    None => 0u8.hash(&mut hasher),
                    Some(super::grid_layout::SubgridAxis::Measure(span)) => {
                        1u8.hash(&mut hasher);
                        span.hash(&mut hasher);
                    }
                    Some(super::grid_layout::SubgridAxis::Edges(edges)) => {
                        2u8.hash(&mut hasher);
                        for edge in edges {
                            edge[0].to_bits().hash(&mut hasher);
                            edge[1].to_bits().hash(&mut hasher);
                        }
                    }
                }
            }
            hasher.finish() | 1
        });
        let key = [
            u64::from(cb.width.to_bits()),
            cb.height
                .map_or(u64::MAX, |height| u64::from(height.to_bits())),
            u64::from(width.to_bits()),
            forced.map_or(u64::MAX, |height| u64::from(height.to_bits())),
            u64::from(cb.direction == super::Direction::Rtl),
            tracks,
        ];
        let cacheable = true;
        if cacheable
            && let Some(laid) = self
                .laid
                .get(&node)
                .and_then(|kept| kept.iter().find(|laid| laid.key == key))
        {
            self.forced_heights.remove(&node);
            for (at, geometry) in &laid.geometry {
                self.geometry[*at] = *geometry;
            }
            for (at, _) in &laid.geometry {
                self.row_baselines.remove(at);
            }
            for (at, baseline) in &laid.baselines {
                self.row_baselines.insert(*at, *baseline);
            }
            self.last_flowed = laid.last_flowed;
            if let Some(wanted) = &laid.wanted {
                self.subgrid_wanted.insert(node, wanted.clone());
            }
            return Ok(laid.height);
        }
        let padding = self.paddings(node, cb.width);
        let height = self.finish_bfc_box(
            node,
            Containing {
                left: 0.0,
                top: Some(0.0),
                ..cb
            },
            (width, 0.0, 0.0),
            0.0,
            0.0,
            padding,
        )?;
        if cacheable {
            // Its subtree, at the origin it was laid out at.
            let mut nodes = vec![node];
            let mut next = 0;
            while let Some(&at) = nodes.get(next) {
                next += 1;
                self.work = self.work.checked_sub(1).ok_or(FlowError::Work)?;
                nodes.extend(self.boxes[at].children.iter().copied());
            }
            let laid = super::LaidItem {
                key,
                wanted: self.subgrid_wanted.get(&node).cloned(),
                height,
                last_flowed: self.last_flowed,
                geometry: nodes.iter().map(|at| (*at, self.geometry[*at])).collect(),
                baselines: nodes
                    .iter()
                    .filter_map(|at| self.row_baselines.get(at).map(|baseline| (*at, *baseline)))
                    .collect(),
            };
            let kept = self.laid.entry(node).or_default();
            if kept.len() == 2 {
                kept.remove(0);
            }
            kept.push(laid);
        }
        Ok(height)
    }

    /// Lays out a flex container's items in its content box `inner`, whose top is
    /// `content_top`. Returns the bottom of the content: what an auto height takes.
    pub(super) fn layout_flex(
        &mut self,
        node: usize,
        flex: FlexContainer,
        inner: Containing,
        content_top: f32,
    ) -> Result<f32, FlowError> {
        let mut children: Vec<usize> = Vec::new();
        for &child in &self.boxes[node].children.clone() {
            self.work = self.work.checked_sub(1).ok_or(FlowError::Work)?;
            if self.boxes[child].out_of_flow {
                // 4.1: its static position is the content box's start corner.
                self.record_static(child, content_top, inner);
            } else {
                children.push(child);
            }
        }
        // `order`, then document order; a reversed direction runs them backwards.
        children.sort_by_key(|child| self.boxes[*child].flex_item.order);
        let basis = inner.width;
        let item_cb = Containing { top: None, ..inner };
        let across_gap = if flex.column {
            flex.gap[1]
        } else {
            flex.gap[0]
        };
        let along_gap = if flex.column {
            flex.gap[0]
        } else {
            flex.gap[1]
        };
        let mut items: Vec<Item> = Vec::with_capacity(children.len());
        for &child in &children {
            let value = &self.boxes[child];
            let padding = self.paddings(child, basis);
            let margin = value.margin.map(|dim| dim.zero_auto(basis));
            let autos = |first: usize, second: usize| {
                usize::from(value.margin[first] == Dim::Auto)
                    + usize::from(value.margin[second] == Dim::Auto)
            };
            let item = if flex.column {
                // Its width first: stretched across, or as wide as its content.
                let frame_across = padding[3]
                    + padding[1]
                    + value.border[3]
                    + value.border[1]
                    + margin[3]
                    + margin[1];
                let align = value.flex_item.align.unwrap_or(flex.align_items);
                let (min_content, preferred) = self.measure(child);
                let width = match value.width.horizontal(basis) {
                    AutoLength::Value(width) => width,
                    AutoLength::Auto if align == FlexAlign::Stretch && autos(3, 1) == 0 => {
                        (basis - frame_across).max(0.0)
                    }
                    AutoLength::Auto => preferred.min((basis - frame_across).max(min_content)),
                };
                let width = clamp(
                    width,
                    value.min_width.zero_auto(basis),
                    match value.max_width {
                        Dim::Auto => None,
                        other => Some(other.zero_auto(basis)),
                    },
                )
                .max(0.0);
                let border_height = self.lay_item(child, item_cb, width)?;
                let frame_down = padding[0] + padding[2] + value.border[0] + value.border[2];
                let value = &self.boxes[child];
                let content = (border_height - frame_down).max(0.0);
                let base = match value.flex_item.basis {
                    Dim::Auto => content,
                    other => other.vertical(inner.height).unwrap_or(content),
                };
                let minimum = value.min_height.vertical(inner.height).unwrap_or(0.0);
                let maximum = value.max_height.vertical(inner.height);
                Item {
                    node: child,
                    grow: value.flex_item.grow.max(0.0),
                    shrink: value.flex_item.shrink.max(0.0),
                    base,
                    hypothetical: clamp(base, minimum, maximum).max(0.0),
                    minimum,
                    maximum,
                    frame: frame_down + margin[0] + margin[2],
                    used: 0.0,
                    margin,
                    auto_margins: autos(0, 2),
                    cross: width + frame_across,
                }
            } else {
                let (min_content, preferred) = self.measure(child);
                let specified = match value.flex_item.basis {
                    Dim::Auto => value.width.horizontal(basis),
                    other => other.horizontal(basis),
                };
                let base = match specified {
                    AutoLength::Value(width) => width,
                    AutoLength::Auto => preferred,
                };
                // 4.5: no narrower than its content, or than a width it gives, unless it
                // clips; a minimum it gives itself stands.
                let floor = value.min_width.zero_auto(basis);
                let automatic = if value.clips || floor > 0.0 {
                    floor
                } else {
                    match value.width.horizontal(basis) {
                        AutoLength::Value(width) => min_content.min(width),
                        AutoLength::Auto => min_content,
                    }
                };
                let maximum = match value.max_width {
                    Dim::Auto => None,
                    other => Some(other.zero_auto(basis)),
                };
                let minimum = maximum.map_or(automatic, |limit| automatic.min(limit));
                Item {
                    node: child,
                    grow: value.flex_item.grow.max(0.0),
                    shrink: value.flex_item.shrink.max(0.0),
                    base,
                    hypothetical: clamp(base, minimum, maximum).max(0.0),
                    minimum,
                    maximum,
                    frame: padding[3]
                        + padding[1]
                        + value.border[3]
                        + value.border[1]
                        + margin[3]
                        + margin[1],
                    used: 0.0,
                    margin,
                    auto_margins: autos(3, 1),
                    cross: 0.0,
                }
            };
            items.push(item);
        }
        // 9.3: lines. A column breaks only against a definite height.
        let available = if flex.column {
            inner.height
        } else {
            Some(inner.width)
        };
        let mut lines: Vec<Vec<Item>> = Vec::new();
        let mut line: Vec<Item> = Vec::new();
        let mut filled = 0.0_f32;
        for item in items {
            let outer = item.hypothetical + item.frame;
            let wraps = flex.wrap != FlexWrap::NoWrap
                && !line.is_empty()
                && available.is_some_and(|available| filled + along_gap + outer > available);
            if wraps {
                lines.push(std::mem::take(&mut line));
                filled = 0.0;
            }
            filled += outer + if line.is_empty() { 0.0 } else { along_gap };
            line.push(item);
        }
        if !line.is_empty() {
            lines.push(line);
        }
        // 9.7 and the cross size of each item and line.
        let mut line_cross: Vec<f32> = Vec::with_capacity(lines.len());
        for line in &mut lines {
            match available {
                Some(available) => resolve_flexible(line, available, along_gap),
                None => {
                    for item in line.iter_mut() {
                        item.used = item.hypothetical;
                    }
                }
            }
            let mut tallest = 0.0_f32;
            for item in line.iter_mut() {
                if flex.column {
                    self.relay_at_height(item.node, item_cb, item.used)?;
                } else {
                    let border_height = self.lay_item(item.node, item_cb, item.used)?;
                    item.cross = border_height + item.margin[0] + item.margin[2];
                }
                tallest = tallest.max(item.cross);
            }
            line_cross.push(tallest);
        }
        // 9.4: one line in a container with a definite cross size fills it.
        let definite_cross = if flex.column {
            Some(inner.width)
        } else {
            inner.height
        };
        if let (Some(cross), [only]) = (definite_cross, line_cross.as_mut_slice()) {
            if flex.wrap == FlexWrap::NoWrap {
                *only = cross;
            }
        }
        // 9.4, 9.6: lines share out the container's free cross space.
        let cross_gaps = across_gap * line_cross.len().saturating_sub(1) as f32;
        let lines_total: f32 = line_cross.iter().sum::<f32>() + cross_gaps;
        let free_cross = definite_cross.map_or(0.0, |cross| cross - lines_total);
        if flex.align_content == FlexSpace::Stretch && free_cross > 0.0 {
            let share = free_cross / line_cross.len().max(1) as f32;
            for cross in &mut line_cross {
                *cross += share;
            }
        }
        let (cross_start, cross_between) = spread(flex.align_content, free_cross, line_cross.len());
        // Which way each axis runs: a row's main axis follows the direction, reversed by
        // a `-reverse`; `wrap-reverse` stacks the lines from the far edge.
        let rtl = inner.direction == Direction::Rtl;
        let main_reversed = if flex.column {
            flex.reverse
        } else {
            flex.reverse != rtl
        };
        let cross_reversed = if flex.column {
            (flex.wrap == FlexWrap::WrapReverse) != rtl
        } else {
            flex.wrap == FlexWrap::WrapReverse
        };
        let main_extent = available.unwrap_or_else(|| {
            lines
                .iter()
                .map(|line| {
                    line.iter().map(|item| item.used + item.frame).sum::<f32>()
                        + along_gap * line.len().saturating_sub(1) as f32
                })
                .fold(0.0, f32::max)
        });
        let cross_extent = definite_cross.unwrap_or(lines_total);
        let mut cross_at = cross_start;
        for (line, cross_size) in lines.iter().zip(&line_cross) {
            let used: f32 = line.iter().map(|item| item.used + item.frame).sum::<f32>()
                + along_gap * line.len().saturating_sub(1) as f32;
            let free = available.map_or(0.0, |available| available - used);
            // 9.5: auto margins take the free space before `justify-content` sees it.
            let autos: usize = line.iter().map(|item| item.auto_margins).sum();
            let each_auto = if autos > 0 && free > 0.0 {
                free / autos as f32
            } else {
                0.0
            };
            let (main_start, main_between) = if autos > 0 && free > 0.0 {
                (0.0, 0.0)
            } else {
                // Overflowing items stay at the start, but `center` and `end` overflow
                // both ways as free space would be shared.
                match flex.justify {
                    FlexSpace::End => (free, 0.0),
                    FlexSpace::Center => (free / 2.0, 0.0),
                    other => spread(other, free, line.len()),
                }
            };
            let mut main_at = main_start;
            for item in line {
                let value = &self.boxes[item.node];
                let align = value.flex_item.align.unwrap_or(flex.align_items);
                // Margins by flow: a mirrored axis starts at the physical end side.
                let margins = item.margin;
                let auto = |side: usize| value.margin[side] == Dim::Auto;
                let flow = |sides: [usize; 2], reversed: bool| {
                    if reversed {
                        [sides[1], sides[0]]
                    } else {
                        sides
                    }
                };
                let (main_sides, cross_sides) = if flex.column {
                    (flow([0, 2], main_reversed), flow([3, 1], cross_reversed))
                } else {
                    (flow([3, 1], main_reversed), flow([0, 2], cross_reversed))
                };
                let (before, after) = (auto(main_sides[0]), auto(main_sides[1]));
                let cross_autos = (auto(cross_sides[0]), auto(cross_sides[1]));
                let lead = if before { each_auto } else { 0.0 };
                let trail = if after { each_auto } else { 0.0 };
                let geometry = self.geometry[item.node];
                let (border_width, border_height) = (
                    geometry.border.right - geometry.border.left,
                    geometry.border.bottom - geometry.border.top,
                );
                // Across the line: stretch an auto size, or place within the line.
                let (cross_margin_start, cross_margin_end) =
                    (margins[cross_sides[0]], margins[cross_sides[1]]);
                let mut cross_border = if flex.column {
                    border_width
                } else {
                    border_height
                };
                let room = cross_size - cross_margin_start - cross_margin_end;
                // 8.1: auto margins across take the free space, and keep it from stretching.
                if align == FlexAlign::Stretch
                    && !flex.column
                    && value.height == Dim::Auto
                    && cross_autos == (false, false)
                {
                    let frame = border_height - (geometry.content.bottom - geometry.content.top);
                    let stretched = clamp(
                        room - frame,
                        value.min_height.vertical(inner.height).unwrap_or(0.0),
                        value.max_height.vertical(inner.height),
                    )
                    .max(0.0);
                    self.relay_at_height(item.node, item_cb, stretched)?;
                    cross_border = stretched + frame;
                }
                let free_across = room - cross_border;
                let within = match cross_autos {
                    (true, true) => free_across.max(0.0) / 2.0,
                    (true, false) => free_across.max(0.0),
                    (false, true) => 0.0,
                    (false, false) => match align {
                        FlexAlign::Stretch | FlexAlign::Start => 0.0,
                        FlexAlign::End => free_across,
                        FlexAlign::Center => free_across / 2.0,
                    },
                };
                let outer_main = item.used + item.frame + lead + trail;
                let main_margin_start = margins[main_sides[0]];
                // Offsets from the content box's start edges, before any reversal.
                let main_offset = main_at + lead + main_margin_start;
                let cross_offset = cross_at + cross_margin_start + within;
                let main_border = if flex.column {
                    border_height.max(
                        self.geometry[item.node].border.bottom
                            - self.geometry[item.node].border.top,
                    )
                } else {
                    border_width
                };
                let main_position = if main_reversed {
                    main_extent - main_offset - main_border
                } else {
                    main_offset
                };
                let cross_position = if cross_reversed {
                    cross_extent - cross_offset - cross_border
                } else {
                    cross_offset
                };
                let (x, y) = if flex.column {
                    (inner.left + cross_position, content_top + main_position)
                } else {
                    (inner.left + main_position, content_top + cross_position)
                };
                let now = self.geometry[item.node].border;
                self.translate(item.node, x - now.left, y - now.top);
                self.geometry[item.node].margin = item.margin;
                main_at += outer_main + along_gap + main_between;
            }
            cross_at += cross_size + across_gap + cross_between;
        }
        // What an auto height takes: the items down a column, or the lines of a row.
        let content_height = if flex.column {
            main_extent
        } else {
            line_cross.iter().sum::<f32>() + cross_gaps
        };
        Ok(content_top + content_height)
    }
}
