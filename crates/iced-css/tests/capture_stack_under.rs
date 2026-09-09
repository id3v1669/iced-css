use iced::{Length, advanced::widget::Tree};
use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
use std::num::NonZeroUsize;
type Child = CapturedElement<'static, (), iced::Theme, ()>;
fn limits() -> CaptureLimits {
    let nz = |n| NonZeroUsize::new(n).unwrap();
    CaptureLimits {
        nodes: nz(1000),
        depth: nz(100),
        retained_bytes: nz(100000),
        work: nz(100000),
    }
}
fn child(w: f32, h: f32) -> Child {
    adapters::space(limits())
        .unwrap()
        .width(w)
        .height(h)
        .erase()
}
fn shape(c: &iced_css::capture::Construction) -> Vec<(&'static str, Option<usize>, Option<usize>)> {
    c.nodes()
        .iter()
        .map(|n| (n.canonical_type(), n.parent(), n.role_position()))
        .collect()
}
#[test]
fn prepend_nested_subtree_matches_batch_order_and_retains_void_native_children() {
    let stack = adapters::stack_from_vec(vec![child(10.0, 10.0)], limits())
        .unwrap()
        .push_under(
            adapters::row_from_vec(vec![child(0.0, 0.0), child(20.0, 20.0)], limits()).unwrap(),
            limits(),
        )
        .unwrap();
    let expected = adapters::stack_from_vec(
        vec![
            adapters::row_from_vec(vec![child(0.0, 0.0), child(20.0, 20.0)], limits())
                .unwrap()
                .erase(),
            child(10.0, 10.0),
        ],
        limits(),
    )
    .unwrap();
    assert_eq!(shape(stack.construction()), shape(expected.construction()));
    let (native, _) = stack.erase().into_parts();
    let tree = Tree::new(&native);
    assert_eq!(tree.children.len(), 2);
    assert_eq!(tree.children[0].children.len(), 2);
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
fn native_base_remains_above_larger_underlay_and_empty_stack_stays_without_a_base() {
    let stack = adapters::stack_from_vec(vec![child(10.0, 10.0)], limits())
        .unwrap()
        .push_under(child(20.0, 20.0), limits())
        .unwrap();
    let (native, _) = stack.erase().into_parts();
    let actual = rectangles(native);
    let expected = rectangles(
        iced::widget::Stack::from_vec(vec![
            iced::widget::Space::new().width(10.0).height(10.0).into(),
        ])
        .push_under(iced::widget::Space::new().width(20.0).height(20.0))
        .into(),
    );
    assert_eq!(actual, expected);
    assert_eq!(actual[0].size(), iced::Size::new(10.0, 10.0));
    let stack = adapters::stack_from_vec(Vec::<Child>::new(), limits())
        .unwrap()
        .push_under(child(20.0, 20.0), limits())
        .unwrap();
    let (native, _) = stack.erase().into_parts();
    assert_eq!(Tree::new(&native).children.len(), 1);
    let actual = rectangles(native);
    assert_eq!(
        actual,
        rectangles(
            iced::widget::Stack::new()
                .push_under(iced::widget::Space::new().width(20.0).height(20.0))
                .into()
        )
    );
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].size(), iced::Size::ZERO);
}
#[test]
fn repeated_front_insertion_matches_batch_evidence_and_does_not_accumulate_hints() {
    let mut stack = adapters::stack_from_vec(vec![child(10.0, 10.0)], limits()).unwrap();
    for i in 0..24 {
        stack = stack
            .push_under(
                adapters::column_from_vec(vec![child(i as f32, 1.0)], limits()).unwrap(),
                limits(),
            )
            .unwrap();
        let mut expected: Vec<Child> = (0..=i)
            .rev()
            .map(|j| {
                adapters::column_from_vec(vec![child(j as f32, 1.0)], limits())
                    .unwrap()
                    .erase()
            })
            .collect();
        expected.push(child(10.0, 10.0));
        let expected = adapters::stack_from_vec(expected, limits()).unwrap();
        assert_eq!(shape(stack.construction()), shape(expected.construction()));
    }
    let (native, _) = stack.erase().into_parts();
    assert_eq!(Tree::new(&native).children.len(), 25);
    assert_eq!(
        native.as_widget().size(),
        iced::Size::new(Length::Shrink, Length::Shrink)
    );
}
#[test]
fn prepend_enforces_all_aggregate_limits() {
    for (field, error) in [
        (0, CaptureError::Nodes),
        (1, CaptureError::Depth),
        (2, CaptureError::Bytes),
        (3, CaptureError::Work),
    ] {
        let stack = adapters::stack_from_vec(vec![child(10.0, 10.0)], limits()).unwrap();
        let mut cap = limits();
        match field {
            0 => cap.nodes = NonZeroUsize::MIN,
            1 => cap.depth = NonZeroUsize::MIN,
            2 => cap.retained_bytes = NonZeroUsize::MIN,
            _ => cap.work = NonZeroUsize::MIN,
        }
        assert!(matches!(stack.push_under(child(0.0,0.0),cap),Err(actual) if actual==error));
    }
}

#[test]
fn prepend_work_boundary_and_existing_attribute_storage_are_accounted() {
    for budget in [3, 4] {
        let stack = adapters::stack_from_vec(vec![child(10.0, 10.0)], limits()).unwrap();
        let mut cap = limits();
        cap.work = NonZeroUsize::new(budget).unwrap();
        let result = stack.push_under(child(0.0, 0.0), cap);
        if budget == 4 {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(CaptureError::Work)));
        }
    }
    let attrs = iced_css_core::source::AttributeLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(10000).unwrap(),
    };
    let stack = adapters::stack_from_vec(vec![child(10.0, 10.0)], limits())
        .unwrap()
        .style_classes(&[&"x".repeat(1024)], attrs)
        .unwrap();
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::new(512).unwrap();
    assert!(matches!(
        stack.push_under(child(0.0, 0.0), cap),
        Err(CaptureError::Bytes)
    ));
}
