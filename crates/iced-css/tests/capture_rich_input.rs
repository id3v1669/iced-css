#[path = "support/rich_input_renderer.rs"]
mod renderer_fixture;
use iced::{
    Element, Event, Point, Rectangle, Size,
    advanced::{Layout, layout, widget::Tree},
    mouse,
    widget::text::{Rich, Span},
};
use iced_css::capture::{CaptureLimits, adapters};
use renderer_fixture::FixedRenderer;
use std::{cell::Cell, num::NonZeroUsize, rc::Rc};
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
struct Link {
    id: &'static str,
    clones: Rc<Cell<usize>>,
}
impl Clone for Link {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
// Message intentionally has no Clone implementation.
struct Message(String);
fn build(
    captured: bool,
    enabled: bool,
    clones: &Rc<Cell<usize>>,
    callbacks: &Rc<Cell<usize>>,
) -> Element<'static, String, iced::Theme, FixedRenderer> {
    let spans: Vec<Span<'static, Link, iced::Font>> = vec![
        Span::new("a").link(Link {
            id: "a",
            clones: clones.clone(),
        }),
        Span::new("plain"),
        Span::new("b").link(Link {
            id: "b",
            clones: clones.clone(),
        }),
    ];
    let callbacks = callbacks.clone();
    let on_click = move |link: Link| {
        callbacks.set(callbacks.get() + 1);
        Message(link.id.to_owned())
    };
    if captured {
        let mut rich =
            adapters::rich_text::<_, Message, iced::Theme, FixedRenderer>(spans, limits())
                .unwrap()
                .width(60)
                .height(20);
        if enabled {
            rich = rich.on_link_click(on_click);
        }
        let rich = rich.erase().map(|Message(id)| format!("mapped:{id}"));
        assert_eq!(rich.construction().nodes().len(), 1);
        assert_eq!(rich.construction().nodes()[0].canonical_type(), "rich_text");
        rich.into_parts().0
    } else {
        let mut rich: Rich<'_, Link, Message, iced::Theme, FixedRenderer> =
            Rich::with_spans(spans).width(60).height(20);
        if enabled {
            rich = rich.on_link_click(on_click);
        }
        let element: Element<'_, Message, iced::Theme, FixedRenderer> = rich.into();
        element.map(|Message(id)| format!("mapped:{id}"))
    }
}
#[derive(Debug, PartialEq)]
struct Outcome {
    messages: Vec<String>,
    captured: Vec<bool>,
    pointer: Vec<bool>,
    redraw: Vec<bool>,
    clones: usize,
    callbacks: usize,
}
fn exercise(captured: bool, enabled: bool) -> Outcome {
    use iced_test::core::{Shell, clipboard::Null, window::RedrawRequest};
    let clones = Rc::new(Cell::new(0));
    let callbacks = Rc::new(Cell::new(0));
    let mut element = build(captured, enabled, &clones, &callbacks);
    assert_eq!(clones.get(), 0);
    assert_eq!(callbacks.get(), 0);
    let renderer = FixedRenderer;
    let mut tree = Tree::new(&element);
    let node = element.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );
    assert_eq!(clones.get(), 2);
    assert_eq!(callbacks.get(), 0);
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));
    let events = [
        (0, 10.0),
        (1, 10.0),
        (2, 10.0),
        (1, 10.0),
        (2, 50.0),
        (1, 30.0),
        (2, 30.0),
        (1, 10.0),
        (0, 90.0),
        (2, 90.0),
        (1, 50.0),
        (2, 50.0),
    ];
    let mut messages = Vec::new();
    let mut captures = Vec::new();
    let mut pointer = Vec::new();
    let mut redraw = Vec::new();
    for (kind, x) in events {
        let position = Point::new(x, 10.0);
        let event = Event::Mouse(match kind {
            0 => mouse::Event::CursorMoved { position },
            1 => mouse::Event::ButtonPressed(mouse::Button::Left),
            _ => mouse::Event::ButtonReleased(mouse::Button::Left),
        });
        let cursor = mouse::Cursor::Available(position);
        let mut shell = Shell::new(&mut messages);
        element.as_widget_mut().update(
            &mut tree,
            &event,
            Layout::new(&node),
            cursor,
            &renderer,
            &mut Null,
            &mut shell,
            &viewport,
        );
        captures.push(shell.is_event_captured());
        redraw.push(matches!(shell.redraw_request(), RedrawRequest::NextFrame));
        pointer.push(
            element.as_widget().mouse_interaction(
                &tree,
                Layout::new(&node),
                cursor,
                &viewport,
                &renderer,
            ) == mouse::Interaction::Pointer,
        );
    }
    Outcome {
        messages,
        captured: captures,
        pointer,
        redraw,
        clones: clones.get(),
        callbacks: callbacks.get(),
    }
}
#[test]
fn matching_release_maps_non_clone_messages_while_cancellation_preserves_native_behavior() {
    let actual = exercise(true, true);
    assert_eq!(actual, exercise(false, true));
    assert_eq!(actual.messages, ["mapped:a", "mapped:b"]);
    assert_eq!(actual.callbacks, 2);
    assert_eq!(actual.clones, 4);
    assert_eq!(
        actual.captured,
        [
            false, true, false, true, false, false, false, true, false, false, true, false
        ]
    );
    assert_eq!(
        actual.pointer,
        [
            true, true, true, true, true, false, false, true, false, false, true, true
        ]
    );
    assert!(actual.redraw[0] && actual.redraw[8]);
}
#[test]
fn spans_without_an_enabled_link_callback_do_not_capture_or_publish_input() {
    let actual = exercise(true, false);
    assert_eq!(actual, exercise(false, false));
    assert!(actual.messages.is_empty());
    assert_eq!(actual.callbacks, 0);
    assert_eq!(actual.clones, 2);
    assert!(actual.captured.iter().all(|v| !*v));
    assert!(actual.pointer.iter().all(|v| !*v));
    assert!(actual.redraw.iter().all(|v| !*v));
}
