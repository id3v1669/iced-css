use iced_css_core::formatting::{
    display::Display as D,
    table_repair_stage1::{
        Disposition, RepairKind, RepairLimits, RepairNode, suppress_table_boxes,
    },
    table_repair_stage2::{
        RowWrapperReason as R, Stage2Error, Stage2Limits, Stage2Node, Stage2Origin as O,
        wrap_table_children,
    },
};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn first_limits() -> RepairLimits {
    RepairLimits {
        nodes: nz(10000),
        retained_bytes: nz(1_000_000),
        work: nz(1_000_000),
    }
}
fn limits() -> Stage2Limits {
    Stage2Limits {
        input_nodes: nz(10000),
        output_nodes: nz(30000),
        retained_bytes: nz(4_000_000),
        work: nz(1_000_000),
    }
}
fn original(parent: Option<u32>, role: D) -> RepairNode {
    RepairNode {
        parent,
        kind: RepairKind::Box(role),
    }
}
fn output(parent: Option<u32>, origin: O) -> Stage2Node {
    Stage2Node { parent, origin }
}

#[test]
fn independently_declared_parent_role_matrix_includes_generated_rows() {
    let children = [
        D::Inline,
        D::Block,
        D::ListItem,
        D::InlineBlock,
        D::Table,
        D::InlineTable,
        D::TableRowGroup,
        D::TableHeaderGroup,
        D::TableFooterGroup,
        D::TableRow,
        D::TableColumnGroup,
        D::TableColumn,
        D::TableCell,
        D::TableCaption,
    ];
    // Expected wrapper counts, stated separately from the production role predicates.
    let rows = [
        (D::Table, [2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0, 1, 0]),
        (D::InlineTable, [2, 2, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0, 1, 0]),
        (D::TableRowGroup, [2, 2, 2, 2, 2, 2, 2, 2, 2, 0, 2, 2, 1, 2]),
        (
            D::TableHeaderGroup,
            [2, 2, 2, 2, 2, 2, 2, 2, 2, 0, 2, 2, 1, 2],
        ),
        (
            D::TableFooterGroup,
            [2, 2, 2, 2, 2, 2, 2, 2, 2, 0, 2, 2, 1, 2],
        ),
        (D::TableRow, [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1]),
    ];
    for (parent, counts) in rows {
        for (child, count) in children.into_iter().zip(counts) {
            let input = [original(None, child)];
            let stage1 = suppress_table_boxes(parent, &input, first_limits()).unwrap();
            let stage2 = wrap_table_children(&stage1, limits()).unwrap();
            let mut expected = Vec::new();
            if count > 0 && parent != D::TableRow {
                expected.push(output(
                    None,
                    O::GeneratedRow(if matches!(parent, D::Table | D::InlineTable) {
                        R::TableChildren
                    } else {
                        R::GroupChildren
                    }),
                ));
            }
            if count == 2 || (count == 1 && parent == D::TableRow) {
                expected.push(output(
                    if expected.is_empty() { None } else { Some(0) },
                    O::GeneratedCell,
                ));
            }
            expected.push(output(
                if count == 0 { None } else { Some(count - 1) },
                O::Original { index: 0 },
            ));
            assert_eq!(stage2.nodes(), expected, "{parent:?}/{child:?}");
            assert_eq!(stage2.original_mapping(), [Some(count)]);
            assert_eq!(stage2.kind(count), Some(RepairKind::Box(child)));
        }
    }
    for parent in [
        D::Inline,
        D::Block,
        D::InlineBlock,
        D::ListItem,
        D::TableCell,
        D::TableCaption,
    ] {
        for child in children {
            let input = [original(None, child)];
            let stage1 = suppress_table_boxes(parent, &input, first_limits()).unwrap();
            assert_eq!(
                wrap_table_children(&stage1, limits()).unwrap().nodes(),
                [output(None, O::Original { index: 0 })]
            );
        }
    }
}

