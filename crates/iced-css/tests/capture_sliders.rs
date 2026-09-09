use iced::{
    Element, Event, Length, Pixels, Point, Rectangle, Size,
    advanced::{Layout, layout, widget::Tree},
    keyboard, mouse,
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
struct Dimension<'a>(&'a Cell<usize>);
impl From<Dimension<'_>> for Length {
    fn from(d: Dimension<'_>) -> Self {
        d.0.set(d.0.get() + 1);
        Self::Fixed(80.0)
    }
}
impl From<Dimension<'_>> for Pixels {
    fn from(d: Dimension<'_>) -> Self {
        d.0.set(d.0.get() + 1);
        Self(10.0)
    }
}
impl From<Dimension<'_>> for f32 {
    fn from(d: Dimension<'_>) -> Self {
        d.0.set(d.0.get() + 1);
        5.0
    }
}
fn bounds(mut e: Element<'_, f32, iced::Theme, ()>) -> Rectangle {
    let mut t = Tree::new(&e);
    e.as_widget_mut()
        .layout(
            &mut t,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
        )
        .bounds()
}
#[test]
fn slider_leaves_preserve_native_conversion_and_layout_contracts() {
    let calls = Cell::new(0);
    let changes = Cell::new(0);
    let callback = |v| {
        changes.set(changes.get() + 1);
        v
    };
    let h = adapters::slider::<_, _, iced::Theme>(0.0..=100.0, 25.0, callback, limits()).unwrap();
    let ptr = h.construction().nodes().as_ptr();
    let h = h
        .width(Dimension(&calls))
        .height(Dimension(&calls))
        .default(Dimension(&calls))
        .step(Dimension(&calls))
        .shift_step(Dimension(&calls))
        .on_release(-1.0);
    assert_eq!(calls.get(), 5);
    assert_eq!(h.construction().nodes().as_ptr(), ptr);
    assert_eq!(h.construction().nodes()[0].canonical_type(), "slider");
    assert_eq!(
        bounds(h.erase().into_parts().0),
        bounds(
            iced::widget::Slider::new(0.0..=100.0, 25.0, |v| v)
                .width(80.0)
                .height(10.0)
                .into()
        )
    );
    let v = adapters::vertical_slider::<_, _, iced::Theme>(0.0..=100.0, 25.0, callback, limits())
        .unwrap();
    let ptr = v.construction().nodes().as_ptr();
    let v = v
        .width(Dimension(&calls))
        .height(Dimension(&calls))
        .default(Dimension(&calls))
        .step(5.0)
        .shift_step(Dimension(&calls))
        .on_release(-1.0);
    assert_eq!(calls.get(), 9);
    assert_eq!(v.construction().nodes().as_ptr(), ptr);
    assert_eq!(
        v.construction().nodes()[0].canonical_type(),
        "vertical_slider"
    );
    assert_eq!(
        bounds(v.erase().into_parts().0),
        bounds(
            iced::widget::VerticalSlider::new(0.0..=100.0, 25.0, |v| v)
                .width(10.0)
                .height(80.0)
                .into()
        )
    );
    assert_eq!(changes.get(), 0);
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        adapters::slider::<_, _, iced::Theme>(0u8..=100, 25, |v| v, cap),
        Err(CaptureError::Bytes)
    ));
    assert!(matches!(
        adapters::vertical_slider::<_, _, iced::Theme>(0u8..=100, 25, |v| v, cap),
        Err(CaptureError::Bytes)
    ));
}
fn interact(
    mut e: Element<'_, f32, iced::Theme, ()>,
    vertical: bool,
    modifiers: keyboard::Modifiers,
) -> (Vec<f32>, Vec<bool>) {
    let mut tree = Tree::new(&e);
    let node = e.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );
    let point = if vertical {
        Point::new(5.0, 62.0)
    } else {
        Point::new(38.0, 5.0)
    };
    let mut messages = Vec::new();
    let mut captured = Vec::new();
    let input = [
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ];
    for event in input {
        let mut shell = iced::advanced::Shell::new(&mut messages);
        e.as_widget_mut().update(
            &mut tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Available(point),
            &(),
            &mut iced_test::core::clipboard::Null,
            &mut shell,
            &Rectangle::with_size(Size::new(100.0, 100.0)),
        );
        captured.push(shell.is_event_captured());
    }
    (messages, captured)
}
#[test]
fn sliders_deliver_native_step_shift_default_and_release_messages() {
    for vertical in [false, true] {
        for (modifier, expected) in [
            (keyboard::Modifiers::empty(), vec![40.0, -1.0]),
            (keyboard::Modifiers::SHIFT, vec![50.0, -1.0]),
            (
                keyboard::Modifiers::CTRL | keyboard::Modifiers::LOGO,
                vec![60.0],
            ),
        ] {
            let calls = Cell::new(0);
            let callback = |v| {
                calls.set(calls.get() + 1);
                v
            };
            let captured = if vertical {
                adapters::vertical_slider::<_, _, iced::Theme>(0.0..=100.0, 0.0, callback, limits())
                    .unwrap()
                    .width(10.0)
                    .height(100.0)
                    .step(10.0_f32)
                    .shift_step(25.0_f32)
                    .default(60.0_f32)
                    .on_release(-1.0)
                    .erase()
                    .into_parts()
                    .0
            } else {
                adapters::slider::<_, _, iced::Theme>(0.0..=100.0, 0.0, callback, limits())
                    .unwrap()
                    .width(100.0)
                    .height(10.0)
                    .step(10.0_f32)
                    .shift_step(25.0_f32)
                    .default(60.0_f32)
                    .on_release(-1.0)
                    .erase()
                    .into_parts()
                    .0
            };
            assert_eq!(calls.get(), 0);
            let stock = if vertical {
                iced::widget::VerticalSlider::new(0.0..=100.0, 0.0, |v| v)
                    .width(10.0)
                    .height(100.0)
                    .step(10.0_f32)
                    .shift_step(25.0_f32)
                    .default(60.0_f32)
                    .on_release(-1.0)
                    .into()
            } else {
                iced::widget::Slider::new(0.0..=100.0, 0.0, |v| v)
                    .width(100.0)
                    .height(10.0)
                    .step(10.0_f32)
                    .shift_step(25.0_f32)
                    .default(60.0_f32)
                    .on_release(-1.0)
                    .into()
            };
            let actual = interact(captured, vertical, modifier);
            assert_eq!(actual, interact(stock, vertical, modifier));
            assert_eq!(actual.0, expected);
            assert_eq!(calls.get(), 1);
            assert!(actual.1[1]);
        }
    }
}

