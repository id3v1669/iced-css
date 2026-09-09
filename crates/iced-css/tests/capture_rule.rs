use iced::{
    Element, Length, Rectangle, Size,
    advanced::{layout, widget::Tree},
    mouse,
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
fn bounds(mut e: Element<'_, (), iced::Theme, ()>) -> Rectangle {
    let mut tree = Tree::new(&e);
    assert!(tree.children.is_empty());
    e.as_widget_mut()
        .layout(
            &mut tree,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(100.0, 80.0)),
        )
        .bounds()
}
#[test]
fn both_orientations_have_one_rule_source_and_match_native_bounds() {
    for vertical in [false, true] {
        let r = if vertical {
            adapters::vertical_rule::<iced::Theme>(3.0, limits())
        } else {
            adapters::horizontal_rule::<iced::Theme>(3.0, limits())
        }
        .unwrap();
        let nodes = r.construction().nodes();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].canonical_type(), "rule");
        assert_eq!(nodes[0].parent(), None);
        assert_eq!(nodes[0].text(), None);
        let (native, _) = r.erase::<(), iced::Theme, ()>().into_parts();
        assert_eq!(
            native.as_widget().size(),
            if vertical {
                Size::new(Length::Fixed(3.0), Length::Fill)
            } else {
                Size::new(Length::Fill, Length::Fixed(3.0))
            }
        );
        let stock = if vertical {
            iced::widget::rule::vertical(3.0)
        } else {
            iced::widget::rule::horizontal(3.0)
        };
        assert_eq!(bounds(native), bounds(stock.into()));
    }
}
#[test]
fn thickness_converts_once_after_leaf_admission() {
    struct Pixels<'a>(&'a Cell<usize>);
    impl From<Pixels<'_>> for iced::Pixels {
        fn from(v: Pixels<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self(3.0)
        }
    }
    let calls = Cell::new(0);
    let r = adapters::horizontal_rule::<iced::Theme>(Pixels(&calls), limits()).unwrap();
    assert_eq!(calls.get(), 1);
    let _ = r.erase::<(), iced::Theme, ()>();
    assert_eq!(calls.get(), 1);
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        adapters::vertical_rule::<iced::Theme>(Pixels(&calls), cap),
        Err(CaptureError::Bytes)
    ));
    assert_eq!(calls.get(), 1);
}
#[test]
fn zero_thickness_rules_survive_managed_row_push() {
    let row = adapters::row_from_vec(
        Vec::<CapturedElement<'_, (), iced::Theme, ()>>::new(),
        limits(),
    )
    .unwrap()
    .push(
        adapters::horizontal_rule::<iced::Theme>(0.0, limits()).unwrap(),
        limits(),
    )
    .unwrap()
    .push(
        adapters::vertical_rule::<iced::Theme>(0.0, limits()).unwrap(),
        limits(),
    )
    .unwrap();
    let n = row.construction().nodes();
    assert_eq!(n.len(), 3);
    assert_eq!(n[0].canonical_type(), "rule");
    assert_eq!(n[1].canonical_type(), "rule");
    assert_eq!(n[0].role_position(), Some(0));
    assert_eq!(n[1].role_position(), Some(1));
    let (native, _) = row.erase().into_parts();
    assert_eq!(Tree::new(&native).children.len(), 2);
}
#[test]
fn borrowed_rule_styles_and_native_classes_are_drawn_later() {
    let calls = Cell::new(0);
    let converted = Cell::new(0);
    struct Class<'a>(
        &'a Cell<usize>,
        iced::widget::rule::StyleFn<'a, iced::Theme>,
    );
    impl<'a> From<Class<'a>> for iced::widget::rule::StyleFn<'a, iced::Theme> {
        fn from(v: Class<'a>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    for use_class in [false, true] {
        let callback = |theme: &iced::Theme| {
            calls.set(calls.get() + 1);
            iced::widget::rule::default(theme)
        };
        let r = adapters::horizontal_rule::<iced::Theme>(3.0, limits()).unwrap();
        let ptr = r.construction().nodes().as_ptr();
        let before = calls.get();
        let r = if use_class {
            r.class(Class(&converted, Box::new(callback)))
        } else {
            r.style(callback)
        };
        assert_eq!(r.construction().nodes().as_ptr(), ptr);
        assert_eq!(calls.get(), before);
        assert_eq!(converted.get(), usize::from(use_class));
        let mut renderer = ();
        let mut ui = iced_test::runtime::UserInterface::build(
            r.erase::<(), iced::Theme, ()>().into_parts().0,
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