#[test]
fn row_and_cell_runs_preserve_all_proper_separators() {
    for separator in [
        D::TableRow,
        D::TableRowGroup,
        D::TableHeaderGroup,
        D::TableFooterGroup,
        D::TableColumn,
        D::TableColumnGroup,
        D::TableCaption,
    ] {
        let input = [
            original(None, D::Inline),
            original(None, D::TableCell),
            original(None, D::Block),
            original(None, separator),
            original(None, D::Inline),
        ];
        let stage1 = suppress_table_boxes(D::Table, &input, first_limits()).unwrap();
        let stage2 = wrap_table_children(&stage1, limits()).unwrap();
        assert_eq!(
            stage2.nodes(),
            [
                output(None, O::GeneratedRow(R::TableChildren)),
                output(Some(0), O::GeneratedCell),
                output(Some(1), O::Original { index: 0 }),
                output(Some(0), O::Original { index: 1 }),
                output(Some(0), O::GeneratedCell),
                output(Some(4), O::Original { index: 2 }),
                output(None, O::Original { index: 3 }),
                output(None, O::GeneratedRow(R::TableChildren)),
                output(Some(7), O::GeneratedCell),
                output(Some(8), O::Original { index: 4 }),
            ]
        );
        assert_eq!(
            stage2.original_mapping(),
            [Some(2), Some(3), Some(5), Some(6), Some(9)]
        );
    }
    let input = [original(None, D::TableCell), original(None, D::TableCell)];
    let stage1 = suppress_table_boxes(D::Table, &input, first_limits()).unwrap();
    assert_eq!(
        wrap_table_children(&stage1, limits()).unwrap().nodes(),
        [
            output(None, O::GeneratedRow(R::TableChildren)),
            output(Some(0), O::Original { index: 0 }),
            output(Some(0), O::Original { index: 1 }),
        ]
    );
}

#[test]
fn non_depth_first_siblings_keep_separate_parent_run_state() {
    let input = [
        original(None, D::Table),
        original(None, D::TableRow),
        original(Some(0), D::Inline),
        original(Some(1), D::Inline),
        original(Some(2), D::TableRow),
        original(Some(0), D::Block),
        original(Some(1), D::Block),
        original(Some(4), D::Inline),
    ];
    let before = input;
    let stage1 = suppress_table_boxes(D::Block, &input, first_limits()).unwrap();
    let stage2 = wrap_table_children(&stage1, limits()).unwrap();
    assert_eq!(
        stage2.nodes(),
        [
            output(None, O::Original { index: 0 }),
            output(None, O::Original { index: 1 }),
            output(Some(0), O::GeneratedRow(R::TableChildren)),
            output(Some(2), O::GeneratedCell),
            output(Some(3), O::Original { index: 2 }),
            output(Some(1), O::GeneratedCell),
            output(Some(5), O::Original { index: 3 }),
            output(Some(4), O::Original { index: 4 }),
            output(Some(3), O::Original { index: 5 }),
            output(Some(5), O::Original { index: 6 }),
            output(Some(7), O::GeneratedCell),
            output(Some(10), O::Original { index: 7 }),
        ]
    );
    assert_eq!(
        stage2.original_mapping(),
        [
            Some(0),
            Some(1),
            Some(4),
            Some(6),
            Some(7),
            Some(8),
            Some(9),
            Some(11)
        ]
    );
    assert_eq!(input, before);
    assert!(std::ptr::eq(stage2.stage1(), &stage1));
    assert_eq!(stage2.external_parent(), D::Block);
}

