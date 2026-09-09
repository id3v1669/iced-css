use iced::{
    Element, Point, Rectangle, Size, Vector,
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
fn float_source_is_distinct_from_forwarded_native_state_and_bounded() {
    let child =
        adapters::button::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let f = adapters::float(child, limits()).unwrap();
    let n = f.construction().nodes();
    assert_eq!(n.len(), 3);
    assert_eq!(n[2].canonical_type(), "float");
    assert_eq!(n[1].parent(), Some(2));
    assert_eq!(n[1].role(), Some("content"));
    let (native, _) = f.erase().into_parts();
    let tree = Tree::new(&native);
    let b: Element<'_, (), iced::Theme, ()> =
        iced::widget::Button::new(iced::widget::Space::new()).into();
    assert_eq!(tree.tag, Tree::new(&b).tag);
    assert_eq!(tree.children.len(), 1);
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
            matches!(adapters::float::<(),iced::Theme,(),_>(adapters::space(limits()).unwrap(),cap),Err(actual) if actual==error)
        );
    }
}
fn geometry(mut e: Element<'_, (), iced::Theme, ()>) -> (Rectangle, Option<Rectangle>) {
    let mut state = Tree::new(&e);
    let node = e.as_widget_mut().layout(
        &mut state,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );
    let viewport = Rectangle::with_size(Size::new(100.0, 100.0));
    let overlay_bounds = if let Some(mut overlay) = e.as_widget_mut().overlay(
        &mut state,
        Layout::new(&node),
        &(),
        &viewport,
        Vector::new(7.0, 9.0),
    ) {
        let n = overlay.as_overlay_mut().layout(&(), viewport.size());
        overlay.as_overlay().draw(
            &mut (),
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style {
                text_color: iced::Color::BLACK,
            },
            Layout::new(&n),
            mouse::Cursor::Unavailable,
        );
        Some(n.bounds())
    } else {
        None
    };
    if overlay_bounds.is_none() {
        e.as_widget().draw(
            &state,
            &mut (),
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style {
                text_color: iced::Color::BLACK,
            },
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
    }
    (node.bounds(), overlay_bounds)
}
#[test]
fn finite_scale_and_translation_geometry_match_stock_float() {
    for (scale, translation, overlay) in [
        (1.0, Vector::ZERO, false),
        (0.5, Vector::ZERO, false),
        (2.0, Vector::ZERO, true),
        (1.0, Vector::new(3.0, 4.0), true),
    ] {
        let f = adapters::float::<(), iced::Theme, (), _>(
            adapters::space(limits()).unwrap().width(20.0).height(10.0),
            limits(),
        )
        .unwrap()
        .scale(scale)
        .translate(move |_, _| translation);
        let actual = geometry(f.erase().into_parts().0);
        let native = iced::widget::Float::new(iced::widget::Space::new().width(20.0).height(10.0))
            .scale(scale)
            .translate(move |_, _| translation);
        assert_eq!(actual, geometry(native.into()));
        assert_eq!(actual.0.size(), Size::new(20.0, 10.0));
        assert_eq!(actual.1.is_some(), overlay);
    }
}
#[test]
fn translation_is_deferred_until_overlay_query_with_original_bounds_and_viewport() {
    let calls = Cell::new(0);
    let args = Cell::new(None);
    let f = adapters::float::<(), iced::Theme, (), _>(
        adapters::space(limits()).unwrap().width(20.0).height(10.0),
        limits(),
    )
    .unwrap()
    .scale(2.0)
    .translate(|bounds, viewport| {
        calls.set(calls.get() + 1);
        args.set(Some((bounds, viewport)));
        Vector::new(3.0, 4.0)
    });
    assert_eq!(calls.get(), 0);
    let (mut e, _) = f.erase().into_parts();
    let mut state = Tree::new(&e);
    let node = e.as_widget_mut().layout(
        &mut state,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );
    assert_eq!(calls.get(), 0);
    let viewport = Rectangle::with_size(Size::new(100.0, 100.0));
    let overlay = e.as_widget_mut().overlay(
        &mut state,
        Layout::new(&node),
        &(),
        &viewport,
        Vector::new(7.0, 9.0),
    );
    assert!(overlay.is_some());
    assert_eq!(calls.get(), 1);
    assert_eq!(
        args.get(),
        Some((
            Rectangle::new(Point::new(7.0, 9.0), Size::new(20.0, 10.0)),
            viewport
        ))
    );
}
#[test]
fn borrowed_style_and_class_callbacks_are_drawn_in_base_or_overlay() {
    let calls = Cell::new(0);
    let converted = Cell::new(0);
    struct Class<'a>(
        &'a Cell<usize>,
        iced::widget::float::StyleFn<'a, iced::Theme>,
    );
    impl<'a> From<Class<'a>> for iced::widget::float::StyleFn<'a, iced::Theme> {
        fn from(v: Class<'a>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    for scale in [1.0, 2.0] {
        for use_class in [false, true] {
            let callback = |_: &iced::Theme| {
                calls.set(calls.get() + 1);
                iced::widget::float::Style::default()
            };
            let f = adapters::float::<(), iced::Theme, (), _>(
                adapters::space(limits()).unwrap().width(20.0).height(10.0),
                limits(),
            )
            .unwrap()
            .scale(scale);
            let before = calls.get();
            let before_converted = converted.get();
            let f = if use_class {
                f.class(Class(&converted, Box::new(callback)))
            } else {
                f.style(callback)
            };
            assert_eq!(calls.get(), before);
            assert_eq!(converted.get(), before_converted + usize::from(use_class));
            let _ = geometry(f.erase().into_parts().0);
            assert_eq!(calls.get(), before + 1);
        }
    }
}
