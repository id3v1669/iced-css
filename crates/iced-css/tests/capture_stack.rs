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
fn stack_keeps_void_children_and_ordered_nested_source_membership() {
    let nested = adapters::row_from_vec(
        vec![space(Length::Fixed(0.0), Length::Fixed(0.0))],
        limits(),
    )
    .unwrap()
    .erase();
    let stack = adapters::stack_from_vec(
        vec![
            space(Length::Fixed(0.0), Length::Fixed(0.0)),
            nested,
            space(Length::Fill, Length::Fill),
        ],
        limits(),
    )
    .unwrap();
    let (native, construction) = stack.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
    let tree = Tree::new(&native);
    assert_eq!(tree.children.len(), 3);
    assert_eq!(tree.children[1].children.len(), 1);
    assert_eq!(
        (
            native.as_widget().size().width,
            native.as_widget().size().height
        ),
        (Length::Shrink, Length::Shrink)
    );
    let nodes = construction.nodes();
    assert_eq!(
        nodes.iter().map(|n| n.canonical_type()).collect::<Vec<_>>(),
        ["space", "space", "row", "space", "stack"]
    );
    assert_eq!(
        nodes.iter().map(|n| n.parent()).collect::<Vec<_>>(),
        [Some(4), Some(2), Some(4), Some(4), None]
    );
    assert_eq!(
        (
            nodes[0].role_position(),
            nodes[2].role_position(),
            nodes[3].role_position()
        ),
        (Some(0), Some(1), Some(2))
    );
    assert!(nodes[4].text().is_none());
}
#[test]
fn stack_configuration_preserves_evidence_and_converts_each_dimension_once() {
    struct Dimension<'a>(&'a Cell<usize>, Length);
    impl From<Dimension<'_>> for Length {
        fn from(v: Dimension<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    let calls = Cell::new(0);
    let stack =
        adapters::stack_from_vec(vec![space(Length::Shrink, Length::Shrink)], limits()).unwrap();
    let evidence = stack.construction().nodes().as_ptr();
    let stack = stack
        .width(Dimension(&calls, Length::Fixed(25.0)))
        .height(Dimension(&calls, Length::Fill))
        .clip(true)
        .clip(false);
    assert_eq!(calls.get(), 2);
    assert_eq!(stack.construction().nodes().as_ptr(), evidence);
    let (native, _) = stack.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
    assert_eq!(
        (
            native.as_widget().size().width,
            native.as_widget().size().height
        ),
        (Length::Fixed(25.0), Length::Fill)
    );
}
#[test]
fn empty_stack_and_aggregate_bounds_are_explicit() {
    let empty = adapters::stack_from_vec(
        Vec::<CapturedElement<'_, (), iced::Theme, ()>>::new(),
        limits(),
    )
    .unwrap();
    assert_eq!(empty.construction().nodes().len(), 1);
    assert_eq!(empty.construction().nodes()[0].canonical_type(), "stack");
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
            matches!(adapters::stack_from_vec(vec![space(Length::Shrink,Length::Shrink)],cap),Err(actual) if actual==error)
        );
    }
}
