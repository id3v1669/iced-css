//! Explicit animation requests over restyled trees (Q039, section 13). The stylesheet stays
//! the only store of permanent values: `to` is what the cascade computed, and this keeps only
//! the temporary presentation on the way there. Style changes without a request stay
//! immediate. Luminate owns the time, through the progress engine; colors mix per component
//! in gamma-encoded sRGB (Q070).
use super::{BoxTree, BoxTreeError, failed};
use crate::animation::{
    clock::AnimationTiming,
    engine::{ProgressEngine, ProgressLimits},
};
use iced::animation::Easing;
use iced_css_core::{
    animation::registry::AnimationSelection,
    formatting::block_flow::Dim,
    identity::{SourceRef, SourceRegistry},
    values::registry::{AnimationRule, PropertyId},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
    time::Instant,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Value {
    Color([f32; 3]),
    /// A background on its way to or from transparent: color and opacity. Transparent has
    /// opacity 0.
    Fade([f32; 3], f32),
    Length(Dim),
    /// `visibility`: shown or not; `collapse` shows as `hidden` does here.
    Visible(bool),
}

fn rgba(value: Value) -> Option<([f32; 3], f32)> {
    match value {
        Value::Color(color) => Some((color, 1.0)),
        Value::Fade(color, alpha) => Some((color, alpha)),
        Value::Length(_) | Value::Visible(_) => None,
    }
}

/// Where one longhand lives in a styled tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Field {
    Color,
    Background,
    BorderColor(usize),
    OutlineColor,
    OutlineWidth,
    Width,
    MinWidth,
    MaxWidth,
    Height,
    MinHeight,
    MaxHeight,
    Margin(usize),
    Padding(usize),
    BorderWidth(usize),
    Offset(usize),
    Visibility,
    FontSize,
}

const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

fn field(name: &str) -> Option<Field> {
    let side = |rest: &str| SIDES.iter().position(|side| *side == rest);
    Some(match name {
        "color" => Field::Color,
        "background-color" => Field::Background,
        "outline-color" => Field::OutlineColor,
        "outline-width" => Field::OutlineWidth,
        "width" => Field::Width,
        "min-width" => Field::MinWidth,
        "max-width" => Field::MaxWidth,
        "height" => Field::Height,
        "min-height" => Field::MinHeight,
        "max-height" => Field::MaxHeight,
        "visibility" => Field::Visibility,
        "font-size" => Field::FontSize,
        _ => {
            if let Some(rest) = name.strip_prefix("margin-") {
                Field::Margin(side(rest)?)
            } else if let Some(rest) = name.strip_prefix("padding-") {
                Field::Padding(side(rest)?)
            } else if let Some(rest) = name.strip_prefix("border-") {
                if let Some(at) = rest.strip_suffix("-color") {
                    Field::BorderColor(side(at)?)
                } else {
                    Field::BorderWidth(side(rest.strip_suffix("-width")?)?)
                }
            } else {
                Field::Offset(side(name)?)
            }
        }
    })
}

/// Between two values of one kind. `None` has no in-between: keywords, or `auto`.
fn mix(from: Value, to: Value, progress: f32) -> Option<Value> {
    let along = |a: f32, b: f32| a + (b - a) * progress;
    match (from, to) {
        (Value::Color(_) | Value::Fade(..), Value::Color(_) | Value::Fade(..)) => {
            // Q070: gamma-encoded sRGB with premultiplied alpha. Fully transparent has no
            // color of its own, so premultiplying takes the other end's.
            let ((a, from_alpha), (b, to_alpha)) = (rgba(from)?, rgba(to)?);
            let alpha = along(from_alpha, to_alpha);
            let mixed = std::array::from_fn(|at| {
                let value = along(a[at] * from_alpha, b[at] * to_alpha);
                if alpha > 0.0 { value / alpha } else { value }
            });
            Some(if alpha == 1.0 {
                Value::Color(mixed)
            } else {
                Value::Fade(mixed, alpha)
            })
        }
        (Value::Length(Dim::Px(a)), Value::Length(Dim::Px(b))) => {
            Some(Value::Length(Dim::Px(along(a, b))))
        }
        (Value::Length(Dim::Percent(a)), Value::Length(Dim::Percent(b))) => {
            Some(Value::Length(Dim::Percent(along(a, b))))
        }
        // A length and a percentage mix as `calc()` does, for layout to resolve.
        (Value::Length(a), Value::Length(b)) => {
            let parts = |dim: Dim| match dim {
                Dim::Px(pixels) => Some((pixels, 0.0)),
                Dim::Percent(percent) => Some((0.0, percent)),
                Dim::Mix(pixels, percent) => Some((pixels, percent)),
                Dim::Auto => None,
            };
            let ((a_pixels, a_percent), (b_pixels, b_percent)) = (parts(a)?, parts(b)?);
            Some(Value::Length(if progress <= 0.0 {
                a
            } else if progress >= 1.0 {
                b
            } else {
                Dim::Mix(along(a_pixels, b_pixels), along(a_percent, b_percent))
            }))
        }
        // CSS Transitions: shown all the way between, when either end is shown.
        (Value::Visible(a), Value::Visible(b)) => Some(Value::Visible(if progress <= 0.0 {
            a
        } else if progress >= 1.0 {
            b
        } else {
            a || b
        })),
        _ => None,
    }
}

/// What in a descendant, or the element itself, takes its color from an animated `color`;
/// for `visibility`, only `node` counts.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Follower {
    node: usize,
    borders: [bool; 4],
    outline: bool,
    marker: bool,
}

