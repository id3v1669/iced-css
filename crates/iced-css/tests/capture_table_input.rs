use iced::{
    Element, Event, Point, Rectangle, Size,
    advanced::{Layout, Shell, layout, widget::Tree},
    mouse,
    widget::{Checkbox, table},
};
use iced_css::capture::{CaptureLimits, adapters};
use std::num::NonZeroUsize;
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(100000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
// Intentionally not Clone: native checkbox callbacks and table forwarding do not require it.
struct Message(&'static str, bool);
fn build(captured: bool, enabled: bool) -> Element<'static, String, iced::Theme, ()> {
    if captured {
        let control = move |name| {
            let control = adapters::checkbox::<Message, iced::Theme, ()>(false, limits())
                .unwrap()
                .size(20);
            if enabled {
                control.on_toggle(move |value| Message(name, value))
            } else {
                control
            }
        };
        let column = adapters::table_column(control("header"), move |()| control("body")).width(60);
        adapters::table_from_vecs(vec![column], vec![()], limits())
            .unwrap()
            .erase()
            .map(|Message(name, value)| format!("{name}:{value}"))
            .into_parts()
            .0
    } else {
        let control = move |name| {
            let control = Checkbox::new(false).size(20);
            if enabled {
                control.on_toggle(move |value| Message(name, value))
            } else {
                control
            }
        };
        let native: Element<'_, Message, iced::Theme, ()> = table::table(
            [table::column(control("header"), move |()| control("body")).width(60)],
            [()],
        )
        .into();
        native.map(|Message(name, value)| format!("{name}:{value}"))
    }
}
#[derive(Debug, PartialEq)]
struct Outcome {
    messages: Vec<String>,
    captured: Vec<bool>,
    pointer: Vec<bool>,
}
fn exercise(captured: bool, enabled: bool) -> Outcome {
    let mut element = build(captured, enabled);
    let mut tree = Tree::new(&element);
    let viewport = Rectangle::with_size(Size::new(300.0, 200.0));
    let node = element.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    assert_eq!(node.children().len(), 2);
    let points = [
        node.children()[0].bounds().center(),
        node.children()[1].bounds().center(),
        Point::new(299.0, 199.0),
    ];
    let mut result = Outcome {
        messages: vec![],
        captured: vec![],
        pointer: vec![],
    };
    for point in points {
        for event in [
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ] {
            let cursor = mouse::Cursor::Available(point);
            let mut shell = Shell::new(&mut result.messages);
            element.as_widget_mut().update(
                &mut tree,
                &Event::Mouse(event),
                Layout::new(&node),
                cursor,
                &(),
                &mut iced_test::core::clipboard::Null,
                &mut shell,
                &viewport,
            );
            result.captured.push(shell.is_event_captured());
            result.pointer.push(
                element.as_widget().mouse_interaction(
                    &tree,
                    Layout::new(&node),
                    cursor,
                    &viewport,
                    &(),
                ) == mouse::Interaction::Pointer,
            );
        }
    }
    result
}
#[test]
fn table_header_and_body_forward_native_input_and_non_clone_message_mapping() {
    let captured = exercise(true, true);
    let native = exercise(false, true);
    assert_eq!(captured, native);
    assert_eq!(captured.messages, ["header:true", "body:true"]);
    assert_eq!(captured.captured, [true, false, true, false, false, false]);
    assert_eq!(captured.pointer, [true, true, true, true, false, false]);
}
#[test]
fn disabled_table_controls_do_not_publish_or_capture() {
    let captured = exercise(true, false);
    assert_eq!(captured, exercise(false, false));
    assert!(captured.messages.is_empty());
    assert_eq!(captured.captured, [false; 6]);
    assert_eq!(captured.pointer, [false; 6]);
}

fn menu_table(captured: bool) -> Element<'static, String, iced::Theme, ()> {
    if captured {
        let column = adapters::table_column(adapters::space(limits()).unwrap(), |()| {
            adapters::pick_list::<_, _, _, _, iced::Theme, ()>([1u8, 2], Some(1u8), |v| v, limits())
                .unwrap()
                .width(70)
                .on_open(100)
                .on_close(101)
        })
        .width(90);
        adapters::table_from_vecs(vec![column], vec![()], limits())
            .unwrap()
            .erase()
            .map(|v| format!("mapped:{v}"))
            .into_parts()
            .0
    } else {
        let column = table::column(iced::widget::Space::new(), |()| {
            iced::widget::PickList::new([1u8, 2], Some(1u8), |v| v)
                .width(70)
                .on_open(100)
                .on_close(101)
        })
        .width(90);
        let native: Element<'_, u8, iced::Theme, ()> = table::table([column], [()]).into();
        native.map(|v| format!("mapped:{v}"))
    }
}
fn menu_lifecycle(captured: bool) -> (Vec<String>, Vec<Rectangle>) {
    let mut element = menu_table(captured);
    let mut tree = Tree::new(&element);
    let viewport = Rectangle::with_size(Size::new(300.0, 250.0));
    let node = element.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    let position = node.children()[1].bounds().center();
    let translation = iced::Vector::new(7.0, 11.0);
    assert!(
        element
            .as_widget_mut()
            .overlay(&mut tree, Layout::new(&node), &(), &viewport, translation)
            .is_none()
    );
    let mut messages = vec![];
    let mut overlay_bounds = vec![];
    for index in 0..2 {
        let mut shell = Shell::new(&mut messages);
        element.as_widget_mut().update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(position),
            &(),
            &mut iced_test::core::clipboard::Null,
            &mut shell,
            &viewport,
        );
        assert!(shell.is_event_captured());
        if index == 0 {
            let mut overlay = element
                .as_widget_mut()
                .overlay(&mut tree, Layout::new(&node), &(), &viewport, translation)
                .expect("table cell menu opens");
            let layout = overlay.as_overlay_mut().layout(&(), viewport.size());
            fn collect(node: &layout::Node, bounds: &mut Vec<Rectangle>) {
                bounds.push(node.bounds());
                for child in node.children() {
                    collect(child, bounds);
                }
            }
            collect(&layout, &mut overlay_bounds);
            overlay.as_overlay().draw(
                &mut (),
                &iced::Theme::Dark,
                &iced::advanced::renderer::Style::default(),
                Layout::new(&layout),
                mouse::Cursor::Unavailable,
            );
        }
    }
    assert!(
        element
            .as_widget_mut()
            .overlay(&mut tree, Layout::new(&node), &(), &viewport, translation)
            .is_none()
    );
    (messages, overlay_bounds)
}
#[test]
fn table_cell_menu_overlay_opens_draws_and_closes_like_stock_iced() {
    let captured = menu_lifecycle(true);
    assert_eq!(captured, menu_lifecycle(false));
    assert_eq!(captured.0, ["mapped:100", "mapped:101"]);
    assert!(captured.1.len() > 1);
}
