use iced::{Length, advanced::widget::Tree};
use iced_css::capture::{CaptureLimits, adapters};
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
fn rectangles(mut e: iced::Element<'_, (), iced::Theme, ()>) -> Vec<iced::Rectangle> {
    let mut tree = Tree::new(&e);
    let node = e.as_widget_mut().layout(
        &mut tree,
        &(),
        &iced::advanced::layout::Limits::new(iced::Size::ZERO, iced::Size::new(100.0, 100.0)),
    );
    std::iter::once(node.bounds())
        .chain(node.children().iter().map(|n| n.bounds()))
        .collect()
}
#[test]
fn button_configuration_is_eager_and_matches_stock_layout() {
    struct Dimension<'a>(&'a Cell<usize>, f32);
    impl From<Dimension<'_>> for Length {
        fn from(v: Dimension<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::Fixed(v.1)
        }
    }
    struct Padding<'a>(&'a Cell<usize>);
    impl From<Padding<'_>> for iced::Padding {
        fn from(v: Padding<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::from(3.0)
        }
    }
    let calls = Cell::new(0);
    let b = adapters::button::<(), iced::Theme, (), _>(
        adapters::space(limits()).unwrap().width(7.0).height(5.0),
        limits(),
    )
    .unwrap();
    let ptr = b.construction().nodes().as_ptr();
    let b = b
        .width(Dimension(&calls, 40.0))
        .height(Dimension(&calls, 30.0))
        .padding(Padding(&calls))
        .clip(true)
        .on_press(());
    assert_eq!(calls.get(), 3);
    assert_eq!(b.construction().nodes().as_ptr(), ptr);
    let native = iced::widget::Button::new(iced::widget::Space::new().width(7.0).height(5.0))
        .width(40.0)
        .height(30.0)
        .padding(3.0)
        .clip(true)
        .on_press(());
    assert_eq!(
        rectangles(b.erase().into_parts().0),
        rectangles(native.into())
    );
    assert_eq!(calls.get(), 3);
}
#[test]
fn borrowed_style_and_class_callbacks_are_deferred_without_changing_css_classes() {
    let calls = Cell::new(0);
    let converted = Cell::new(0);
    struct Class<'a>(
        &'a Cell<usize>,
        iced::widget::button::StyleFn<'a, iced::Theme>,
    );
    impl<'a> From<Class<'a>> for iced::widget::button::StyleFn<'a, iced::Theme> {
        fn from(v: Class<'a>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    for use_class in [false, true] {
        let callback = |_: &iced::Theme, status: iced::widget::button::Status| {
            assert_eq!(status, iced::widget::button::Status::Disabled);
            calls.set(calls.get() + 1);
            iced::widget::button::Style::default()
        };
        let n = NonZeroUsize::new(10000).unwrap();
        let attrs = iced_css_core::source::AttributeLimits {
            entries: n,
            retained_bytes: n,
            work: n,
        };
        let b = adapters::button::<(), iced::Theme, (), _>(
            adapters::space(limits()).unwrap().width(7.0).height(5.0),
            limits(),
        )
        .unwrap()
        .style_classes(&["css-only"], attrs)
        .unwrap();
        let b = if use_class {
            b.class(Class(&converted, Box::new(callback)))
        } else {
            b.style(callback)
        };
        assert_eq!(converted.get(), usize::from(use_class));
        assert_eq!(
            b.construction()
                .nodes()
                .last()
                .unwrap()
                .metadata()
                .attribute("class"),
            Some("css-only")
        );
        let before = calls.get();
        let (native, _) = b.erase().into_parts();
        let mut renderer = ();
        let mut ui = iced_test::runtime::UserInterface::build(
            native,
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
