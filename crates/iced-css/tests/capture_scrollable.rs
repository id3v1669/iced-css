use iced::{
    Length,
    advanced::widget::Tree,
    widget::scrollable::{Direction, Scrollbar},
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
fn fill() -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::space(limits())
        .unwrap()
        .width(Length::Fill)
        .height(Length::Fill)
        .erase()
}
#[test]
fn scrollable_retains_single_source_child_native_tree_and_bounds() {
    let scroll = adapters::scrollable(fill(), limits()).unwrap();
    let nodes = scroll.construction().nodes();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[1].canonical_type(), "scrollable");
    assert_eq!(nodes[0].parent(), Some(1));
    assert_eq!(nodes[0].role(), Some("content"));
    assert_eq!(nodes[1].text(), None);
    let (native, _) = scroll.erase().into_parts();
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
        assert!(matches!(adapters::scrollable(fill(),cap),Err(actual) if actual==error));
    }
}
#[test]
fn changed_direction_preserves_prior_hints_instead_of_reconstructing() {
    let changed = adapters::scrollable(fill(), limits())
        .unwrap()
        .horizontal()
        .erase()
        .into_parts()
        .0;
    let direct = adapters::scrollable_with_direction(
        fill(),
        Direction::Horizontal(Scrollbar::default()),
        limits(),
    )
    .unwrap()
    .erase()
    .into_parts()
    .0;
    assert_eq!(
        changed.as_widget().size(),
        iced::Size::new(Length::Fill, Length::Fill)
    );
    assert_eq!(
        direct.as_widget().size(),
        iced::Size::new(Length::Shrink, Length::Fill)
    );
    let expected: iced::Element<'_, (), iced::Theme, ()> = iced::widget::Scrollable::new(
        iced::widget::Space::new()
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .horizontal()
    .into();
    assert_eq!(changed.as_widget().size(), expected.as_widget().size());
}
#[test]
fn spacing_preserves_branch_specific_conversion_and_argument_drop() {
    struct Arg<'a>(&'a Cell<usize>, &'a Cell<usize>);
    impl From<Arg<'_>> for iced::Pixels {
        fn from(v: Arg<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self(3.0)
        }
    }
    impl Drop for Arg<'_> {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1)
        }
    }
    let converted = Cell::new(0);
    let dropped = Cell::new(0);
    let expressions = Cell::new(0);
    let argument = || {
        expressions.set(expressions.get() + 1);
        Arg(&converted, &dropped)
    };
    let s = adapters::scrollable_with_direction(
        fill(),
        Direction::Both {
            horizontal: Scrollbar::default(),
            vertical: Scrollbar::default(),
        },
        limits(),
    )
    .unwrap();
    let ptr = s.construction().nodes().as_ptr();
    let s = s.spacing(argument());
    assert_eq!(
        (expressions.get(), converted.get(), dropped.get()),
        (1, 0, 1)
    );
    let s = s.horizontal().spacing(argument());
    assert_eq!(
        (expressions.get(), converted.get(), dropped.get()),
        (2, 1, 2)
    );
    assert_eq!(s.construction().nodes().as_ptr(), ptr);
}
#[test]
fn direction_conversion_occurs_once_at_constructor_and_builder_time() {
    struct D<'a>(&'a Cell<usize>);
    impl From<D<'_>> for Direction {
        fn from(v: D<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::Horizontal(Scrollbar::default())
        }
    }
    let calls = Cell::new(0);
    let s = adapters::scrollable_with_direction(fill(), D(&calls), limits()).unwrap();
    assert_eq!(calls.get(), 1);
    let s = s.direction(D(&calls));
    assert_eq!(calls.get(), 2);
    let _ = s.erase();
    assert_eq!(calls.get(), 2);
}
#[test]
fn scroll_callback_is_stored_then_delivered_by_a_native_wheel_event() {
    let calls = Cell::new(0);
    let child = adapters::space(limits())
        .unwrap()
        .width(100.0)
        .height(300.0);
    let scroll = adapters::scrollable::<f32, iced::Theme, (), _>(child, limits())
        .unwrap()
        .width(100.0)
        .height(100.0)
        .on_scroll(|view| {
            calls.set(calls.get() + 1);
            view.absolute_offset().y
        });
    assert_eq!(calls.get(), 0);
    let mut renderer = ();
    let mut ui = iced_test::runtime::UserInterface::build(
        scroll.erase().into_parts().0,
        iced::Size::new(100.0, 100.0),
        iced_test::runtime::user_interface::Cache::default(),
        &mut renderer,
    );
    assert_eq!(calls.get(), 0);
    let mut messages = Vec::new();
    let _ = ui.update(
        &[iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Pixels { x: 0.0, y: -20.0 },
        })],
        iced::mouse::Cursor::Available(iced::Point::new(40.0, 40.0)),
        &mut renderer,
        &mut iced_test::core::clipboard::Null,
        &mut messages,
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(messages, [20.0]);
}

