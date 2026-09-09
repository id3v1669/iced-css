use iced::{
    Element, Point, Rectangle, Size,
    advanced::{layout, widget::Tree},
    mouse,
};
use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
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
fn space(w: f32, h: f32) -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::space(limits())
        .unwrap()
        .width(w)
        .height(h)
        .erase()
}
#[test]
fn hover_keeps_fixed_source_roles_and_void_native_members() {
    let h = adapters::hover(
        space(20.0, 10.0),
        adapters::row_from_vec(vec![space(0.0, 0.0)], limits()).unwrap(),
        limits(),
    )
    .unwrap();
    let n = h.construction().nodes();
    assert_eq!(n.len(), 4);
    assert_eq!(
        n.iter().map(|n| n.canonical_type()).collect::<Vec<_>>(),
        ["space", "space", "row", "hover"]
    );
    assert_eq!(
        n.iter().map(|n| n.parent()).collect::<Vec<_>>(),
        [Some(3), Some(2), Some(3), None]
    );
    assert_eq!(n[0].role(), Some("base"));
    assert_eq!(n[2].role(), Some("top"));
    let (native, _) = h.into_parts();
    let tree = Tree::new(&native);
    assert_eq!(tree.children.len(), 2);
    assert_eq!(tree.children[1].children.len(), 1);
}
#[test]
fn hover_merge_bounds_include_both_children_and_attributes() {
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
            matches!(adapters::hover(space(20.0,20.0),space(0.0,0.0),cap),Err(actual) if actual==error)
        );
    }
    let n = NonZeroUsize::new(10000).unwrap();
    let attrs = iced_css_core::source::AttributeLimits {
        entries: n,
        retained_bytes: n,
        work: n,
    };
    let top = space(0.0, 0.0)
        .style_classes(&[&"x".repeat(1024)], attrs)
        .unwrap();
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::new(512).unwrap();
    assert!(matches!(
        adapters::hover(space(0.0, 0.0), top, cap),
        Err(CaptureError::Bytes)
    ));
}
#[test]
fn native_layout_uses_base_bounds_and_matches_stock_hover() {
    fn rectangles(mut e: Element<'_, (), iced::Theme, ()>) -> Vec<Rectangle> {
        let mut tree = Tree::new(&e);
        let n = e.as_widget_mut().layout(
            &mut tree,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
        );
        std::iter::once(n.bounds())
            .chain(n.children().iter().map(|n| n.bounds()))
            .collect()
    }
    let h = adapters::hover(space(20.0, 10.0), space(5.0, 4.0), limits()).unwrap();
    let actual = rectangles(h.into_parts().0);
    let expected = rectangles(iced::widget::hover(
        iced::widget::Space::new().width(20.0).height(10.0),
        iced::widget::Space::new().width(5.0).height(4.0),
    ));
    assert_eq!(actual, expected);
    assert_eq!(actual[0].size(), Size::new(20.0, 10.0));
    assert_eq!(actual[2].size(), Size::new(5.0, 4.0));
}
#[test]
fn hover_visibility_controls_top_drawing_without_removing_source() {
    let base_calls = Cell::new(0);
    let top_calls = Cell::new(0);
    let base = adapters::container::<(), iced::Theme, (), _>(space(20.0, 10.0), limits())
        .unwrap()
        .style(|_| {
            base_calls.set(base_calls.get() + 1);
            iced::widget::container::Style::default()
        });
    let top = adapters::container::<(), iced::Theme, (), _>(space(5.0, 4.0), limits())
        .unwrap()
        .style(|_| {
            top_calls.set(top_calls.get() + 1);
            iced::widget::container::Style::default()
        });
    let h = adapters::hover(base, top, limits()).unwrap();
    let (native, source) = h.into_parts();
    assert_eq!(source.nodes().len(), 5);
    let mut renderer = ();
    let mut ui = iced_test::runtime::UserInterface::build(
        native,
        Size::new(100.0, 100.0),
        iced_test::runtime::user_interface::Cache::default(),
        &mut renderer,
    );
    assert_eq!((base_calls.get(), top_calls.get()), (0, 0));
    for (cursor, expected) in [
        (mouse::Cursor::Unavailable, (1, 0)),
        (mouse::Cursor::Available(Point::new(1.0, 1.0)), (2, 1)),
        (mouse::Cursor::Unavailable, (3, 1)),
    ] {
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style {
                text_color: iced::Color::BLACK,
            },
            cursor,
        );
        assert_eq!((base_calls.get(), top_calls.get()), expected);
        assert_eq!(source.nodes().len(), 5);
    }
}
#[test]
fn top_button_has_click_priority_only_inside_its_bounds() {
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
    for (point, expected) in [(Point::new(5.0, 5.0), 1), (Point::new(15.0, 15.0), 2)] {
        let base = adapters::button::<u8, iced::Theme, (), _>(
            adapters::space(limits()).unwrap(),
            limits(),
        )
        .unwrap()
        .width(20.0)
        .height(20.0)
        .padding(0.0)
        .on_press(2);
        let top = adapters::button::<u8, iced::Theme, (), _>(
            adapters::space(limits()).unwrap(),
            limits(),
        )
        .unwrap()
        .width(10.0)
        .height(10.0)
        .padding(0.0)
        .on_press(1);
        let actual = click(
            adapters::hover(base, top, limits()).unwrap().into_parts().0,
            point,
        );
        let native = iced::widget::hover(
            iced::widget::Button::new(iced::widget::Space::new())
                .width(20.0)
                .height(20.0)
                .padding(0.0)
                .on_press(2),
            iced::widget::Button::new(iced::widget::Space::new())
                .width(10.0)
                .height(10.0)
                .padding(0.0)
                .on_press(1),
        );
        assert_eq!(actual, click(native, point));
        assert_eq!(actual, [expected]);
    }
}
