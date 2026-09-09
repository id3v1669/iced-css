use iced_css_core::formatting::{display::Display as D, table_repair_stage1::*};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> RepairLimits {
    RepairLimits {
        nodes: nz(1000),
        retained_bytes: nz(100000),
        work: nz(100000),
    }
}
fn node(parent: Option<u32>, kind: RepairKind) -> RepairNode {
    RepairNode { parent, kind }
}
fn ordinary(parent: Option<u32>, display: D) -> RepairNode {
    node(parent, RepairKind::Box(display))
}
fn white(parent: Option<u32>) -> RepairNode {
    node(parent, RepairKind::AnonymousWhitespace)
}
fn run(parent: D, input: &[RepairNode]) -> Vec<Disposition> {
    suppress_table_boxes(parent, input, limits())
        .unwrap()
        .dispositions()
        .to_vec()
}
#[test]
fn independently_declared_48_pair_matrix() {
    let descendants = [
        D::TableCaption,
        D::TableCell,
        D::TableRow,
        D::TableRowGroup,
        D::TableHeaderGroup,
        D::TableFooterGroup,
        D::TableColumn,
        D::TableColumnGroup,
    ];
    let parents = [
        D::Table,
        D::InlineTable,
        D::TableRowGroup,
        D::TableHeaderGroup,
        D::TableFooterGroup,
        D::TableRow,
    ];
    let expected = [
        [true, true, true, true, true, true, true, true],
        [true, true, true, true, true, true, true, true],
        [false, true, true, false, false, false, false, false],
        [false, true, true, false, false, false, false, false],
        [false, true, true, false, false, false, false, false],
        [false, true, false, false, false, false, false, false],
    ];
    for (p, parent) in parents.into_iter().enumerate() {
        for (d, descendant) in descendants.into_iter().enumerate() {
            // Leading whitespace has only one neighbor: rule 1.4 cannot mask 1.3.
            let actual = run(parent, &[white(None), ordinary(None, descendant)]);
            let trailing = run(parent, &[ordinary(None, descendant), white(None)]);
            assert_eq!(trailing[1], actual[0], "trailing {parent:?}/{descendant:?}");
            assert_eq!(
                actual[0],
                if expected[p][d] {
                    Disposition::TabularWhitespace
                } else {
                    Disposition::Survives
                },
                "{parent:?}/{descendant:?}"
            );
        }
    }
}
#[test]
fn edges_between_and_protected_inlines() {
    assert_eq!(
        run(D::Table, &[white(None)]),
        [Disposition::TabularWhitespace]
    );
    assert_eq!(
        run(
            D::TableRowGroup,
            &[ordinary(None, D::TableCell), white(None)]
        ),
        [Disposition::Survives, Disposition::TabularWhitespace]
    );
    assert_eq!(
        run(
            D::Block,
            &[
                ordinary(None, D::TableRow),
                white(None),
                ordinary(None, D::TableCaption)
            ]
        ),
        [
            Disposition::Survives,
            Disposition::BetweenInternalWhitespace,
            Disposition::Survives
        ]
    );
    assert_eq!(
        run(
            D::TableRow,
            &[
                ordinary(None, D::TableCaption),
                white(None),
                ordinary(None, D::TableColumn)
            ]
        ),
        [
            Disposition::Survives,
            Disposition::BetweenInternalWhitespace,
            Disposition::Survives
        ]
    );
    assert_eq!(run(D::Block, &[white(None)]), [Disposition::Survives]);
    assert_eq!(
        run(
            D::Table,
            &[
                ordinary(None, D::Inline),
                white(None),
                ordinary(None, D::TableCell)
            ]
        ),
        [Disposition::Survives; 3]
    );
    assert_eq!(
        run(
            D::Table,
            &[
                ordinary(None, D::Inline),
                white(None),
                node(None, RepairKind::OutOfFlowPlaceholder)
            ]
        ),
        [Disposition::Survives; 3]
    );
    assert_eq!(
        run(
            D::Table,
            &[node(None, RepairKind::OutOfFlowPlaceholder), white(None)]
        ),
        [Disposition::Survives; 2]
    );
}
#[test]
fn columns_ancestors_and_borrowed_mapping() {
    let input = [
        ordinary(None, D::TableColumn),
        ordinary(Some(0), D::TableRow),
        ordinary(Some(1), D::TableColumn),
        ordinary(Some(2), D::TableCell),
        ordinary(None, D::TableColumnGroup),
        ordinary(Some(4), D::TableColumn),
        ordinary(Some(4), D::TableCell),
    ];
    let result = suppress_table_boxes(D::Table, &input, limits()).unwrap();
    assert!(std::ptr::eq(result.input(), input.as_slice()));
    assert_eq!(result.external_parent(), D::Table);
    assert_eq!(
        result.dispositions(),
        [
            Disposition::Survives,
            Disposition::ColumnChild,
            Disposition::Ancestor { index: 1 },
            Disposition::Ancestor { index: 1 },
            Disposition::Survives,
            Disposition::Survives,
            Disposition::NonColumnGroupChild
        ]
    );
    for role in [
        D::Inline,
        D::Block,
        D::ListItem,
        D::InlineBlock,
        D::TableCell,
        D::TableRow,
        D::TableColumn,
        D::TableColumnGroup,
        D::TableCaption,
        D::TableRowGroup,
        D::TableHeaderGroup,
        D::TableFooterGroup,
        D::Table,
        D::InlineTable,
    ] {
        assert_eq!(
            run(D::TableColumn, &[ordinary(None, role)]),
            [Disposition::ColumnChild]
        );
        assert_eq!(
            run(D::TableColumnGroup, &[ordinary(None, role)]),
            [if role == D::TableColumn {
                Disposition::Survives
            } else {
                Disposition::NonColumnGroupChild
            }]
        );
    }
    for kind in [
        RepairKind::AnonymousWhitespace,
        RepairKind::OutOfFlowPlaceholder,
    ] {
        assert_eq!(
            run(D::TableColumn, &[node(None, kind)]),
            [Disposition::ColumnChild]
        );
        assert_eq!(
            run(D::TableColumnGroup, &[node(None, kind)]),
            [Disposition::NonColumnGroupChild]
        );
    }
}
#[test]
fn sibling_links_follow_parent_not_storage() {
    let separate = [
        ordinary(None, D::Block),
        ordinary(None, D::Block),
        white(Some(0)),
        white(Some(1)),
    ];
    assert_eq!(run(D::Block, &separate), [Disposition::Survives; 4]);
    let input = [
        ordinary(None, D::Table),
        ordinary(Some(0), D::TableRow),
        ordinary(Some(1), D::TableCell),
        white(Some(0)),
        ordinary(None, D::Block),
        ordinary(Some(0), D::TableCaption),
    ];
    assert_eq!(
        run(D::Block, &input),
        [
            Disposition::Survives,
            Disposition::Survives,
            Disposition::Survives,
            Disposition::TabularWhitespace,
            Disposition::Survives,
            Disposition::Survives
        ]
    );
    let adjacent = [
        ordinary(None, D::Block),
        white(None),
        ordinary(Some(0), D::Inline),
        white(None),
    ];
    assert_eq!(
        suppress_table_boxes(D::Block, &adjacent, limits()).unwrap_err(),
        RepairError::AdjacentWhitespace {
            previous: 1,
            index: 3
        }
    );
}
#[test]
fn malformed_input_is_rejected_even_in_suppressed_subtrees() {
    assert_eq!(
        suppress_table_boxes(D::None, &[], limits()).unwrap_err(),
        RepairError::ExternalNone
    );
    for parent in [0, 1, u32::MAX] {
        assert_eq!(
            suppress_table_boxes(D::Block, &[ordinary(Some(parent), D::Block)], limits())
                .unwrap_err(),
            RepairError::Parent { index: 0, parent }
        );
    }
    for kind in [
        RepairKind::AnonymousWhitespace,
        RepairKind::OutOfFlowPlaceholder,
    ] {
        assert_eq!(
            suppress_table_boxes(
                D::TableColumn,
                &[node(None, kind), ordinary(Some(0), D::Block)],
                limits()
            )
            .unwrap_err(),
            RepairError::LeafParent {
                index: 1,
                parent: 0
            }
        );
    }
    let late = [
        ordinary(None, D::TableColumn),
        ordinary(Some(0), D::Block),
        ordinary(Some(1), D::None),
    ];
    assert_eq!(
        suppress_table_boxes(D::Block, &late, limits()).unwrap_err(),
        RepairError::NoneDisplay { index: 2 }
    );
    assert_eq!(late[2].kind, RepairKind::Box(D::None));
    let late_parent = [
        ordinary(None, D::TableColumn),
        ordinary(Some(0), D::Block),
        ordinary(Some(9), D::Block),
    ];
    assert_eq!(
        suppress_table_boxes(D::Block, &late_parent, limits()).unwrap_err(),
        RepairError::Parent {
            index: 2,
            parent: 9
        }
    );
}
#[test]
fn explicit_resource_boundaries_and_empty_input() {
    let input = [ordinary(None, D::Block), ordinary(Some(0), D::Inline)];
    let mut cap = limits();
    cap.nodes = nz(1);
    assert_eq!(
        suppress_table_boxes(D::Block, &input, cap).unwrap_err(),
        RepairError::Nodes
    );
    cap = limits();
    cap.work = nz(8); // documented 4*n+1 exact admission
    assert_eq!(
        suppress_table_boxes(D::Block, &input, cap).unwrap_err(),
        RepairError::Work
    );
    cap.work = nz(9);
    let result = suppress_table_boxes(D::Block, &input, cap).unwrap();
    let bytes = result.peak_retained_bytes();
    assert!(bytes > result.retained_bytes());
    cap.retained_bytes = nz(bytes);
    assert!(suppress_table_boxes(D::Block, &input, cap).is_ok());
    cap.retained_bytes = nz(bytes - 1);
    assert_eq!(
        suppress_table_boxes(D::Block, &input, cap).unwrap_err(),
        RepairError::Bytes
    );
    cap = limits();
    cap.retained_bytes = nz(1);
    assert_eq!(
        suppress_table_boxes(D::Block, &input, cap).unwrap_err(),
        RepairError::Bytes
    );
    cap = limits();
    cap.work = nz(1);
    let empty = suppress_table_boxes(D::Block, &[], cap).unwrap();
    assert!(empty.dispositions().is_empty());
    assert_eq!(empty.retained_bytes(), 0);
}