#[test]
fn anchor_configuration_matches_stock_wheel_delivery() {
    fn wheel(e: iced::Element<'_, (f32, f32), iced::Theme, ()>) -> Vec<(f32, f32)> {
        let mut renderer = ();
        let mut ui = iced_test::runtime::UserInterface::build(
            e,
            iced::Size::new(100.0, 100.0),
            iced_test::runtime::user_interface::Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        let _ = ui.update(
            &[iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                delta: iced::mouse::ScrollDelta::Pixels { x: -15.0, y: -20.0 },
            })],
            iced::mouse::Cursor::Available(iced::Point::new(40.0, 40.0)),
            &mut renderer,
            &mut iced_test::core::clipboard::Null,
            &mut messages,
        );
        messages
    }
    for direction in [
        Direction::default(),
        Direction::Horizontal(Scrollbar::default()),
        Direction::Both {
            horizontal: Scrollbar::default(),
            vertical: Scrollbar::default(),
        },
    ] {
        for end in [false, true] {
            let c = adapters::scrollable_with_direction::<(f32, f32), iced::Theme, (), _>(
                adapters::space(limits())
                    .unwrap()
                    .width(300.0)
                    .height(300.0),
                direction,
                limits(),
            )
            .unwrap()
            .width(100.0)
            .height(100.0)
            .auto_scroll(true)
            .spacing(3.0)
            .id("scroll-native");
            let n = iced::widget::Scrollable::with_direction(
                iced::widget::Space::new().width(300.0).height(300.0),
                direction,
            )
            .width(100.0)
            .height(100.0)
            .auto_scroll(true)
            .spacing(3.0)
            .id("scroll-native");
            let (c, n) = if end {
                (
                    c.anchor_bottom().anchor_right(),
                    n.anchor_bottom().anchor_right(),
                )
            } else {
                (c.anchor_top().anchor_left(), n.anchor_top().anchor_left())
            };
            let view = |v: iced::widget::scrollable::Viewport| {
                let a = v.absolute_offset();
                (a.x, a.y)
            };
            let actual = wheel(c.on_scroll(view).erase().into_parts().0);
            assert_eq!(actual, wheel(n.on_scroll(view).into()));
            if !end {
                assert!(!actual.is_empty());
            }
        }
    }
}
#[test]
fn native_style_and_class_callbacks_remain_borrowed_until_draw() {
    let calls = Cell::new(0);
    let converted = Cell::new(0);
    struct Class<'a>(
        &'a Cell<usize>,
        iced::widget::scrollable::StyleFn<'a, iced::Theme>,
    );
    impl<'a> From<Class<'a>> for iced::widget::scrollable::StyleFn<'a, iced::Theme> {
        fn from(v: Class<'a>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    for use_class in [false, true] {
        let callback = |theme: &iced::Theme, status| {
            calls.set(calls.get() + 1);
            iced::widget::scrollable::default(theme, status)
        };
        let c = adapters::scrollable(fill(), limits())
            .unwrap()
            .width(100.0)
            .height(100.0);
        let c = if use_class {
            c.class(Class(&converted, Box::new(callback)))
        } else {
            c.style(callback)
        };
        assert_eq!(converted.get(), usize::from(use_class));
        let before = calls.get();
        let mut renderer = ();
        let mut ui = iced_test::runtime::UserInterface::build(
            c.erase().into_parts().0,
            iced::Size::new(100.0, 100.0),
            iced_test::runtime::user_interface::Cache::default(),
            &mut renderer,
        );
        assert_eq!(calls.get(), before);
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style {
                text_color: iced::Color::BLACK,
            },
            iced::mouse::Cursor::Unavailable,
        );
        assert_eq!(calls.get(), before + 1);
    }
}
