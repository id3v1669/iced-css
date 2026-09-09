use iced::{
    Element, Length, Point, Rectangle, Size,
    advanced::{layout, widget::Tree},
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
fn pin_has_its_own_source_node_while_forwarding_native_state_shape() {
    let child =
        adapters::button::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let pin = adapters::pin(child, limits()).unwrap();
    let nodes = pin.construction().nodes();
    assert_eq!(
        nodes.iter().map(|n| n.canonical_type()).collect::<Vec<_>>(),
        ["space", "button", "pin"]
    );
    assert_eq!(nodes[1].parent(), Some(2));
    assert_eq!(nodes[1].role(), Some("content"));
    let (native, _) = pin.erase().into_parts();
    let tree = Tree::new(&native);
    let button: Element<'_, (), iced::Theme, ()> =
        iced::widget::Button::new(iced::widget::Space::new()).into();
    let button_tree = Tree::new(&button);
    assert_eq!(tree.tag, button_tree.tag);
    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].children.len(), 0);
    assert_eq!(
        native.as_widget().size(),
        Size::new(Length::Fill, Length::Fill)
    );
}
fn rectangles(mut e: Element<'_, (), iced::Theme, ()>) -> Vec<Rectangle> {
    let mut tree = Tree::new(&e);
    let node = e.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );
    std::iter::once(node.bounds())
        .chain(node.children().iter().map(|n| n.bounds()))
        .collect()
}
#[test]
fn position_and_axis_setters_preserve_order_and_native_layout() {
    for reset in [false, true] {
        let p = adapters::pin::<(), iced::Theme, (), _>(
            adapters::space(limits()).unwrap().width(5.0).height(7.0),
            limits(),
        )
        .unwrap()
        .width(80.0)
        .height(60.0)
        .position(Point::new(2.0, 3.0))
        .x(9.0)
        .y(11.0);
        let n = iced::widget::Pin::new(iced::widget::Space::new().width(5.0).height(7.0))
            .width(80.0)
            .height(60.0)
            .position(Point::new(2.0, 3.0))
            .x(9.0)
            .y(11.0);
        let (p, n) = if reset {
            (
                p.position(Point::new(4.0, 6.0)),
                n.position(Point::new(4.0, 6.0)),
            )
        } else {
            (p, n)
        };
        let actual = rectangles(p.erase().into_parts().0);
        assert_eq!(actual, rectangles(n.into()));
        assert_eq!(
            actual[1].position(),
            if reset {
                Point::new(4.0, 6.0)
            } else {
                Point::new(9.0, 11.0)
            }
        );
    }
}
#[test]
fn conversions_are_eager_once_and_source_allocation_survives() {
    struct Dimension<'a>(&'a Cell<usize>);
    impl From<Dimension<'_>> for Length {
        fn from(v: Dimension<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::Fixed(40.0)
        }
    }
    struct Position<'a>(&'a Cell<usize>);
    impl From<Position<'_>> for Point {
        fn from(v: Position<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::new(1.0, 2.0)
        }
    }
    struct Pixels<'a>(&'a Cell<usize>);
    impl From<Pixels<'_>> for iced::Pixels {
        fn from(v: Pixels<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self(3.0)
        }
    }
    let calls = Cell::new(0);
    let p = adapters::pin::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
        .unwrap();
    let ptr = p.construction().nodes().as_ptr();
    let p = p
        .width(Dimension(&calls))
        .height(Dimension(&calls))
        .position(Position(&calls))
        .x(Pixels(&calls))
        .y(Pixels(&calls));
    assert_eq!(calls.get(), 5);
    assert_eq!(p.construction().nodes().as_ptr(), ptr);
    let _ = p.erase();
    assert_eq!(calls.get(), 5);
}
#[test]
fn pin_checks_aggregate_source_bounds() {
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
            matches!(adapters::pin::<(),iced::Theme,(),_>(adapters::space(limits()).unwrap(),cap),Err(actual) if actual==error)
        );
    }
}
#[test]
fn shifted_button_click_delivery_matches_stock_pin() {
    fn click(e: Element<'_, u8, iced::Theme, ()>, point: Point) -> Vec<u8> {
        let mut renderer = ();
        let mut ui = iced_test::runtime::UserInterface::build(
            e,
            Size::new(100.0, 100.0),
            iced_test::runtime::user_interface::Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        let _ = ui.update(
            &iced_test::simulator::click().collect::<Vec<_>>(),
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut iced_test::core::clipboard::Null,
            &mut messages,
        );
        messages
    }
    for (point, expected) in [
        (Point::new(8.0, 10.0), vec![1]),
        (Point::new(1.0, 1.0), vec![]),
    ] {
        let button = adapters::button::<u8, iced::Theme, (), _>(
            adapters::space(limits()).unwrap(),
            limits(),
        )
        .unwrap()
        .width(10.0)
        .height(10.0)
        .padding(0.0)
        .on_press(1);
        let pin = adapters::pin(button, limits())
            .unwrap()
            .width(50.0)
            .height(50.0)
            .position(Point::new(7.0, 9.0));
        let actual = click(pin.erase().into_parts().0, point);
        let stock = iced::widget::Pin::new(
            iced::widget::Button::new(iced::widget::Space::new())
                .width(10.0)
                .height(10.0)
                .padding(0.0)
                .on_press(1),
        )
        .width(50.0)
        .height(50.0)
        .position(Point::new(7.0, 9.0));
        assert_eq!(actual, click(stock.into(), point));
        assert_eq!(actual, expected);
    }
}
