use iced::{
    Element, Length, Rectangle, Size,
    advanced::{Layout, layout, mouse, renderer, widget::Tree},
    widget::text::{self, Rich, Span},
};
use iced_css::capture::{CaptureError, CaptureLimits, adapters};
use std::{cell::Cell, num::NonZeroUsize, rc::Rc};
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
thread_local! { static DEFAULTS:Cell<usize>=const {Cell::new(0)}; static STYLES:Cell<usize>=const {Cell::new(0)}; }
struct Theme;
impl text::Catalog for Theme {
    type Class<'a> = text::StyleFn<'a, Self>;
    fn default<'a>() -> Self::Class<'a> {
        DEFAULTS.set(DEFAULTS.get() + 1);
        Box::new(|_| text::Style::default())
    }
    fn style(&self, class: &Self::Class<'_>) -> text::Style {
        STYLES.set(STYLES.get() + 1);
        class(self)
    }
}
struct Link(Rc<Cell<usize>>);
impl Clone for Link {
    fn clone(&self) -> Self {
        self.0.set(self.0.get() + 1);
        Self(self.0.clone())
    }
}
fn observe(mut e: Element<'_, (), Theme, ()>) -> (Size<Length>, Rectangle, usize) {
    let hint = e.as_widget().size_hint();
    let mut tree = Tree::new(&e);
    let node = e.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
    );
    e.as_widget().draw(
        &tree,
        &mut (),
        &Theme,
        &renderer::Style::default(),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &Rectangle::new(iced::Point::ORIGIN, Size::new(200.0, 100.0)),
    );
    (hint, node.bounds(), tree.children.len())
}
#[test]
fn owned_and_borrowed_spans_have_one_source_and_exact_utf8_run_boundaries() {
    let calls = Rc::new(Cell::new(0));
    let text = String::from("é");
    let spans: Vec<Span<'_, Link, iced::Font>> = vec![
        Span::new(text.as_str()).link(Link(calls.clone())),
        Span::new(""),
        Span::new("中\r\n"),
    ];
    DEFAULTS.set(0);
    let captured = adapters::rich_text_slice::<_, (), Theme, ()>(&spans, limits()).unwrap();
    assert_eq!(DEFAULTS.get(), 1);
    assert_eq!(calls.get(), 0);
    let nodes = captured.construction().nodes();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].canonical_type(), "rich_text");
    assert_eq!(nodes[0].text(), Some("é中\r\n"));
    assert_eq!(nodes[0].text_runs().unwrap(), [0..2, 2..2, 2..7]);
    let (native, construction) = captured.into_parts();
    drop(native);
    drop(spans);
    drop(text);
    assert_eq!(construction.nodes()[0].text(), Some("é中\r\n"));
    assert_eq!(calls.get(), 0);
    let native_spans: Vec<Span<'_, (), iced::Font>> = vec![Span::new("one"), Span::new("two")];
    let captured = adapters::rich_text_vec::<_, (), Theme, ()>(native_spans, limits()).unwrap();
    assert_eq!(
        captured.construction().nodes()[0].text_runs().unwrap(),
        [0..3, 3..6]
    );
    let empty = adapters::rich_text_new::<(), (), Theme, ()>(limits()).unwrap();
    assert_eq!(empty.construction().nodes()[0].text(), Some(""));
    assert!(
        empty.construction().nodes()[0]
            .text_runs()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn evidence_budget_rejects_before_theme_default_and_preserves_empty_runs() {
    let spans: Vec<Span<'_, (), iced::Font>> = vec![Span::new("a"), Span::new("")];
    for limited in [
        CaptureLimits {
            retained_bytes: NonZeroUsize::new(1).unwrap(),
            ..limits()
        },
        CaptureLimits {
            work: NonZeroUsize::new(5).unwrap(),
            ..limits()
        },
    ] {
        DEFAULTS.set(0);
        assert!(matches!(
            adapters::rich_text_slice::<_, (), Theme, ()>(&spans, limited),
            Err(CaptureError::Bytes | CaptureError::Work)
        ));
        assert_eq!(DEFAULTS.get(), 0);
    }
    let captured = adapters::rich_text_slice::<_, (), Theme, ()>(
        &spans,
        CaptureLimits {
            work: NonZeroUsize::new(6).unwrap(),
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(
        captured.construction().nodes()[0].text_runs().unwrap(),
        [0..1, 1..1]
    );
    let mut small = limits();
    small.retained_bytes = NonZeroUsize::new(captured.construction().retained_bytes() - 1).unwrap();
    DEFAULTS.set(0);
    assert!(matches!(
        adapters::rich_text_slice::<_, (), Theme, ()>(&spans, small),
        Err(CaptureError::Bytes)
    ));
    assert_eq!(DEFAULTS.get(), 0);
}
#[test]
fn native_configuration_and_rendering_match_stock_without_eager_link_callbacks() {
    fn spans() -> Vec<Span<'static, u8, iced::Font>> {
        vec![
            Span::new("hello").link(1).underline(true),
            Span::new(" world").color(iced::Color::BLACK),
        ]
    }
    let links = Cell::new(0);
    DEFAULTS.set(0);
    STYLES.set(0);
    let captured = adapters::rich_text_vec::<_, (), Theme, ()>(spans(), limits())
        .unwrap()
        .size(18)
        .line_height(1.5)
        .font(iced::Font::DEFAULT)
        .width(120)
        .height(30)
        .center()
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Bottom)
        .wrapping(text::Wrapping::Word)
        .on_link_click(|_| links.set(links.get() + 1))
        .color(iced::Color::WHITE)
        .color_maybe(None::<iced::Color>)
        .style(|_| text::Style {
            color: Some(iced::Color::BLACK),
        })
        .class(Box::new(|_: &Theme| text::Style::default()) as text::StyleFn<'_, Theme>);
    assert_eq!(DEFAULTS.get(), 1);
    assert_eq!(STYLES.get(), 0);
    assert_eq!(links.get(), 0);
    let observed = observe(captured.erase().into_parts().0);
    let style_calls = STYLES.get();
    assert!(style_calls > 0);
    assert_eq!(links.get(), 0);
    DEFAULTS.set(0);
    STYLES.set(0);
    let native: Rich<'_, u8, (), Theme, ()> = Rich::with_spans(spans());
    let native = native
        .size(18)
        .line_height(1.5)
        .font(iced::Font::DEFAULT)
        .width(120)
        .height(30)
        .center()
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Bottom)
        .wrapping(text::Wrapping::Word)
        .on_link_click(|_| links.set(links.get() + 1))
        .color(iced::Color::WHITE)
        .color_maybe(None::<iced::Color>)
        .style(|_| text::Style {
            color: Some(iced::Color::BLACK),
        })
        .class(Box::new(|_: &Theme| text::Style::default()) as text::StyleFn<'_, Theme>);
    assert_eq!(DEFAULTS.get(), 1);
    assert_eq!(observed, observe(native.into()));
    assert_eq!(STYLES.get(), style_calls);
    assert_eq!(links.get(), 0);
}

#[test]
fn scalar_conversions_follow_native_order_and_optional_none_skips_conversion() {
    use std::cell::RefCell;
    type Calls = Rc<RefCell<Vec<&'static str>>>;
    struct Value(Calls);
    impl From<Value> for iced::Pixels {
        fn from(v: Value) -> Self {
            v.0.borrow_mut().push("size");
            Self(18.0)
        }
    }
    impl From<Value> for text::LineHeight {
        fn from(v: Value) -> Self {
            v.0.borrow_mut().push("height");
            Self::Relative(1.5)
        }
    }
    impl From<Value> for iced::Font {
        fn from(v: Value) -> Self {
            v.0.borrow_mut().push("font");
            Self::DEFAULT
        }
    }
    impl From<Value> for iced::Color {
        fn from(v: Value) -> Self {
            v.0.borrow_mut().push("color");
            Self::BLACK
        }
    }
    let calls = Calls::default();
    let captured = adapters::rich_text_new::<(), (), Theme, ()>(limits())
        .unwrap()
        .size(Value(calls.clone()))
        .line_height(Value(calls.clone()))
        .font(Value(calls.clone()))
        .color(Value(calls.clone()))
        .color_maybe(Some(Value(calls.clone())))
        .color_maybe(None::<Value>);
    let native: Rich<'_, (), (), Theme, ()> = Rich::new();
    let expected = calls.borrow().clone();
    calls.borrow_mut().clear();
    let native = native
        .size(Value(calls.clone()))
        .line_height(Value(calls.clone()))
        .font(Value(calls.clone()))
        .color(Value(calls.clone()))
        .color_maybe(Some(Value(calls.clone())))
        .color_maybe(None::<Value>);
    assert_eq!(expected, ["size", "height", "font", "color", "color"]);
    assert_eq!(*calls.borrow(), expected);
    assert_eq!(
        observe(captured.erase().into_parts().0),
        observe(native.into())
    );
    assert_eq!(*calls.borrow(), expected);
}
