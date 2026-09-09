use iced_css::capture::{CaptureError, CaptureLimits, Construction, adapters};
use std::num::NonZeroUsize;
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(100000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
fn cell(s: &str) -> Construction {
    adapters::text::<iced::Theme, ()>(s, limits())
        .unwrap()
        .into_parts()
        .1
}
#[test]
fn table_cells_have_structured_parents_and_preserve_native_flat_order() {
    let tree = Construction::table(
        2,
        2,
        ["H1", "H2", "é", "B", "C", "D"]
            .into_iter()
            .map(cell)
            .collect(),
        limits(),
    )
    .unwrap();
    let nodes = tree.nodes();
    assert_eq!(nodes.len(), 18);
    assert_eq!(tree.root(), 17);
    let expected = [
        ("text", Some(1)),
        ("table_header_cell", Some(4)),
        ("text", Some(3)),
        ("table_header_cell", Some(4)),
        ("table_row", Some(5)),
        ("table_header_group", Some(17)),
        ("text", Some(7)),
        ("table_cell", Some(10)),
        ("text", Some(9)),
        ("table_cell", Some(10)),
        ("table_row", Some(16)),
        ("text", Some(12)),
        ("table_cell", Some(15)),
        ("text", Some(14)),
        ("table_cell", Some(15)),
        ("table_row", Some(16)),
        ("table_row_group", Some(17)),
        ("table", None),
    ];
    assert_eq!(
        nodes
            .iter()
            .map(|n| (n.canonical_type(), n.parent()))
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        nodes.iter().filter_map(|n| n.text()).collect::<Vec<_>>(),
        ["H1", "H2", "é", "B", "C", "D"]
    );
    assert_eq!(nodes[0].role(), Some("content"));
    assert_eq!(nodes[3].role_position(), Some(1));
    assert_eq!(nodes[15].role_position(), Some(1));
}
#[test]
fn empty_columns_retain_rows_and_dimension_errors_do_not_wrap() {
    for (rows, count) in [(0, 4), (2, 6)] {
        let tree = Construction::table(0, rows, vec![], limits()).unwrap();
        assert_eq!(tree.nodes().len(), count);
        assert_eq!(
            tree.nodes()
                .iter()
                .filter(|n| n.canonical_type() == "table_row")
                .count(),
            rows + 1
        );
    }
    for (cols, rows) in [(1, 0), (usize::MAX, 1), (0, usize::MAX)] {
        assert!(matches!(
            Construction::table(cols, rows, vec![], limits()),
            Err(CaptureError::Dimensions)
        ));
    }
}
#[test]
fn final_structure_is_admitted_against_all_limits() {
    let build = |l| Construction::table(1, 0, vec![cell("a")], l);
    let mut l = limits();
    l.nodes = NonZeroUsize::new(5).unwrap();
    assert!(matches!(build(l), Err(CaptureError::Nodes)));
    let mut l = limits();
    l.depth = NonZeroUsize::new(4).unwrap();
    assert!(matches!(build(l), Err(CaptureError::Depth)));
    let mut l = limits();
    l.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(build(l), Err(CaptureError::Bytes)));
    let mut l = limits();
    l.work = NonZeroUsize::new(13).unwrap();
    assert!(matches!(build(l), Err(CaptureError::Work)));
    l.work = NonZeroUsize::new(14).unwrap();
    assert_eq!(build(l).unwrap().nodes().len(), 6);
}

#[test]
fn nested_content_keeps_metadata_keys_and_matches_table_selectors() {
    use iced_css::capture::source::{ConstructionSource, ConstructionState};
    use iced_css_core::{
        selectors::{MatchLimits, SelectorLimits, matching_specificity, parse_selector_list},
        source::{
            AttributeLimits,
            selector_tree::{DocumentLanguage, SelectorTreeLimits},
        },
    };
    let n = limits().nodes;
    let leaf = adapters::text::<iced::Theme, ()>("value", limits())
        .unwrap()
        .style_id(
            "cell-value",
            AttributeLimits {
                entries: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap()
        .source_key(b"stable", limits())
        .unwrap();
    let nested = adapters::container::<(), iced::Theme, (), _>(leaf, limits())
        .unwrap()
        .into_parts()
        .1;
    let tree = Construction::table(1, 1, vec![cell("header"), nested], limits()).unwrap();
    let pos = tree
        .nodes()
        .iter()
        .position(|node| node.text() == Some("value"))
        .unwrap();
    assert_eq!(tree.nodes()[pos].source_key(), Some(b"stable".as_slice()));
    let source = ConstructionSource::new(
        &tree,
        &vec![ConstructionState::default(); tree.nodes().len()],
        DocumentLanguage::Unknown,
        SelectorTreeLimits {
            entries: n,
            retained_bytes: n,
            work: n,
        },
    )
    .unwrap();
    for (css, want) in [
        (
            "table > table_row_group > table_row:first-child > table_cell > container > #cell-value",
            true,
        ),
        ("table_header_group #cell-value", false),
        ("table_row_group > table_row + table_row #cell-value", false),
    ] {
        let selectors = parse_selector_list(
            css,
            SelectorLimits {
                entries: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        assert_eq!(
            matching_specificity(
                &selectors,
                &source,
                pos,
                None,
                MatchLimits {
                    retained_bytes: n,
                    work: n
                }
            )
            .unwrap()
            .is_some(),
            want,
            "{css}"
        );
    }
}

// Qualification boundary: source-only empty tables must not be sent to stock
// Table layout. This fixture is discarded after its caught native panic.
#[test]
fn stock_table_zero_column_layout_requires_a_separate_native_path() {
    use iced::{
        Element, Size,
        advanced::{layout, widget::Tree},
        widget::table,
    };
    let columns: Vec<table::Column<'_, '_, (), (), iced::Theme, ()>> = vec![];
    let mut element: Element<'_, (), iced::Theme, ()> = table::table(columns, [()]).into();
    let mut tree = Tree::new(&element);
    assert!(tree.children.is_empty());
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        element.as_widget_mut().layout(
            &mut tree,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
        );
    }))
    .unwrap_err();
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(message.contains("divide by zero"), "{message}");
}
