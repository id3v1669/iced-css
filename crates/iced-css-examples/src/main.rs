//! Manual check: a page layout drawn from a stylesheet. Resize the window: percentage
//! widths, float avoidance and the absolute badge follow without rebuilding the tree.
use iced::{Element, Theme};
use iced_css::{
    animation::clock::AnimationTiming,
    box_tree::{BoxTree, BoxTreeAnimations, Transition, capture_limits},
    capture::{CapturedElement, adapters},
};
use iced_css_core::source::AttributeLimits;
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

type Node = CapturedElement<'static, (), Theme, iced::Renderer>;

const CSS: &str = "
    #page    { background-color:#f4f1ea; padding:16px; height:100%; }
    #sidebar { float:left; width:25%; height:320px; margin-right:16px;
               background-color:#2a4d69; }
    #header  { height:64px; margin-bottom:16px; background-color:#4b86b4;
               border-bottom:4px solid #2a4d69; }
    #content { overflow:hidden; padding:12px; background-color:#ffffff;
               border:2px solid #adcbe3; }
    #card    { height:96px; margin-bottom:12px; background-color:#e7eff6; }
    #card:hover  { background-color:#cfe0ee; }
    #card:active { background-color:#adcbe3; height:104px; }
    #action  { width:200px; margin-bottom:12px; }
    #wide    { height:48px; width:60%; margin:0 auto; background-color:#63ace5; }
    #title   { display:block; font-size:24px; color:#ffffff; padding:16px; }
    #copy    { display:block; color:#2a4d69; margin-bottom:12px; }
    #badge   { position:absolute; top:24px; right:24px; width:40px; height:40px;
               background-color:#d9534f; }
";

/// Pressing the card widens and recolors the bar below it, on request.
const OPEN: &str = "#wide { width:100%; height:72px; background-color:#d9534f; }";

fn node(id: &str, children: Vec<Node>) -> Node {
    let bound = NonZeroUsize::new(1000).expect("positive");
    adapters::column_from_vec(children, capture_limits())
        .expect("capture")
        .style_id(
            id,
            AttributeLimits {
                entries: bound,
                retained_bytes: bound,
                work: bound,
            },
        )
        .expect("style id")
        .erase()
}

fn text(id: &'static str, content: &'static str) -> Node {
    let bound = NonZeroUsize::new(1000).expect("positive");
    adapters::text(content, capture_limits())
        .expect("capture")
        .style_id(
            id,
            AttributeLimits {
                entries: bound,
                retained_bytes: bound,
                work: bound,
            },
        )
        .expect("style id")
        .erase()
}

fn tree() -> BoxTree {
    let root = node(
        "page",
        vec![
            node("header", vec![text("title", "A page from a stylesheet")]),
            node("sidebar", vec![]),
            node(
                "content",
                vec![
                    text(
                        "copy",
                        "Resize the window: this text wraps at its box's used width, \
                         and the boxes below move with its height. The sidebar is a \
                         float, and this panel is a new formatting context beside it.",
                    ),
                    node("action", vec![]),
                    node("card", vec![]),
                    node("wide", vec![]),
                ],
            ),
            node("badge", vec![]),
        ],
    );
    BoxTree::new(root, CSS).expect("stylesheet")
}

#[cfg(test)]
#[test]
fn hosted_button_and_box_press_work_on_the_real_renderer() {
    let mut app = App::new();
    app.button = 3;
    app.card = 1;
    let mut ui = iced_test::simulator(view(&app));
    ui.click("Button: 3  Card: 1")
        .expect("the hosted button's label is findable");
    // Well inside the card, below the two lines of text and the button.
    ui.point_at(iced::Point::new(600.0, 250.0));
    let _ = ui.simulate(iced_test::simulator::click());
    let messages: Vec<_> = ui.into_messages().collect();
    assert!(matches!(messages[..], [Message::Button, Message::Card]));
}

#[cfg(test)]
#[test]
fn the_example_stylesheet_is_accepted() {
    let tree = tree();
    let geometry = tree
        .layout_text(800.0, Some(600.0), &mut |_, _| 20.0)
        .unwrap();
    assert!(geometry.iter().all(Option::is_some));
}

#[cfg(test)]
#[test]
fn pressing_the_card_animates_and_finishes() {
    let mut app = App::new();
    update(&mut app, Message::Card);
    assert!(app.animations.is_animating());
    update(
        &mut app,
        Message::Frame(Instant::now() + Duration::from_secs(1)),
    );
    assert!(!app.animations.is_animating());
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Button,
    Card,
    Frame(Instant),
}

struct App {
    tree: BoxTree,
    animations: BoxTreeAnimations,
    button: u32,
    card: u32,
}

impl App {
    fn new() -> Self {
        Self {
            tree: tree(),
            animations: BoxTreeAnimations::new(),
            button: 0,
            card: 0,
        }
    }
}

fn update(app: &mut App, message: Message) {
    match message {
        Message::Button => app.button += 1,
        Message::Card => {
            app.card += 1;
            let sheets: &[&str] = if app.card % 2 == 1 {
                &[CSS, OPEN]
            } else {
                &[CSS]
            };
            let next = app.tree.restyled(sheets).expect("stylesheet");
            let transition = Transition {
                timing: AnimationTiming::new(Duration::from_millis(300), Duration::ZERO),
                easing: iced::animation::Easing::EaseInOut,
            };
            app.animations
                .request(
                    &app.tree,
                    &next,
                    "wide",
                    &["width", "height", "background-color"],
                    transition,
                    Instant::now(),
                )
                .expect("known properties");
            app.tree = next;
        }
        Message::Frame(now) => app.animations.advance(now, &app.tree),
    }
}

fn view(app: &App) -> Element<'_, Message> {
    let label = format!("Button: {}  Card: {}", app.button, app.card);
    app.animations
        .presented(&app.tree)
        .interactive()
        .widget(
            "action",
            iced::widget::button(iced::widget::text(label))
                .on_press(Message::Button)
                .width(iced::Length::Fill),
        )
        .on_press("card", Message::Card)
        .into()
}

fn main() -> iced::Result {
    iced::application(App::new, update, view)
        .subscription(|app: &App| app.animations.frames(Message::Frame))
        .title("iced-css box tree")
        .run()
}
