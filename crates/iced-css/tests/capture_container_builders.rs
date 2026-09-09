use iced::{Length, advanced::widget::Tree};
use iced_css::capture::{CaptureLimits, Captured, adapters};
use std::{cell::Cell, num::NonZeroUsize};
type Native = iced::widget::Container<'static, (), iced::Theme, ()>;
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(100000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
fn captured() -> Captured<Native> {
    adapters::container(
        adapters::space(limits()).unwrap().width(7.0).height(5.0),
        limits(),
    )
    .unwrap()
}
fn native() -> Native {
    iced::widget::Container::new(iced::widget::Space::new().width(7.0).height(5.0))
}
fn rectangles(mut element: iced::Element<'_, (), iced::Theme, ()>) -> Vec<iced::Rectangle> {
    let mut tree = Tree::new(&element);
    let node = element.as_widget_mut().layout(
        &mut tree,
        &(),
        &iced::advanced::layout::Limits::new(iced::Size::ZERO, iced::Size::new(100.0, 100.0)),
    );
    std::iter::once(node.bounds())
        .chain(node.children().iter().map(|n| n.bounds()))
        .collect()
}
#[test]
fn configuration_and_composites_match_stock_container_layout() {
    for mode in 0..8 {
        let c = captured()
            .width(80.0)
            .height(60.0)
            .max_width(70.0)
            .max_height(50.0)
            .padding(3.0)
            .clip(true);
        let p = c.construction().nodes().as_ptr();
        let n = native()
            .width(80.0)
            .height(60.0)
            .max_width(70.0)
            .max_height(50.0)
            .padding(3.0)
            .clip(true);
        let (c, n) = match mode {
            0 => (c.center(40.0), n.center(40.0)),
            1 => (c.center_x(40.0), n.center_x(40.0)),
            2 => (c.center_y(40.0), n.center_y(40.0)),
            3 => (c.align_left(40.0), n.align_left(40.0)),
            4 => (c.align_right(40.0), n.align_right(40.0)),
            5 => (c.align_top(40.0), n.align_top(40.0)),
            6 => (c.align_bottom(40.0), n.align_bottom(40.0)),
            _ => (
                c.align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Bottom),
                n.align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Bottom),
            ),
        };
        assert_eq!(c.construction().nodes().as_ptr(), p);
        assert_eq!(c.construction().nodes().len(), 2);
        assert_eq!(rectangles(c.erase().into_parts().0), rectangles(n.into()));
    }
}
#[test]
fn center_converts_its_argument_once_immediately() {
    struct Dimension<'a>(&'a Cell<usize>);
    impl From<Dimension<'_>> for Length {
        fn from(v: Dimension<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::Fixed(40.0)
        }
    }
    let calls = Cell::new(0);
    let c = captured().center(Dimension(&calls));
    assert_eq!(calls.get(), 1);
    let native = c.erase().into_parts().0;
    assert_eq!(calls.get(), 1);
    assert_eq!(
        native.as_widget().size(),
        iced::Size::new(Length::Fixed(40.0), Length::Fixed(40.0))
    );
}
#[test]
fn borrowed_style_and_native_class_callbacks_run_only_when_drawn() {
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
        let style = |_: &iced::Theme| {
            calls.set(calls.get() + 1);
            iced::widget::container::Style::default()
        };
        let child = adapters::space(limits()).unwrap().width(7.0).height(5.0);
        let c = adapters::container::<(), iced::Theme, (), _>(child, limits()).unwrap();
        let c = if use_class {
            c.class(Class(&converted, Box::new(style)))
        } else {
            c.style(style)
        };
        assert_eq!(converted.get(), usize::from(use_class));
        let before = calls.get();
        let (element, _) = c.erase().into_parts();
        let mut renderer = ();
        let mut ui = iced_test::runtime::UserInterface::build(
            element,
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
#[test]
fn native_id_does_not_change_source_css_id() {
    let n = NonZeroUsize::new(10000).unwrap();
    let attrs = iced_css_core::source::AttributeLimits {
        entries: n,
        retained_bytes: n,
        work: n,
    };
    let c = captured()
        .style_id("css-only", attrs)
        .unwrap()
        .id("native-only");
    let (mut element, construction) = c.erase().into_parts();
    let states = [iced_css::capture::source::ConstructionState::default(); 2];
    let source = iced_css::capture::source::ConstructionSource::new(
        &construction,
        &states,
        iced_css_core::source::selector_tree::DocumentLanguage::Unknown,
        iced_css_core::source::selector_tree::SelectorTreeLimits {
            entries: n,
            retained_bytes: n,
            work: n,
        },
    )
    .unwrap();
    use iced_css_core::selectors::SelectorSource;
    assert_eq!(
        source.facts(1).unwrap().metadata.attribute("id"),
        Some("css-only")
    );
    struct Ids(Vec<iced::advanced::widget::Id>);
    impl iced::advanced::widget::Operation for Ids {
        fn traverse(&mut self, op: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation)) {
            op(self)
        }
        fn container(&mut self, id: Option<&iced::advanced::widget::Id>, _: iced::Rectangle) {
            if let Some(id) = id {
                self.0.push(id.clone());
            }
        }
    }
    let mut tree = Tree::new(&element);
    let node = element.as_widget_mut().layout(
        &mut tree,
        &(),
        &iced::advanced::layout::Limits::new(iced::Size::ZERO, iced::Size::new(100.0, 100.0)),
    );
    let mut ids = Ids(Vec::new());
    element
        .as_widget_mut()
        .operate(&mut tree, iced::advanced::Layout::new(&node), &(), &mut ids);
    assert_eq!(ids.0, [iced::advanced::widget::Id::new("native-only")]);
}
