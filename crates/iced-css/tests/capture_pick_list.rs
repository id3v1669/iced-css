use iced::{
    Element, Event, Point, Rectangle, Size, Vector,
    advanced::{Layout, layout, widget::Tree},
    keyboard, mouse,
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
struct OptionValue<'a>(u8, &'a RefCell<Vec<&'static str>>);
impl Clone for OptionValue<'_> {
    fn clone(&self) -> Self {
        self.1.borrow_mut().push("clone");
        Self(self.0, self.1)
    }
}
impl PartialEq for OptionValue<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.1.borrow_mut().push("eq");
        self.0 == other.0
    }
}
impl std::fmt::Display for OptionValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.1.borrow_mut().push("display");
        self.0.fmt(f)
    }
}

fn lifecycle(mut element: Element<'_, u8, iced::Theme, ()>) -> (Rectangle, Vec<u8>) {
    let mut tree = Tree::new(&element);
    let viewport = Rectangle::with_size(Size::new(200.0, 200.0));
    let node = element.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    assert!(
        element
            .as_widget_mut()
            .overlay(&mut tree, Layout::new(&node), &(), &viewport, Vector::ZERO)
            .is_none()
    );
    element.as_widget().draw(
        &tree,
        &mut (),
        &iced::Theme::Dark,
        &iced::advanced::renderer::Style {
            text_color: iced::Color::BLACK,
        },
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &viewport,
    );
    let mut messages = vec![];
    for index in 0..2 {
        let mut shell = iced::advanced::Shell::new(&mut messages);
        element.as_widget_mut().update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(1.0, 1.0)),
            &(),
            &mut iced_test::core::clipboard::Null,
            &mut shell,
            &viewport,
        );
        assert!(shell.is_event_captured());
        if index == 0 {
            let mut overlay = element
                .as_widget_mut()
                .overlay(&mut tree, Layout::new(&node), &(), &viewport, Vector::ZERO)
                .expect("native menu opens");
            let layout = overlay.as_overlay_mut().layout(&(), viewport.size());
            overlay.as_overlay().draw(
                &mut (),
                &iced::Theme::Dark,
                &iced::advanced::renderer::Style {
                    text_color: iced::Color::BLACK,
                },
                Layout::new(&layout),
                mouse::Cursor::Unavailable,
            );
        }
    }
    assert!(
        element
            .as_widget_mut()
            .overlay(&mut tree, Layout::new(&node), &(), &viewport, Vector::ZERO)
            .is_none()
    );
    for event in [
        Event::Keyboard(keyboard::Event::ModifiersChanged(
            keyboard::Modifiers::CTRL | keyboard::Modifiers::LOGO,
        )),
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
        }),
    ] {
        let mut shell = iced::advanced::Shell::new(&mut messages);
        element.as_widget_mut().update(
            &mut tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(1.0, 1.0)),
            &(),
            &mut iced_test::core::clipboard::Null,
            &mut shell,
            &viewport,
        );
    }
    (node.bounds(), messages)
}

