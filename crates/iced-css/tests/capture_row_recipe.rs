use iced::{Length, advanced::widget::Tree};
use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
use std::{cell::Cell, num::NonZeroUsize};
fn limits() -> CaptureLimits {
    let nz = |n| NonZeroUsize::new(n).unwrap();
    CaptureLimits {
        nodes: nz(100),
        depth: nz(100),
        retained_bytes: nz(100000),
        work: nz(100000),
    }
}
fn space(width: Length, height: Length) -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::space(limits())
        .unwrap()
        .width(width)
        .height(height)
        .erase()
}
#[test]
fn pushed_void_children_retain_native_payload_and_source_without_accumulating_hints() {
    let row = adapters::row_from_vec(
        Vec::<CapturedElement<'_, (), iced::Theme, ()>>::new(),
        limits(),
    )
    .unwrap()
    .push(space(Length::Fixed(0.0), Length::Fill), limits())
    .unwrap();
    assert_eq!(row.construction().nodes().len(), 2);
    assert_eq!(row.construction().nodes()[0].role_position(), Some(0));
    let (recipe, _) = row.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = recipe.into_native().into();
    assert_eq!(Tree::new(&native).children.len(), 1);
    assert_eq!(
        (
            native.as_widget().size().width,
            native.as_widget().size().height
        ),
        (Length::Shrink, Length::Shrink)
    );
}
#[test]
fn from_vec_and_push_keep_their_distinct_hint_semantics() {
    let row = adapters::row_from_vec(vec![space(Length::Fill, Length::Shrink)], limits()).unwrap();
    let (recipe, _) = row.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = recipe.into();
    assert_eq!(native.as_widget().size().width, Length::Shrink);
    for width in [Length::Shrink, Length::Fixed(10.0)] {
        let row = adapters::row_from_vec(
            Vec::<CapturedElement<'_, (), iced::Theme, ()>>::new(),
            limits(),
        )
        .unwrap()
        .width(width)
        .push(space(Length::Fill, Length::Shrink), limits())
        .unwrap()
        .height(20.0);
        let stock = iced::widget::Row::new()
            .width(width)
            .push(
                iced::widget::Space::new()
                    .width(Length::Fill)
                    .height(Length::Shrink),
            )
            .height(20.0);
        let (recipe, _) = row.into_parts();
        let actual: iced::Element<'_, (), iced::Theme, ()> = recipe.into();
        let expected: iced::Element<'_, (), iced::Theme, ()> = stock.into();
        assert_eq!(actual.as_widget().size(), expected.as_widget().size());
    }
}
#[test]
fn configuration_conversions_happen_once_before_native_closure() {
    struct Dimension<'a>(&'a Cell<usize>, Length);
    impl From<Dimension<'_>> for Length {
        fn from(v: Dimension<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    struct Pixels<'a>(&'a Cell<usize>, f32);
    impl From<Pixels<'_>> for iced::Pixels {
        fn from(v: Pixels<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            iced::Pixels(v.1)
        }
    }
    let calls = Cell::new(0);
    let row =
        adapters::row_from_vec(vec![space(Length::Shrink, Length::Shrink)], limits()).unwrap();
    let pointer = row.construction().nodes().as_ptr();
    let row = row
        .width(Dimension(&calls, Length::Fixed(60.0)))
        .height(Dimension(&calls, Length::Fixed(20.0)))
        .spacing(Pixels(&calls, 4.0))
        .padding(2.0)
        .align_y(iced::alignment::Vertical::Center)
        .clip(true);
    assert_eq!(calls.get(), 3);
    assert_eq!(row.construction().nodes().as_ptr(), pointer);
    let (recipe, _) = row.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = recipe.into();
    assert_eq!(calls.get(), 3);
    assert_eq!(
        (
            native.as_widget().size().width,
            native.as_widget().size().height
        ),
        (Length::Fixed(60.0), Length::Fixed(20.0))
    );
}
#[test]
fn managed_push_rejects_evidence_limits_before_returning_a_partial_row() {
    for (field, error) in [
        (0, CaptureError::Nodes),
        (1, CaptureError::Depth),
        (2, CaptureError::Bytes),
        (3, CaptureError::Work),
    ] {
        let row =
            adapters::row_from_vec(vec![space(Length::Shrink, Length::Shrink)], limits()).unwrap();
        let mut cap = limits();
        match field {
            0 => cap.nodes = NonZeroUsize::MIN,
            1 => cap.depth = NonZeroUsize::MIN,
            2 => cap.retained_bytes = NonZeroUsize::MIN,
            _ => cap.work = NonZeroUsize::MIN,
        }
        assert!(
            matches!(row.push(space(Length::Fixed(0.0),Length::Fill),cap),Err(actual) if actual==error)
        );
    }
}

#[test]
fn deferred_configuration_matches_stock_row_layout_for_retained_children() {
    fn rectangles(mut native: iced::Element<'_, (), iced::Theme, ()>) -> Vec<iced::Rectangle> {
        let mut tree = Tree::new(&native);
        let node = native.as_widget_mut().layout(
            &mut tree,
            &(),
            &iced::advanced::layout::Limits::new(iced::Size::ZERO, iced::Size::new(200.0, 100.0)),
        );
        std::iter::once(node.bounds())
            .chain(node.children().iter().map(|child| child.bounds()))
            .collect()
    }
    let captured = adapters::row_from_vec(
        vec![
            space(Length::Fixed(5.0), Length::Fixed(5.0)),
            space(Length::Fixed(7.0), Length::Fixed(3.0)),
        ],
        limits(),
    )
    .unwrap()
    .width(60.0)
    .height(20.0)
    .spacing(4.0)
    .padding(2.0)
    .align_y(iced::alignment::Vertical::Center)
    .clip(true);
    let (recipe, _) = captured.into_parts();
    let stock = iced::widget::Row::from_vec(vec![
        iced::widget::Space::new().width(5.0).height(5.0).into(),
        iced::widget::Space::new().width(7.0).height(3.0).into(),
    ])
    .width(60.0)
    .height(20.0)
    .spacing(4.0)
    .padding(2.0)
    .align_y(iced::alignment::Vertical::Center)
    .clip(true);
    assert_eq!(rectangles(recipe.into()), rectangles(stock.into()));
}