#[test]
fn groups_and_original_rows_end_runs_at_their_own_child_boundaries() {
    let input = [
        original(None, D::TableCell),
        original(None, D::Inline),
        original(None, D::Block),
        original(None, D::TableRow),
        original(None, D::TableCell),
    ];
    for parent in [D::TableRowGroup, D::TableHeaderGroup, D::TableFooterGroup] {
        let stage1 = suppress_table_boxes(parent, &input, first_limits()).unwrap();
        let result = wrap_table_children(&stage1, limits()).unwrap();
        assert_eq!(
            result.nodes(),
            [
                output(None, O::GeneratedRow(R::GroupChildren)),
                output(Some(0), O::Original { index: 0 }),
                output(Some(0), O::GeneratedCell),
                output(Some(2), O::Original { index: 1 }),
                output(Some(2), O::Original { index: 2 }),
                output(None, O::Original { index: 3 }),
                output(None, O::GeneratedRow(R::GroupChildren)),
                output(Some(6), O::Original { index: 4 }),
            ]
        );
        assert_eq!(
            result.original_mapping(),
            [Some(1), Some(3), Some(4), Some(5), Some(7)]
        );
    }
    let input = [
        original(None, D::Inline),
        original(None, D::Block),
        original(None, D::TableCell),
        original(None, D::Inline),
    ];
    let stage1 = suppress_table_boxes(D::TableRow, &input, first_limits()).unwrap();
    let result = wrap_table_children(&stage1, limits()).unwrap();
    assert_eq!(
        result.nodes(),
        [
            output(None, O::GeneratedCell),
            output(Some(0), O::Original { index: 0 }),
            output(Some(0), O::Original { index: 1 }),
            output(None, O::Original { index: 2 }),
            output(None, O::GeneratedCell),
            output(Some(4), O::Original { index: 3 }),
        ]
    );
    assert_eq!(
        result.original_mapping(),
        [Some(1), Some(2), Some(3), Some(5)]
    );
}

#[test]
fn surviving_whitespace_and_placeholders_are_not_reclassified() {
    let input = [
        original(None, D::TableCell),
        RepairNode {
            parent: None,
            kind: RepairKind::AnonymousWhitespace,
        },
        RepairNode {
            parent: None,
            kind: RepairKind::OutOfFlowPlaceholder,
        },
        original(None, D::TableCell),
    ];
    let stage1 = suppress_table_boxes(D::Table, &input, first_limits()).unwrap();
    assert_eq!(stage1.dispositions(), [Disposition::Survives; 4]);
    let stage2 = wrap_table_children(&stage1, limits()).unwrap();
    assert_eq!(
        stage2.nodes(),
        [
            output(None, O::GeneratedRow(R::TableChildren)),
            output(Some(0), O::Original { index: 0 }),
            output(Some(0), O::GeneratedCell),
            output(Some(2), O::Original { index: 1 }),
            output(Some(2), O::Original { index: 2 }),
            output(Some(0), O::Original { index: 3 }),
        ]
    );
    assert_eq!(stage2.kind(3), Some(RepairKind::AnonymousWhitespace));
    assert_eq!(stage2.kind(4), Some(RepairKind::OutOfFlowPlaceholder));
    assert_eq!(stage2.kind(0), Some(RepairKind::Box(D::TableRow)));
    assert_eq!(stage2.kind(2), Some(RepairKind::Box(D::TableCell)));
    assert_eq!(stage2.kind(u32::MAX), None);
}

#[test]
fn suppression_keeps_original_indices_and_never_splices_descendants() {
    let input = [
        original(None, D::TableColumn),
        original(Some(0), D::TableRow),
        original(Some(1), D::Inline),
        original(None, D::TableColumnGroup),
        original(Some(3), D::TableColumn),
        original(Some(3), D::TableCell),
    ];
    let stage1 = suppress_table_boxes(D::Table, &input, first_limits()).unwrap();
    let dispositions = stage1.dispositions().to_vec();
    let stage2 = wrap_table_children(&stage1, limits()).unwrap();
    assert_eq!(
        stage2.nodes(),
        [
            output(None, O::Original { index: 0 }),
            output(None, O::Original { index: 3 }),
            output(Some(1), O::Original { index: 4 })
        ]
    );
    assert_eq!(
        stage2.original_mapping(),
        [Some(0), None, None, Some(1), Some(2), None]
    );
    assert_eq!(stage1.dispositions(), dispositions);
    let suppressed = suppress_table_boxes(D::TableColumn, &input, first_limits()).unwrap();
    let result = wrap_table_children(&suppressed, limits()).unwrap();
    assert!(result.nodes().is_empty());
    assert_eq!(result.original_mapping(), [None; 6]);
}

