#![allow(dead_code)]

use iced_test::core::renderer::Headless;
use iced_test::core::theme::Base;
use iced_test::core::widget::operation::{Focusable, Operation, Scrollable, TextInput};
use iced_test::core::widget::Id;
use iced_test::core::{clipboard, mouse, time, window, Event, Point, Rectangle, Size, Vector};
use iced_test::runtime::user_interface::Cache;
use iced_test::runtime::UserInterface;

#[allow(unused_imports)]
pub mod prelude {
    pub use super::{
        El, Msg, Sim, assert_equivalent, assert_path, click_at, mount, mount_at, resize,
    };
    pub use iced::widget::{button, container, text};
    pub use iced_css::StyleClasses;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Css(iced_css::Event),
    Pressed,
    Noop,
}

impl From<iced_css::Event> for Msg {
    fn from(event: iced_css::Event) -> Self {
        Msg::Css(event)
    }
}

pub type El = iced::Element<'static, Msg>;

pub const WINDOW_WIDTH: f32 = 800.0;
pub const WINDOW_HEIGHT: f32 = 600.0;

/// loose strict for text to avoid rounding bug. Chage that val or another approach?
const EPSILON: f32 = 0.01;

#[derive(Debug, Clone, PartialEq)]
struct Node {
    depth: usize,
    kind: &'static str,
    content: Option<String>,
    bounds: Rectangle,
}

type Ui = UserInterface<'static, Msg, iced::Theme, iced::Renderer>;

pub struct Sim {
    ui: Option<Ui>,
    renderer: iced::Renderer,
    size: Size,
    messages: Vec<Msg>,
}

pub fn mount(view: fn() -> El) -> Sim {
    mount_at(view, WINDOW_WIDTH, WINDOW_HEIGHT)
}

pub fn mount_at(view: fn() -> El, width: f32, height: f32) -> Sim {
    let settings = iced_test::core::Settings::default();
    let backend = std::env::var("ICED_TEST_BACKEND").ok();
    let mut renderer = iced_test::futures::futures::executor::block_on(
        <iced::Renderer as Headless>::new(
            iced::Font::with_name("Fira Sans"),
            settings.default_text_size,
            backend.as_deref(),
        ),
    )
    .expect("create headless renderer");

    let size = Size::new(width, height);
    let ui = UserInterface::build(view(), size, Cache::default(), &mut renderer);

    Sim {
        ui: Some(ui),
        renderer,
        size,
        messages: Vec::new(),
    }
}

pub fn resize(sim: &mut Sim, width: f32, height: f32) {
    let ui = sim.ui.take().expect("mounted");
    sim.size = Size::new(width, height);
    sim.ui = Some(ui.relayout(sim.size, &mut sim.renderer));
}

pub fn click_at(sim: &mut Sim, x: f32, y: f32) -> Vec<Msg> {
    let cursor = mouse::Cursor::Available(Point::new(x, y));
    let events: Vec<Event> = iced_test::simulator::click().collect();
    let ui = sim.ui.as_mut().expect("mounted");
    let _ = ui.update(
        &events,
        cursor,
        &mut sim.renderer,
        &mut clipboard::Null,
        &mut sim.messages,
    );
    std::mem::take(&mut sim.messages)
}

pub fn assert_equivalent(actual: &mut Sim, expected: &mut Sim) {
    let actual_nodes = collect(actual);
    let expected_nodes = collect(expected);

    match (actual_nodes.first(), expected_nodes.first()) {
        (Some(a), Some(e)) if !matches(a, e) => panic!(
            "root boxes differ:\n  actual: {a:?}\n  expected: {e:?}\n  \
             actual tree: {actual_nodes:#?}\n  expected tree: {expected_nodes:#?}"
        ),
        _ => {}
    }

    let matched = embed(&actual_nodes, &expected_nodes).unwrap_or_else(|missing| {
        panic!(
            "expected widget not found (in place) in actual tree:\n  missing: {missing:?}\n  \
             actual tree: {actual_nodes:#?}\n  expected tree: {expected_nodes:#?}"
        )
    });

    for (index, node) in actual_nodes.iter().enumerate() {
        if node.kind == "text" && !matched.contains(&index) {
            panic!(
                "actual tree displays text the expected tree does not:\n  extra: {node:?}\n  \
                 actual tree: {actual_nodes:#?}\n  expected tree: {expected_nodes:#?}"
            );
        }
    }

    assert!(
        render(actual) == render(expected),
        "trees lay out identically but render differently\n  \
         actual tree: {actual_nodes:#?}\n  expected tree: {expected_nodes:#?}"
    );
}