/// One presented value this frame. It holds only while the tree still computes `target`: any
/// other value is a newer style, and an unrequested style change applies at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum InlineSide {
    Top,
    Bottom,
    Start,
    End,
}

/// Everything an animated `color` or `visibility` carries to, as of the request.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Followers {
    boxes: Vec<Follower>,
    /// Text runs by arena slot, place and owner, each showing the color at both ends. An
    /// interaction styling can shift slots, so the owner is checked again when presenting.
    runs: Vec<(usize, usize, usize)>,
    /// Inline box sides showing the color at both ends, by box and side.
    inline_sides: BTreeSet<(usize, InlineSide)>,
    /// For `visibility`: runs, by owner, text and innermost box, and block generated boxes,
    /// by owner and order, that keep a visibility of their own: a `:before` or `:after`
    /// that sets one.
    /// By content, not slot, so an interaction styling cannot shift them; another run of
    /// the same owner and text keeps its own too.
    fixed_runs: BTreeSet<RunKey>,
    fixed_boxes: BTreeSet<(usize, usize)>,
    /// For `font-size`: the text runs, by `run_key` and target size, and the descendants,
    /// by source node with their target size, that keep their size's share of the
    /// element's at both ends. Only what still has its target size scales.
    sized_runs: BTreeSet<(RunKey, u32)>,
    sized_nodes: BTreeMap<usize, f32>,
}

#[derive(Debug, Clone)]
pub(super) struct Presented {
    id: String,
    field: Field,
    target: Value,
    now: Value,
    followers: Followers,
}

impl BoxTree {
    /// The first element with this CSS id, by projected index.
    fn element(&self, id: &str) -> Option<usize> {
        let nodes = self.source.construction.nodes();
        self.originals
            .iter()
            .position(|node| nodes[*node].metadata().css_id() == Some(id))
    }

    /// An inline element: no box of its own, its slot the anonymous block of its line.
    fn under_text(&self, projected: usize) -> bool {
        self.prepared.inline_boxes[projected].is_some()
            && self.prepared.slot[projected].is_some_and(|slot| self.prepared.anonymous[slot])
    }

    fn read(&self, projected: usize, field: Field) -> Option<Value> {
        let paint = &self.paints[projected];
        // An inline element's background lies under its text, which has no opacity: it runs
        // from one color to another, and a transparent end applies at once.
        if field == Field::Background && self.under_text(projected) {
            return paint.under_text.or(paint.background).map(Value::Color);
        }
        // Its border colors likewise paint on its line's sides.
        if let Field::BorderColor(side) = field
            && self.under_text(projected)
        {
            return paint.under_text_border[side]
                .or(paint.border[side])
                .map(Value::Color);
        }
        // Display none shows nothing to come from or go to.
        self.prepared.slot[projected]?;
        let color = |value: Option<[f32; 3]>| value.map(Value::Color);
        // An inline element's slot is its paragraph's box, which is not this element's.
        let own = self.prepared.slot[projected].filter(|slot| !self.prepared.anonymous[*slot]);
        let boxed =
            || own.and_then(|slot| self.prepared.boxes.get(slot).map(|value| (slot, value)));
        let length = |value: Dim| Some(Value::Length(value));
        match field {
            Field::Visibility => Some(Value::Visible(paint.visible)),
            Field::FontSize => Some(Value::Length(Dim::Px(self.prepared.font_sizes[projected]))),
            Field::Color => color(paint.color),
            Field::Background => own.map(|_| match (paint.background, paint.background_fade) {
                (None, _) => Value::Fade([0.0; 3], 0.0),
                (Some(value), None) => Value::Color(value),
                (Some(value), Some(alpha)) => Value::Fade(value, alpha),
            }),
            Field::BorderColor(side) => color(own.and(paint.border[side])),
            Field::OutlineColor => color(own.and(paint.outline).map(|outline| outline.1)),
            Field::OutlineWidth => own
                .and(paint.outline)
                .and_then(|outline| length(Dim::Px(outline.0))),
            Field::Width => length(boxed()?.1.width),
            Field::MinWidth => length(boxed()?.1.min_width),
            Field::MaxWidth => length(boxed()?.1.max_width),
            Field::Height => length(boxed()?.1.height),
            Field::MinHeight => length(boxed()?.1.min_height),
            Field::MaxHeight => length(boxed()?.1.max_height),
            Field::Margin(side) => length(boxed()?.1.margin[side]),
            Field::Padding(side) => length(boxed()?.1.padding[side]),
            Field::BorderWidth(side) => length(Dim::Px(boxed()?.1.border[side])),
            Field::Offset(side) => {
                let (slot, _) = boxed()?;
                let offsets = self.prepared.offsets.iter().find(|entry| entry.0 == slot)?;
                length(offsets.1[side])
            }
        }
    }

