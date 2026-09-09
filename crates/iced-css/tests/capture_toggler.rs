use iced::{
    Element, Event, Length, Point, Rectangle, Size,
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
fn toggler_label_is_replaceable_source_text_without_a_child_element() {
    struct Label<'a>(&'a str, &'a Cell<usize>);
    impl<'a> iced::advanced::text::IntoFragment<'a> for Label<'a> {
        fn into_fragment(self) -> iced::advanced::text::Fragment<'a> {
            self.1.set(self.1.get() + 1);
            self.0.into()
        }
    }
    let calls = Cell::new(0);
    let text = String::from("authored\r\n中");
    let toggler = adapters::toggler::<bool, iced::Theme, ()>(true, limits()).unwrap();
    assert_eq!(
        toggler.construction().nodes()[0].canonical_type(),
        "toggler"
    );
    assert_eq!(toggler.construction().nodes()[0].text(), None);
    let toggler = toggler.source_key(b"stable", limits()).unwrap();
    let pointer = toggler.construction().nodes().as_ptr();
    let base = toggler.construction().retained_bytes();
    let toggler = toggler.label(Label(&text, &calls), limits()).unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        toggler.construction().nodes()[0].text(),
        Some(text.as_str())
    );
    let toggler = toggler.label(Label("", &calls), limits()).unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(toggler.construction().nodes().len(), 1);
    assert_eq!(
        toggler.construction().nodes()[0].canonical_type(),
        "toggler"
    );
    assert_eq!(toggler.construction().nodes().as_ptr(), pointer);
    assert_eq!(toggler.construction().nodes()[0].text(), Some(""));
    assert_eq!(
        toggler.construction().nodes()[0].source_key(),
        Some(b"stable".as_slice())
    );
    assert_eq!(toggler.construction().retained_bytes(), base);
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        adapters::toggler::<bool, iced::Theme, ()>(false, cap),
        Err(CaptureError::Bytes)
    ));
    let toggler = adapters::toggler::<bool, iced::Theme, ()>(false, limits()).unwrap();
    assert!(matches!(
        toggler.label("excess", cap),
        Err(CaptureError::Bytes)
    ));
    let toggler = adapters::toggler::<bool, iced::Theme, ()>(false, limits()).unwrap();
    cap = limits();
    cap.work = NonZeroUsize::MIN;
    assert!(matches!(
        toggler.label("excess", cap),
        Err(CaptureError::Work)
    ));
}

fn press<Message>(
    mut element: Element<'_, Message, iced::Theme, ()>,
) -> (Rectangle, Vec<Message>, bool) {
    let mut tree = Tree::new(&element);
    let node = element.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
    );
    let bounds = node.bounds();
    let mut messages = vec![];
    let mut shell = iced::advanced::Shell::new(&mut messages);
    element.as_widget_mut().update(
        &mut tree,
        &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Layout::new(&node),
        mouse::Cursor::Available(Point::new(1.0, 1.0)),
        &(),
        &mut iced_test::core::clipboard::Null,
        &mut shell,
        &Rectangle::with_size(Size::new(200.0, 100.0)),
    );
    let captured = shell.is_event_captured();
    (bounds, messages, captured)
}

