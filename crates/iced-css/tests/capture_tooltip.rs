use iced::{
    Element, Event, Point, Rectangle, Size, Vector,
    advanced::{Layout, Shell, layout, widget::Tree},
    mouse,
    widget::tooltip::Position,
};
use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
use std::{cell::Cell, num::NonZeroUsize, time::Duration};
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
fn both_roles_exist_before_any_overlay_and_keep_void_nested_tooltip_content() {
    let tip = adapters::column_from_vec(vec![space(0.0, 0.0)], limits()).unwrap();
    let t = adapters::tooltip(space(20.0, 20.0), tip, Position::Bottom, limits()).unwrap();
    let n = t.construction().nodes();
    assert_eq!(n.len(), 4);
    assert_eq!(
        n.iter().map(|n| n.canonical_type()).collect::<Vec<_>>(),
        ["space", "space", "column", "tooltip"]
    );
    assert_eq!(
        n.iter().map(|n| n.parent()).collect::<Vec<_>>(),
        [Some(3), Some(2), Some(3), None]
    );
    assert_eq!(n[0].role(), Some("content"));
    assert_eq!(n[2].role(), Some("tooltip"));
    assert_eq!(n[0].role_position(), None);
    assert_eq!(n[2].role_position(), None);
    let (native, _) = t.erase().into_parts();
    let tree = Tree::new(&native);
    assert_eq!(tree.children.len(), 2);
    assert_eq!(tree.children[1].children.len(), 1);
}
#[test]
fn pair_merge_obeys_aggregate_limits_and_exact_work_budget() {
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
            matches!(adapters::tooltip(space(20.0,20.0),space(0.0,0.0),Position::Bottom,cap),Err(actual) if actual==error)
        );
    }
    for budget in [8, 9] {
        let mut cap = limits();
        cap.work = NonZeroUsize::new(budget).unwrap();
        let result = adapters::tooltip(space(20.0, 20.0), space(0.0, 0.0), Position::Bottom, cap);
        if budget == 9 {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(CaptureError::Work)));
        }
    }
}
fn lifecycle(mut element: Element<'_, (), iced::Theme, ()>) -> Rectangle {
    let mut state = Tree::new(&element);
    let node = element.as_widget_mut().layout(
        &mut state,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));
    let translation = Vector::new(7.0, 9.0);
    assert!(
        element
            .as_widget_mut()
            .overlay(&mut state, Layout::new(&node), &(), &viewport, translation)
            .is_none()
    );
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    element.as_widget_mut().update(
        &mut state,
        &Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(5.0, 5.0),
        }),
        Layout::new(&node),
        mouse::Cursor::Available(Point::new(5.0, 5.0)),
        &(),
        &mut iced_test::core::clipboard::Null,
        &mut shell,
        &viewport,
    );
    let bounds = {
        let mut overlay = element
            .as_widget_mut()
            .overlay(&mut state, Layout::new(&node), &(), &viewport, translation)
            .expect("tooltip opens on hover");
        let layout = overlay.as_overlay_mut().layout(&(), viewport.size());
        assert_eq!(layout.children().len(), 1);
        overlay.as_overlay().draw(
            &mut (),
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style {
                text_color: iced::Color::BLACK,
            },
            Layout::new(&layout),
            mouse::Cursor::Unavailable,
        );
        layout.children()[0].bounds()
    };
    element.as_widget_mut().update(
        &mut state,
        &Event::Mouse(mouse::Event::CursorLeft),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &(),
        &mut iced_test::core::clipboard::Null,
        &mut shell,
        &viewport,
    );
    assert!(
        element
            .as_widget_mut()
            .overlay(&mut state, Layout::new(&node), &(), &viewport, translation)
            .is_none()
    );
    bounds
}
#[test]
fn captured_overlay_lifecycle_and_translated_bounds_match_stock_tooltip() {
    let t = adapters::tooltip(
        space(20.0, 20.0),
        space(10.0, 8.0),
        Position::Bottom,
        limits(),
    )
    .unwrap()
    .padding(0.0)
    .gap(0.0)
    .delay(Duration::ZERO)
    .snap_within_viewport(false);
    let (native, source) = t.erase().into_parts();
    let before = source.nodes().as_ptr();
    let actual = lifecycle(native);
    let stock = iced::widget::Tooltip::new(
        iced::widget::Space::new().width(20.0).height(20.0),
        iced::widget::Space::new().width(10.0).height(8.0),
        Position::Bottom,
    )
    .padding(0.0)
    .gap(0.0)
    .delay(Duration::ZERO)
    .snap_within_viewport(false);
    assert_eq!(actual, lifecycle(stock.into()));
    assert_eq!(
        actual,
        Rectangle::new(Point::new(12.0, 29.0), Size::new(10.0, 8.0))
    );
    assert_eq!(source.nodes().len(), 3);
    assert_eq!(source.nodes().as_ptr(), before);
}
#[test]
fn pixel_conversions_are_eager_and_configuration_keeps_source_allocation() {
    struct Pixels<'a>(&'a Cell<usize>);
    impl From<Pixels<'_>> for iced::Pixels {
        fn from(v: Pixels<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self(0.0)
        }
    }
    let calls = Cell::new(0);
    let t = adapters::tooltip(
        space(20.0, 20.0),
        space(10.0, 8.0),
        Position::Bottom,
        limits(),
    )
    .unwrap();
    let ptr = t.construction().nodes().as_ptr();
    let t = t
        .gap(Pixels(&calls))
        .padding(Pixels(&calls))
        .delay(Duration::ZERO)
        .snap_within_viewport(false);
    assert_eq!(calls.get(), 2);
    assert_eq!(t.construction().nodes().as_ptr(), ptr);
    let _ = lifecycle(t.erase().into_parts().0);
    assert_eq!(calls.get(), 2);
}

