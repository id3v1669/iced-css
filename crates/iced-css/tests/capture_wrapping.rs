use iced::{Length, advanced::widget::Tree};
use iced_css::capture::{CaptureLimits, CapturedElement, adapters};
use std::{cell::Cell, num::NonZeroUsize};
type Child = CapturedElement<'static, (), iced::Theme, ()>;
fn limits() -> CaptureLimits {
    let nz = |n| NonZeroUsize::new(n).unwrap();
    CaptureLimits {
        nodes: nz(100),
        depth: nz(100),
        retained_bytes: nz(100000),
        work: nz(100000),
    }
}
fn children() -> Vec<Child> {
    [(0.0, 0.0), (12.0, 8.0), (10.0, 11.0), (15.0, 9.0)]
        .into_iter()
        .map(|(w, h)| {
            adapters::space(limits())
                .unwrap()
                .width(w)
                .height(h)
                .erase()
        })
        .collect()
}
fn native_children() -> Vec<iced::Element<'static, (), iced::Theme, ()>> {
    [(0.0, 0.0), (12.0, 8.0), (10.0, 11.0), (15.0, 9.0)]
        .into_iter()
        .map(|(w, h)| iced::widget::Space::new().width(w).height(h).into())
        .collect()
}
fn rectangles(mut element: iced::Element<'_, (), iced::Theme, ()>) -> Vec<iced::Rectangle> {
    let mut tree = Tree::new(&element);
    assert_eq!(tree.children.len(), 4);
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
fn row_wrap_preserves_source_and_matches_native_wrapped_layout() {
    for explicit in [false, true] {
        let row = adapters::row_from_vec(children(), limits())
            .unwrap()
            .width(30.0)
            .height(80.0)
            .padding(2.0)
            .spacing(3.0)
            .align_y(iced::alignment::Vertical::Center)
            .clip(true);
        let pointer = row.construction().nodes().as_ptr();
        let wrapped = row.wrap();
        assert_eq!(wrapped.construction().nodes().as_ptr(), pointer);
        assert_eq!(wrapped.construction().nodes().len(), 5);
        let mut stock = iced::widget::Row::from_vec(native_children())
            .width(30.0)
            .height(80.0)
            .padding(2.0)
            .spacing(3.0)
            .align_y(iced::alignment::Vertical::Center)
            .clip(true)
            .wrap();
        let wrapped = if explicit {
            stock = stock
                .vertical_spacing(7.0)
                .align_x(iced::alignment::Horizontal::Right);
            wrapped
                .vertical_spacing(7.0)
                .align_x(iced::alignment::Horizontal::Right)
        } else {
            wrapped
        };
        let (recipe, source) = wrapped.into_parts();
        assert_eq!(source.nodes()[0].role_position(), Some(0));
        let actual = rectangles(recipe.into_native().into());
        assert_eq!(actual, rectangles(stock.into()));
        assert!(actual[4].y > actual[2].y, "fixture must actually wrap");
    }
}
#[test]
fn column_wrap_preserves_source_and_matches_native_wrapped_layout() {
    for explicit in [false, true] {
        let column = adapters::column_from_vec(children(), limits())
            .unwrap()
            .width(80.0)
            .height(25.0)
            .max_width(60.0)
            .padding(2.0)
            .spacing(3.0)
            .align_x(iced::alignment::Horizontal::Center)
            .clip(true);
        let pointer = column.construction().nodes().as_ptr();
        let wrapped = column.wrap();
        assert_eq!(wrapped.construction().nodes().as_ptr(), pointer);
        assert_eq!(wrapped.construction().nodes().len(), 5);
        let mut stock = iced::widget::Column::from_vec(native_children())
            .width(80.0)
            .height(25.0)
            .max_width(60.0)
            .padding(2.0)
            .spacing(3.0)
            .align_x(iced::alignment::Horizontal::Center)
            .clip(true)
            .wrap();
        let wrapped = if explicit {
            stock = stock
                .horizontal_spacing(7.0)
                .align_x(iced::alignment::Vertical::Bottom);
            wrapped
                .horizontal_spacing(7.0)
                .align_x(iced::alignment::Vertical::Bottom)
        } else {
            wrapped
        };
        let (recipe, source) = wrapped.into_parts();
        assert_eq!(source.nodes()[0].role_position(), Some(0));
        let actual = rectangles(recipe.into_native());
        assert_eq!(actual, rectangles(stock.into()));
        assert!(actual[4].x > actual[2].x, "fixture must actually wrap");
    }
}
#[test]
fn wrapping_conversions_are_eager_once_and_nested_erasure_keeps_children() {
    struct Pixels<'a>(&'a Cell<usize>);
    impl From<Pixels<'_>> for iced::Pixels {
        fn from(p: Pixels<'_>) -> Self {
            p.0.set(p.0.get() + 1);
            Self(5.0)
        }
    }
    struct Horizontal<'a>(&'a Cell<usize>);
    impl From<Horizontal<'_>> for iced::alignment::Horizontal {
        fn from(p: Horizontal<'_>) -> Self {
            p.0.set(p.0.get() + 1);
            Self::Center
        }
    }
    struct Vertical<'a>(&'a Cell<usize>);
    impl From<Vertical<'_>> for iced::alignment::Vertical {
        fn from(p: Vertical<'_>) -> Self {
            p.0.set(p.0.get() + 1);
            Self::Center
        }
    }
    let calls = Cell::new(0);
    let row = adapters::row_from_vec(children(), limits())
        .unwrap()
        .width(Length::Fill)
        .wrap()
        .vertical_spacing(Pixels(&calls))
        .align_x(Horizontal(&calls));
    assert_eq!(calls.get(), 2);
    let column = adapters::column_from_vec(vec![row.erase()], limits())
        .unwrap()
        .wrap()
        .horizontal_spacing(Pixels(&calls))
        .align_x(Vertical(&calls));
    assert_eq!(calls.get(), 4);
    assert_eq!(column.construction().nodes().len(), 6);
    let (native, _) = column.erase().into_parts();
    let tree = Tree::new(&native);
    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].children.len(), 4);
    assert_eq!(calls.get(), 4);
}
