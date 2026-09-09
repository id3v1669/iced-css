use iced::advanced::widget::Tree;
use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
use std::num::NonZeroUsize;

fn limits() -> CaptureLimits {
    CaptureLimits {
        nodes: NonZeroUsize::new(100).unwrap(),
        depth: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(100_000).unwrap(),
        work: NonZeroUsize::new(100_000).unwrap(),
    }
}
fn text(value: &str) -> CapturedElement<'_, (), iced::Theme, ()> {
    adapters::text(value, limits()).unwrap().erase()
}

#[test]
fn nested_rows_preserve_sibling_order_and_distinct_parent_links() {
    let left = adapters::container(text("a"), limits()).unwrap().erase();
    let right = adapters::row_from_vec(vec![text("b"), text("c")], limits())
        .unwrap()
        .erase();
    let root = adapters::row_from_vec(vec![left, right, text("d")], limits()).unwrap();
    let (native, evidence) = root.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
    let tree = Tree::new(&native);
    assert_eq!(tree.children.len(), 3);
    assert_eq!(tree.children[1].children.len(), 2);
    assert_eq!(evidence.depth(), 3);
    assert_eq!(evidence.root(), 6);
    let nodes = evidence.nodes();
    let parents: Vec<_> = nodes.iter().map(|n| n.parent()).collect();
    assert_eq!(
        parents,
        [Some(1), Some(6), Some(4), Some(4), Some(6), Some(6), None]
    );
    let direct: Vec<_> = nodes
        .iter()
        .filter(|n| n.parent() == Some(6))
        .map(|n| (n.canonical_type(), n.role(), n.role_position()))
        .collect();
    assert_eq!(
        direct,
        [
            ("container", Some("children"), Some(0)),
            ("row", Some("children"), Some(1)),
            ("text", Some("children"), Some(2))
        ]
    );
    assert_eq!(nodes[0].role(), Some("content"));
    assert_eq!(nodes[0].role_position(), None);
    let texts: Vec<_> = nodes.iter().filter_map(|n| n.text()).collect();
    assert_eq!(texts, ["a", "b", "c", "d"]);
}

#[test]
fn empty_row_is_one_source_with_no_native_children() {
    let row = adapters::row_from_vec(
        Vec::<CapturedElement<'_, (), iced::Theme, ()>>::new(),
        limits(),
    )
    .unwrap();
    assert_eq!(row.construction().nodes().len(), 1);
    assert_eq!(row.construction().depth(), 1);
    assert_eq!(row.construction().nodes()[0].canonical_type(), "row");
    let (native, _) = row.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
    assert!(Tree::new(&native).children.is_empty());
}

#[test]
fn aggregate_limits_apply_to_all_sibling_subtrees() {
    for error in [
        CaptureError::Nodes,
        CaptureError::Depth,
        CaptureError::Bytes,
        CaptureError::Work,
    ] {
        let mut small = limits();
        match error {
            CaptureError::Nodes => small.nodes = NonZeroUsize::new(2).unwrap(),
            CaptureError::Depth => small.depth = NonZeroUsize::new(1).unwrap(),
            CaptureError::Bytes => small.retained_bytes = NonZeroUsize::new(1).unwrap(),
            CaptureError::Work => small.work = NonZeroUsize::new(1).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            matches!(adapters::row_from_vec(vec![text("a"),text("b")], small), Err(actual) if actual == error)
        );
    }
}