#[test]
fn toggler_callbacks_remain_deferred_and_none_disables_native_input() {
    for checked in [false, true] {
        for enabled in [false, true] {
            let calls = Cell::new(0);
            let callback = |value| {
                calls.set(calls.get() + 1);
                value
            };
            let captured = adapters::toggler::<bool, iced::Theme, ()>(checked, limits())
                .unwrap()
                .on_toggle(callback)
                .on_toggle_maybe(enabled.then_some(callback))
                .label("label", limits())
                .unwrap()
                .size(20.0)
                .width(80.0)
                .spacing(7.0);
            assert_eq!(calls.get(), 0);
            let actual = press(captured.erase().into_parts().0);
            let stock = iced::widget::Toggler::new(checked)
                .on_toggle_maybe(enabled.then_some(|v| v))
                .label("label")
                .size(20.0)
                .width(80.0)
                .spacing(7.0);
            assert_eq!(actual, press(stock.into()));
            assert_eq!(actual.1, if enabled { vec![!checked] } else { vec![] });
            assert_eq!(actual.2, enabled);
            assert_eq!(calls.get(), usize::from(enabled));
        }
    }
    #[derive(Debug, PartialEq)]
    struct NonClone(bool);
    let control = adapters::toggler::<NonClone, iced::Theme, ()>(false, limits())
        .unwrap()
        .on_toggle(NonClone);
    assert_eq!(
        press(control.erase().into_parts().0).1,
        vec![NonClone(true)]
    );
}

#[test]
fn toggler_native_style_and_class_callbacks_execute_only_when_drawn() {
    let calls = Cell::new(0);
    for use_class in [false, true] {
        let before = calls.get();
        let callback = |theme: &iced::Theme, status| {
            calls.set(calls.get() + 1);
            iced::widget::toggler::default(theme, status)
        };
        let control = adapters::toggler::<bool, iced::Theme, ()>(false, limits()).unwrap();
        let control = if use_class {
            control.class(Box::new(callback) as iced::widget::toggler::StyleFn<'_, iced::Theme>)
        } else {
            control.style(callback)
        };
        assert_eq!(calls.get(), before);
        let mut renderer = ();
        let mut ui = iced_test::runtime::UserInterface::build(
            control.erase().into_parts().0,
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

#[test]
fn toggler_configuration_converts_once_and_style_is_not_eager() {
    struct Dimension<'a>(&'a Cell<usize>);
    impl From<Dimension<'_>> for iced::Pixels {
        fn from(value: Dimension<'_>) -> Self {
            value.0.set(value.0.get() + 1);
            Self(18.0)
        }
    }
    impl From<Dimension<'_>> for Length {
        fn from(value: Dimension<'_>) -> Self {
            value.0.set(value.0.get() + 1);
            Self::Fixed(80.0)
        }
    }
    impl From<Dimension<'_>> for iced::widget::text::LineHeight {
        fn from(value: Dimension<'_>) -> Self {
            value.0.set(value.0.get() + 1);
            Self::Relative(1.2)
        }
    }
    impl From<Dimension<'_>> for iced::Font {
        fn from(value: Dimension<'_>) -> Self {
            value.0.set(value.0.get() + 1);
            Self::DEFAULT
        }
    }
    impl From<Dimension<'_>> for iced::widget::text::Alignment {
        fn from(value: Dimension<'_>) -> Self {
            value.0.set(value.0.get() + 1);
            Self::Center
        }
    }
    let conversions = Cell::new(0);
    let styles = Cell::new(0);
    let toggler = adapters::toggler::<bool, iced::Theme, ()>(false, limits())
        .unwrap()
        .label("label", limits())
        .unwrap()
        .size(Dimension(&conversions))
        .width(Dimension(&conversions))
        .spacing(Dimension(&conversions))
        .text_size(Dimension(&conversions))
        .text_line_height(Dimension(&conversions))
        .font(Dimension(&conversions))
        .text_alignment(Dimension(&conversions))
        .text_shaping(iced::widget::text::Shaping::Advanced)
        .text_wrapping(iced::widget::text::Wrapping::Word)
        .style(|theme, status| {
            styles.set(styles.get() + 1);
            iced::widget::toggler::default(theme, status)
        })
        .class(Box::new(iced::widget::toggler::default)
            as iced::widget::toggler::StyleFn<'_, iced::Theme>);
    assert_eq!(conversions.get(), 7);
    assert_eq!(styles.get(), 0);
    assert_eq!(toggler.construction().nodes()[0].text(), Some("label"));
    assert_eq!(press(toggler.erase().into_parts().0).0.width, 80.0);
    assert_eq!(styles.get(), 0);
}
