//! Explicit animation requests over restyled trees (Q039, section 13). The stylesheet stays
//! the only store of permanent values: `to` is what the cascade computed, and this keeps only
//! the temporary presentation on the way there. Style changes without a request stay
//! immediate. Luminate owns the time, through the progress engine; colors mix per component
//! in gamma-encoded sRGB (Q070).
use super::{BoxTree, BoxTreeError, failed};
use crate::animation::{
    clock::AnimationTiming,
    easing::CssEasing,
    engine::{ProgressEngine, ProgressLimits},
};
use crate::evaluation::tree_layout::{AnimationDirection, ElementAnimation, RunSource};
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
    time::{Duration, Instant},
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
    /// `font-weight`, 100 to 900.
    Weight(f32),
}

fn rgba(value: Value) -> Option<([f32; 3], f32)> {
    match value {
        Value::Color(color) => Some((color, 1.0)),
        Value::Fade(color, alpha) => Some((color, alpha)),
        Value::Length(_) | Value::Visible(_) | Value::Weight(_) => None,
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
    FontWeight,
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
        "font-weight" => Field::FontWeight,
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
        // In steps of 100, as CSS transitions take it, and as iced has its nine weights.
        (Value::Weight(a), Value::Weight(b)) => {
            Some(Value::Weight((along(a, b) / 100.0).round() * 100.0))
        }
        (Value::Length(Dim::Percent(a)), Value::Length(Dim::Percent(b))) => {
            Some(Value::Length(Dim::Percent(along(a, b))))
        }
        // A length and a percentage mix as `calc()` does, for layout to resolve.
        (Value::Length(a), Value::Length(b)) => {
            let parts = |dim: Dim| match dim {
                Dim::Px(pixels) => Some((pixels, 0.0)),
                Dim::Percent(percent) => Some((0.0, percent)),
                // A clipped sum (a `calc()` size) mixes as its parts do.
                Dim::Mix(pixels, percent) | Dim::Clamped(pixels, percent) => {
                    Some((pixels, percent))
                }
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
    /// Lengths of the element and those descendants that are a share of their font size
    /// at both ends, as `em` gives: by source node and field, and line heights by arena slot.
    /// Each with its target pixels: only what still has them scales.
    em_lengths: Vec<(usize, Field, f32)>,
    em_line_heights: Vec<(usize, f32)>,
    /// `text-indent` in `em`: the leaf, its block's source node and the indent's pixels.
    em_indents: Vec<(usize, usize, f32)>,
    /// For `font-weight`: text runs by arena slot, owner, text, whether spaces stand in, and
    /// occurrence of those
    /// in the leaf, so another element's runs shown by an interaction styling do not
    /// shift them.
    weighed_runs: Vec<(usize, usize, String, bool, usize)>,
}

#[derive(Debug, Clone)]
pub(super) struct Presented {
    id: String,
    property: PropertyId,
    field: Field,
    target: Value,
    now: Value,
    followers: Followers,
}

impl BoxTree {
    /// The first element with this CSS id, by projected index.
    fn element(&self, id: &str) -> Option<usize> {
        // An element with no id of its own, as a transition addresses it.
        if let Some(projected) = id.strip_prefix(UNNAMED) {
            return projected
                .parse()
                .ok()
                .filter(|projected| *projected < self.originals.len());
        }
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

    /// The element `id` names, and for `id::before` or `id::after` (or one colon) its
    /// `:before` or `:after`, by `generated_owner` key.
    fn target_of(&self, id: &str) -> Option<(usize, Option<usize>)> {
        let pseudo = [
            ("::before", 0),
            (":before", 0),
            ("::after", 1),
            (":after", 1),
        ]
        .into_iter()
        .find_map(|(suffix, side)| id.strip_suffix(suffix).map(|base| (base, side)));
        match pseudo {
            Some((base, side)) => {
                let projected = self.element(base)?;
                Some((projected, Some(super::generated_owner(projected, side))))
            }
            None => Some((self.element(id)?, None)),
        }
    }

    fn read_any(&self, projected: usize, generated: Option<usize>, field: Field) -> Option<Value> {
        match generated {
            Some(key) => self.read_generated(key, field),
            None => self.read(projected, field),
        }
    }

    /// The arena slot of a block-level `:before` or `:after`'s own box, by key.
    fn generated_block(&self, key: usize) -> Option<usize> {
        let (element, side) = ((usize::MAX - key) / 2, (usize::MAX - key) % 2);
        self.prepared.generated_boxes.iter().position(|(arena, _)| {
            self.prepared.runs[*arena].iter().any(|run| match run {
                RunSource::Before(at) => side == 0 && *at == element,
                RunSource::After(at) => side == 1 && *at == element,
                RunSource::Node(_) => false,
            })
        })
    }

    /// A `:before` or `:after`'s own color, background, border colors or visibility: on its
    /// block box's paint, or on its line, its text and inline box. On its line, transparent
    /// has no in-between: its background and border colors move from a color to a color.
    fn read_generated(&self, key: usize, field: Field) -> Option<Value> {
        let runs = || self.texts.iter().flatten().flat_map(|leaf| &leaf.runs);
        let own = || runs().find(|run| run.generated == Some(key));
        if let Some(at) = self.generated_block(key) {
            let (arena, paint) = self.prepared.generated_boxes[at];
            return match field {
                // Its text draws from its runs, not its box.
                Field::Color => own()?.color.map(Value::Color),
                Field::Visibility => Some(Value::Visible(paint.visible)),
                Field::Background => Some(match (paint.background, paint.background_fade) {
                    (None, _) => Value::Fade([0.0; 3], 0.0),
                    (Some(value), None) => Value::Color(value),
                    (Some(value), Some(alpha)) => Value::Fade(value, alpha),
                }),
                Field::BorderColor(side) => match (paint.border[side], paint.border_fade[side]) {
                    (None, _) if self.prepared.boxes[arena].border[side] > 0.0 => {
                        Some(Value::Fade([0.0; 3], 0.0))
                    }
                    (None, _) => None,
                    (Some(value), None) => Some(Value::Color(value)),
                    (Some(value), Some(alpha)) => Some(Value::Fade(value, alpha)),
                },
                // Its box's own lengths, as a box's; it takes no min or max sizes.
                Field::Width => Some(Value::Length(self.prepared.boxes[arena].width)),
                Field::Height => Some(Value::Length(self.prepared.boxes[arena].height)),
                Field::Margin(side) => Some(Value::Length(self.prepared.boxes[arena].margin[side])),
                Field::Padding(side) => {
                    Some(Value::Length(self.prepared.boxes[arena].padding[side]))
                }
                Field::BorderWidth(side) => Some(Value::Length(Dim::Px(
                    self.prepared.boxes[arena].border[side],
                ))),
                Field::FontSize => own().map(|run| Value::Length(Dim::Px(run.size))),
                _ => None,
            };
        }
        // Its sides on its line: its box's bands above and below, and its spaces across, a
        // side's border and padding in one (its bar the border's width) and its margin in
        // another. A side with none of these, or that a negative margin pulls, has no
        // in-between.
        let spaces = |side: usize, inner: bool| {
            runs().find(|run| {
                run.side_of == Some((key, side))
                    && run.percent_room.is_some_and(|room| room.2 == inner)
            })
        };
        let length = |pixels: f32, percent: f32| {
            Some(Value::Length(match (pixels, percent) {
                (_, 0.0) => Dim::Px(pixels),
                (0.0, _) => Dim::Percent(percent),
                _ => Dim::Mix(pixels, percent),
            }))
        };
        let pulled = |side: usize| {
            spaces(side, true)
                .is_some_and(|run| run.overhang > 0.0 || run.pull > 0.0 || run.pulled > 0.0)
        };
        let decoration = || {
            runs()
                .flat_map(|run| &run.inline_boxes)
                .find(|sides| sides.owner == key)
        };
        match field {
            Field::Padding(side @ (0 | 2)) => {
                return length(decoration()?.padding[side / 2], 0.0);
            }
            Field::BorderWidth(side @ (0 | 2)) => {
                return length(decoration()?.border[side / 2].0, 0.0);
            }
            Field::Padding(side) if !pulled(side) => {
                let run = spaces(side, true)?;
                let (fixed, percent, _) = run.percent_room?;
                let bar = run.edge.map_or(0.0, |edge| edge.1);
                return length(fixed - bar, percent);
            }
            Field::BorderWidth(side) if !pulled(side) => {
                return length(spaces(side, true)?.edge?.1, 0.0);
            }
            Field::Margin(side @ (1 | 3)) if !pulled(side) => {
                let (fixed, percent, _) = spaces(side, false)?.percent_room?;
                return length(fixed, percent);
            }
            Field::Margin(_) | Field::Padding(_) | Field::BorderWidth(_) => return None,
            _ => {}
        }
        match field {
            Field::Color => own()?.color.map(Value::Color),
            Field::Visibility => Some(Value::Visible(own()?.visible)),
            Field::FontSize => own().map(|run| Value::Length(Dim::Px(run.size))),
            // An inside marker piece before it carries no backgrounds: any of its runs that do.
            // Transparent is a fade at none, a layer on its line.
            Field::Background => {
                own()?;
                Some(
                    runs()
                        .filter(|run| run.generated == Some(key))
                        .find_map(|run| run.backgrounds.iter().find(|entry| entry.1 == key))
                        .map_or(Value::Fade([0.0; 3], 0.0), |entry| Value::Color(entry.0)),
                )
            }
            // Above and below, where its band has a width.
            Field::BorderColor(side @ (0 | 2)) => {
                let (width, color) = decoration()?.border[side / 2];
                match color {
                    Some(color) => Some(Value::Color(color)),
                    None if width > 0.0 => Some(Value::Fade([0.0; 3], 0.0)),
                    None => None,
                }
            }
            Field::BorderColor(side) => runs()
                .find(|run| run.side_of == Some((key, side)))
                .and_then(|run| run.edge)
                .map(|edge| Value::Color(edge.2)),
            _ => None,
        }
    }

    /// A `:before` or `:after`'s side bar across, on its line: its width.
    fn generated_bar(&self, key: usize, side: usize) -> Option<f32> {
        self.texts
            .iter()
            .flatten()
            .flat_map(|leaf| &leaf.runs)
            .find(|run| run.side_of == Some((key, side)) && run.edge.is_some())
            .and_then(|run| run.edge.map(|edge| edge.1))
    }

    /// Lays a presented value on a `:before` or `:after` (`read_generated`).
    fn write_generated(&mut self, key: usize, field: Field, value: Value) {
        if let (Field::FontSize, Value::Length(Dim::Px(now))) = (field, value) {
            self.resize_generated(key, now);
            return;
        }
        if let Some(at) = self.generated_block(key) {
            let paint = &mut self.prepared.generated_boxes[at].1;
            match (field, value) {
                (Field::Color, Value::Color(value)) => paint.color = Some(value),
                (Field::Visibility, Value::Visible(value)) => paint.visible = value,
                (Field::Background, Value::Color(value)) => {
                    paint.background = Some(value);
                    paint.background_fade = None;
                }
                (Field::Background, Value::Fade(value, alpha)) => {
                    paint.background = Some(value);
                    paint.background_fade = Some(alpha);
                }
                (Field::BorderColor(side), Value::Color(value)) => {
                    paint.border[side] = Some(value);
                    paint.border_fade[side] = None;
                }
                (Field::BorderColor(side), Value::Fade(value, alpha)) => {
                    paint.border[side] = Some(value);
                    paint.border_fade[side] = Some(alpha);
                }
                _ => {}
            }
            let arena = self.prepared.generated_boxes[at].0;
            let target = &mut self.prepared.boxes[arena];
            match (field, value) {
                (Field::Width, Value::Length(value)) => target.width = value,
                (Field::Height, Value::Length(value)) => target.height = value,
                (Field::Margin(side), Value::Length(value)) => target.margin[side] = value,
                (Field::Padding(side), Value::Length(value)) => target.padding[side] = value,
                (Field::BorderWidth(side), Value::Length(Dim::Px(value))) => {
                    target.border[side] = value;
                }
                _ => {}
            }
        }
        for run in self
            .texts
            .iter_mut()
            .flatten()
            .flat_map(|leaf| &mut leaf.runs)
        {
            let ours = run.generated == Some(key);
            let sides = run.side_of.is_some_and(|side| side.0 == key);
            let parts = |dim: Dim| match dim {
                Dim::Px(pixels) => (pixels, 0.0),
                Dim::Percent(percent) => (0.0, percent),
                Dim::Mix(pixels, percent) | Dim::Clamped(pixels, percent) => (pixels, percent),
                Dim::Auto => (0.0, 0.0),
            };
            // Its sides on its line (`read_generated`).
            match (field, value) {
                (Field::Padding(side @ (0 | 2)), Value::Length(Dim::Px(pixels))) => {
                    for decoration in run.inline_boxes.iter_mut().filter(|d| d.owner == key) {
                        decoration.padding[side / 2] = pixels;
                    }
                }
                (Field::BorderWidth(side @ (0 | 2)), Value::Length(Dim::Px(pixels))) => {
                    for decoration in run.inline_boxes.iter_mut().filter(|d| d.owner == key) {
                        decoration.border[side / 2].0 = pixels;
                    }
                }
                (Field::Padding(side), Value::Length(dim))
                    if run.side_of == Some((key, side))
                        && run.percent_room.is_some_and(|room| room.2) =>
                {
                    let (pixels, percent) = parts(dim);
                    let bar = run.edge.map_or(0.0, |edge| edge.1);
                    run.percent_room = Some((bar + pixels, percent.max(0.0), true));
                    if let Some(text) = super::room_text(run, 0.0) {
                        run.text = text;
                    }
                }
                (Field::BorderWidth(side), Value::Length(Dim::Px(pixels)))
                    if run.side_of == Some((key, side)) =>
                {
                    if let (Some(edge), Some(room)) = (&mut run.edge, &mut run.percent_room) {
                        room.0 += pixels - edge.1;
                        edge.1 = pixels;
                    }
                    if let Some(text) = super::room_text(run, 0.0) {
                        run.text = text;
                    }
                }
                (Field::Margin(side), Value::Length(dim))
                    if run.side_of == Some((key, side))
                        && run.percent_room.is_some_and(|room| !room.2) =>
                {
                    let (pixels, percent) = parts(dim);
                    run.percent_room = Some((pixels.max(0.0), percent.max(0.0), false));
                    if let Some(text) = super::room_text(run, 0.0) {
                        run.text = text;
                    }
                }
                _ => {}
            }
            match (field, value) {
                (Field::Color, Value::Color(value)) if ours => run.color = Some(value),
                (Field::Visibility, Value::Visible(value)) => {
                    if ours || sides {
                        run.visible = value;
                    }
                    for decoration in run.inline_boxes.iter_mut().filter(|d| d.owner == key) {
                        decoration.visible = value;
                    }
                    // Its own background under its text shows with it; hidden, the nearest
                    // that shows beyond it, as `reveal` picks for an element.
                    if let Some(own) = run.backgrounds.iter().position(|entry| entry.1 == key) {
                        let was = run.backgrounds.iter().find(|entry| entry.2).copied();
                        run.backgrounds[own].2 = value;
                        let now = run.backgrounds.iter().find(|entry| entry.2).copied();
                        if ours && was.map(|entry| entry.1) != now.map(|entry| entry.1) {
                            run.background = now.map(|entry| entry.0);
                            run.background_from = now
                                .map(|entry| entry.1)
                                .filter(|key| !super::is_generated(*key));
                        }
                    }
                }
                (Field::Background, Value::Color(value)) => {
                    for entry in run.backgrounds.iter_mut().filter(|entry| entry.1 == key) {
                        entry.0 = value;
                    }
                    if run
                        .backgrounds
                        .iter()
                        .find(|entry| entry.2)
                        .is_some_and(|entry| entry.1 == key)
                        && run.background.is_some()
                    {
                        run.background = Some(value);
                    }
                    for decoration in run.inline_boxes.iter_mut().filter(|d| d.owner == key) {
                        if decoration.background.is_some() {
                            decoration.background = Some(value);
                        }
                    }
                }
                (Field::BorderColor(side @ (0 | 2)), Value::Color(value)) => {
                    for decoration in run.inline_boxes.iter_mut().filter(|d| d.owner == key) {
                        if decoration.border[side / 2].1.is_some() {
                            decoration.border[side / 2].1 = Some(value);
                        }
                    }
                }
                (Field::BorderColor(side), Value::Color(value))
                    if run.side_of == Some((key, side)) =>
                {
                    if let Some(edge) = &mut run.edge {
                        edge.2 = value;
                    }
                }
                _ => {}
            }
        }
    }

    /// A `:before` or `:after`'s text at the presented size `now`: its runs by their share of
    /// the size it has (small capitals keep theirs), and the stand-in spaces beside them,
    /// which set the line's height too, keeping a fixed side's or indent's pixels. Lengths it
    /// gives in `em` keep their pixels.
    fn resize_generated(&mut self, key: usize, now: f32) {
        let Some(target) = self
            .texts
            .iter()
            .flatten()
            .flat_map(|leaf| &leaf.runs)
            .find(|run| run.generated == Some(key))
            .map(|run| run.size)
            .filter(|size| *size > 0.0)
        else {
            return;
        };
        let factor = now / target;
        for leaf in self.texts.iter_mut().flatten() {
            // Its runs, and the stand-in spaces made from them: an end side's from the text
            // before it, any other's (a start side's, an indent's) from the text after it.
            let own: Vec<bool> = leaf
                .runs
                .iter()
                .map(|run| run.generated == Some(key))
                .collect();
            if !own.contains(&true) {
                continue;
            }
            let ours: Vec<bool> = (0..leaf.runs.len())
                .map(|at| {
                    let run = &leaf.runs[at];
                    if !run.spacer {
                        return own[at];
                    }
                    let text = |near: usize| !leaf.runs[near].spacer;
                    let from = if run.side_of.is_some_and(|side| side.1 == 1) {
                        (0..at).rev().find(|near| text(*near))
                    } else {
                        (at + 1..leaf.runs.len()).find(|near| text(*near))
                    };
                    from.is_some_and(|near| own[near])
                })
                .collect();
            for (run, _) in leaf.runs.iter_mut().zip(&ours).filter(|(_, ours)| **ours) {
                run.size *= factor;
                run.line_size = run.line_size.map(|size| size * factor);
                if run.percent_room.is_some_and(|room| room.1 == 0.0)
                    && let Some(text) = super::room_text(run, 0.0)
                {
                    run.text = text;
                }
            }
        }
    }

    /// A border side's computed width, on its box or on its line; a style of `none` or
    /// `hidden` computes it to 0.
    /// The outer cells, by arena slot, whose `side` draws the border of the collapsed table
    /// `projected` is, which won there.
    fn table_sides(&self, projected: usize, side: usize) -> impl Iterator<Item = usize> + '_ {
        let slot = self.prepared.slot[projected];
        self.prepared
            .table_borders
            .iter()
            .filter(move |entry| Some(entry.2) == slot && entry.1 == side)
            .map(|entry| entry.0)
    }

    fn side_width(&self, projected: usize, side: usize) -> f32 {
        // A collapsed table's border is as wide as its outer cells draw it.
        if let Some(width) = self
            .table_sides(projected, side)
            .map(|cell| self.prepared.boxes[cell].border[side])
            .reduce(f32::max)
        {
            return width;
        }
        if self.under_text(projected) {
            return self.prepared.inline_boxes[projected].map_or(0.0, |sides| sides.border[side]);
        }
        self.prepared.slot[projected]
            .filter(|slot| !self.prepared.anonymous[*slot])
            .and_then(|slot| self.prepared.boxes.get(slot))
            .map_or(0.0, |value| value.border[side])
    }

    fn read(&self, projected: usize, field: Field) -> Option<Value> {
        // A collapsed table's border draws on the outer cells it won: its color is theirs.
        if let Field::BorderColor(side) = field
            && let Some(cell) = self.table_sides(projected, side).next()
        {
            return self.read(self.prepared.positions[cell].0, field);
        }
        let paint = &self.paints[projected];
        // An inline element's background lies under its text; transparent is a fade at none.
        let opaque =
            |color: Option<[f32; 3]>| color.map_or(Value::Fade([0.0; 3], 0.0), Value::Color);
        if field == Field::Background && self.under_text(projected) {
            return Some(opaque(paint.under_text.or(paint.background)));
        }
        // Its border colors likewise paint on its line's sides.
        if let Field::BorderColor(side) = field
            && self.under_text(projected)
        {
            return Some(opaque(paint.under_text_border[side].or(paint.border[side])));
        }
        // And its outline, drawn around its pieces on the lines from its paint.
        if self.under_text(projected) {
            match field {
                Field::OutlineColor => return paint.outline.map(|outline| Value::Color(outline.1)),
                Field::OutlineWidth => {
                    return paint
                        .outline
                        .map(|outline| Value::Length(Dim::Px(outline.0)));
                }
                _ => {}
            }
        }
        // An inline element's margin across and padding: the spaces of its line.
        if self.under_text(projected)
            && let Some(sides) = self.prepared.inline_boxes[projected]
            && matches!(
                field,
                Field::Margin(_) | Field::Padding(_) | Field::BorderWidth(_)
            )
        {
            // With no run in its box, as an empty inline without sides has, there is no
            // place for its sides: they apply at once.
            let inside = self.chain_test(projected);
            if !self
                .texts
                .iter()
                .flatten()
                .flat_map(|leaf| &leaf.runs)
                .any(|run| inside(run.owner))
            {
                return None;
            }
            let both = |pixels: f32, percent: f32| {
                Value::Length(match (pixels, percent) {
                    (_, 0.0) => Dim::Px(pixels),
                    (0.0, _) => Dim::Percent(percent),
                    _ => Dim::Mix(pixels, percent),
                })
            };
            match field {
                Field::Margin(side @ (1 | 3)) => {
                    return Some(both(sides.margin[side], sides.percent_margin[side]));
                }
                Field::Padding(side) => {
                    return Some(both(sides.padding[side], sides.percent_padding[side]));
                }
                Field::BorderWidth(side) => {
                    return Some(Value::Length(Dim::Px(sides.border[side])));
                }
                // Vertical margins move nothing on a line.
                Field::Margin(_) => return None,
                _ => {}
            }
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
            Field::FontWeight => Some(Value::Weight(f32::from(self.weights[projected]))),
            Field::Color => color(paint.color),
            Field::Background => own.map(|_| match (paint.background, paint.background_fade) {
                (None, _) => Value::Fade([0.0; 3], 0.0),
                (Some(value), None) => Value::Color(value),
                (Some(value), Some(alpha)) => Value::Fade(value, alpha),
            }),
            // Transparent is a fade at none, but not on a collapsed cell's side that gives way
            // along all of its edge: the neighbour's color draws there. Color picks no
            // winner, so a color change leaves them as they are. (`request` skips a side with
            // no width at either end.)
            Field::BorderColor(side) => {
                let (slot, _) = boxed()?;
                let shows = self
                    .prepared
                    .collapsed_losers
                    .binary_search(&(slot, side))
                    .is_err();
                match (paint.border[side], paint.border_fade[side]) {
                    (None, _) if shows => Some(Value::Fade([0.0; 3], 0.0)),
                    (None, _) => None,
                    (Some(value), None) => Some(Value::Color(value)),
                    (Some(value), Some(alpha)) => Some(Value::Fade(value, alpha)),
                }
            }
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
        // A collapsed table's border color moves on the outer cells that draw it.
        if let Field::BorderColor(side) = field {
            let cells: Vec<usize> = self.table_sides(projected, side).collect();
            if !cells.is_empty() {
                for cell in cells {
                    self.write(self.prepared.positions[cell].0, field, value);
                }
                return;
            }
        }
        if let (Field::Margin(_) | Field::Padding(_) | Field::BorderWidth(_), Value::Length(dim)) =
            (field, value)
            && self.under_text(projected)
        {
            self.inline_side(projected, field, dim);
            if let Field::Margin(side) | Field::Padding(side) | Field::BorderWidth(side) = field
                && matches!(side, 1 | 3)
            {
                self.settle_pull(projected, side);
            }
            return;
        }
        // Its text carries a weight, not its box: `reweigh`.
        if field == Field::FontWeight {
            return;
        }
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
            // A fade under text paints on its line, not on its paint (`inline_fade`).
            (Field::Background | Field::BorderColor(_), Value::Fade(..)) if under_text => {}
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
            (Field::BorderColor(side), Value::Color(value)) => {
                paint.border[side] = Some(value);
                paint.border_fade[side] = None;
            }
            (Field::BorderColor(side), Value::Fade(value, alpha)) => {
                paint.border[side] = Some(value);
                paint.border_fade[side] = Some(alpha);
            }
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

    /// An inline element's side as its margin now pulls it (`TextRun::overhang`): its border
    /// and padding's spaces narrow by as much of a negative margin as they cover, and its box
    /// draws that far past them, as the styling makes them. What a pull takes from the space
    /// beside them stays as the target styling laid that space out.
    fn settle_pull(&mut self, projected: usize, side: usize) {
        let Some(sides) = self.prepared.inline_boxes[projected] else {
            return;
        };
        let node = self.originals[projected];
        let pulled = (-sides.margin[side]).max(0.0);
        let room = sides.border[side] + sides.padding[side];
        let percent = sides.percent_padding[side];
        for run in self
            .texts
            .iter_mut()
            .flatten()
            .flat_map(|leaf| &mut leaf.runs)
            .filter(|run| {
                run.side_of == Some((node, side)) && run.percent_room.is_some_and(|r| r.2)
            })
        {
            let overhang = pulled.min(room);
            run.overhang = overhang;
            // Beside a percentage padding, taken again out of its room at layout.
            run.pulled = if percent > 0.0 { pulled } else { 0.0 };
            run.percent_room = Some((
                if percent > 0.0 { room } else { room - overhang },
                percent,
                true,
            ));
            if let Some(text) = super::room_text(run, 0.0) {
                run.text = text;
            }
        }
    }

    /// An inline element's margin or padding on its line: its decoration's bands above and
    /// below, and its side spaces across, made where the styling has none.
    fn inline_side(&mut self, projected: usize, field: Field, value: Dim) {
        let (pixels, percent) = match value {
            Dim::Px(pixels) => (pixels, 0.0),
            Dim::Percent(percent) => (0.0, percent),
            Dim::Mix(pixels, percent) | Dim::Clamped(pixels, percent) => (pixels, percent),
            Dim::Auto => (0.0, 0.0),
        };
        // A border's width is in its box with the padding: the same spaces inside.
        let (side, margin) = match field {
            Field::Margin(side) => (side, true),
            Field::Padding(side) | Field::BorderWidth(side) => (side, false),
            _ => return,
        };
        let Some(sides) = self.prepared.inline_boxes[projected].as_mut() else {
            return;
        };
        match field {
            Field::Margin(_) => {
                sides.margin[side] = pixels;
                sides.percent_margin[side] = percent.max(0.0);
            }
            Field::BorderWidth(_) => sides.border[side] = pixels.max(0.0),
            _ => {
                sides.padding[side] = pixels;
                sides.percent_padding[side] = percent.max(0.0);
            }
        }
        let sides = *sides;
        let node = self.originals[projected];
        let paint = self.paints[projected];
        // Opening its line, an inline's colors went under its text.
        let background = paint.under_text.or(paint.background);
        let border = [0, 2].map(|at| paint.under_text_border[at].or(paint.border[at]));
        let inside = self.chain_test(projected);
        // Its box around its runs, after the boxes around it, as the styling would make it.
        let own = super::InlineDecoration {
            padding: [sides.padding[0], sides.padding[2]],
            padding_share: [
                (sides.padding[0], sides.percent_padding[0]),
                (sides.padding[2], sides.percent_padding[2]),
            ],
            border: [(sides.border[0], border[0]), (sides.border[2], border[1])],
            border_style: [paint.border_style[0], paint.border_style[2]],
            background,
            owner: node,
            visible: paint.visible,
            alpha: 1.0,
            border_alpha: [1.0; 2],
        };
        let boxed = self
            .texts
            .iter()
            .flatten()
            .flat_map(|leaf| &leaf.runs)
            .any(|run| run.inline_boxes.iter().any(|sides| sides.owner == node));
        let around = |key: usize| key != node && !super::is_generated(key) && !inside(key);
        for run in self
            .texts
            .iter_mut()
            .flatten()
            .flat_map(|leaf| &mut leaf.runs)
        {
            if !boxed && inside(run.owner) && run.side_of.is_none_or(|at| at.0 != node) {
                let at = run
                    .inline_boxes
                    .iter()
                    .take_while(|sides| around(sides.owner))
                    .count();
                run.inline_boxes.insert(at, own);
            }
            for sides in run
                .inline_boxes
                .iter_mut()
                .filter(|sides| sides.owner == node)
            {
                sides.padding = own.padding;
                sides.padding_share = own.padding_share;
                sides.border[0].0 = own.border[0].0;
                sides.border[1].0 = own.border[1].0;
            }
        }
        if !matches!(side, 1 | 3) {
            return;
        }
        // Across: its spaces, resized for their room, or made beside its first or last run.
        // A negative margin has no spaces of its own: it narrows the others (`settle_pull`).
        let room = if margin {
            (
                sides.margin[side].max(0.0),
                sides.percent_margin[side],
                false,
            )
        } else {
            (
                sides.border[side] + sides.padding[side],
                sides.percent_padding[side],
                true,
            )
        };
        // Its side bar, on its border and padding's spaces.
        let bar_color = paint.under_text_border[side].or(paint.border[side]);
        let bar = (!margin && sides.border[side] > 0.0)
            .then_some(bar_color)
            .flatten()
            .map(|color| (side == 3, sides.border[side], color));
        let ours = |run: &super::TextRun| {
            run.side_of == Some((node, side)) && run.percent_room.is_some_and(|r| r.2 == !margin)
        };
        let mut found = false;
        for run in self
            .texts
            .iter_mut()
            .flatten()
            .flat_map(|leaf| &mut leaf.runs)
        {
            if ours(run) {
                run.percent_room = Some(room);
                if !margin {
                    run.edge = bar;
                }
                if let Some(text) = super::room_text(run, 0.0) {
                    run.text = text;
                }
                found = true;
            }
        }
        // A margin with no room makes no spaces: a negative one draws as `settle_pull` has it.
        if found || (margin && room.0 <= 0.0 && room.1 <= 0.0) {
            return;
        }
        // Its runs on its own lines: not its margin's spaces, nor what nests in blocks.
        let in_box = |run: &super::TextRun| {
            inside(run.owner)
                && !(run.side_of.is_some_and(|at| at.0 == node)
                    && !run.percent_room.is_some_and(|r| r.2))
        };
        let leaves: Vec<usize> = self
            .texts
            .iter()
            .enumerate()
            .filter(|(_, leaf)| {
                leaf.as_ref()
                    .is_some_and(|leaf| leaf.runs.iter().any(in_box))
            })
            .map(|(arena, _)| arena)
            .collect();
        let arena = if side == 3 {
            leaves.first()
        } else {
            leaves.last()
        };
        // The nearest shown inline around it answers for its margin; nothing hidden does.
        let mut answer = self.prepared.inline_parent[projected];
        while let Some(here) = answer.filter(|here| !self.paints[*here].visible) {
            answer = self.prepared.inline_parent[here];
        }
        // Else its block container, the first box around it that is no inline.
        let container = || {
            let nodes = self.source.construction.nodes();
            let mut at = nodes.get(node).and_then(|value| value.parent());
            while let Some(here) = at {
                let inline = self
                    .originals
                    .iter()
                    .position(|original| *original == here)
                    .is_some_and(|projected| self.prepared.inline_boxes[projected].is_some());
                if !inline {
                    return here;
                }
                at = nodes.get(here).and_then(|value| value.parent());
            }
            node
        };
        let margin_owner = answer.map_or_else(container, |here| self.originals[here]);
        let Some(leaf) = arena.and_then(|arena| self.texts[*arena].as_mut()) else {
            return;
        };
        let place = if side == 3 {
            leaf.runs.iter().position(in_box)
        } else {
            leaf.runs.iter().rposition(in_box)
        };
        let Some(place) = place else { return };
        let base = &leaf.runs[place];
        let inline_boxes: Vec<super::InlineDecoration> = base
            .inline_boxes
            .iter()
            .filter(|sides| around(sides.owner) || (!margin && sides.owner == node))
            .copied()
            .collect();
        let backgrounds: Vec<([f32; 3], usize, bool)> = base
            .backgrounds
            .iter()
            .filter(|entry| around(entry.1) || (!margin && entry.1 == node))
            .copied()
            .collect();
        let shows_own = !margin && background.is_some() && paint.visible;
        let shown = backgrounds
            .iter()
            .find(|entry| entry.2 && entry.1 != node)
            .filter(|_| !shows_own);
        let mut spacer = super::TextRun {
            text: String::new(),
            spacer: true,
            edge: bar,
            edge_style: paint.border_style[side],
            background: shown.map(|entry| entry.0),
            background_from: shown.map(|entry| entry.1),
            backgrounds,
            percent_room: Some(room),
            side_of: Some((node, side)),
            marker_image: None,
            underline: false,
            line_through: false,
            overline: false,
            inline_boxes,
            visible: if margin { base.visible } else { paint.visible },
            owner: if margin { margin_owner } else { node },
            // Its own box's sides: nothing of another's negative margin.
            overhang: 0.0,
            pull: 0.0,
            pulled: 0.0,
            ..base.clone()
        };
        if let Some(text) = super::room_text(&spacer, 0.0) {
            spacer.text = text;
        }
        // Opening, the margin outside the padding; closing, the padding first.
        let at = if side == 3 { place } else { place + 1 };
        // 9.10: a paragraph's direction mark leads it, so it moves onto spaces put first.
        if side == 3
            && !spacer.text.is_empty()
            && leaf.runs[..at].iter().all(|run| run.text.is_empty())
            && let Some(mark) = leaf.runs[at]
                .text
                .chars()
                .next()
                .filter(|mark| matches!(mark, '\u{200E}' | '\u{200F}'))
        {
            leaf.runs[at].text.remove(0);
            spacer.text.insert(0, mark);
        }
        leaf.runs.insert(at, spacer);
    }

    /// Whether a source node lies in the inline box of the projected `element` on its lines:
    /// by inline parents, so not inside a block or inline-block that nests in it.
    fn chain_test(&self, element: usize) -> impl Fn(usize) -> bool + 'static {
        let projected: BTreeMap<usize, usize> = self
            .originals
            .iter()
            .enumerate()
            .map(|(at, node)| (*node, at))
            .collect();
        let parents = self.prepared.inline_parent.clone();
        move |node: usize| {
            let mut at = projected.get(&node).copied();
            while let Some(here) = at {
                if here == element {
                    return true;
                }
                at = parents[here];
            }
            false
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
        // Sizes after the rest: scaling rewrites side spaces, which other followers know by
        // text. An inline's sides last: they may add spaces and boxes, shifting the runs
        // `color` knows by place and the boxes sizes know runs by; they make them at the
        // scaled size.
        let (sizes, rest): (Vec<_>, Vec<_>) = presentation
            .iter()
            .partition(|presented| presented.field == Field::FontSize);
        // A `:before` or `:after`'s own first: what its element's `color` carries to it then
        // finds it moved on, and leaves it to its own timing.
        let (mut rest, others): (Vec<_>, Vec<_>) = rest
            .into_iter()
            .partition(|presented| matches!(self.target_of(&presented.id), Some((_, Some(_)))));
        rest.extend(others);
        let (sides, rest): (Vec<_>, Vec<_>) = rest.into_iter().partition(|presented| {
            matches!(
                presented.field,
                Field::Margin(_) | Field::Padding(_) | Field::BorderWidth(_)
            )
        });
        // What animates of its own, by source node and field.
        let animated: Vec<(usize, Field)> = presentation
            .iter()
            .filter_map(|presented| {
                let projected = self.element(&presented.id)?;
                Some((self.originals[projected], presented.field))
            })
            // A `:before` or `:after` is no element: `element` finds none for it.
            .collect();
        // Fades on a line last: a layer over what the rest leaves beneath it, shown or not.
        let mut fades = Vec::new();
        // Inline backgrounds presented between two colors, by source node: what a fade inside
        // them shows beneath it.
        let mut presented_colors: BTreeMap<usize, [f32; 3]> = BTreeMap::new();
        for presented in rest.into_iter().chain(sizes).chain(sides) {
            let Some((projected, generated)) = self.target_of(&presented.id) else {
                continue;
            };
            // A `:before` or `:after`'s own colors and visibility, where its content shows.
            if let Some(key) = generated {
                if self.read_generated(key, presented.field) != Some(presented.target) {
                    continue;
                }
                // On its line, to or from transparent: a layer, as an inline element's.
                if matches!(
                    presented.field,
                    Field::Background | Field::BorderColor(0 | 2)
                ) && self.generated_block(key).is_none()
                    && (matches!(presented.target, Value::Fade(..))
                        || matches!(presented.now, Value::Fade(..)))
                {
                    if let Some((color, alpha)) = rgba(presented.now) {
                        fades.push((projected, Some(key), presented.field, color, alpha));
                    }
                    continue;
                }
                self.write_generated(key, presented.field, presented.now);
                // Its currentColor borders, where they still show the target.
                for follower in &presented.followers.boxes {
                    for side in (0..4).filter(|side| follower.borders[*side]) {
                        let field = Field::BorderColor(side);
                        if self.read_generated(key, field) == Some(presented.target) {
                            self.write_generated(key, field, presented.now);
                        }
                    }
                }
                continue;
            }
            // Section 13: an important value holds without a temporary override, in an
            // interaction variant as in the committed tree.
            if self.read(projected, presented.field) != Some(presented.target)
                || self.important[projected].contains(&presented.property)
            {
                continue;
            }
            if matches!(presented.field, Field::Background | Field::BorderColor(_))
                && self.under_text(projected)
                && (matches!(presented.target, Value::Fade(..))
                    || matches!(presented.now, Value::Fade(..)))
            {
                if let Some((color, alpha)) = rgba(presented.now) {
                    fades.push((projected, None, presented.field, color, alpha));
                }
                continue;
            }
            if let (Field::Background, Value::Color(_), Value::Color(now)) =
                (presented.field, presented.target, presented.now)
                && self.under_text(projected)
            {
                presented_colors.insert(self.originals[projected], now);
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
                    &animated,
                );
            }
            if let (Field::FontWeight, Value::Weight(target), Value::Weight(now)) =
                (presented.field, presented.target, presented.now)
            {
                self.reweigh(
                    &presented.followers,
                    self.originals[projected],
                    (target as u16, now as u16),
                    &animated,
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
        // Outermost first: an inner one's layer goes over theirs.
        let fading: BTreeSet<usize> = fades
            .iter()
            .filter(|fade| fade.2 == Field::Background)
            .map(|fade| fade.1.unwrap_or(self.originals[fade.0]))
            .collect();
        let nodes = self.source.construction.nodes();
        let depth = |node: usize| {
            std::iter::successors(Some(node), |at| {
                nodes.get(*at).and_then(|value| value.parent())
            })
            .count()
        };
        // A `:before` or `:after` inside its element.
        fades.sort_by_cached_key(|fade| {
            depth(self.originals[fade.0]) + usize::from(fade.1.is_some())
        });
        let beneath = Beneath {
            presented: presented_colors,
            // Only for backgrounds fading inside one another.
            blocks: if fading.is_empty() {
                Vec::new()
            } else {
                self.block_bases()
            },
            fading,
        };
        for (projected, generated, field, color, alpha) in fades {
            self.inline_fade(projected, generated, (field, color, alpha), &beneath);
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
    fn resize(&mut self, followers: &Followers, root: usize, factor: f32, own: &[(usize, Field)]) {
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
        // Lengths in `em` move with the size, unless something else moved them already or
        // an animation of their own does; an inline's sides make their spaces again.
        let projected_of: BTreeMap<usize, usize> = self
            .originals
            .iter()
            .enumerate()
            .map(|(at, node)| (*node, at))
            .collect();
        for &(node, field, target) in &followers.em_lengths {
            let Some(&projected) = projected_of.get(&node) else {
                continue;
            };
            if own.contains(&(node, field)) {
                continue;
            }
            // Nothing of no size moves: an inline would make empty spaces for it.
            if target != 0.0 && self.read(projected, field) == Some(Value::Length(Dim::Px(target)))
            {
                self.write(projected, field, Value::Length(Dim::Px(target * factor)));
            }
        }
        for &(arena, node, target) in &followers.em_indents {
            let Some(leaf) = self.texts.get_mut(arena).and_then(Option::as_mut) else {
                continue;
            };
            for run in leaf
                .runs
                .iter_mut()
                .filter(|run| indent_space(run) && run.owner == node)
            {
                if run.percent_room.is_some_and(|room| room.0 == target) {
                    run.percent_room = Some((target * factor, 0.0, false));
                    if let Some(text) = super::room_text(run, 0.0) {
                        run.text = text;
                    }
                }
            }
        }
        for &(arena, target) in &followers.em_line_heights {
            if let Some(leaf) = self.texts.get_mut(arena).and_then(Option::as_mut)
                && leaf.line_height == super::TextLineHeight::Px(target)
            {
                leaf.line_height = super::TextLineHeight::Px(target * factor);
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
                let alike = |other: &&super::TextRun| {
                    other.owner == run.owner
                        && other.spacer == run.spacer
                        && content(&other.text) == content(&run.text)
                };
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
        // A line height whose pixels are the same share of the size at both ends follows it:
        // it inherits as pixels, so no element's own declaration tells.
        let em = |from: f32, to: f32| {
            (from * target - to * committed).abs() <= 1e-3 * (to * committed).abs()
        };
        let mut fields = vec![
            Field::Width,
            Field::MinWidth,
            Field::MaxWidth,
            Field::Height,
            Field::MinHeight,
            Field::MaxHeight,
            Field::OutlineWidth,
        ];
        for side in 0..4 {
            fields.extend([
                Field::Margin(side),
                Field::Padding(side),
                Field::BorderWidth(side),
                Field::Offset(side),
            ]);
        }
        let nodes: Vec<usize> = std::iter::once(root)
            .chain(followers.sized_nodes.keys().copied())
            .collect();
        // Lengths given in `em` or `ex` follow the size, whatever they were before.
        let indent = PropertyId::lookup("text-indent");
        for node in nodes {
            let Some(after) = to.originals.iter().position(|n| *n == node) else {
                continue;
            };
            let given = &to.em_lengths[after];
            for &field in &fields {
                let named = field_name(field).and_then(PropertyId::lookup);
                if named.is_some_and(|property| given.contains(&property))
                    && let Some(Value::Length(Dim::Px(new))) = to.read(after, field)
                {
                    followers.em_lengths.push((node, field, new));
                }
            }
            // An indent in `em`: its spaces' room at the start of its block's lines.
            if indent.is_some_and(|property| given.contains(&property)) {
                for (arena, leaf) in to.texts.iter().enumerate() {
                    let Some(leaf) = leaf else { continue };
                    if let Some(room) = leaf
                        .runs
                        .iter()
                        .find(|run| indent_space(run) && run.owner == node)
                        .and_then(|run| run.percent_room)
                    {
                        followers.em_indents.push((arena, node, room.0));
                    }
                }
            }
        }
        for (arena, leaf) in to.texts.iter().enumerate() {
            let Some(leaf) = leaf else { continue };
            if let (Some(super::TextLineHeight::Px(old)), super::TextLineHeight::Px(new)) = (
                self.texts
                    .get(arena)
                    .and_then(Option::as_ref)
                    .map(|old| old.line_height),
                leaf.line_height,
            ) && em(old, new)
                && leaf.runs.iter().any(|run| to.within(run.owner, root))
            {
                followers.em_line_heights.push((arena, new));
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
                            !other.spacer && content(&other.text) == content(&run.text)
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

    /// Each leaf's block background, beneath every inline in it: the base the layers of
    /// inlines fading inside one another stack over.
    fn block_bases(&self) -> Vec<Option<[f32; 3]>> {
        let mut parent_of = vec![None; self.prepared.boxes.len()];
        for (at, value) in self.prepared.boxes.iter().enumerate() {
            for &child in &value.children {
                parent_of[child] = Some(at);
            }
        }
        let block_background = |mut at: Option<usize>| {
            while let Some(here) = at {
                let projected = self.prepared.positions[here].0;
                // A hidden block paints nothing of its own: what lies beyond it shows.
                if !self.prepared.anonymous[here] && self.paints[projected].visible {
                    // An image would show through no color painted over it.
                    if self.backgrounds.contains_key(&projected) {
                        return None;
                    }
                    let paint = self.paints[projected];
                    // Nor would a color fading itself: it paints at less than full.
                    if paint.background_fade.is_some() {
                        return None;
                    }
                    if let Some(color) = paint.background {
                        return Some(color);
                    }
                }
                at = parent_of[here];
            }
            None
        };
        (0..self.texts.len())
            .map(|arena| block_background(Some(arena)))
            .collect()
    }

    /// An inline element's background or border color on its way to or from transparent:
    /// a layer of `color` at `alpha` where its box paints, over what shows beneath it.
    /// For a `:before` or `:after` (`generated`, by key), its runs and side spaces are what
    /// lies in it, nothing nests in it, and its bars across keep no width when transparent.
    fn inline_fade(
        &mut self,
        projected: usize,
        generated: Option<usize>,
        (field, color, alpha): (Field, [f32; 3], f32),
        beneath: &Beneath,
    ) {
        let (presented, fading, blocks) = (&beneath.presented, &beneath.fading, &beneath.blocks);
        // A `:before` or `:after` with a box holds the runs that wear it: its text and its
        // border and padding's spaces, not its margin's, nor an inside marker before it.
        let decorated = generated.is_some_and(|key| {
            self.texts
                .iter()
                .flatten()
                .flat_map(|leaf| &leaf.runs)
                .any(|run| run.inline_boxes.iter().any(|sides| sides.owner == key))
        });
        let wears = move |run: &super::TextRun, key: usize| {
            if decorated {
                run.inline_boxes.iter().any(|sides| sides.owner == key)
            } else {
                run.generated == Some(key)
            }
        };
        let visible = match generated {
            Some(key) => self
                .texts
                .iter()
                .flatten()
                .flat_map(|leaf| &leaf.runs)
                .find(|run| wears(run, key) && !run.spacer)
                .is_some_and(|run| run.visible),
            None => self.paints[projected].visible,
        };
        if !visible {
            return;
        }
        let node = generated.unwrap_or(self.originals[projected]);
        let chain = self.chain_test(projected);
        let inside = |key: usize| generated.is_none() && chain(key);
        let holds = |run: &super::TextRun| match generated {
            Some(key) => wears(run, key),
            None => chain(run.owner),
        };
        let width = match generated {
            Some(_) => [0.0; 4],
            None => self.prepared.inline_boxes[projected].map_or([0.0; 4], |sides| sides.border),
        };
        let source = Arc::clone(&self.source);
        let nodes = source.construction.nodes();
        // Whether a box around it lies inside another around it: nearer to it.
        let nearer = |near: usize, far: usize| {
            std::iter::successors(Some(near), |at| {
                nodes.get(*at).and_then(|value| value.parent())
            })
            .any(|at| at == far)
        };
        for (arena, run) in self
            .texts
            .iter_mut()
            .enumerate()
            .filter_map(|(arena, leaf)| leaf.as_mut().map(|leaf| (arena, leaf)))
            .flat_map(|(arena, leaf)| leaf.runs.iter_mut().map(move |run| (arena, run)))
        {
            match field {
                Field::Background => {
                    for sides in run
                        .inline_boxes
                        .iter_mut()
                        .filter(|sides| sides.owner == node)
                    {
                        sides.background = Some(color);
                        sides.alpha = alpha;
                    }
                    if !holds(run) {
                        continue;
                    }
                    // Beneath it, the nearest opaque background around it; one nearer covers
                    // it. A fading one is no cover: its layer is beneath this one.
                    let mut outer = None;
                    let mut covered = false;
                    for &(shade, key, shown) in &run.backgrounds {
                        if key == node || !shown || fading.contains(&key) {
                            continue;
                        }
                        if super::is_generated(key) || inside(key) {
                            covered = true;
                        } else {
                            outer = Some((presented.get(&key).copied().unwrap_or(shade), key));
                        }
                        break;
                    }
                    if covered {
                        continue;
                    }
                    // Any fading box around it with a layer here is nearer than that: one
                    // beyond an opaque box found that box covering it, and left none.
                    debug_assert!(
                        run.fades
                            .iter()
                            .all(|layer| outer.is_none_or(|(_, key)| nearer(layer.0, key)))
                    );
                    // Under a fading box around it, with nothing inline beneath, the block's
                    // color as the base, so the layers stack over it in order.
                    run.background = outer.map(|entry| entry.0).or_else(|| {
                        if run.fades.is_empty() {
                            None
                        } else {
                            blocks[arena]
                        }
                    });
                    run.background_from = outer.map(|entry| entry.1);
                    // With nothing opaque beneath, its box's layer covers the content too.
                    let boxed = run.inline_boxes.iter().any(|sides| sides.owner == node);
                    if run.background.is_some() || !boxed || !run.fades.is_empty() {
                        run.fades.push((node, color, alpha));
                    }
                }
                Field::BorderColor(side @ (0 | 2)) => {
                    for sides in run
                        .inline_boxes
                        .iter_mut()
                        .filter(|sides| sides.owner == node)
                    {
                        sides.border[side / 2].1 = Some(color);
                        sides.border_alpha[side / 2] = alpha;
                    }
                }
                // Its bar, on its border and padding's spaces on that side.
                Field::BorderColor(side)
                    if run.side_of == Some((node, side))
                        && run.percent_room.is_some_and(|room| room.2)
                        && width[side] > 0.0 =>
                {
                    run.edge = Some((side == 3, width[side], color));
                    run.edge_alpha = alpha;
                }
                _ => {}
            }
        }
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
                // A line an ancestor set changes color with it (16.3.1).
                for from in [
                    &mut run.underline_from,
                    &mut run.line_through_from,
                    &mut run.overline_from,
                ]
                .into_iter()
                .flatten()
                {
                    if from.1 == target && followers.boxes.iter().any(|f| f.node == from.0) {
                        from.1 = now;
                    }
                }
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
                let alike = |other: &&super::TextRun| {
                    other.owner == run.owner
                        && other.spacer == run.spacer
                        && content(&other.text) == content(&run.text)
                };
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

    /// What shows the target's weight at both ends, inside it: the outside markers of it
    /// and its descendants, by node, and text runs by arena slot, owner, text and occurrence,
    /// each by its own weight, so a `:before` or `:after` with a weight of its own, or a `bolder`
    /// one, keeps it. A side's spaces count as inside the box they are a side of.
    /// This checks the old end; the new one is checked as it presents (`reweigh`).
    fn weight_followers(&self, to: &Self, target: usize, from: Value) -> Followers {
        let Value::Weight(from) = from else {
            return Followers::default();
        };
        let from = from as u16;
        let root = to.originals[target];
        let mut followers = Followers::default();
        for node in &to.originals {
            let Some(before) = self.originals.iter().position(|n| n == node) else {
                continue;
            };
            if to.within(*node, root) && self.weights[before] == from {
                followers.boxes.push(Follower {
                    node: *node,
                    borders: [false; 4],
                    outline: false,
                    marker: true,
                });
            }
        }
        for (arena, leaf) in to.texts.iter().enumerate() {
            let Some(leaf) = leaf else { continue };
            for (at, run) in leaf.runs.iter().enumerate() {
                if !to.within(run.side_of.map_or(run.owner, |side| side.0), root) {
                    continue;
                }
                // By content, not place, as `followers` finds the color's runs.
                let alike = |other: &&super::TextRun| {
                    other.owner == run.owner
                        && other.spacer == run.spacer
                        && content(&other.text) == content(&run.text)
                };
                let occurrence = leaf.runs[..at].iter().filter(alike).count();
                let old = self
                    .texts
                    .get(arena)
                    .and_then(Option::as_ref)
                    .and_then(|old| old.runs.iter().filter(alike).nth(occurrence));
                if old.is_some_and(|old| old.weight == from) {
                    followers.weighed_runs.push((
                        arena,
                        run.owner,
                        content(&run.text).to_owned(),
                        run.spacer,
                        occurrence,
                    ));
                }
            }
        }
        followers
    }

    /// Carries an animated weight from `root` to its followers: whatever still has the
    /// target weight, and is not inside a descendant animating a weight of its own.
    fn reweigh(
        &mut self,
        followers: &Followers,
        root: usize,
        (target, now): (u16, u16),
        animated: &[(usize, Field)],
    ) {
        let own: Vec<usize> = animated
            .iter()
            .filter(|(node, field)| *field == Field::FontWeight && *node != root)
            .map(|(node, _)| *node)
            .collect();
        let apart = |tree: &Self, node: usize| {
            own.iter()
                .any(|inner| tree.within(node, *inner) && tree.within(*inner, root))
        };
        for follower in &followers.boxes {
            if apart(self, follower.node) {
                continue;
            }
            let Some(projected) = self.originals.iter().position(|n| *n == follower.node) else {
                continue;
            };
            if let Some(marker) = self.markers.get_mut(&projected) {
                for run in marker.runs.iter_mut().filter(|run| run.weight == target) {
                    run.weight = now;
                }
            }
        }
        for (arena, owner, text, spacer, occurrence) in &followers.weighed_runs {
            let Some(leaf) = self.texts.get(*arena).and_then(Option::as_ref) else {
                continue;
            };
            let Some((at, run)) = leaf
                .runs
                .iter()
                .enumerate()
                .filter(|(_, run)| {
                    run.owner == *owner && run.spacer == *spacer && content(&run.text) == text
                })
                .nth(*occurrence)
                .filter(|(_, run)| run.weight == target)
            else {
                continue;
            };
            if apart(self, run.side_of.map_or(run.owner, |side| side.0)) {
                continue;
            }
            if let Some(leaf) = self.texts[*arena].as_mut() {
                leaf.runs[at].weight = now;
            }
        }
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

/// The longhand a length field animates.
fn field_name(field: Field) -> Option<&'static str> {
    let side = |names: [&'static str; 4], at: usize| names.get(at).copied();
    match field {
        Field::Width => Some("width"),
        Field::MinWidth => Some("min-width"),
        Field::MaxWidth => Some("max-width"),
        Field::Height => Some("height"),
        Field::MinHeight => Some("min-height"),
        Field::MaxHeight => Some("max-height"),
        Field::OutlineWidth => Some("outline-width"),
        Field::Margin(at) => side(
            ["margin-top", "margin-right", "margin-bottom", "margin-left"],
            at,
        ),
        Field::Padding(at) => side(
            [
                "padding-top",
                "padding-right",
                "padding-bottom",
                "padding-left",
            ],
            at,
        ),
        Field::BorderWidth(at) => side(
            [
                "border-top-width",
                "border-right-width",
                "border-bottom-width",
                "border-left-width",
            ],
            at,
        ),
        Field::Offset(at) => side(["top", "right", "bottom", "left"], at),
        _ => None,
    }
}

/// A `text-indent`'s spaces: a sized spacer of no side, with no share of the width.
fn indent_space(run: &super::TextRun) -> bool {
    run.spacer && run.side_of.is_none() && run.percent_room.is_some_and(|room| room.1 == 0.0)
}

/// A run's text as followers compare it: without the spaces at its ends, which a negative
/// margin beside it narrows or takes away (`pull_aside`), so the same run matches at both
/// ends of an animation that changes the pull.
fn content(text: &str) -> &str {
    text.trim_matches(|character| matches!(character, ' ' | '\u{2002}'..='\u{2006}'))
}

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

/// How a track eases: an iced easing from a request, or a transition's CSS one,
/// begun partway when its delay was negative.
#[derive(Debug, Clone, Copy)]
enum Ease {
    Iced(Easing),
    Css { easing: CssEasing, from: f32 },
}

impl Ease {
    fn value(self, linear: f32) -> f32 {
        match self {
            Self::Iced(easing) => easing.value(linear),
            Self::Css { easing, from } => easing.value(from + linear * (1.0 - from)),
        }
    }
}

/// How a transition names an element that has no id: never a CSS id, which cannot hold
/// this character.
const UNNAMED: &str = "\u{0}";

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
    easing: Ease,
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
    /// CSS animations of `@keyframes`, by element and place in its list.
    keyframed: Vec<Keyframed>,
    /// The styling their keyframe values were read from: source, interaction state.
    keyframes_of: Option<(usize, InteractionKey)>,
    /// The time last given; keyframe animations are sampled at it.
    clock: Option<Instant>,
}

type InteractionKey = (Option<usize>, Option<usize>, Option<usize>);

/// One keyframe of one animated longhand: where it sits, its value, and the easing from
/// it to the next (the keyframe's `animation-timing-function`, else the animation's).
#[derive(Debug, Clone, Copy)]
struct Stop {
    offset: f32,
    value: Value,
    easing: Option<CssEasing>,
}

/// One longhand a keyframe animation moves.
#[derive(Debug, Clone)]
struct KeyTrack {
    property: PropertyId,
    field: Field,
    stops: Vec<Stop>,
}

/// One running CSS animation (CSS Animations 1). The time comes from the frames given to
/// `play` and `advance`: a keyframe timeline repeats, reverses and pauses, which a two-ended
/// progress cannot model.
#[derive(Debug, Clone)]
struct Keyframed {
    capture: u64,
    id: String,
    projected: usize,
    index: usize,
    animation: ElementAnimation,
    started: Instant,
    paused_at: Option<Instant>,
    paused_for: Duration,
    tracks: Vec<KeyTrack>,
}

impl Keyframed {
    /// Milliseconds since its delay ended, at `now`.
    fn local_time(&self, now: Instant) -> f32 {
        let at = self.paused_at.unwrap_or(now).max(self.started);
        let elapsed = at
            .duration_since(self.started)
            .saturating_sub(self.paused_for);
        elapsed.as_secs_f32() * 1000.0 - self.animation.delay
    }

    /// The active duration, in milliseconds.
    fn active(&self) -> f32 {
        let iterations = self.animation.iterations;
        if iterations == 0.0 || self.animation.duration == 0.0 {
            0.0
        } else {
            self.animation.duration * iterations
        }
    }

    /// Whether it still has frames to show at `now`.
    fn running(&self, now: Instant) -> bool {
        self.paused_at.is_none() && self.local_time(now) < self.active()
    }

    /// 4.x: where in its keyframes it is at `now`, from 0 to 1, or `None` where it shows
    /// nothing (before its delay without a backwards fill, after it without a forwards one).
    fn progress(&self, now: Instant) -> Option<f32> {
        let animation = &self.animation;
        let time = self.local_time(now);
        let active = self.active();
        let (iteration, within) = if time < 0.0 {
            if !animation.fill_backwards {
                return None;
            }
            (0.0, 0.0)
        } else if time >= active {
            if !animation.fill_forwards {
                return None;
            }
            let iterations = animation.iterations;
            if iterations.fract() == 0.0 && iterations > 0.0 {
                (iterations - 1.0, 1.0)
            } else {
                (iterations.floor(), iterations.fract())
            }
        } else {
            let at = time / animation.duration;
            (at.floor(), at.fract())
        };
        let odd = iteration % 2.0 == 1.0;
        let reversed = match animation.direction {
            AnimationDirection::Normal => false,
            AnimationDirection::Reverse => true,
            AnimationDirection::Alternate => odd,
            AnimationDirection::AlternateReverse => !odd,
        };
        Some(if reversed { 1.0 - within } else { within })
    }
}

impl KeyTrack {
    /// Its value at `progress`, easing each stretch between two keyframes.
    fn value(&self, progress: f32, easing: CssEasing) -> Option<Value> {
        let next = self
            .stops
            .iter()
            .position(|stop| stop.offset > progress)
            .unwrap_or(self.stops.len() - 1)
            .max(1);
        let (from, to) = (self.stops[next - 1], self.stops[next]);
        let span = to.offset - from.offset;
        if span <= 0.0 {
            return Some(if progress >= to.offset {
                to.value
            } else {
                from.value
            });
        }
        let local = ((progress - from.offset) / span).clamp(0.0, 1.0);
        let eased = from.easing.unwrap_or(easing).value(local);
        mix(from.value, to.value, eased)
    }
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
            keyframed: Vec::new(),
            keyframes_of: None,
            clock: None,
        }
    }

    /// Animates `properties` (longhands or shorthands) of the element with CSS id `id`, from
    /// what `from` presents now toward what `to` computes. Keep `to` as your tree. A newer
    /// request starts from the current presented value.
    ///
    /// Returns the animations started. Nothing starts for a property that did not change, is
    /// not animatable or not yet supported here, is won by `!important` in `to`, or has no
    /// in-between (`auto`; a `font-size` of 0 at either end; a transparent border color on a side that draws nothing, or on a
    /// collapsed cell's side that gives way along all of its edge): those take `to`'s value
    /// at once. A border color on a side with no width still starts while a width animates
    /// there, or where an interaction styling could give it one: a `:hover`, `:active` or
    /// `:focus` rule that could select the element, an ancestor or an earlier sibling of
    /// either, whatever it declares.
    ///
    /// # Errors
    /// Returns an unknown property name or a refused timing. Nothing changes then, except
    /// that time moves on to `now` once the timing is accepted.
    pub fn request(
        &mut self,
        from: &BoxTree,
        to: &BoxTree,
        id: &str,
        properties: &[&str],
        transition: impl Into<Transition>,
        now: Instant,
    ) -> Result<usize, BoxTreeError> {
        // One name per `:before` or `:after`, so either spelling continues the other's.
        let canonical = id
            .strip_suffix(":before")
            .filter(|base| !base.ends_with(':'))
            .map(|base| format!("{base}::before"))
            .or_else(|| {
                id.strip_suffix(":after")
                    .filter(|base| !base.ends_with(':'))
                    .map(|base| format!("{base}::after"))
            });
        let id = canonical.as_deref().unwrap_or(id);
        let Transition { timing, easing } = transition.into();
        let mut longhands = Vec::new();
        for name in properties {
            let property = PropertyId::lookup(name)
                .ok_or_else(|| BoxTreeError(format!("unknown property {name}")))?;
            longhands.extend_from_slice(property.longhands());
        }
        self.start(from, to, id, longhands, timing, Ease::Iced(easing), now)
    }

    /// Starts `longhands` of the element `id` addresses; see [`Self::request`].
    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        from: &BoxTree,
        to: &BoxTree,
        id: &str,
        mut longhands: Vec<PropertyId>,
        timing: AnimationTiming,
        easing: Ease,
        now: Instant,
    ) -> Result<usize, BoxTreeError> {
        let mut seen = Vec::new();
        longhands.retain(|longhand| {
            let new = !seen.contains(longhand);
            seen.push(*longhand);
            new
        });
        // Refused before anything moves: time, sources or tracks.
        timing.accept(now).map_err(failed)?;
        self.engine.advance(now);
        let capture = to.source.capture;
        if from.source.capture != capture {
            return Ok(0);
        }
        let (Some((before, from_key)), Some((after, to_key))) =
            (from.target_of(id), to.target_of(id))
        else {
            return Ok(0);
        };
        let generated = to_key.is_some();
        let mut started = Vec::new();
        let mut dropped = Vec::new();
        for property in longhands {
            let Some(field) = field(property.descriptor().name) else {
                continue;
            };
            if !matches!(
                property.descriptor().animation,
                Some(
                    AnimationRule::ByComputedValue
                        | AnimationRule::Visibility
                        | AnimationRule::FontWeight
                )
            ) {
                continue;
            }
            let (Some(committed), Some(target)) = (
                from.read_any(before, from_key, field),
                to.read_any(after, to_key, field),
            ) else {
                dropped.push(property);
                continue;
            };
            // Section 13: an important target holds without a temporary override, a
            // running one toward the same value included.
            if !generated && to.important[after].contains(&property) {
                dropped.push(property);
                continue;
            }
            if committed == target {
                continue;
            }
            // A side with no width at either end draws no color to move: at once. A width
            // still animating there draws one (a finished one is gone), and so can one an
            // interaction styling gives, wherever a `:hover`, `:active` or `:focus` rule
            // could restyle it.
            if let Field::BorderColor(side) = field
                && !generated
                && from.side_width(before, side) <= 0.0
                && to.side_width(after, side) <= 0.0
                && !field_name(Field::BorderWidth(side))
                    .and_then(PropertyId::lookup)
                    .and_then(|width| self.current(id, width))
                    .is_some_and(|(track, _)| track.capture == capture)
                && !to.source.interaction_reaches(to.originals[after])
            {
                dropped.push(property);
                continue;
            }
            // Between a box and an inline, `display` changes at once and the color moves in
            // the new one: it presents as `to` paints it, a box's or on its line.
            // Section 13: an important target holds without a temporary override.
            // A `:before` or `:after`'s padding across, with a bar at one end only: a border
            // that shows no color has none, and its width reads as padding.
            if let (Some(one), Some(two), Field::Padding(side @ (1 | 3))) =
                (from_key, to_key, field)
                && from.generated_bar(one, side).is_some() != to.generated_bar(two, side).is_some()
            {
                dropped.push(property);
                continue;
            }
            // A `font-size` of 0 at either end leaves its text no share of it to keep, so
            // nothing would move in between: at once.
            let zero_size = matches!(
                (field, committed, target),
                (Field::FontSize, Value::Length(Dim::Px(a)), Value::Length(Dim::Px(b)))
                    if a <= 0.0 || b <= 0.0
            );
            if zero_size || mix(committed, target, 0.0).is_none() {
                dropped.push(property);
                continue;
            }
            let start = self
                .current(id, property)
                .filter(|(track, _)| track.target == committed && track.capture == capture)
                .map_or(committed, |(_, value)| value);
            started.push(Track {
                id: id.to_owned(),
                property,
                field,
                start,
                target,
                followers: match field {
                    // A `:before` or `:after`'s borders that show its color at both ends:
                    // its currentColor ones.
                    Field::Color if generated => match from_key {
                        Some(one) => Followers {
                            boxes: vec![Follower {
                                node: one,
                                // The other end is checked as it presents: what still shows
                                // the target moves.
                                borders: std::array::from_fn(|side| {
                                    from.read_generated(one, Field::BorderColor(side))
                                        == Some(committed)
                                }),
                                outline: false,
                                marker: false,
                            }],
                            ..Followers::default()
                        },
                        None => Followers::default(),
                    },
                    _ if generated => Followers::default(),
                    Field::Color => from.followers(to, after, committed, target),
                    Field::FontWeight => from.weight_followers(to, after, committed),
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
        let registered = !self.sources.contains_key(id);
        if registered {
            let source = self.engine.register_source().map_err(failed)?;
            self.sources.insert(id.to_owned(), source);
        }
        let source = &self.sources[id];
        let work = NonZeroUsize::new(100_000).expect("positive");
        if !started.is_empty() {
            let chosen: Vec<PropertyId> = started.iter().map(|track| track.property).collect();
            let begun = AnimationSelection::new(&chosen, work)
                .map_err(failed)
                .and_then(|selection| {
                    self.engine
                        .start_progress(source, &selection, timing, now)
                        .map_err(failed)
                });
            if let Err(error) = begun {
                // A source registered for this request holds nothing: let it go again.
                if registered && let Some(source) = self.sources.remove(id) {
                    let _ = self.engine.remove_source(&source);
                }
                return Err(error);
            }
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
        // Only dropped properties, and none left running: the source holds nothing.
        if !self.tracks.iter().any(|track| track.id == id)
            && let Some(source) = self.sources.remove(id)
        {
            let _ = self.engine.remove_source(&source);
        }
        Ok(count)
    }

    /// CSS Animations 1: runs the animations `tree`'s `animation-*` properties name with its
    /// `@keyframes`. Call it with each new tree you keep, as with `advance`: an animation
    /// whose name stays at its place in an element's list keeps running, a new one starts at
    /// `now`, one no longer named stops, and `animation-play-state` pauses and resumes. The
    /// widget does this for the trees it draws by itself; call it to draw a
    /// [`Self::presented`] tree elsewhere.
    ///
    /// Returns the animations started. Keyframe values that hold `var()`, and properties
    /// that cannot move between values (as `display`), are left out; `:before` and
    /// `:after` are not animated.
    pub fn play(&mut self, tree: &BoxTree, now: Instant) -> usize {
        self.clock = Some(now);
        let capture = tree.source.capture;
        let nodes = tree.source.construction.nodes();
        let mut listed: Vec<(String, usize, String)> = Vec::new();
        let mut started = 0;
        for (projected, animations) in tree.animations.iter().enumerate() {
            if animations.is_empty() {
                continue;
            }
            let id = nodes[tree.originals[projected]]
                .metadata()
                .css_id()
                .filter(|id| tree.element(id) == Some(projected))
                .map_or_else(|| format!("{UNNAMED}{projected}"), str::to_owned);
            for (index, animation) in animations.iter().enumerate() {
                listed.push((id.clone(), index, animation.name.clone()));
                match self.keyframed.iter_mut().find(|running| {
                    running.capture == capture
                        && running.id == id
                        && running.index == index
                        && running.animation.name == animation.name
                }) {
                    Some(running) => {
                        match (running.paused_at, animation.paused) {
                            (None, true) => running.paused_at = Some(now),
                            (Some(at), false) => {
                                running.paused_for += now.saturating_duration_since(at);
                                running.paused_at = None;
                            }
                            _ => {}
                        }
                        running.animation = animation.clone();
                        running.projected = projected;
                    }
                    None => {
                        started += 1;
                        self.keyframed.push(Keyframed {
                            capture,
                            id: id.clone(),
                            projected,
                            index,
                            animation: animation.clone(),
                            started: now,
                            paused_at: animation.paused.then_some(now),
                            paused_for: Duration::ZERO,
                            tracks: Vec::new(),
                        });
                        // Its keyframes are read below, with the others.
                        self.keyframes_of = None;
                    }
                }
            }
        }
        self.keyframed.retain(|running| {
            running.capture == capture
                && listed.iter().any(|(id, index, name)| {
                    *id == running.id && *index == running.index && *name == running.animation.name
                })
        });
        let styling = (Arc::as_ptr(&tree.source) as usize, tree.key);
        if self.keyframes_of != Some(styling) {
            self.keyframes_of = Some(styling);
            self.read_keyframes(tree);
        }
        started
    }

    /// Reads every animation's keyframe values from `tree` styled with each keyframe laid
    /// over it: one styling per keyframe place in the longest rule.
    fn read_keyframes(&mut self, tree: &BoxTree) {
        let rules: Vec<Option<Vec<crate::box_tree::KeyframeSummary>>> = self
            .keyframed
            .iter()
            .map(|running| tree.keyframe_rule(&running.animation.name))
            .collect();
        let frames = rules.iter().flatten().map(Vec::len).max().unwrap_or(0);
        let mut layers: Vec<Option<BoxTree>> = Vec::with_capacity(frames);
        for frame in 0..frames {
            let layer: Vec<(usize, String, usize)> = self
                .keyframed
                .iter()
                .zip(&rules)
                .filter(|(_, rule)| rule.as_ref().is_some_and(|rule| frame < rule.len()))
                .map(|(running, _)| {
                    (
                        tree.originals[running.projected],
                        running.animation.name.clone(),
                        frame,
                    )
                })
                .collect();
            layers.push(tree.keyframe_layer(&layer));
        }
        let animatable = |property: PropertyId| {
            matches!(
                property.descriptor().animation,
                Some(
                    AnimationRule::ByComputedValue
                        | AnimationRule::Visibility
                        | AnimationRule::FontWeight
                )
            )
        };
        for (running, rule) in self.keyframed.iter_mut().zip(&rules) {
            running.tracks.clear();
            let Some(rule) = rule else { continue };
            let projected = running.projected;
            let mut properties: Vec<PropertyId> = Vec::new();
            for frame in rule {
                for longhand in &frame.longhands {
                    if !properties.contains(longhand) && animatable(*longhand) {
                        properties.push(*longhand);
                    }
                }
            }
            for property in properties {
                let Some(field) = field(property.descriptor().name) else {
                    continue;
                };
                let Some(base) = tree.read(projected, field) else {
                    continue;
                };
                let mut stops: Vec<Stop> = Vec::new();
                for (frame, summary) in rule.iter().enumerate() {
                    if !summary.longhands.contains(&property) {
                        continue;
                    }
                    let Some(value) = layers[frame]
                        .as_ref()
                        .and_then(|layer| layer.read(projected, field))
                    else {
                        continue;
                    };
                    for offset in &summary.offsets {
                        // A later keyframe at the same place wins it.
                        stops.retain(|stop| stop.offset != *offset);
                        stops.push(Stop {
                            offset: *offset,
                            value,
                            easing: summary.easing,
                        });
                    }
                }
                stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
                // 3: where no keyframe gives 0% or 100%, the element's own value stands.
                if stops.first().is_none_or(|stop| stop.offset > 0.0) {
                    stops.insert(
                        0,
                        Stop {
                            offset: 0.0,
                            value: base,
                            easing: None,
                        },
                    );
                }
                if stops.last().is_some_and(|stop| stop.offset < 1.0) {
                    stops.push(Stop {
                        offset: 1.0,
                        value: base,
                        easing: None,
                    });
                }
                // Two values with nothing between them do not animate.
                if stops
                    .windows(2)
                    .any(|pair| mix(pair[0].value, pair[1].value, 0.5).is_none())
                {
                    continue;
                }
                running.tracks.push(KeyTrack {
                    property,
                    field,
                    stops,
                });
            }
        }
    }

    /// CSS Transitions 1: starts what `to`'s `transition-*` properties ask for, on every
    /// element whose values there differ from what `from` presents. Keep `to` as your
    /// tree, as after [`Self::request`]; `from` is the tree it replaces, restyled from the
    /// same capture (another capture starts nothing). Each transition runs as a request
    /// would, with the same exceptions; an easing that overshoots is cut off at the ends,
    /// and a negative delay starts that far in.
    ///
    /// Returns the animations started.
    ///
    /// # Errors
    /// Returns a refused timing; transitions already started stay.
    pub fn transition(
        &mut self,
        from: &BoxTree,
        to: &BoxTree,
        now: Instant,
    ) -> Result<usize, BoxTreeError> {
        if from.source.capture != to.source.capture {
            return Ok(0);
        }
        let nodes = to.source.construction.nodes();
        let mut started = 0;
        for (projected, transitions) in to.transitions.iter().enumerate() {
            if transitions.is_empty() || projected >= from.originals.len() {
                continue;
            }
            // Its id, when that finds it; otherwise its place.
            let id = nodes[to.originals[projected]]
                .metadata()
                .css_id()
                .filter(|id| to.element(id) == Some(projected))
                .map_or_else(|| format!("{UNNAMED}{projected}"), str::to_owned);
            // A later transition naming a longhand wins it.
            let mut chosen: Vec<(PropertyId, usize)> = Vec::new();
            for (at, transition) in transitions.iter().enumerate() {
                let longhands: Vec<PropertyId> = match &transition.properties {
                    Some(longhands) => longhands.clone(),
                    None => PropertyId::all()
                        .filter(|property| {
                            !property.descriptor().shorthand
                                && field(property.descriptor().name).is_some()
                        })
                        .collect(),
                };
                for longhand in longhands {
                    match chosen.iter_mut().find(|(held, _)| *held == longhand) {
                        Some(entry) => entry.1 = at,
                        None => chosen.push((longhand, at)),
                    }
                }
            }
            for (at, transition) in transitions.iter().enumerate() {
                let longhands: Vec<PropertyId> = chosen
                    .iter()
                    .filter(|(_, index)| *index == at)
                    .map(|(longhand, _)| *longhand)
                    .collect();
                if longhands.is_empty() {
                    continue;
                }
                let millis =
                    |value: f32| Duration::from_secs_f64(f64::from(value.max(0.0)) / 1000.0);
                // A negative delay has used up part of the duration already.
                let skipped = (-transition.delay).max(0.0);
                if skipped >= transition.duration {
                    continue;
                }
                let timing = AnimationTiming::new(
                    millis(transition.duration - skipped),
                    millis(transition.delay),
                );
                let easing = Ease::Css {
                    easing: transition.easing,
                    from: skipped / transition.duration,
                };
                started += self.start(from, to, &id, longhands, timing, easing, now)?;
            }
        }
        Ok(started)
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
        self.clock = Some(now);
        self.engine.advance(now);
        let (running, done): (Vec<Track>, Vec<Track>) = std::mem::take(&mut self.tracks)
            .into_iter()
            .partition(|track| {
                self.progress(track).is_some()
                    && track.capture == tree.source.capture
                    && tree.target_of(&track.id).is_some_and(|(element, key)| {
                        // Section 13: a value that became important holds at once.
                        !(key.is_none() && tree.important[element].contains(&track.property))
                            && tree.read_any(element, key, track.field) == Some(track.target)
                    })
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
            || self.clock.is_some_and(|now| {
                self.keyframed
                    .iter()
                    .any(|running| !running.tracks.is_empty() && running.running(now))
            })
    }

    /// Whether anything shows over a tree: a running track, or a keyframe animation with a
    /// value now (one that ended or paused may hold one).
    pub(crate) fn presents(&self) -> bool {
        !self.tracks.is_empty()
            || self.clock.is_some_and(|now| {
                self.keyframed
                    .iter()
                    .any(|running| !running.tracks.is_empty() && running.progress(now).is_some())
            })
    }

    /// Whether any keyframe animation is held, running or not.
    pub(crate) fn has_keyframes(&self) -> bool {
        !self.keyframed.is_empty()
    }

    /// `tree` with the current presented values over it, for drawing.
    #[must_use]
    pub fn presented(&self, tree: &BoxTree) -> BoxTree {
        self.presented_over(tree, None)
    }

    /// `tree` with `under` (another set's presented values), then these, over it.
    pub(super) fn presented_over(
        &self,
        tree: &BoxTree,
        under: Option<Arc<Vec<Presented>>>,
    ) -> BoxTree {
        let mut presented = tree.clone();
        let mut list: Vec<Presented> = under.map(|under| (*under).clone()).unwrap_or_default();
        // Keyframe animations first: a transition of the same value lies over them.
        if let Some(now) = self.clock {
            for running in &self.keyframed {
                if running.capture != tree.source.capture {
                    continue;
                }
                let Some(progress) = running.progress(now) else {
                    continue;
                };
                for track in &running.tracks {
                    let (Some(value), Some(target)) = (
                        track.value(progress, running.animation.easing),
                        tree.read(running.projected, track.field),
                    ) else {
                        continue;
                    };
                    let projected = running.projected;
                    list.push(Presented {
                        id: running.id.clone(),
                        property: track.property,
                        field: track.field,
                        target,
                        now: value,
                        followers: match track.field {
                            Field::Color => tree.followers(tree, projected, target, target),
                            Field::FontWeight => tree.weight_followers(tree, projected, target),
                            Field::FontSize => match target {
                                Value::Length(Dim::Px(size)) => {
                                    tree.size_followers(tree, projected, size, size)
                                }
                                _ => Followers::default(),
                            },
                            Field::Visibility => match target {
                                Value::Visible(shown) => {
                                    tree.visibility_followers(tree, projected, shown, shown)
                                }
                                _ => Followers::default(),
                            },
                            _ => Followers::default(),
                        },
                    });
                }
            }
        }
        list.extend(
            self.tracks
                .iter()
                .filter(|track| track.capture == tree.source.capture)
                .filter_map(|track| {
                    let now = mix(track.start, track.target, self.progress(track)?)?;
                    Some(Presented {
                        id: track.id.clone(),
                        property: track.property,
                        field: track.field,
                        target: track.target,
                        now,
                        followers: track.followers.clone(),
                    })
                }),
        );
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

/// What shows beneath inline fades as they stack: the inline colors presented under the
/// text, the inline boxes fading, and each leaf's block base.
struct Beneath {
    presented: BTreeMap<usize, [f32; 3]>,
    fading: BTreeSet<usize>,
    blocks: Vec<Option<[f32; 3]>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{CapturedElement, adapters};
    use iced::Theme;
    use iced_css_core::source::AttributeLimits;
    use std::time::Duration;

    fn trees(from: &str, to: &str) -> (BoxTree, BoxTree) {
        type Node = CapturedElement<'static, (), Theme, iced::Renderer>;
        let bound = NonZeroUsize::new(1000).expect("positive");
        let limits = AttributeLimits {
            entries: bound,
            retained_bytes: bound,
            work: bound,
        };
        let root: Node = adapters::text("words", super::super::capture_limits())
            .unwrap()
            .style_id("box", limits)
            .unwrap()
            .erase();
        let from = BoxTree::new(root, from).unwrap();
        let to = from.restyled(&[to]).unwrap();
        (from, to)
    }

    #[test]
    fn a_refused_timing_leaves_no_source() {
        let (from, to) = trees("#box { color:#000000; }", "#box { color:#ffffff; }");
        let mut animations = BoxTreeAnimations::new();
        let refused = AnimationTiming::new(Duration::MAX, Duration::ZERO);
        let now = Instant::now();
        assert!(
            animations
                .request(&from, &to, "box", &["color"], refused, now)
                .is_err()
        );
        assert!(animations.sources.is_empty());
        assert_eq!(animations.engine.live_source_count(), 0);
    }

    #[test]
    fn a_request_that_only_drops_lets_its_source_go() {
        let (from, to) = trees("#box { font-size:16px; }", "#box { font-size:32px; }");
        let zero = to.restyled(&["#box { font-size:0px; }"]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let second = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
        let now = Instant::now();
        let count = animations
            .request(&from, &to, "box", &["font-size"], second, now)
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(animations.engine.live_source_count(), 1);
        // A size of 0 applies at once: the running one is dropped and nothing is left.
        let count = animations
            .request(&to, &zero, "box", &["font-size"], second, now)
            .unwrap();
        assert_eq!(count, 0);
        assert!(animations.tracks.is_empty());
        assert!(animations.sources.is_empty());
        assert_eq!(animations.engine.live_source_count(), 0);
    }
}
