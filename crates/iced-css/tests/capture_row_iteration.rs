use iced::{Length, advanced::widget::Tree};
use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
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
fn child(width: Length, height: Length) -> Child {
    adapters::space(limits())
        .unwrap()
        .width(width)
        .height(height)
        .erase()
}
#[test]
fn row_extend_retains_void_children_and_accumulates_only_new_nonvoid_hints() {
    let next = Cell::new(0);
    let mut children = vec![
        child(Length::Fixed(0.0), Length::Fill),
        child(Length::Fill, Length::Shrink),
    ]
    .into_iter();
    let sequence = std::iter::from_fn(|| {
        next.set(next.get() + 1);
        children.next()
    });
    let row = adapters::row_from_vec(Vec::<Child>::new(), limits())
        .unwrap()
        .extend(sequence, limits())
        .unwrap();
    assert_eq!(next.get(), 3);
    assert_eq!(
        row.construction()
            .nodes()
            .iter()
            .map(|n| n.role_position())
            .collect::<Vec<_>>(),
        [Some(0), Some(1), None]
    );
    let (recipe, _) = row.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = recipe.into();
    assert_eq!(Tree::new(&native).children.len(), 2);
    assert_eq!(native.as_widget().size().width, Length::Fill);
    assert_eq!(native.as_widget().size().height, Length::Shrink);
}
#[test]
fn existing_from_vec_hints_are_not_replayed_by_extension() {
    for add_void in [false, true] {
        let children = if add_void {
            vec![child(Length::Fixed(0.0), Length::Fill)]
        } else {
            Vec::new()
        };
        let row = adapters::row_from_vec(vec![child(Length::Fill, Length::Shrink)], limits())
            .unwrap()
            .extend(children, limits())
            .unwrap();
        let (recipe, _) = row.into_parts();
        let native: iced::Element<'_, (), iced::Theme, ()> = recipe.into();
        assert_eq!(native.as_widget().size().width, Length::Shrink);
        assert_eq!(
            Tree::new(&native).children.len(),
            if add_void { 2 } else { 1 }
        );
    }
}
#[test]
fn row_extension_shares_the_whole_operation_budget() {
    for (budget, expected_next, success) in [(7, 2, false), (13, 2, false), (14, 3, true)] {
        let next = Cell::new(0);
        let mut values = vec![
            child(Length::Shrink, Length::Shrink),
            child(Length::Shrink, Length::Shrink),
        ]
        .into_iter();
        let sequence = std::iter::from_fn(|| {
            next.set(next.get() + 1);
            values.next()
        });
        let mut cap = limits();
        cap.work = NonZeroUsize::new(budget).unwrap();
        let result = adapters::row_from_vec(Vec::<Child>::new(), limits())
            .unwrap()
            .extend(sequence, cap);
        if success {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(CaptureError::Work)));
        }
        assert_eq!(next.get(), expected_next);
    }
}
