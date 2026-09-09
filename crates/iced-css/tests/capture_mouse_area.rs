use iced::{
    Element, Event, Point, Size,
    advanced::{Layout, layout, widget::Tree},
    mouse,
};
use iced_css::capture::{CaptureError, CaptureLimits, adapters};
use std::{cell::Cell, num::NonZeroUsize};
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
fn mouse_area_retains_content_and_checks_aggregate_bounds() {
    let a = adapters::mouse_area::<(), iced::Theme, (), _>(
        adapters::space(limits()).unwrap(),
        limits(),
    )
    .unwrap();
    let n = a.construction().nodes();
    assert_eq!(n.len(), 2);
    assert_eq!(n[1].canonical_type(), "mouse_area");
    assert_eq!(n[0].role(), Some("content"));
    assert_eq!(n[0].parent(), Some(1));
    let (native, _) = a.erase().into_parts();
    assert_eq!(Tree::new(&native).children.len(), 1);
    for (field, error) in [
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
            matches!(adapters::mouse_area::<(),iced::Theme,(),_>(adapters::space(limits()).unwrap(),cap),Err(actual) if actual==error)
        );
    }
}
#[test]
fn construction_does_not_impose_cloning_before_native_widget_conversion() {
    struct Message<'a>(&'a Cell<usize>);
    impl Drop for Message<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1)
        }
    }
    let drops = Cell::new(0);
    let area = adapters::mouse_area::<Message<'_>, iced::Theme, (), _>(
        adapters::space(limits()).unwrap(),
        limits(),
    )
    .unwrap()
    .on_press(Message(&drops));
    assert_eq!(drops.get(), 0);
    drop(area);
    assert_eq!(drops.get(), 1);
}
fn messages<M: Clone>(
    e: Element<'_, M, iced::Theme, ()>,
    events: &[Event],
    cursor: mouse::Cursor,
) -> Vec<M> {
    let mut renderer = ();
    let mut ui = iced_test::runtime::UserInterface::build(
        e,
        Size::new(100.0, 100.0),
        iced_test::runtime::user_interface::Cache::default(),
        &mut renderer,
    );
    let mut messages = Vec::new();
    let _ = ui.update(
        events,
        cursor,
        &mut renderer,
        &mut iced_test::core::clipboard::Null,
        &mut messages,
    );
    messages
}
#[test]
fn left_right_middle_message_delivery_matches_stock_area() {
    let area = adapters::mouse_area::<u8, iced::Theme, (), _>(
        adapters::space(limits()).unwrap().width(20.0).height(20.0),
        limits(),
    )
    .unwrap()
    .on_press(1)
    .on_release(2)
    .on_right_press(3)
    .on_right_release(4)
    .on_middle_press(5)
    .on_middle_release(6)
    .on_double_click(7);
    let native = iced::widget::MouseArea::new(iced::widget::Space::new().width(20.0).height(20.0))
        .on_press(1)
        .on_release(2)
        .on_right_press(3)
        .on_right_release(4)
        .on_middle_press(5)
        .on_middle_release(6)
        .on_double_click(7);
    let events: Vec<_> = [
        mouse::Button::Left,
        mouse::Button::Right,
        mouse::Button::Middle,
    ]
    .into_iter()
    .flat_map(|b| {
        [
            Event::Mouse(mouse::Event::ButtonPressed(b)),
            Event::Mouse(mouse::Event::ButtonReleased(b)),
        ]
    })
    .collect();
    let cursor = mouse::Cursor::Available(Point::new(5.0, 5.0));
    let actual = messages(area.erase().into_parts().0, &events, cursor);
    assert_eq!(actual, messages(native.into(), &events, cursor));
    assert_eq!(actual, [1, 2, 3, 4, 5, 6]);
}
#[test]
fn child_button_captures_events_before_parent_area() {
    let b =
        adapters::button::<u8, iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap()
            .width(20.0)
            .height(20.0)
            .padding(0.0)
            .on_press(1);
    let a = adapters::mouse_area(b, limits())
        .unwrap()
        .on_press(2)
        .on_release(3);
    let native = iced::widget::MouseArea::new(
        iced::widget::Button::new(iced::widget::Space::new())
            .width(20.0)
            .height(20.0)
            .padding(0.0)
            .on_press(1),
    )
    .on_press(2)
    .on_release(3);
    let events = iced_test::simulator::click().collect::<Vec<_>>();
    let cursor = mouse::Cursor::Available(Point::new(5.0, 5.0));
    let actual = messages(a.erase().into_parts().0, &events, cursor);
    assert_eq!(actual, messages(native.into(), &events, cursor));
    assert_eq!(actual, [1]);
}
#[test]
fn borrowed_move_and_scroll_callbacks_deliver_local_coordinates_on_events() {
    let moved = Cell::new(0);
    let scrolled = Cell::new(0);
    let a = adapters::mouse_area::<(&str, f32, f32), iced::Theme, (), _>(
        adapters::space(limits()).unwrap().width(20.0).height(20.0),
        limits(),
    )
    .unwrap()
    .on_enter(("enter", 0.0, 0.0))
    .on_exit(("exit", 0.0, 0.0))
    .on_move(|p| {
        moved.set(moved.get() + 1);
        ("move", p.x, p.y)
    })
    .on_scroll(|d| {
        scrolled.set(scrolled.get() + 1);
        let (mouse::ScrollDelta::Pixels { x, y } | mouse::ScrollDelta::Lines { x, y }) = d;
        ("scroll", x, y)
    });
    let p = adapters::pin(a, limits())
        .unwrap()
        .position(Point::new(10.0, 20.0));
    let (native, _) = p.erase().into_parts();
    let mut renderer = ();
    let mut ui = iced_test::runtime::UserInterface::build(
        native,
        Size::new(100.0, 100.0),
        iced_test::runtime::user_interface::Cache::default(),
        &mut renderer,
    );
    assert_eq!((moved.get(), scrolled.get()), (0, 0));
    let mut result = Vec::new();
    for (event, cursor) in [
        (
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(15.0, 25.0),
            }),
            mouse::Cursor::Available(Point::new(15.0, 25.0)),
        ),
        (
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(16.0, 27.0),
            }),
            mouse::Cursor::Available(Point::new(16.0, 27.0)),
        ),
        (
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 3.0, y: 4.0 },
            }),
            mouse::Cursor::Available(Point::new(16.0, 27.0)),
        ),
        (
            Event::Mouse(mouse::Event::CursorLeft),
            mouse::Cursor::Unavailable,
        ),
    ] {
        let _ = ui.update(
            &[event],
            cursor,
            &mut renderer,
            &mut iced_test::core::clipboard::Null,
            &mut result,
        );
    }
    assert_eq!(
        result,
        [
            ("enter", 0.0, 0.0),
            ("move", 6.0, 7.0),
            ("scroll", 3.0, 4.0),
            ("exit", 0.0, 0.0)
        ]
    );
    assert_eq!((moved.get(), scrolled.get()), (1, 1));
}
#[test]
fn interaction_is_a_fallback_after_child_interaction() {
    fn interaction(
        mut e: Element<'_, (), iced::Theme, ()>,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let mut tree = Tree::new(&e);
        let n = e.as_widget_mut().layout(
            &mut tree,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
        );
        e.as_widget().mouse_interaction(
            &tree,
            Layout::new(&n),
            cursor,
            &iced::Rectangle::with_size(Size::new(100.0, 100.0)),
            &(),
        )
    }
    for cursor in [
        mouse::Cursor::Unavailable,
        mouse::Cursor::Available(Point::new(5.0, 5.0)),
    ] {
        let space = adapters::space(limits()).unwrap().width(20.0).height(20.0);
        let area = adapters::mouse_area::<(), iced::Theme, (), _>(space, limits())
            .unwrap()
            .interaction(mouse::Interaction::Crosshair);
        assert_eq!(
            interaction(area.erase().into_parts().0, cursor),
            if cursor.position().is_some() {
                mouse::Interaction::Crosshair
            } else {
                mouse::Interaction::None
            }
        );
        let button = adapters::button::<(), iced::Theme, (), _>(
            adapters::space(limits()).unwrap(),
            limits(),
        )
        .unwrap()
        .width(20.0)
        .height(20.0)
        .on_press(());
        let area = adapters::mouse_area(button, limits())
            .unwrap()
            .interaction(mouse::Interaction::Crosshair);
        assert_eq!(
            interaction(area.erase().into_parts().0, cursor),
            if cursor.position().is_some() {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::None
            }
        );
    }
}