pub fn assert_path(sim: &mut Sim, path: &[(&'static str, Rectangle)]) {
    let nodes = collect(sim);
    let mut range = 0..nodes.len();

    for (kind, bounds) in path {
        let found = range
            .clone()
            .find(|&i| nodes[i].kind == *kind && bounds_match(nodes[i].bounds, *bounds));
        let Some(index) = found else {
            panic!("no {kind} node with bounds {bounds:?} nested at this point of the path\n  tree: {nodes:#?}");
        };
        range = (index + 1)..subtree_end(&nodes, index);
    }
}

fn subtree_end(nodes: &[Node], index: usize) -> usize {
    let depth = nodes[index].depth;
    (index + 1..nodes.len())
        .find(|&i| nodes[i].depth <= depth)
        .unwrap_or(nodes.len())
}

fn embed(actual: &[Node], expected: &[Node]) -> Result<Vec<usize>, Node> {
    fn place(
        actual: &[Node],
        expected: &[Node],
        placed: &mut Vec<usize>,
        furthest: &mut usize,
    ) -> bool {
        let i = placed.len();
        if i == expected.len() {
            return true;
        }
        let node = &expected[i];

        let parent = (0..i).rev().find(|&j| expected[j].depth < node.depth);
        let (start, end) = match parent {
            Some(p) => (placed[p] + 1, subtree_end(actual, placed[p])),
            None => (0, actual.len()),
        };
        let start = start.max(placed.last().map_or(0, |&last| last + 1));

        for k in start..end {
            if !matches(&actual[k], node) {
                continue;
            }
            placed.push(k);
            *furthest = (*furthest).max(placed.len());
            if place(actual, expected, placed, furthest) {
                return true;
            }
            placed.pop();
        }
        false
    }

    let mut placed = Vec::with_capacity(expected.len());
    let mut furthest = 0;
    if place(actual, expected, &mut placed, &mut furthest) {
        Ok(placed)
    } else {
        Err(expected[furthest.min(expected.len() - 1)].clone())
    }
}

fn bounds_match(a: Rectangle, b: Rectangle) -> bool {
    (a.x - b.x).abs() <= EPSILON
        && (a.y - b.y).abs() <= EPSILON
        && (a.width - b.width).abs() <= EPSILON
        && (a.height - b.height).abs() <= EPSILON
}

fn matches(actual: &Node, expected: &Node) -> bool {
    actual.kind == expected.kind
        && actual.content == expected.content
        && bounds_match(actual.bounds, expected.bounds)
}

/// Records every reported widget with its nesting depth.
struct Recorder {
    depth: usize,
    nodes: Vec<Node>,
}

impl Recorder {
    fn push(&mut self, kind: &'static str, bounds: Rectangle, content: Option<String>) {
        self.nodes.push(Node {
            depth: self.depth,
            kind,
            content,
            bounds,
        });
    }
}

impl Operation for Recorder {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        self.depth += 1;
        operate(self);
        self.depth -= 1;
    }
    fn container(&mut self, _id: Option<&Id>, bounds: Rectangle) {
        self.push("container", bounds, None);
    }
    fn scrollable(
        &mut self,
        _id: Option<&Id>,
        bounds: Rectangle,
        _content_bounds: Rectangle,
        _translation: Vector,
        _state: &mut dyn Scrollable,
    ) {
        self.push("scrollable", bounds, None);
    }
    fn focusable(&mut self, _id: Option<&Id>, bounds: Rectangle, _state: &mut dyn Focusable) {
        self.push("focusable", bounds, None);
    }
    fn text_input(&mut self, _id: Option<&Id>, bounds: Rectangle, _state: &mut dyn TextInput) {
        self.push("text_input", bounds, None);
    }
    fn text(&mut self, _id: Option<&Id>, bounds: Rectangle, text: &str) {
        self.push("text", bounds, Some(text.to_string()));
    }
    fn custom(&mut self, _id: Option<&Id>, bounds: Rectangle, _state: &mut dyn std::any::Any) {
        self.push("custom", bounds, None);
    }
}

fn collect(sim: &mut Sim) -> Vec<Node> {
    let mut recorder = Recorder {
        depth: 0,
        nodes: Vec::new(),
    };
    sim.ui
        .as_mut()
        .expect("mounted")
        .operate(&sim.renderer, &mut recorder);
    recorder.nodes
}

fn render(sim: &mut Sim) -> Vec<u8> {
    let theme = iced::Theme::Light;
    let base = theme.base();
    let cursor = mouse::Cursor::Unavailable;
    let ui = sim.ui.as_mut().expect("mounted");

    let _ = ui.update(
        &[Event::Window(window::Event::RedrawRequested(
            time::Instant::now(),
        ))],
        cursor,
        &mut sim.renderer,
        &mut clipboard::Null,
        &mut sim.messages,
    );
    let _ = ui.draw(
        &mut sim.renderer,
        &theme,
        &iced_test::core::renderer::Style {
            text_color: base.text_color,
        },
        cursor,
    );

    let scale_factor = 2.0;
    let physical = Size::new(
        (sim.size.width * scale_factor).round() as u32,
        (sim.size.height * scale_factor).round() as u32,
    );
    sim.renderer
        .screenshot(physical, scale_factor, base.background_color)
}

/// INPUT tree (with style_classes) styled by the inline CSS must be
/// equivalent to the hand-written EXPECTED plain iced tree the macro must
/// effectively produce.
#[macro_export]
macro_rules! css_test {
    ($name:ident, $css:literal, $input:expr, $expected:expr $(,)?) => {
        #[test]
        fn $name() {
            #[iced_css::style(inline = $css, policy = Compile)]
            fn input() -> El {
                iced::Element::from($input)
            }
            fn expected() -> El {
                iced::Element::from($expected)
            }
            assert_equivalent(&mut mount(input), &mut mount(expected));
        }
    };
}

/// unimplemented ignored
#[macro_export]
macro_rules! css_todo_test {
    ($name:ident, $why:literal) => {
        #[test]
        #[ignore = $why]
        fn $name() {
            // TODO, figure out later
        }
    };
}