#[test]
fn borrowed_style_and_class_are_used_only_by_overlay_drawing() {
    let calls = Cell::new(0);
    let converted = Cell::new(0);
    struct Class<'a>(
        &'a Cell<usize>,
        iced::widget::container::StyleFn<'a, iced::Theme>,
    );
    impl<'a> From<Class<'a>> for iced::widget::container::StyleFn<'a, iced::Theme> {
        fn from(v: Class<'a>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    for use_class in [false, true] {
        let callback = |_: &iced::Theme| {
            calls.set(calls.get() + 1);
            iced::widget::container::Style::default()
        };
        let t = adapters::tooltip(
            space(20.0, 20.0),
            space(10.0, 8.0),
            Position::Bottom,
            limits(),
        )
        .unwrap()
        .padding(0.0)
        .snap_within_viewport(false);
        let before = calls.get();
        let t = if use_class {
            t.class(Class(&converted, Box::new(callback)))
        } else {
            t.style(callback)
        };
        assert_eq!(converted.get(), usize::from(use_class));
        assert_eq!(calls.get(), before);
        let (native, _) = t.erase().into_parts();
        assert_eq!(calls.get(), before);
        let _ = lifecycle(native);
        assert_eq!(calls.get(), before + 1);
    }
}
#[test]
fn pair_merge_counts_attribute_storage_on_both_children() {
    let n = NonZeroUsize::new(10000).unwrap();
    let attrs = iced_css_core::source::AttributeLimits {
        entries: n,
        retained_bytes: n,
        work: n,
    };
    for first in [false, true] {
        let large = space(0.0, 0.0)
            .style_classes(&[&"x".repeat(1024)], attrs)
            .unwrap();
        let small = space(0.0, 0.0);
        let (a, b) = if first {
            (large, small)
        } else {
            (small, large)
        };
        let mut cap = limits();
        cap.retained_bytes = NonZeroUsize::new(512).unwrap();
        assert!(matches!(
            adapters::tooltip(a, b, Position::Bottom, cap),
            Err(CaptureError::Bytes)
        ));
    }
}
