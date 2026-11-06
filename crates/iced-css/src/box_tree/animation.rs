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
    collections::BTreeMap,
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
}

fn rgba(value: Value) -> Option<([f32; 3], f32)> {
    match value {
        Value::Color(color) => Some((color, 1.0)),
        Value::Fade(color, alpha) => Some((color, alpha)),
        Value::Length(_) => None,
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

/// Between two values of one kind. `None` has no in-between: keywords, or a length and a
/// percentage, whose mix only layout could resolve.
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
        _ => None,
    }
}

/// What in a descendant, or the element itself, takes its color from an animated `color`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Follower {
    node: usize,
    borders: [bool; 4],
    outline: bool,
    marker: bool,
}

/// One presented value this frame. It holds only while the tree still computes `target`: any
/// other value is a newer style, and an unrequested style change applies at once.
/// Everything an animated `color` carries to, as of the request.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Followers {
    boxes: Vec<Follower>,
    /// Text runs by arena slot, place and owner, each showing the color at both ends. An
    /// interaction styling can shift slots, so the owner is checked again when presenting.
    runs: Vec<(usize, usize, usize)>,
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

    fn read(&self, projected: usize, field: Field) -> Option<Value> {
        // Display none shows nothing to come from or go to.
        self.prepared.slot[projected]?;
        let paint = &self.paints[projected];
        let color = |value: Option<[f32; 3]>| value.map(Value::Color);
        // An inline element's slot is its paragraph's box, which is not this element's.
        let own = self.prepared.slot[projected].filter(|slot| !self.prepared.anonymous[*slot]);
        let boxed =
            || own.and_then(|slot| self.prepared.boxes.get(slot).map(|value| (slot, value)));
        let length = |value: Dim| Some(Value::Length(value));
        match field {
            Field::Color => color(paint.color),
            Field::Background => own.map(|_| match (paint.background, paint.background_fade) {
                (None, _) => Value::Fade([0.0; 3], 0.0),
                (Some(value), None) => Value::Color(value),
                (Some(value), Some(alpha)) => Value::Fade(value, alpha),
            }),
            Field::BorderColor(side) => color(paint.border[side]),
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
        let paint = &mut self.paints[projected];
        match (field, value) {
            (Field::Color, Value::Color(value)) => paint.color = Some(value),
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
    fn within(&self, mut node: usize, ancestor: usize) -> bool {
        let nodes = self.source.construction.nodes();
        loop {
            if node == ancestor {
                return true;
            }
            match nodes[node].parent() {
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
        for presented in presentation.iter() {
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
                let old = self
                    .texts
                    .get(arena)
                    .and_then(Option::as_ref)
                    .and_then(|leaf| leaf.runs.get(at));
                if followers.boxes.iter().any(|f| f.node == run.owner)
                    && old.is_some_and(|old| old.owner == run.owner && same(old.color, run.color))
                {
                    followers.runs.push((arena, at, run.owner));
                }
            }
        }
        followers
    }
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
    /// not animatable or not yet supported here, is won by `!important` in `to`, or has no in-between (`auto`, a transparent
    /// border, a length and a percentage): those take `to`'s value at once.
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
            if property.descriptor().animation != Some(AnimationRule::ByComputedValue) {
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
            // Section 13: an important target holds without a temporary override.
            if mix(committed, target, 0.0).is_none() || to.important[after].contains(&property) {
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
                followers: if field == Field::Color {
                    from.followers(to, after, committed, target)
                } else {
                    Followers::default()
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