#[test]
fn borrowed_slider_styles_and_classes_are_drawn_later() {
    let calls = Cell::new(0);
    let converted = Cell::new(0);
    struct Class<'a>(
        &'a Cell<usize>,
        iced::widget::slider::StyleFn<'a, iced::Theme>,
    );
    impl<'a> From<Class<'a>> for iced::widget::slider::StyleFn<'a, iced::Theme> {
        fn from(v: Class<'a>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    for (vertical, use_class) in [(false, false), (false, true), (true, false), (true, true)] {
        converted.set(0);
        let callback = |theme: &iced::Theme, status| {
            calls.set(calls.get() + 1);
            iced::widget::slider::default(theme, status)
        };
        let before = calls.get();
        let r = if vertical {
            let r = adapters::vertical_slider::<f32, _, iced::Theme>(
                0.0..=100.0,
                25.0,
                |v| v,
                limits(),
            )
            .unwrap();
            if use_class {
                r.class(Class(&converted, Box::new(callback)))
                    .erase::<f32, iced::Theme, ()>()
            } else {
                r.style(callback).erase::<f32, iced::Theme, ()>()
            }
        } else {
            let r = adapters::slider::<f32, _, iced::Theme>(0.0..=100.0, 25.0, |v| v, limits())
                .unwrap();
            if use_class {
                r.class(Class(&converted, Box::new(callback)))
                    .erase::<f32, iced::Theme, ()>()
            } else {
                r.style(callback).erase::<f32, iced::Theme, ()>()
            }
        };
        assert_eq!(calls.get(), before);
        assert_eq!(converted.get(), usize::from(use_class));
        let mut renderer = ();
        let mut ui = iced_test::runtime::UserInterface::build(
            r.into_parts().0,
            Size::new(100.0, 80.0),
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
            mouse::Cursor::Unavailable,
        );
        assert_eq!(calls.get(), before + 1);
    }
}
