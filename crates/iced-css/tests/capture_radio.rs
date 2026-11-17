use iced::{
    Element, Event, Point, Rectangle, Size,
    advanced::{Layout, layout, widget::Tree},
    mouse,
};
use iced_css::capture::{CaptureError, CaptureLimits, adapters};
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
};
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
#[derive(Copy, Clone)]
struct Value<'a>(u8, &'a RefCell<Vec<&'static str>>);
impl PartialEq for Value<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.1.borrow_mut().push("equal");
        self.0 == other.0
    }
}
impl Eq for Value<'_> {}
struct Label<'a>(&'a RefCell<Vec<&'static str>>);
impl From<Label<'_>> for String {
    fn from(value: Label<'_>) -> Self {
        value.0.borrow_mut().push("label");
        String::from("radio label")
    }
}
struct Message<'a>(u8, &'a RefCell<Vec<&'static str>>);
impl Clone for Message<'_> {
    fn clone(&self) -> Self {
        self.1.borrow_mut().push("clone");
        Self(self.0, self.1)
    }
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
    (node.bounds(), messages, captured)
}
#[test]
fn radio_preserves_selection_factory_label_and_message_clone_order() {
    for selected in [None, Some(1), Some(2)] {
        let events = RefCell::new(vec![]);
        let token = Box::new(());
        let factory = |value: Value<'_>| {
            drop(token);
            events.borrow_mut().push("factory");
            Message(value.0, &events)
        };
        let radio = adapters::radio::<_, iced::Theme, (), _>(
            Label(&events),
            Value(1, &events),
            selected.map(|v| Value(v, &events)),
            factory,
            limits(),
        )
        .unwrap();
        // One comparison of the adapter's own, for `:checked`, before iced's.
        let expected = if selected.is_some() {
            vec!["equal", "equal", "factory", "label"]
        } else {
            vec!["factory", "label"]
        };
        assert_eq!(*events.borrow(), expected);
        assert_eq!(radio.construction().nodes().len(), 1);
        assert_eq!(radio.construction().nodes()[0].canonical_type(), "radio");
        assert_eq!(radio.construction().nodes()[0].text(), Some("radio label"));
        let actual = press(radio.erase().into_parts().0);
        assert_eq!(actual.1.len(), 1);
        assert_eq!(actual.1[0].0, 1);
        assert!(actual.2);
        let mut full = expected;
        full.push("clone");
        assert_eq!(*events.borrow(), full);
        let stock_events = RefCell::new(vec![]);
        let stock = iced::widget::Radio::new(
            Label(&stock_events),
            Value(1, &stock_events),
            selected.map(|v| Value(v, &stock_events)),
            |value| {
                stock_events.borrow_mut().push("factory");
                Message(value.0, &stock_events)
            },
        );
        let native = press(stock.into());
        assert_eq!(actual.0, native.0);
        assert_eq!(native.1[0].0, 1);
        assert!(native.2);
        // The stock widget makes the same calls but the adapter's own comparison.
        let mut stock_full = full.clone();
        if selected.is_some() {
            stock_full.remove(0);
        }
        assert_eq!(*stock_events.borrow(), stock_full);
    }
}
#[test]
fn radio_evidence_failure_reports_after_native_conversion_phases() {
    for bytes in [false, true] {
        let events = RefCell::new(vec![]);
        let mut cap = limits();
        if bytes {
            cap.retained_bytes = NonZeroUsize::MIN;
        } else {
            cap.work = NonZeroUsize::MIN;
        }
        let result = adapters::radio::<_, iced::Theme, (), _>(
            Label(&events),
            Value(1, &events),
            None,
            |v| {
                events.borrow_mut().push("factory");
                v.0
            },
            cap,
        );
        if bytes {
            assert!(matches!(result, Err(CaptureError::Bytes)));
        } else {
            assert!(matches!(result, Err(CaptureError::Work)));
        }
        assert_eq!(*events.borrow(), ["factory", "label"]);
    }
}
#[test]
fn radio_builders_preserve_conversions_and_deferred_native_drawing() {
    struct Converted<'a>(&'a Cell<usize>);
    impl From<Converted<'_>> for iced::Pixels {
        fn from(v: Converted<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self(18.0)
        }
    }
    impl From<Converted<'_>> for iced::Length {
        fn from(v: Converted<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::Fixed(80.0)
        }
    }
    impl From<Converted<'_>> for iced::Font {
        fn from(v: Converted<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::DEFAULT
        }
    }
    impl From<Converted<'_>> for iced::widget::text::LineHeight {
        fn from(v: Converted<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::Relative(1.2)
        }
    }
    for use_class in [false, true] {
        let converted = Cell::new(0);
        let drawn = Cell::new(0);
        let radio = adapters::radio::<_, iced::Theme, (), _>("label", 1, None, |v| v, limits())
            .unwrap()
            .size(Converted(&converted))
            .width(Converted(&converted))
            .spacing(Converted(&converted))
            .text_size(Converted(&converted))
            .text_line_height(Converted(&converted))
            .font(Converted(&converted))
            .text_shaping(iced::widget::text::Shaping::Advanced)
            .text_wrapping(iced::widget::text::Wrapping::Word);
        assert_eq!(converted.get(), 6);
        let style = |theme: &iced::Theme, status| {
            drawn.set(drawn.get() + 1);
            iced::widget::radio::default(theme, status)
        };
        let radio = if use_class {
            radio.class(Box::new(style) as iced::widget::radio::StyleFn<'_, iced::Theme>)
        } else {
            radio.style(style)
        };
        assert_eq!(drawn.get(), 0);
        let mut renderer = ();
        let mut ui = iced_test::runtime::UserInterface::build(
            radio.erase().into_parts().0,
            Size::new(100.0, 80.0),
            iced_test::runtime::user_interface::Cache::default(),
            &mut renderer,
        );
        assert_eq!(drawn.get(), 0);
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style {
                text_color: iced::Color::BLACK,
            },
            mouse::Cursor::Unavailable,
        );
        assert_eq!(drawn.get(), 1);
    }
}
