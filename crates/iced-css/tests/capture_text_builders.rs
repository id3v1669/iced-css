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
#[test]
fn optional_and_direct_conversions_are_eager_once_without_changing_text_evidence() {
    struct Font<'a>(&'a Cell<usize>);
    impl From<Font<'_>> for iced::Font {
        fn from(v: Font<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::DEFAULT
        }
    }
    struct Color<'a>(&'a Cell<usize>);
    impl From<Color<'_>> for iced::Color {
        fn from(v: Color<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::BLACK
        }
    }
    struct Pixels<'a>(&'a Cell<usize>);
    impl From<Pixels<'_>> for iced::Pixels {
        fn from(v: Pixels<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self(18.0)
        }
    }
    struct Height<'a>(&'a Cell<usize>);
    impl From<Height<'_>> for iced::widget::text::LineHeight {
        fn from(v: Height<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::Relative(1.5)
        }
    }
    let calls = Cell::new(0);
    let text = String::from("A\r\n中");
    let t = adapters::text::<iced::Theme, ()>(&text, limits()).unwrap();
    let p = t.construction().nodes().as_ptr();
    let t = t
        .size(Pixels(&calls))
        .line_height(Height(&calls))
        .font(Font(&calls))
        .font_maybe(Some(Font(&calls)))
        .font_maybe(None::<Font<'_>>)
        .color(Color(&calls))
        .color_maybe(Some(Color(&calls)))
        .color_maybe(None::<Color<'_>>);
    assert_eq!(calls.get(), 6);
    assert_eq!(t.construction().nodes().as_ptr(), p);
    assert_eq!(t.construction().nodes()[0].text(), Some(text.as_str()));
    let _: iced_css::capture::CapturedElement<'_, (), iced::Theme, ()> = t.erase();
    assert_eq!(calls.get(), 6);
}
#[test]
fn configured_native_bounds_match_stock_text_with_identical_options() {
    fn bounds(mut e: iced::Element<'_, (), iced::Theme, ()>) -> iced::Rectangle {
        let mut tree = Tree::new(&e);
        e.as_widget_mut()
            .layout(
                &mut tree,
                &(),
                &iced::advanced::layout::Limits::new(
                    iced::Size::ZERO,
                    iced::Size::new(100.0, 100.0),
                ),
            )
            .bounds()
    }
    let t = adapters::text::<iced::Theme, ()>("wrapped text", limits())
        .unwrap()
        .width(Length::Fixed(40.0))
        .height(30.0)
        .size(18.0)
        .line_height(iced::widget::text::LineHeight::Relative(1.5))
        .font(iced::Font::DEFAULT)
        .center()
        .align_x(iced::widget::text::Alignment::Right)
        .align_y(iced::alignment::Vertical::Bottom)
        .shaping(iced::widget::text::Shaping::Advanced)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph);
    let n = iced::widget::Text::<iced::Theme, ()>::new("wrapped text")
        .width(Length::Fixed(40.0))
        .height(30.0)
        .size(18.0)
        .line_height(iced::widget::text::LineHeight::Relative(1.5))
        .font(iced::Font::DEFAULT)
        .center()
        .align_x(iced::widget::text::Alignment::Right)
        .align_y(iced::alignment::Vertical::Bottom)
        .shaping(iced::widget::text::Shaping::Advanced)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph);
    assert_eq!(bounds(t.erase().into_parts().0), bounds(n.into()));
}
#[test]
fn borrowed_style_and_native_class_are_not_evaluated_until_drawing() {
    let calls = Cell::new(0);
    let converted = Cell::new(0);
    struct Class<'a>(
        &'a Cell<usize>,
        iced::widget::text::StyleFn<'a, iced::Theme>,
    );
    impl<'a> From<Class<'a>> for iced::widget::text::StyleFn<'a, iced::Theme> {
        fn from(v: Class<'a>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    for use_class in [false, true] {
        let callback = |_: &iced::Theme| {
            calls.set(calls.get() + 1);
            iced::widget::text::Style::default()
        };
        let text = String::from("borrowed");
        let t = adapters::text::<iced::Theme, ()>(&text, limits())
            .unwrap()
            .width(30.0)
            .height(20.0);
        let t = if use_class {
            t.class(Class(&converted, Box::new(callback)))
        } else {
            t.style(callback)
        };
        assert_eq!(converted.get(), usize::from(use_class));
        let before = calls.get();
        let (native, _) = t.erase::<(), iced::Theme, ()>().into_parts();
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