#[test]
fn borrowed_options_are_not_inspected_until_native_layout_and_input() {
    for selected in [false, true] {
        let events = RefCell::new(vec![]);
        let options = [OptionValue(1, &events), OptionValue(2, &events)];
        let value = selected.then_some(&options[0]);
        let pick = adapters::pick_list::<_, _, _, _, iced::Theme, ()>(
            &options[..],
            value,
            |v| {
                events.borrow_mut().push("select");
                v.0
            },
            limits(),
        )
        .unwrap()
        .on_open(100)
        .on_close(101);
        assert!(events.borrow().is_empty());
        let (element, source) = pick.erase().into_parts();
        assert_eq!(source.nodes().len(), 1);
        assert_eq!(source.nodes()[0].canonical_type(), "pick_list");
        assert_eq!(source.nodes()[0].text(), None);
        let actual = lifecycle(element);
        assert_eq!(source.nodes().len(), 1);
        assert_eq!(actual.1, vec![100, 101, if selected { 2 } else { 1 }]);
        assert_eq!(
            events
                .borrow()
                .iter()
                .filter(|&&event| event == "clone")
                .count(),
            1
        );
        assert_eq!(
            events
                .borrow()
                .iter()
                .filter(|&&event| event == "select")
                .count(),
            1
        );
        let stock_events = RefCell::new(vec![]);
        let stock_options = [OptionValue(1, &stock_events), OptionValue(2, &stock_events)];
        let stock = iced::widget::PickList::new(
            &stock_options[..],
            selected.then_some(&stock_options[0]),
            |v: OptionValue<'_>| {
                stock_events.borrow_mut().push("select");
                v.0
            },
        )
        .on_open(100)
        .on_close(101);
        assert!(stock_events.borrow().is_empty());
        assert_eq!(actual, lifecycle(stock.into()));
        assert_eq!(*events.borrow(), *stock_events.borrow());
    }
}

#[test]
fn pick_list_builders_convert_once_and_draw_control_and_menu_styles_later() {
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
    impl From<Converted<'_>> for iced::Padding {
        fn from(v: Converted<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::new(4.0)
        }
    }
    impl From<Converted<'_>> for iced::Font {
        fn from(v: Converted<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::DEFAULT
        }
    }
    impl From<Converted<'_>> for String {
        fn from(v: Converted<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::from("placeholder")
        }
    }
    impl From<Converted<'_>> for iced::widget::text::LineHeight {
        fn from(v: Converted<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::Relative(1.2)
        }
    }
    for classes in [false, true] {
        let converted = Cell::new(0);
        let control = Cell::new(0);
        let menu = Cell::new(0);
        let pick = adapters::pick_list::<_, _, _, _, iced::Theme, ()>(
            [1u8, 2],
            Some(1u8),
            |v| v,
            limits(),
        )
        .unwrap()
        .placeholder(Converted(&converted))
        .width(Converted(&converted))
        .menu_height(Converted(&converted))
        .padding(Converted(&converted))
        .text_size(Converted(&converted))
        .text_line_height(Converted(&converted))
        .font(Converted(&converted))
        .text_shaping(iced::widget::text::Shaping::Advanced)
        .handle(iced::widget::pick_list::Handle::None)
        .on_open(100)
        .on_close(101);
        assert_eq!(converted.get(), 7);
        let control_style = |theme: &iced::Theme, status| {
            control.set(control.get() + 1);
            iced::widget::pick_list::default(theme, status)
        };
        let menu_style = |theme: &iced::Theme| {
            menu.set(menu.get() + 1);
            iced::widget::overlay::menu::default(theme)
        };
        let pick = if classes {
            pick.class(Box::new(control_style) as iced::widget::pick_list::StyleFn<'_, iced::Theme>)
                .menu_class(
                    Box::new(menu_style) as iced::widget::overlay::menu::StyleFn<'_, iced::Theme>
                )
        } else {
            pick.style(control_style).menu_style(menu_style)
        };
        assert_eq!(control.get(), 0);
        assert_eq!(menu.get(), 0);
        let result = lifecycle(pick.erase().into_parts().0);
        assert_eq!(result.0.width, 80.0);
        assert_eq!(result.1, vec![100, 101, 2]);
        assert!(control.get() > 0);
        assert!(menu.get() > 0);
    }
}

#[test]
fn pick_list_evidence_limit_does_not_stringify_or_select_options() {
    let events = RefCell::new(vec![]);
    let options = [OptionValue(1, &events)];
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::MIN;
    let result = adapters::pick_list::<_, _, _, _, iced::Theme, ()>(
        &options[..],
        Some(&options[0]),
        |v| {
            events.borrow_mut().push("select");
            v.0
        },
        cap,
    );
    assert!(matches!(result, Err(CaptureError::Bytes)));
    assert!(events.borrow().is_empty());
}