#[test]
fn exact_output_work_and_actual_capacity_admission() {
    let input = [original(None, D::Block)];
    let stage1 = suppress_table_boxes(D::Table, &input, first_limits()).unwrap();
    let exact = Stage2Limits {
        input_nodes: nz(1),
        output_nodes: nz(3),
        work: nz(14),
        ..limits()
    };
    let result = wrap_table_children(&stage1, exact).unwrap();
    let peak = result.peak_retained_bytes();
    assert!(peak > result.retained_bytes());
    assert_eq!(result.nodes().len(), 3);
    assert!(
        wrap_table_children(
            &stage1,
            Stage2Limits {
                retained_bytes: nz(peak),
                ..exact
            }
        )
        .is_ok()
    );
    for (cap, error) in [
        (
            Stage2Limits {
                output_nodes: nz(2),
                ..exact
            },
            Stage2Error::OutputNodes,
        ),
        (
            Stage2Limits {
                work: nz(13),
                ..exact
            },
            Stage2Error::Work,
        ),
        (
            Stage2Limits {
                retained_bytes: nz(peak - 1),
                ..exact
            },
            Stage2Error::Bytes,
        ),
        (
            Stage2Limits {
                retained_bytes: nz(1),
                ..exact
            },
            Stage2Error::Bytes,
        ),
    ] {
        assert_eq!(wrap_table_children(&stage1, cap).unwrap_err(), error);
    }
    let proper = [original(None, D::TableCaption)];
    let proper = suppress_table_boxes(D::Table, &proper, first_limits()).unwrap();
    let result = wrap_table_children(
        &proper,
        Stage2Limits {
            output_nodes: nz(1),
            work: nz(10),
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(result.nodes().len(), 1); // Actual count, not conservative 3*s.
    let two = [original(None, D::Inline); 2];
    let two = suppress_table_boxes(D::Table, &two, first_limits()).unwrap();
    assert_eq!(
        wrap_table_children(
            &two,
            Stage2Limits {
                input_nodes: nz(1),
                ..limits()
            }
        )
        .unwrap_err(),
        Stage2Error::InputNodes
    );
    let empty = suppress_table_boxes(D::Table, &[], first_limits()).unwrap();
    let result = wrap_table_children(
        &empty,
        Stage2Limits {
            work: nz(1),
            retained_bytes: nz(1),
            ..limits()
        },
    )
    .unwrap();
    assert!(result.nodes().is_empty());
    assert!(result.original_mapping().is_empty());
    assert_eq!(
        (result.retained_bytes(), result.peak_retained_bytes()),
        (0, 0)
    );
    assert_eq!(stage1.input(), input);
}

#[test]
fn deep_and_wide_forests_remain_bounded_without_recursive_traversal() {
    let depth = 1000u32;
    let input: Vec<_> = (0..depth)
        .map(|index| original(index.checked_sub(1), D::Table))
        .collect();
    let stage1 = suppress_table_boxes(D::Table, &input, first_limits()).unwrap();
    let result = wrap_table_children(
        &stage1,
        Stage2Limits {
            output_nodes: nz(3 * depth as usize),
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(result.nodes().len(), 3 * depth as usize);
    for index in 0..depth {
        assert_eq!(
            result.original_mapping()[index as usize],
            Some(3 * index + 2)
        );
    }
    let input = vec![original(None, D::Inline); 1000];
    let stage1 = suppress_table_boxes(D::Table, &input, first_limits()).unwrap();
    let result = wrap_table_children(&stage1, limits()).unwrap();
    assert_eq!(result.nodes().len(), 1002);
    assert!(
        result.nodes()[2..]
            .iter()
            .all(|node| node.parent == Some(1))
    );
    for (index, node) in result.nodes().iter().enumerate() {
        assert!(node.parent.is_none_or(|parent| (parent as usize) < index));
    }
}
