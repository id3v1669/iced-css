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
fn space() -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::space(limits())
        .unwrap()
        .width(0.0)
        .height(0.0)
        .erase()
}
#[test]
fn grid_from_vec_retains_zero_size_payloads_and_native_default_hints() {
    let grid = adapters::grid_from_vec(vec![space(), space()], limits()).unwrap();
    assert_eq!(
        grid.construction()
            .nodes()
            .iter()
            .map(|n| n.canonical_type())
            .collect::<Vec<_>>(),
        ["space", "space", "grid"]
    );
    let (native, _) = grid.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
    assert_eq!(Tree::new(&native).children.len(), 2);
    assert_eq!(
        (
            native.as_widget().size().width,
            native.as_widget().size().height
        ),
        (Length::Fill, Length::Shrink)
    );
}
#[test]
fn appending_nested_subtrees_moves_the_root_and_preserves_source_roles() {
    let nested = adapters::row_from_vec(vec![space(), space()], limits()).unwrap();
    let grid = adapters::grid_from_vec(vec![space()], limits())
        .unwrap()
        .push(nested, limits())
        .unwrap();
    let nodes = grid.construction().nodes();
    assert_eq!(
        nodes.iter().map(|n| n.canonical_type()).collect::<Vec<_>>(),
        ["space", "space", "space", "row", "grid"]
    );
    assert_eq!(
        nodes.iter().map(|n| n.parent()).collect::<Vec<_>>(),
        [Some(4), Some(3), Some(3), Some(4), None]
    );
    assert_eq!(
        (nodes[0].role_position(), nodes[3].role_position()),
        (Some(0), Some(1))
    );
    assert_eq!(grid.construction().depth(), 3);
    let (native, _) = grid.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
    let tree = Tree::new(&native);
    assert_eq!(tree.children.len(), 2);
    assert_eq!(tree.children[1].children.len(), 2);
}
#[test]
fn empty_grid_accepts_a_child_and_preserves_native_configuration_conversions() {
    struct Pixels<'a>(&'a Cell<usize>, f32);
    impl From<Pixels<'_>> for iced::Pixels {
        fn from(v: Pixels<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            iced::Pixels(v.1)
        }
    }
    let calls = Cell::new(0);
    let grid = adapters::grid_from_vec(
        Vec::<CapturedElement<'_, (), iced::Theme, ()>>::new(),
        limits(),
    )
    .unwrap();
    assert_eq!(grid.construction().nodes().len(), 1);
    let pointer = grid.construction().nodes().as_ptr();
    let grid = grid
        .width(Pixels(&calls, 60.0))
        .spacing(Pixels(&calls, 4.0))
        .fluid(Pixels(&calls, 20.0))
        .columns(2)
        .height(Length::Fixed(40.0));
    assert_eq!(calls.get(), 3);
    assert_eq!(grid.construction().nodes().as_ptr(), pointer);
    let grid = grid.push(space(), limits()).unwrap();
    assert_eq!(grid.construction().nodes()[0].role_position(), Some(0));
    let (native, _) = grid.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
    assert_eq!(
        (
            native.as_widget().size().width,
            native.as_widget().size().height
        ),
        (Length::Fixed(60.0), Length::Fixed(40.0))
    );
    assert_eq!(Tree::new(&native).children.len(), 1);
}
#[test]
fn constructor_and_append_apply_aggregate_limits() {
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
        assert!(matches!(adapters::grid_from_vec(vec![space()],cap),Err(actual) if actual==error));
        let grid = adapters::grid_from_vec(vec![space()], limits()).unwrap();
        assert!(matches!(grid.push(space(),cap),Err(actual) if actual==error));
    }
}

#[test]
fn repeated_append_matches_batch_construction_for_nested_ordered_children() {
    fn child(index: usize) -> CapturedElement<'static, (), iced::Theme, ()> {
        if index.is_multiple_of(2) {
            space()
        } else {
            adapters::row_from_vec((0..index % 4).map(|_| space()).collect(), limits())
                .unwrap()
                .erase()
        }
    }
    fn shape(
        c: &iced_css::capture::Construction,
    ) -> Vec<(&'static str, Option<usize>, Option<usize>)> {
        c.nodes()
            .iter()
            .map(|n| (n.canonical_type(), n.parent(), n.role_position()))
            .collect()
    }
    for count in 0..24 {
        let batch = adapters::grid_from_vec((0..count).map(child).collect(), limits()).unwrap();
        let mut appended = adapters::grid_from_vec(
            Vec::<CapturedElement<'static, (), iced::Theme, ()>>::new(),
            limits(),
        )
        .unwrap();
        for index in 0..count {
            appended = appended.push(child(index), limits()).unwrap();
        }
        assert_eq!(shape(batch.construction()), shape(appended.construction()));
        assert_eq!(
            batch.construction().depth(),
            appended.construction().depth()
        );
        let (native, _) = appended.into_parts();
        let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
        assert_eq!(Tree::new(&native).children.len(), count);
    }
}
#[test]
fn append_counts_existing_root_attribute_storage() {
    let attrs = iced_css_core::source::AttributeLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(10000).unwrap(),
    };
    let class = "x".repeat(1024);
    let grid = adapters::grid_from_vec(vec![space()], limits())
        .unwrap()
        .style_classes(&[&class], attrs)
        .unwrap();
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::new(512).unwrap();
    assert!(matches!(grid.push(space(), cap), Err(CaptureError::Bytes)));
}
