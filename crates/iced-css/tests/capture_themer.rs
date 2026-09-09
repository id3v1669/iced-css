use iced::{Background, Color, Element, Size, advanced::widget::Tree, mouse};
use iced_css::capture::{CaptureError, CaptureLimits, adapters};
use iced_test::core::theme::{Base, Mode, Palette, Style};
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
};
thread_local! { static TRACE:RefCell<Vec<&'static str>>=const {RefCell::new(Vec::new())}; }
fn record(s: &'static str) {
    TRACE.with(|t| t.borrow_mut().push(s));
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Inner {
    Light,
    Dark,
}
impl Base for Inner {
    fn default(mode: Mode) -> Self {
        record("default");
        if mode == Mode::Dark {
            Self::Dark
        } else {
            Self::Light
        }
    }
    fn mode(&self) -> Mode {
        if *self == Self::Dark {
            Mode::Dark
        } else {
            Mode::Light
        }
    }
    fn base(&self) -> Style {
        Style {
            background_color: Color::BLACK,
            text_color: Color::WHITE,
        }
    }
    fn palette(&self) -> Option<Palette> {
        None
    }
    fn name(&self) -> &str {
        match self {
            Self::Light => "inner-light",
            Self::Dark => "inner-dark",
        }
    }
}
impl iced::widget::container::Catalog for Inner {
    type Class<'a> = iced::widget::container::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        Box::new(|_| iced::widget::container::Style::default())
    }
    fn style(&self, class: &Self::Class<'_>) -> iced::widget::container::Style {
        class(self)
    }
}
struct Outer(Mode);
impl Base for Outer {
    fn default(mode: Mode) -> Self {
        Self(mode)
    }
    fn mode(&self) -> Mode {
        self.0
    }
    fn base(&self) -> Style {
        Style {
            background_color: Color::WHITE,
            text_color: Color::BLACK,
        }
    }
    fn palette(&self) -> Option<Palette> {
        None
    }
    fn name(&self) -> &str {
        "outer"
    }
}
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
fn background(_: &Inner) -> Background {
    record("background");
    Color::BLACK.into()
}
fn text_color(_: &Inner) -> Color {
    record("text");
    Color::WHITE
}
#[test]
fn source_is_distinct_while_native_state_and_cross_theme_erasure_are_preserved() {
    let child =
        adapters::container::<(), Inner, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let t = adapters::themer(Some(Inner::Dark), child, limits()).unwrap();
    let n = t.construction().nodes();
    assert_eq!(
        n.iter().map(|n| n.canonical_type()).collect::<Vec<_>>(),
        ["space", "container", "themer"]
    );
    assert_eq!(n[1].parent(), Some(2));
    assert_eq!(n[1].role(), Some("content"));
    let (native, _) = t.erase::<(), Outer, ()>().into_parts();
    let tree = Tree::new(&native);
    let child: Element<'_, (), Inner, ()> =
        iced::widget::Container::new(iced::widget::Space::new()).into();
    let child_tree = Tree::new(&child);
    assert_eq!(tree.tag, child_tree.tag);
    assert_eq!(tree.children.len(), child_tree.children.len());
}
#[test]
fn explicit_and_default_inner_themes_and_function_pointer_order_match_native() {
    for mode in [Mode::Light, Mode::Dark] {
        for selected in [None, Some(Inner::Dark)] {
            let expected = selected.unwrap_or(if mode == Mode::Dark {
                Inner::Dark
            } else {
                Inner::Light
            });
            for captured in [false, true] {
                TRACE.with(|t| t.borrow_mut().clear());
                let observed = Cell::new(None);
                let style = |theme: &Inner| {
                    record("content");
                    observed.set(Some(*theme));
                    iced::widget::container::Style::default()
                };
                let native: Element<'_, (), Outer, ()> = if captured {
                    let child = adapters::container::<(), Inner, (), _>(
                        adapters::space(limits()).unwrap().width(10.0).height(10.0),
                        limits(),
                    )
                    .unwrap()
                    .style(style);
                    let t = adapters::themer(selected, child, limits()).unwrap();
                    let ptr = t.construction().nodes().as_ptr();
                    let t = t.text_color(text_color).background(background);
                    assert_eq!(t.construction().nodes().as_ptr(), ptr);
                    t.erase().into_parts().0
                } else {
                    iced::widget::Themer::new(
                        selected,
                        iced::widget::Container::new(
                            iced::widget::Space::new().width(10.0).height(10.0),
                        )
                        .style(style),
                    )
                    .text_color(text_color)
                    .background(background)
                    .into()
                };
                TRACE.with(|t| assert!(t.borrow().is_empty()));
                let mut renderer = ();
                let mut ui = iced_test::runtime::UserInterface::build(
                    native,
                    Size::new(100.0, 100.0),
                    iced_test::runtime::user_interface::Cache::default(),
                    &mut renderer,
                );
                TRACE.with(|t| assert!(t.borrow().is_empty()));
                ui.draw(
                    &mut renderer,
                    &Outer(mode),
                    &iced::advanced::renderer::Style {
                        text_color: Color::BLACK,
                    },
                    mouse::Cursor::Unavailable,
                );
                TRACE.with(|t| {
                    assert_eq!(
                        t.borrow().as_slice(),
                        ["default", "background", "text", "content"]
                    )
                });
                assert_eq!(observed.get(), Some(expected));
            }
        }
    }
}
#[test]
fn themer_checks_aggregate_source_bounds() {
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
            matches!(adapters::themer::<(),Inner,(),_>(None,adapters::space(limits()).unwrap(),cap),Err(actual) if actual==error)
        );
    }
}
