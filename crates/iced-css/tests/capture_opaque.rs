use iced::{
    Element, Event, Point, Rectangle, Size,
    advanced::{Layout, layout, widget::Tree},
    mouse,
};
use iced_css::capture::{CaptureError, CaptureLimits, adapters};
use std::num::NonZeroUsize;
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
#[test]
fn opaque_retains_its_managed_child_and_forwards_native_state() {
    let child =
        adapters::button::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let captured = adapters::opaque(child, limits()).unwrap();
    let nodes = captured.construction().nodes();
    assert_eq!(
        nodes.iter().map(|n| n.canonical_type()).collect::<Vec<_>>(),
        ["space", "button", "opaque"]
    );
    assert_eq!(nodes[1].parent(), Some(2));
    assert_eq!(nodes[1].role(), Some("content"));
    let (e, _) = captured.into_parts();
    let stock: Element<'_, (), iced::Theme, ()> =
        iced::widget::opaque(iced::widget::Button::new(iced::widget::Space::new()));
    let a = Tree::new(&e);
    let b = Tree::new(&stock);
    assert_eq!(a.tag, b.tag);
    assert_eq!(a.children.len(), 1);
    assert_eq!(a.children.len(), b.children.len());
    assert_eq!(e.as_widget().size_hint(), stock.as_widget().size_hint());
}
#[test]
fn opaque_checks_aggregate_bounds() {
    for (field, expected) in [
        (0, CaptureError::Nodes),
        (1, CaptureError::Depth),
        (2, CaptureError::Bytes),
        (3, CaptureError::Work),
    ] {
        let mut cap = limits();
        match field {
            0 => cap.nodes = NonZeroUsize::MIN,
            1 => cap.depth = NonZeroUsize::MIN,
            2 => cap.retained_bytes = NonZeroUsize::MIN,
            _ => cap.work = NonZeroUsize::MIN,
        }
        assert!(
            matches!(adapters::opaque::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), cap), Err(e) if e == expected)
        );
    }
}
fn events(
    mut e: Element<'_, u8, iced::Theme, ()>,
    cursor: mouse::Cursor,
    events: &[Event],
) -> (Vec<bool>, Vec<u8>, mouse::Interaction) {
    let mut tree = Tree::new(&e);
    let node = e.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );
    let viewport = Rectangle::with_size(Size::new(100.0, 100.0));
    let mut messages = Vec::new();
    let mut captured = Vec::new();
    for event in events {
        let mut shell = iced::advanced::Shell::new(&mut messages);
        e.as_widget_mut().update(
            &mut tree,
            event,
            Layout::new(&node),
            cursor,
            &(),
            &mut iced_test::core::clipboard::Null,
            &mut shell,
            &viewport,
        );
        captured.push(shell.is_event_captured());
    }
    let interaction =
        e.as_widget()
            .mouse_interaction(&tree, Layout::new(&node), cursor, &viewport, &());
    (captured, messages, interaction)
}
#[test]
fn native_opaque_captures_only_presses_inside_and_preserves_child_delivery() {
    let input = [
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)),
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        }),
    ];
    for (cursor, inside) in [
        (mouse::Cursor::Available(Point::new(5.0, 5.0)), true),
        (mouse::Cursor::Available(Point::new(25.0, 25.0)), false),
        (mouse::Cursor::Unavailable, false),
    ] {
        let c = adapters::opaque::<u8, iced::Theme, (), _>(
            adapters::space(limits()).unwrap().width(20.0).height(20.0),
            limits(),
        )
        .unwrap();
        let actual = events(c.into_parts().0, cursor, &input);
        let stock = iced::widget::opaque(iced::widget::Space::new().width(20.0).height(20.0));
        assert_eq!(actual, events(stock, cursor, &input));
        assert_eq!(actual.0, [inside, false, inside, false, false]);
        assert!(actual.1.is_empty());
        assert_eq!(
            actual.2,
            if inside {
                mouse::Interaction::Idle
            } else {
                mouse::Interaction::None
            }
        );
    }
    let b =
        adapters::button::<u8, iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap()
            .width(20.0)
            .height(20.0)
            .padding(0.0)
            .on_press(7);
    let c = adapters::opaque(b, limits()).unwrap();
    let cursor = mouse::Cursor::Available(Point::new(5.0, 5.0));
    let actual = events(c.into_parts().0, cursor, &input[..2]);
    let stock = iced::widget::opaque(
        iced::widget::Button::new(iced::widget::Space::new())
            .width(20.0)
            .height(20.0)
            .padding(0.0)
            .on_press(7),
    );
    assert_eq!(actual, events(stock, cursor, &input[..2]));
    assert_eq!(actual.1, [7]);
    assert_eq!(actual.2, mouse::Interaction::Pointer);
}
