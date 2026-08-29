#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use iced_test::selector::Candidate;
use iced_test::Simulator;

#[allow(unused_imports)]
pub mod prelude {
    pub use super::{El, Msg, Sim, assert_equivalent, mount};
    pub use iced::widget::{button, container, text};
    pub use iced_css::StyleClasses;
}

#[derive(Debug, Clone)]
pub enum Msg {
    Css(iced_css::Event),
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

#[derive(Debug, Clone, PartialEq)]
struct Node {
    kind: &'static str,
    content: Option<String>,
    bounds: iced::Rectangle,
}

pub struct Sim {
    sim: Simulator<'static, Msg, iced::Theme, iced::Renderer>,
}

pub fn mount(view: fn() -> El) -> Sim {
    Sim {
        sim: Simulator::with_size(
            iced_test::core::Settings::default(),
            iced::Size::new(WINDOW_WIDTH, WINDOW_HEIGHT),
            view(),
        ),
    }
}

pub fn assert_equivalent(mut actual: Sim, mut expected: Sim) {
    let actual_nodes = collect(&mut actual.sim);
    let expected_nodes = collect(&mut expected.sim);

    let mut unmatched = actual_nodes.clone();
    for node in &expected_nodes {
        match unmatched.iter().position(|candidate| matches(candidate, node)) {
            Some(index) => {
                let _ = unmatched.remove(index);
            }
            None => panic!(
                "expected widget not found in actual tree:\n  missing: {node:?}\n  \
                 actual tree: {actual_nodes:#?}\n  expected tree: {expected_nodes:#?}"
            ),
        }
    }

    for node in &unmatched {
        if node.kind == "text" {
            panic!(
                "actual tree displays text the expected tree does not:\n  extra: {node:?}\n  \
                 actual tree: {actual_nodes:#?}\n  expected tree: {expected_nodes:#?}"
            );
        }
    }

    let theme = iced::Theme::Light;
    let actual_snapshot = actual.sim.snapshot(&theme).expect("render actual tree");
    let expected_snapshot = expected.sim.snapshot(&theme).expect("render expected tree");

    static UNIQUE: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "iced-css-equiv-{}-{}",
        std::process::id(),
        UNIQUE.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&dir).expect("create snapshot scratch dir");
    let path = dir.join("frame");

    let first = actual_snapshot.matches_hash(&path).expect("hash actual");
    assert!(first, "first matches_hash call should write the reference");
    let identical = expected_snapshot.matches_hash(&path).expect("hash expected");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        identical,
        "trees lay out identically but render differently\n  \
         actual tree: {actual_nodes:#?}\n  expected tree: {expected_nodes:#?}"
    );
}

fn matches(actual: &Node, expected: &Node) -> bool {
    const EPSILON: f32 = 0.5;

    actual.kind == expected.kind
        && actual.content == expected.content
        && (actual.bounds.x - expected.bounds.x).abs() <= EPSILON
        && (actual.bounds.y - expected.bounds.y).abs() <= EPSILON
        && (actual.bounds.width - expected.bounds.width).abs() <= EPSILON
        && (actual.bounds.height - expected.bounds.height).abs() <= EPSILON
}

fn collect(sim: &mut Simulator<'static, Msg, iced::Theme, iced::Renderer>) -> Vec<Node> {
    let nodes = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&nodes);

    let observe = move |candidate: Candidate<'_>| -> Option<()> {
        let (kind, content) = match &candidate {
            Candidate::Container { .. } => ("container", None),
            Candidate::Focusable { .. } => ("focusable", None),
            Candidate::Scrollable { .. } => ("scrollable", None),
            Candidate::TextInput { .. } => ("text_input", None),
            Candidate::Text { content, .. } => ("text", Some(content.to_string())),
            Candidate::Custom { .. } => ("custom", None),
        };
        sink.lock().unwrap().push(Node {
            kind,
            content,
            bounds: candidate.bounds(),
        });
        None
    };

    let _not_found = sim.find(observe);

    let nodes = nodes.lock().unwrap().clone();
    nodes
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
            assert_equivalent(mount(input), mount(expected));
        }
    };
}

#[macro_export]
macro_rules! css_todo_test {
    ($name:ident, $why:literal) => {
        #[test]
        fn $name() {
            // TODO, figure out later
            panic!("TODO, figure out later: {}", $why);
        }
    };
}
