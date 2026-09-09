use iced::{Length, advanced::widget::Tree};
use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
use std::{cell::Cell, num::NonZeroUsize};
fn limits() -> CaptureLimits {
    CaptureLimits {
        nodes: NonZeroUsize::new(100).unwrap(),
        depth: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(100000).unwrap(),
        work: NonZeroUsize::new(100000).unwrap(),
    }
}
#[test]
fn space_builders_convert_once_and_keep_empty_source_content() {
    struct Dimension<'a>(&'a Cell<usize>, Length);
    impl From<Dimension<'_>> for Length {
        fn from(v: Dimension<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            v.1
        }
    }
    let calls = Cell::new(0);
    let space = adapters::space(limits()).unwrap();
    let evidence = space.construction().nodes().as_ptr();
    let space = space
        .width(Dimension(&calls, Length::Fixed(0.0)))
        .height(Dimension(&calls, Length::Fill));
    assert_eq!(calls.get(), 2);
    assert_eq!(space.construction().nodes().as_ptr(), evidence);
    assert_eq!(space.construction().nodes()[0].canonical_type(), "space");
    assert_eq!(space.construction().nodes()[0].text(), None);
    let (native, _) = space.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
    assert_eq!(native.as_widget().size().width, Length::Fixed(0.0));
    assert_eq!(native.as_widget().size().height, Length::Fill);
}
fn space(width: Length, height: Length) -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::space(limits())
        .unwrap()
        .width(width)
        .height(height)
        .erase()
}
#[test]
fn column_from_vec_keeps_zero_size_children_and_its_own_canonical_identity() {
    let row = adapters::row_from_vec(vec![space(Length::Fixed(0.0), Length::Fill)], limits())
        .unwrap()
        .erase();
    let column =
        adapters::column_from_vec(vec![row, space(Length::Fill, Length::Fixed(0.0))], limits())
            .unwrap();
    let (native, construction) = column.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
    let tree = Tree::new(&native);
    assert_eq!(tree.children.len(), 2);
    assert_eq!(tree.children[0].children.len(), 1);
    assert_eq!(native.as_widget().size().width, Length::Shrink);
    assert_eq!(native.as_widget().size().height, Length::Shrink);
    assert_eq!(
        construction
            .nodes()
            .iter()
            .map(|n| n.canonical_type())
            .collect::<Vec<_>>(),
        ["space", "row", "space", "column"]
    );
    assert_eq!(
        construction
            .nodes()
            .iter()
            .map(|n| n.parent())
            .collect::<Vec<_>>(),
        [Some(1), Some(3), Some(3), None]
    );
    assert_eq!(construction.nodes()[2].role_position(), Some(1));
}
#[test]
fn empty_column_and_aggregate_limits_follow_ordered_construction_rules() {
    let empty = adapters::column_from_vec(
        Vec::<CapturedElement<'_, (), iced::Theme, ()>>::new(),
        limits(),
    )
    .unwrap();
    assert_eq!(empty.construction().nodes().len(), 1);
    assert_eq!(empty.construction().nodes()[0].canonical_type(), "column");
    let mut small = limits();
    small.nodes = NonZeroUsize::MIN;
    assert!(matches!(
        adapters::column_from_vec(vec![space(Length::Shrink, Length::Shrink)], small),
        Err(CaptureError::Nodes)
    ));
    small = limits();
    small.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(adapters::space(small), Err(CaptureError::Bytes)));
}