    fn write(&mut self, projected: usize, field: Field, value: Value) {
        let slot = self.prepared.slot[projected];
        let under_text = self.under_text(projected);
        let paint = &mut self.paints[projected];
        match (field, value) {
            (Field::Background, Value::Color(value)) if under_text => {
                if paint.under_text.is_some() {
                    paint.under_text = Some(value);
                } else {
                    paint.background = Some(value);
                }
            }
            (Field::BorderColor(side), Value::Color(value)) if under_text => {
                if paint.under_text_border[side].is_some() {
                    paint.under_text_border[side] = Some(value);
                } else {
                    paint.border[side] = Some(value);
                }
            }
            (Field::Color, Value::Color(value)) => paint.color = Some(value),
            // `reveal` shows or hides the element with what follows it.
            (Field::Background, Value::Color(value)) => {
                paint.background = Some(value);
                paint.background_fade = None;
            }
            (Field::Background, Value::Fade(value, alpha)) => {
                paint.background = Some(value);
                paint.background_fade = Some(alpha);
            }
            (Field::BorderColor(side), Value::Color(value)) => paint.border[side] = Some(value),
            (Field::OutlineColor, Value::Color(value)) => {
                if let Some(outline) = &mut paint.outline {
                    outline.1 = value;
                }
            }
            (Field::OutlineWidth, Value::Length(Dim::Px(value))) => {
                if let Some(outline) = &mut paint.outline {
                    outline.0 = value;
                }
            }
            (_, Value::Length(value)) => {
                let Some(slot) = slot else { return };
                if let Field::Offset(side) = field {
                    if let Some(entry) = self.prepared.offsets.iter_mut().find(|e| e.0 == slot) {
                        entry.1[side] = value;
                    }
                    return;
                }
                let target = &mut self.prepared.boxes[slot];
                match field {
                    Field::Width => target.width = value,
                    Field::MinWidth => target.min_width = value,
                    Field::MaxWidth => target.max_width = value,
                    Field::Height => target.height = value,
                    Field::MinHeight => target.min_height = value,
                    Field::MaxHeight => target.max_height = value,
                    Field::Margin(side) => target.margin[side] = value,
                    Field::Padding(side) => target.padding[side] = value,
                    Field::BorderWidth(side) => {
                        if let Dim::Px(value) = value {
                            target.border[side] = value;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Whether `node` is `ancestor` or lies inside it.
    pub(super) fn within(&self, mut node: usize, ancestor: usize) -> bool {
        let nodes = self.source.construction.nodes();
        loop {
            if node == ancestor {
                return true;
            }
            match nodes.get(node).and_then(|value| value.parent()) {
                Some(parent) => node = parent,
                None => return false,
            }
        }
    }

    /// Lays the presented values over this styling.
    pub(super) fn present(&mut self) {
        let Some(presentation) = self.presentation.clone() else {
            return;
        };
        // Sizes last: scaling rewrites side spaces, which other followers know by text.
        let (sizes, rest): (Vec<_>, Vec<_>) = presentation
            .iter()
            .partition(|presented| presented.field == Field::FontSize);
        for presented in rest.into_iter().chain(sizes) {
            let Some(projected) = self.element(&presented.id) else {
                continue;
            };
            if self.read(projected, presented.field) != Some(presented.target) {
                continue;
            }
            self.write(projected, presented.field, presented.now);
            if let (Field::Color, Value::Color(target), Value::Color(now)) =
                (presented.field, presented.target, presented.now)
            {
                self.follow(&presented.followers, target, now);
            }
            if let (Field::FontSize, Value::Length(Dim::Px(target)), Value::Length(Dim::Px(now))) =
                (presented.field, presented.target, presented.now)
                && target > 0.0
            {
                self.resize(
                    &presented.followers,
                    self.originals[projected],
                    now / target,
                );
            }
            if let (Field::Visibility, Value::Visible(target), Value::Visible(now)) =
                (presented.field, presented.target, presented.now)
            {
                self.reveal(&presented.followers, self.originals[projected], target, now);
            }
            if let (Field::Background, Value::Color(_), Value::Color(now)) =
                (presented.field, presented.target, presented.now)
                && self.under_text(projected)
            {
                self.inline_background(projected, now);
            }
            if let (Field::BorderColor(side), Value::Color(_), Value::Color(now)) =
                (presented.field, presented.target, presented.now)
                && self.under_text(projected)
            {
                self.inline_border(projected, side, now);
            }
        }
    }

    /// Shows or hides, with an animated `visibility`, the element and what follows it at
    /// both ends (`followers`, by source node): their boxes, outside markers, block generated
    /// boxes and text runs, found by owner whatever an interaction styling moved. Only
    /// what still shows `target` changes. Their inline boxes' sides and backgrounds show
    /// or hide with them.
    fn reveal(&mut self, followers: &Followers, root: usize, target: bool, now: bool) {
        let mut nodes: BTreeSet<usize> = followers.boxes.iter().map(|f| f.node).collect();
        nodes.insert(root);
        let boxes: BTreeSet<usize> = self
            .originals
            .iter()
            .enumerate()
            .filter(|(_, node)| nodes.contains(node))
            .map(|(projected, _)| projected)
            .collect();
        for &projected in &boxes {
            if self.paints[projected].visible == target {
                self.paints[projected].visible = now;
            }
            if let Some(marker) = self.markers.get_mut(&projected) {
                for run in marker.runs.iter_mut().filter(|run| run.visible == target) {
                    run.visible = now;
                }
            }
        }
        let positions = &self.prepared.positions;
        let originals = &self.originals;
        let mut order: BTreeMap<usize, usize> = BTreeMap::new();
        for (arena, paint) in &mut self.prepared.generated_boxes {
            let owner = positions[*arena].0;
            let place = order.entry(owner).or_default();
            let fixed = followers.fixed_boxes.contains(&(originals[owner], *place));
            *place += 1;
            if boxes.contains(&owner) && !fixed && paint.visible == target {
                paint.visible = now;
            }
        }
        for run in self
            .texts
            .iter_mut()
            .flatten()
            .flat_map(|leaf| &mut leaf.runs)
        {
            let follows = nodes.contains(&run.owner)
                && run.visible == target
                && (followers.fixed_runs.is_empty()
                    || !followers.fixed_runs.contains(&run_key(run)));
            if follows {
                run.visible = now;
                // Its generated box goes with it.
                for sides in run
                    .inline_boxes
                    .iter_mut()
                    .filter(|sides| super::is_generated(sides.owner))
                {
                    sides.visible = now;
                }
            }
            // The boxes that follow, wherever their sides and backgrounds fall.
            for sides in run
                .inline_boxes
                .iter_mut()
                .filter(|sides| nodes.contains(&sides.owner))
            {
                sides.visible = now;
            }
            // Its background: the nearest that shows now, if any box it lies in moves.
            let moves = |key: usize| nodes.contains(&key) || (follows && super::is_generated(key));
            if run.backgrounds.iter().any(|entry| moves(entry.1)) {
                let shown = run
                    .backgrounds
                    .iter()
                    .find(|(_, key, visible)| if moves(*key) { now } else { *visible })
                    // A side space leaves its own box's background to the box.
                    .filter(|(_, key, _)| {
                        !(run.spacer
                            && run.inline_boxes.last().is_some_and(|own| own.owner == *key))
                    });
                run.background = shown.map(|entry| entry.0);
                run.background_from = shown
                    .map(|entry| entry.1)
                    .filter(|key| !super::is_generated(*key));
            }
        }
    }

    /// Scales, with an animated `font-size`, the element and what keeps its size's share of
    /// it (`followers`) and still has its target size: computed sizes, outside markers, text
    /// runs, and the spaces made from them for sides and indents, which keep their room.
    /// Lengths in `em` keep their target's.
    fn resize(&mut self, followers: &Followers, root: usize, factor: f32) {
        for (projected, node) in self.originals.iter().enumerate() {
            let target = followers.sized_nodes.get(node);
            if *node != root && target != Some(&self.prepared.font_sizes[projected]) {
                continue;
            }
            self.prepared.font_sizes[projected] *= factor;
            if let Some(marker) = self.markers.get_mut(&projected) {
                for run in &mut marker.runs {
                    run.size *= factor;
                    run.line_size = run.line_size.map(|size| size * factor);
                }
            }
        }
        let owners: BTreeSet<usize> = followers.sized_runs.iter().map(|(key, _)| key.0).collect();
        for leaf in self.texts.iter_mut().flatten() {
            let scaled: Vec<bool> = leaf
                .runs
                .iter()
                .map(|run| {
                    !run.spacer
                        && owners.contains(&run.owner)
                        && followers
                            .sized_runs
                            .contains(&(run_key(run), run.size.to_bits()))
                })
                .collect();
            // Spaces for sides and indents are made from the text they open or close, at
            // its size: they scale with it, the nearest text after them, or else before.
            let follows_text = |at: usize| {
                let size = leaf.runs[at].size;
                let text = |other: &usize| !leaf.runs[*other].spacer;
                let after = (at + 1..leaf.runs.len()).find(text);
                let before = (0..at).rev().find(text);
                [after, before]
                    .into_iter()
                    .flatten()
                    .any(|other| scaled[other] && leaf.runs[other].size == size)
            };
            let spaces: Vec<bool> = (0..leaf.runs.len())
                .map(|at| leaf.runs[at].spacer && follows_text(at))
                .collect();
            for (at, run) in leaf.runs.iter_mut().enumerate() {
                if !(scaled[at] || spaces[at]) {
                    continue;
                }
                run.size *= factor;
                run.line_size = run.line_size.map(|size| size * factor);
                // Their room stays: spaces again at the new size, as layout makes them.
                if spaces[at]
                    && run.percent_room.is_some_and(|room| room.1 == 0.0)
                    && let Some(text) = super::room_text(run, 0.0)
                {
                    run.text = text;
                }
            }
        }
    }

    /// What follows an animated `font-size` from `committed` to `target` pixels: the
    /// descendants of `root` (by projected index in `to`) and the text runs inside it whose
    /// size is the same share of the element's at both ends, as inherited, `em` and `%` sizes
    /// are. A size of its own in pixels applies at once.
    fn size_followers(&self, to: &Self, root: usize, committed: f32, target: f32) -> Followers {
        let root = to.originals[root];
        let mut followers = Followers::default();
        if committed <= 0.0 || target <= 0.0 {
            return followers;
        }
        let same = |from: f32, to: f32| (from / committed - to / target).abs() < 1e-3;
        let before: BTreeMap<usize, usize> = self
            .originals
            .iter()
            .enumerate()
            .map(|(projected, node)| (*node, projected))
            .collect();
        for (projected, &node) in to.originals.iter().enumerate() {
            if node == root || !to.within(node, root) {
                continue;
            }
            let size = to.prepared.font_sizes[projected];
            if before
                .get(&node)
                .is_some_and(|at| same(self.prepared.font_sizes[*at], size))
            {
                followers.sized_nodes.insert(node, size);
            }
        }
        for (arena, leaf) in to.texts.iter().enumerate() {
            let Some(leaf) = leaf else { continue };
            for (at, run) in leaf.runs.iter().enumerate() {
                if run.spacer || !to.within(run.owner, root) {
                    continue;
                }
                let alike =
                    |other: &&super::TextRun| other.owner == run.owner && other.text == run.text;
                let occurrence = leaf.runs[..at].iter().filter(alike).count();
                if self
                    .texts
                    .get(arena)
                    .and_then(Option::as_ref)
                    .and_then(|old| old.runs.iter().filter(alike).nth(occurrence))
                    .is_some_and(|old| same(old.size, run.size))
                {
                    followers
                        .sized_runs
                        .insert((run_key(run), run.size.to_bits()));
                }
            }
        }
        followers
    }

    /// The descendants of `root` (by projected index in `to`) that follow its visibility at
    /// both ends: shown as it is in this styling, and as it is in `to`. One that changes on
    /// its own applies at once.
    fn visibility_followers(
        &self,
        to: &Self,
        root: usize,
        committed: bool,
        target: bool,
    ) -> Followers {
        let root = to.originals[root];
        let before: BTreeMap<usize, usize> = self
            .originals
            .iter()
            .enumerate()
            .map(|(projected, node)| (*node, projected))
            .collect();
        let mut followers = Followers::default();
        for (projected, &node) in to.originals.iter().enumerate() {
            if node == root || to.paints[projected].visible != target || !to.within(node, root) {
                continue;
            }
            if before
                .get(&node)
                .is_some_and(|at| self.paints[*at].visible == committed)
            {
                followers.boxes.push(Follower {
                    node,
                    borders: [false; 4],
                    outline: false,
                    marker: false,
                });
            }
        }
        let mut nodes: BTreeSet<usize> = followers.boxes.iter().map(|f| f.node).collect();
        nodes.insert(root);
        // A run shown as `target` that did not show as `committed` keeps its own.
        for (arena, leaf) in to.texts.iter().enumerate() {
            let Some(leaf) = leaf else { continue };
            for (at, run) in leaf.runs.iter().enumerate() {
                if !nodes.contains(&run.owner) || run.visible != target {
                    continue;
                }
                // Side spaces by their box: a font size that changes too sizes them anew.
                let innermost = |run: &super::TextRun| run.inline_boxes.last().map(|d| d.owner);
                let alike = |other: &&super::TextRun| {
                    other.owner == run.owner
                        && if run.spacer {
                            other.spacer && innermost(other) == innermost(run)
                        } else {
                            other.text == run.text
                        }
                };
                let occurrence = leaf.runs[..at].iter().filter(alike).count();
                if self
                    .texts
                    .get(arena)
                    .and_then(Option::as_ref)
                    .and_then(|old| old.runs.iter().filter(alike).nth(occurrence))
                    .is_some_and(|old| old.visible != committed)
                {
                    followers.fixed_runs.insert(run_key(run));
                }
            }
        }
        // Block generated boxes likewise, by owner and order.
        let boxes_of = |tree: &Self| {
            let mut found: BTreeMap<usize, Vec<bool>> = BTreeMap::new();
            for (arena, paint) in &tree.prepared.generated_boxes {
                let node = tree.originals[tree.prepared.positions[*arena].0];
                found.entry(node).or_default().push(paint.visible);
            }
            found
        };
        let (old, new) = (boxes_of(self), boxes_of(to));
        for (node, shown) in &new {
            for (place, visible) in shown.iter().enumerate() {
                let kept = old.get(node).and_then(|old| old.get(place));
                if nodes.contains(node)
                    && *visible == target
                    && kept.is_some_and(|v| *v != committed)
                {
                    followers.fixed_boxes.insert((*node, place));
                }
            }
        }
        followers
    }

    /// Carries an inline element's presented background to where it paints: the runs that
    /// show it, its own and its inline descendants' without one of their own, and its own
    /// inline box sides. Runs and sides name their source node.
    fn inline_background(&mut self, projected: usize, now: [f32; 3]) {
        let root = self.originals[projected];
        for leaf in self.texts.iter_mut().flatten() {
            for run in &mut leaf.runs {
                if run.background_from == Some(root) {
                    run.background = Some(now);
                }
                for sides in &mut run.inline_boxes {
                    if sides.owner == root && sides.background.is_some() {
                        sides.background = Some(now);
                    }
                }
            }
        }
    }

    /// Carries an inline element's presented border color to its line's sides: the top and
    /// bottom bands of its own inline box, and the left or right bar in its runs.
    fn inline_border(&mut self, projected: usize, side: usize, now: [f32; 3]) {
        let root = self.originals[projected];
        for leaf in self.texts.iter_mut().flatten() {
            for run in &mut leaf.runs {
                for sides in &mut run.inline_boxes {
                    if sides.owner != root {
                        continue;
                    }
                    let band = match side {
                        0 => &mut sides.border[0].1,
                        2 => &mut sides.border[1].1,
                        _ => continue,
                    };
                    if band.is_some() {
                        *band = Some(now);
                    }
                }
                if edge_owner(run) == root
                    && let Some((left, _, color)) = &mut run.edge
                    && (side == 3) == *left
                    && matches!(side, 1 | 3)
                {
                    *color = now;
                }
            }
        }
    }

    /// Carries a presented `color` to what inherits it or uses it: 16.1 `color` inherits, and
    /// a border or outline with no color of its own takes it.
    fn follow(&mut self, followers: &Followers, target: [f32; 3], now: [f32; 3]) {
        let swap = |value: &mut Option<[f32; 3]>| {
            if *value == Some(target) {
                *value = Some(now);
            }
        };
        for follower in &followers.boxes {
            let Some(projected) = self.originals.iter().position(|n| *n == follower.node) else {
                continue;
            };
            let paint = &mut self.paints[projected];
            swap(&mut paint.color);
            for (side, follows) in follower.borders.iter().enumerate() {
                if *follows {
                    swap(&mut paint.border[side]);
                }
            }
            if follower.outline
                && let Some(outline) = &mut paint.outline
                && outline.1 == target
            {
                outline.1 = now;
            }
            if follower.marker
                && let Some(marker) = self.markers.get_mut(&projected)
            {
                for run in &mut marker.runs {
                    swap(&mut run.color);
                }
            }
        }
        // An inline box's sides wherever they fall, whatever text color their runs have.
        let follows =
            |node: usize, side: InlineSide| followers.inline_sides.contains(&(node, side));
        for leaf in self.texts.iter_mut().flatten() {
            for run in &mut leaf.runs {
                let owner = edge_owner(run);
                for sides in &mut run.inline_boxes {
                    if follows(sides.owner, InlineSide::Top) {
                        swap(&mut sides.border[0].1);
                    }
                    if follows(sides.owner, InlineSide::Bottom) {
                        swap(&mut sides.border[1].1);
                    }
                }
                if let Some((start, _, edge)) = &mut run.edge {
                    let side = if *start {
                        InlineSide::Start
                    } else {
                        InlineSide::End
                    };
                    if follows(owner, side) && *edge == target {
                        *edge = now;
                    }
                }
            }
        }
        for &(arena, at, owner) in &followers.runs {
            if let Some(run) = self
                .texts
                .get_mut(arena)
                .and_then(Option::as_mut)
                .and_then(|leaf| leaf.runs.get_mut(at))
                .filter(|run| run.owner == owner)
            {
                swap(&mut run.color);
            }
        }
    }

    /// The target and its descendants that show the target's color in both stylings.
    fn followers(&self, to: &Self, target: usize, from_color: Value, to_color: Value) -> Followers {
        let (Value::Color(from_color), Value::Color(to_color)) = (from_color, to_color) else {
            return Followers::default();
        };
        let same = |old: Option<[f32; 3]>, new: Option<[f32; 3]>| {
            old == Some(from_color) && new == Some(to_color)
        };
        let root = to.originals[target];
        let mut followers = Followers::default();
        for (projected, node) in to.originals.iter().enumerate() {
            let Some(before) = self.originals.iter().position(|n| n == node) else {
                continue;
            };
            if !to.within(*node, root) {
                continue;
            }
            let (old, new) = (&self.paints[before], &to.paints[projected]);
            if *node != root && !same(old.color, new.color) {
                continue;
            }
            let color_of = |tree: &Self, at: usize| {
                tree.markers
                    .get(&at)
                    .and_then(|marker| marker.runs.first())
                    .and_then(|run| run.color)
            };
            followers.boxes.push(Follower {
                node: *node,
                borders: std::array::from_fn(|side| same(old.border[side], new.border[side])),
                outline: same(old.outline.map(|o| o.1), new.outline.map(|o| o.1)),
                marker: same(color_of(self, before), color_of(to, projected)),
            });
        }
        for (arena, leaf) in to.texts.iter().enumerate() {
            let Some(leaf) = leaf else { continue };
            for (at, run) in leaf.runs.iter().enumerate() {
                // By content, not place: spaces for sides can differ between the two stylings.
                // The same occurrence: `:before` and `:after` can share owner and text.
                let alike =
                    |other: &&super::TextRun| other.owner == run.owner && other.text == run.text;
                let occurrence = leaf.runs[..at].iter().filter(alike).count();
                let shown_before = self
                    .texts
                    .get(arena)
                    .and_then(Option::as_ref)
                    .and_then(|old| old.runs.iter().filter(alike).nth(occurrence))
                    .is_some_and(|old| same(old.color, run.color));
                if followers.boxes.iter().any(|f| f.node == run.owner) && shown_before {
                    followers.runs.push((arena, at, run.owner));
                }
            }
        }
        let old_sides = self.inline_side_colors();
        for (key, color) in to.inline_side_colors() {
            if to.within(key.0, root) && same(old_sides.get(&key).copied(), Some(color)) {
                followers.inline_sides.insert(key);
            }
        }
        followers
    }

    fn inline_side_colors(&self) -> BTreeMap<(usize, InlineSide), [f32; 3]> {
        let mut found = BTreeMap::new();
        for run in self.texts.iter().flatten().flat_map(|leaf| &leaf.runs) {
            // A generated box's sides key past `usize::MAX / 2`, inside no element.
            for sides in &run.inline_boxes {
                if let (width, Some(color)) = sides.border[0]
                    && width > 0.0
                {
                    found.insert((sides.owner, InlineSide::Top), color);
                }
                if let (width, Some(color)) = sides.border[1]
                    && width > 0.0
                {
                    found.insert((sides.owner, InlineSide::Bottom), color);
                }
            }
            if let Some((start, _, color)) = run.edge {
                let side = if start {
                    InlineSide::Start
                } else {
                    InlineSide::End
                };
                found.insert((edge_owner(run), side), color);
            }
        }
        found
    }
}

/// A run's owner, text and innermost box (`run_key`).
type RunKey = (usize, String, Option<usize>);

/// A run told apart by content, not slot: its owner, text, and innermost box, which keeps
/// a box's side spaces apart from its `:before`'s.
fn run_key(run: &super::TextRun) -> RunKey {
    (
        run.owner,
        run.text.clone(),
        run.inline_boxes.last().map(|sides| sides.owner),
    )
}

/// The box a run's side bar belongs to: the innermost box it wears, which a side spacer's
/// own box is; a generated box's is its `generated_owner`.
fn edge_owner(run: &super::TextRun) -> usize {
    run.inline_boxes
        .last()
        .map_or(run.owner, |sides| sides.owner)
}

/// When an animation runs and how it moves. Plain `AnimationTiming` is linear.
#[derive(Debug, Clone, Copy)]
pub struct Transition {
    pub timing: AnimationTiming,
    /// Stays within its two ends: an easing that overshoots is cut off there, since not
    /// every property has a meaning past them.
    pub easing: Easing,
}

impl From<AnimationTiming> for Transition {
    fn from(timing: AnimationTiming) -> Self {
        Self {
            timing,
            easing: Easing::Linear,
        }
    }
}

#[derive(Debug)]
struct Track {
    id: String,
    property: PropertyId,
    field: Field,
    start: Value,
    target: Value,
    followers: Followers,
    /// The capture it belongs to: another tree with this id is another element.
    capture: u64,
    easing: Easing,
}

/// Running explicit animations. It lives in the application state, beside the tree, and
/// survives view rebuilds. A request is consumed once: calling it again restarts it.
///
/// ```no_run
/// # use iced_css::box_tree::{BoxTree, BoxTreeAnimations};
/// # use iced_css::animation::clock::AnimationTiming;
/// # use std::time::{Duration, Instant};
/// # fn demo(tree: BoxTree) -> Result<(), Box<dyn std::error::Error>> {
/// let mut animations = BoxTreeAnimations::new();
/// let open = tree.restyled(&["#panel { width:300px; }"])?;
/// let timing = AnimationTiming::new(Duration::from_millis(200), Duration::ZERO);
/// animations.request(&tree, &open, "panel", &["width"], timing, Instant::now())?;
/// // Keep `open` as the tree. Each frame: `animations.advance(now, &open)`, and in the view
/// // `animations.presented(&open).view()`. `animations.frames(Message::Frame)` subscribes.
/// # Ok(())
/// # }
/// ```
pub struct BoxTreeAnimations {
    engine: ProgressEngine,
    sources: BTreeMap<String, SourceRef>,
    tracks: Vec<Track>,
}

impl Default for BoxTreeAnimations {
    fn default() -> Self {
        Self::new()
    }
}

impl BoxTreeAnimations {
    #[must_use]
    pub fn new() -> Self {
        let big = NonZeroUsize::new(100_000).expect("positive");
        let most = NonZeroU64::new(u64::MAX).expect("positive");
        Self {
            engine: ProgressEngine::new(
                SourceRegistry::new(most),
                ProgressLimits {
                    addresses: most,
                    bindings: big,
                    tracks: big,
                    retained_bytes: NonZeroUsize::new(64_000_000).expect("positive"),
                    work: NonZeroUsize::new(100_000_000).expect("positive"),
                },
            ),
            sources: BTreeMap::new(),
            tracks: Vec::new(),
        }
    }

    /// Animates `properties` (longhands or shorthands) of the element with CSS id `id`, from
    /// what `from` presents now toward what `to` computes. Keep `to` as your tree. A newer
    /// request starts from the current presented value.
    ///
    /// Returns the animations started. Nothing starts for a property that did not change, is
    /// not animatable or not yet supported here, is won by `!important` in `to`, or has no
    /// in-between (`auto`, a transparent border): those take `to`'s value at once.
    ///
    /// # Errors
    /// Returns an unknown property name or a refused timing. Nothing changes then.
    pub fn request(
        &mut self,
        from: &BoxTree,
        to: &BoxTree,
        id: &str,
        properties: &[&str],
        transition: impl Into<Transition>,
        now: Instant,
    ) -> Result<usize, BoxTreeError> {
        let Transition { timing, easing } = transition.into();
        let mut longhands = Vec::new();
        for name in properties {
            let property = PropertyId::lookup(name)
                .ok_or_else(|| BoxTreeError(format!("unknown property {name}")))?;
            longhands.extend_from_slice(property.longhands());
        }
        let mut seen = Vec::new();
        longhands.retain(|longhand| {
            let new = !seen.contains(longhand);
            seen.push(*longhand);
            new
        });
        self.engine.advance(now);
        let capture = to.source.capture;
        if from.source.capture != capture {
            return Ok(0);
        }
        let (Some(before), Some(after)) = (from.element(id), to.element(id)) else {
            return Ok(0);
        };
        let mut started = Vec::new();
        let mut dropped = Vec::new();
        for property in longhands {
            let Some(field) = field(property.descriptor().name) else {
                continue;
            };
            if !matches!(
                property.descriptor().animation,
                Some(AnimationRule::ByComputedValue | AnimationRule::Visibility)
            ) {
                continue;
            }
            let (Some(committed), Some(target)) = (from.read(before, field), to.read(after, field))
            else {
                dropped.push(property);
                continue;
            };
            if committed == target {
                continue;
            }
            // Under text there is no opacity: a fade between a box and an inline has no
            // in-between there.
            if field == Field::Background
                && (from.under_text(before) || to.under_text(after))
                && !matches!((committed, target), (Value::Color(_), Value::Color(_)))
            {
                dropped.push(property);
                continue;
            }
            // Section 13: an important target holds without a temporary override.
            if mix(committed, target, 0.0).is_none() || to.important[after].contains(&property) {
                dropped.push(property);
                continue;
            }
            let start = self
                .current(id, property)
                .filter(|(track, _)| track.target == committed && track.capture == capture)
                .map_or(committed, |(_, value)| value);
            // A box still fading when it becomes an inline starts from a fade, too.
            if field == Field::Background
                && to.under_text(after)
                && !matches!(start, Value::Color(_))
            {
                dropped.push(property);
                continue;
            }
            started.push(Track {
                id: id.to_owned(),
                property,
                field,
                start,
                target,
                followers: match field {
                    Field::Color => from.followers(to, after, committed, target),
                    Field::FontSize => match (committed, target) {
                        (Value::Length(Dim::Px(committed)), Value::Length(Dim::Px(target))) => {
                            from.size_followers(to, after, committed, target)
                        }
                        _ => Followers::default(),
                    },
                    Field::Visibility => match (committed, target) {
                        (Value::Visible(committed), Value::Visible(target)) => {
                            from.visibility_followers(to, after, committed, target)
                        }
                        _ => Followers::default(),
                    },
                    _ => Followers::default(),
                },
                capture,
                easing,
            });
        }
        if started.is_empty() && !self.sources.contains_key(id) {
            return Ok(0);
        }
        if !self.sources.contains_key(id) {
            let source = self.engine.register_source().map_err(failed)?;
            self.sources.insert(id.to_owned(), source);
        }
        let source = &self.sources[id];
        let work = NonZeroUsize::new(100_000).expect("positive");
        if !started.is_empty() {
            let chosen: Vec<PropertyId> = started.iter().map(|track| track.property).collect();
            let selection = AnimationSelection::new(&chosen, work).map_err(failed)?;
            self.engine
                .start_progress(source, &selection, timing, now)
                .map_err(failed)?;
        }
        if !dropped.is_empty() {
            let selection = AnimationSelection::new(&dropped, work).map_err(failed)?;
            self.engine.cancel(source, &selection).map_err(failed)?;
        }
        self.tracks.retain(|track| {
            track.id != id
                || !(dropped.contains(&track.property)
                    || started.iter().any(|new| new.property == track.property))
        });
        let count = started.len();
        self.tracks.append(&mut started);
        Ok(count)
    }

    /// A track and its value at the engine's current time, unless it has finished.
    fn current(&self, id: &str, property: PropertyId) -> Option<(&Track, Value)> {
        let track = self
            .tracks
            .iter()
            .find(|track| track.id == id && track.property == property)?;
        let progress = self.progress(track)?;
        Some((track, mix(track.start, track.target, progress)?))
    }

    fn progress(&self, track: &Track) -> Option<f32> {
        let source = self.sources.get(&track.id)?;
        let linear = self.engine.sample(source, track.property).ok().flatten()?;
        // `clamp` keeps NaN, which a custom easing could return: that shows the start.
        let eased = track.easing.value(linear);
        Some(if eased.is_nan() {
            0.0
        } else {
            eased.clamp(0.0, 1.0)
        })
    }

    /// Moves time on, and lets go of finished animations and of any `tree`, your current
    /// committed one, no longer heads for: an unrequested style change cancels them.
    pub fn advance(&mut self, now: Instant, tree: &BoxTree) {
        self.engine.advance(now);
        let (running, done): (Vec<Track>, Vec<Track>) = std::mem::take(&mut self.tracks)
            .into_iter()
            .partition(|track| {
                self.progress(track).is_some()
                    && track.capture == tree.source.capture
                    && tree
                        .element(&track.id)
                        .and_then(|element| tree.read(element, track.field))
                        == Some(track.target)
            });
        self.tracks = running;
        let work = NonZeroUsize::new(100_000).expect("positive");
        for track in done {
            if let (Some(source), Ok(selection)) = (
                self.sources.get(&track.id),
                AnimationSelection::new(&[track.property], work),
            ) {
                // Its source lives here and the selection is its own; nothing to report.
                let _ = self.engine.cancel(source, &selection);
            }
            if !self.tracks.iter().any(|other| other.id == track.id)
                && let Some(source) = self.sources.remove(&track.id)
            {
                // A source with no track holds nothing; a later request registers anew.
                let _ = self.engine.remove_source(&source);
            }
        }
        self.engine.accepted_build();
    }

    /// Whether a track is running; frames keep coming until it ends or `advance` sees that
    /// the tree moved on from its target.
    #[must_use]
    pub fn is_animating(&self) -> bool {
        !self.tracks.is_empty()
    }

    /// `tree` with the current presented values over it, for drawing.
    #[must_use]
    pub fn presented(&self, tree: &BoxTree) -> BoxTree {
        let mut presented = tree.clone();
        let list: Vec<Presented> = self
            .tracks
            .iter()
            .filter(|track| track.capture == tree.source.capture)
            .filter_map(|track| {
                let now = mix(track.start, track.target, self.progress(track)?)?;
                Some(Presented {
                    id: track.id.clone(),
                    field: track.field,
                    target: track.target,
                    now,
                    followers: track.followers.clone(),
                })
            })
            .collect();
        if !list.is_empty() {
            presented.presentation = Some(Arc::new(list));
            presented.present();
        }
        presented
    }

    /// A frame clock while anything animates, and nothing otherwise.
    ///
    /// `message` must capture nothing, as `Subscription::map` requires: a function or an
    /// enum variant, not a closure holding state or a function pointer.
    pub fn frames<Message: 'static, F>(&self, message: F) -> iced::Subscription<Message>
    where
        F: Fn(Instant) -> Message + Send + Clone + 'static,
    {
        if self.is_animating() {
            iced::window::frames().map(message)
        } else {
            iced::Subscription::none()
        }
    }
}
