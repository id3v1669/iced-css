use iced_css_core::formatting::{
    display::Display as D,
    table_repair_stage1::{RepairKind, RepairLimits, RepairNode, suppress_table_boxes},
    table_repair_stage2::{Stage2Limits, Stage2Origin, wrap_table_children},
    table_repair_stage3::{
        Stage3, Stage3Limits, Stage3Origin, TableWrapperKind, wrap_table_parents,
    },
    table_row_inventory::{
        BandKind as B, GroupTreatment as G, InventoryError as E, InventoryLimits, RowBand,
        TableCellRef, TableRow, TableTarget as T, collect_table_rows,
    },
};
use std::{num::NonZeroUsize, ops::Range};
fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn limits() -> InventoryLimits {
    InventoryLimits {
        input_nodes: nz(10000),
        bands: nz(10000),
        rows: nz(10000),
        cells: nz(10000),
        retained_bytes: nz(4_000_000),
        work: nz(1_000_000),
    }
}
fn node(parent: Option<u32>, role: D) -> RepairNode {
    RepairNode {
        parent,
        kind: RepairKind::Box(role),
    }
}
fn with_tree(parent: D, input: &[RepairNode], test: impl FnOnce(&Stage3<'_, '_, '_>)) {
    let n = nz(1_000_000);
    let first = suppress_table_boxes(
        parent,
        input,
        RepairLimits {
            nodes: n,
            retained_bytes: n,
            work: n,
        },
    )
    .unwrap();
    let second = wrap_table_children(
        &first,
        Stage2Limits {
            input_nodes: n,
            output_nodes: n,
            retained_bytes: n,
            work: n,
        },
    )
    .unwrap();
    let third = wrap_table_parents(
        &second,
        Stage3Limits {
            input_nodes: n,
            output_nodes: n,
            retained_bytes: n,
            work: n,
        },
    )
    .unwrap();
    test(&third);
}
fn band(node: u32, kind: B, rows: Range<u32>) -> RowBand {
    RowBand { node, kind, rows }
}
fn row(node: u32, group: Option<u32>, cells: Range<u32>) -> TableRow {
    TableRow { node, group, cells }
}
fn cell(node: u32, row: u32) -> TableCellRef {
    TableCellRef { node, row }
}

#[test]
fn non_depth_first_groups_order_rows_and_cells_by_parent_membership() {
    let input = [
        node(None, D::TableRowGroup),
        node(None, D::TableRowGroup),
        node(Some(1), D::TableRow),
        node(Some(0), D::TableRow),
        node(Some(2), D::TableCell),
        node(Some(3), D::TableCell),
    ];
    for parent in [D::Table, D::InlineTable] {
        with_tree(parent, &input, |third| {
            let view = collect_table_rows(
                third,
                T::External,
                InventoryLimits {
                    work: nz(46),
                    ..limits()
                },
            )
            .unwrap();
            assert_eq!(
                view.bands(),
                [
                    band(0, B::Group(G::Body), 0..1),
                    band(1, B::Group(G::Body), 1..2)
                ]
            );
            assert_eq!(view.rows(), [row(3, Some(0), 0..1), row(2, Some(1), 1..2)]);
            assert_eq!(view.cells(), [cell(5, 0), cell(4, 1)]);
            assert!(std::ptr::eq(view.stage3(), third));
            assert_eq!(view.target(), T::External);
            assert_eq!(
                collect_table_rows(
                    third,
                    T::External,
                    InventoryLimits {
                        work: nz(45),
                        ..limits()
                    }
                )
                .unwrap_err(),
                E::Work
            );
        });
    }
}

#[test]
fn first_header_and_footer_move_while_duplicate_groups_keep_middle_order() {
    let input = [
        node(None, D::TableFooterGroup),
        node(None, D::TableRow),
        node(None, D::TableHeaderGroup),
        node(None, D::TableRowGroup),
        node(None, D::TableHeaderGroup),
        node(None, D::TableFooterGroup),
        node(None, D::TableRow),
        node(Some(0), D::TableRow),
        node(Some(2), D::TableRow),
        node(Some(3), D::TableRow),
        node(Some(4), D::TableRow),
        node(Some(5), D::TableRow),
        node(Some(7), D::TableCell),
        node(Some(1), D::TableCell),
        node(Some(8), D::TableCell),
        node(Some(9), D::TableCell),
        node(Some(10), D::TableCell),
        node(Some(11), D::TableCell),
        node(Some(6), D::TableCell),
    ];
    let before = input;
    with_tree(D::Table, &input, |third| {
        // n=19, c=7, h=f=1, g=5, b=r=k=7: W=42, total=150.
        let view = collect_table_rows(
            third,
            T::External,
            InventoryLimits {
                work: nz(150),
                ..limits()
            },
        )
        .unwrap();
        assert_eq!(
            collect_table_rows(
                third,
                T::External,
                InventoryLimits {
                    work: nz(149),
                    ..limits()
                }
            )
            .unwrap_err(),
            E::Work
        );
        assert_eq!(
            view.bands(),
            [
                band(2, B::Group(G::Header), 0..1),
                band(1, B::DirectRow, 1..2),
                band(3, B::Group(G::Body), 2..3),
                band(4, B::Group(G::Body), 3..4),
                band(5, B::Group(G::Body), 4..5),
                band(6, B::DirectRow, 5..6),
                band(0, B::Group(G::Footer), 6..7)
            ]
        );
        assert_eq!(
            view.rows(),
            [
                row(8, Some(2), 0..1),
                row(1, None, 1..2),
                row(9, Some(3), 2..3),
                row(10, Some(4), 3..4),
                row(11, Some(5), 4..5),
                row(6, None, 5..6),
                row(7, Some(0), 6..7)
            ]
        );
        assert_eq!(
            view.cells(),
            [
                cell(14, 0),
                cell(13, 1),
                cell(15, 2),
                cell(16, 3),
                cell(17, 4),
                cell(18, 5),
                cell(12, 6)
            ]
        );
        assert_eq!(third.kind(4), Some(RepairKind::Box(D::TableHeaderGroup)));
        assert_eq!(third.kind(5), Some(RepairKind::Box(D::TableFooterGroup)));
    });
    assert_eq!(input, before);
}

#[test]
fn empty_first_groups_keep_designation_and_empty_rows_keep_grid_ordinals() {
    let input = [
        node(None, D::TableFooterGroup),
        node(None, D::TableHeaderGroup),
        node(None, D::TableHeaderGroup),
        node(None, D::TableFooterGroup),
        node(None, D::TableRow),
        node(None, D::TableRowGroup),
        node(Some(2), D::TableRow),
        node(Some(6), D::TableCell),
        node(Some(3), D::TableRow),
    ];
    with_tree(D::Table, &input, |third| {
        let view = collect_table_rows(third, T::External, limits()).unwrap();
        assert_eq!(
            view.bands(),
            [
                band(1, B::Group(G::Header), 0..0),
                band(2, B::Group(G::Body), 0..1),
                band(3, B::Group(G::Body), 1..2),
                band(4, B::DirectRow, 2..3),
                band(5, B::Group(G::Body), 3..3),
                band(0, B::Group(G::Footer), 3..3)
            ]
        );
        assert_eq!(
            view.rows(),
            [
                row(6, Some(2), 0..1),
                row(8, Some(3), 1..1),
                row(4, None, 1..1)
            ]
        );
        assert_eq!(view.cells(), [cell(7, 0)]);
    });
}

#[test]
fn node_targets_exclude_nested_tables_and_reject_unrelated_contexts() {
    let input = [
        node(None, D::Table),
        node(Some(0), D::TableRow),
        node(Some(1), D::TableCell),
        node(Some(2), D::Table),
        node(Some(3), D::TableRow),
        node(Some(4), D::TableCell),
        node(Some(0), D::TableCaption),
        node(Some(6), D::InlineTable),
        node(Some(7), D::TableRow),
        node(Some(8), D::TableCell),
    ];
    with_tree(D::Block, &input, |third| {
        for (table, expected_row, expected_cell) in [(0, 1, 2), (3, 4, 5), (7, 8, 9)] {
            let view = collect_table_rows(third, T::Node(table), limits()).unwrap();
            assert_eq!(view.target(), T::Node(table));
            assert_eq!(view.bands(), [band(expected_row, B::DirectRow, 0..1)]);
            assert_eq!(view.rows(), [row(expected_row, None, 0..1)]);
            assert_eq!(view.cells(), [cell(expected_cell, 0)]);
        }
        // All ten nodes enter the index, and the direct Caption is a paid visit.
        assert!(
            collect_table_rows(
                third,
                T::Node(0),
                InventoryLimits {
                    work: nz(46),
                    ..limits()
                }
            )
            .is_ok()
        );
        assert_eq!(
            collect_table_rows(
                third,
                T::Node(0),
                InventoryLimits {
                    work: nz(45),
                    ..limits()
                }
            )
            .unwrap_err(),
            E::Work
        );
        assert_eq!(
            collect_table_rows(third, T::Node(u32::MAX), limits()).unwrap_err(),
            E::TargetIndex { index: u32::MAX }
        );
        assert_eq!(
            collect_table_rows(third, T::Node(2), limits()).unwrap_err(),
            E::TargetRole { index: 2 }
        );
        assert_eq!(
            collect_table_rows(third, T::External, limits()).unwrap_err(),
            E::ExternalRole
        );
    });
}

#[test]
fn generated_rows_cells_and_tables_keep_the_repair_ownership_chain() {
    with_tree(D::Table, &[node(None, D::Inline)], |third| {
        let view = collect_table_rows(third, T::External, limits()).unwrap();
        assert_eq!(view.bands(), [band(0, B::DirectRow, 0..1)]);
        assert_eq!(view.rows(), [row(0, None, 0..1)]);
        assert_eq!(view.cells(), [cell(1, 0)]);
        assert_eq!(view.stage3().original_output(0), Some(2));
        assert_eq!(
            third.stage2().nodes()[1].origin,
            Stage2Origin::GeneratedCell
        );
    });
    with_tree(D::Inline, &[node(None, D::TableCell)], |third| {
        let view = collect_table_rows(third, T::Node(0), limits()).unwrap();
        assert_eq!(
            third.nodes()[0].origin,
            Stage3Origin::GeneratedTable(TableWrapperKind::InlineTable)
        );
        assert_eq!(view.bands(), [band(1, B::DirectRow, 0..1)]);
        assert_eq!(view.rows(), [row(1, None, 0..1)]);
        assert_eq!(view.cells(), [cell(2, 0)]);
        assert_eq!(third.original_output(0), Some(2));
    });
}

#[test]
fn column_and_caption_descriptors_do_not_contribute_rows_or_cells() {
    let input = [
        node(None, D::TableCaption),
        node(None, D::TableColumn),
        node(None, D::TableColumnGroup),
        node(Some(2), D::TableColumn),
        node(None, D::TableRow),
        node(Some(4), D::TableCell),
    ];
    with_tree(D::InlineTable, &input, |third| {
        let view = collect_table_rows(
            third,
            T::External,
            InventoryLimits {
                work: nz(40),
                ..limits()
            },
        )
        .unwrap();
        assert_eq!(view.bands(), [band(4, B::DirectRow, 0..1)]);
        assert_eq!(view.rows(), [row(4, None, 0..1)]);
        assert_eq!(view.cells(), [cell(5, 0)]);
        assert_eq!(
            collect_table_rows(
                third,
                T::External,
                InventoryLimits {
                    work: nz(39),
                    ..limits()
                }
            )
            .unwrap_err(),
            E::Work
        );
    });
}

#[test]
fn exact_category_work_and_observed_byte_limits_preserve_prior_owner() {
    let input = [
        node(None, D::TableRow),
        node(None, D::TableRow),
        node(Some(0), D::TableCell),
        node(Some(1), D::TableCell),
    ];
    with_tree(D::Table, &input, |third| {
        let before = third.nodes().to_vec();
        let exact = InventoryLimits {
            bands: nz(2),
            rows: nz(2),
            cells: nz(2),
            work: nz(36),
            ..limits()
        };
        let view = collect_table_rows(third, T::External, exact).unwrap();
        let peak = view.peak_retained_bytes();
        assert!(peak > view.retained_bytes());
        assert!(
            collect_table_rows(
                third,
                T::External,
                InventoryLimits {
                    retained_bytes: nz(peak),
                    ..exact
                }
            )
            .is_ok()
        );
        for (cap, error) in [
            (
                InventoryLimits {
                    input_nodes: nz(3),
                    ..exact
                },
                E::InputNodes,
            ),
            (
                InventoryLimits {
                    bands: nz(1),
                    ..exact
                },
                E::Bands,
            ),
            (
                InventoryLimits {
                    rows: nz(1),
                    ..exact
                },
                E::Rows,
            ),
            (
                InventoryLimits {
                    cells: nz(1),
                    ..exact
                },
                E::Cells,
            ),
            (
                InventoryLimits {
                    work: nz(35),
                    ..exact
                },
                E::Work,
            ),
            (
                InventoryLimits {
                    retained_bytes: nz(peak - 1),
                    ..exact
                },
                E::Bytes,
            ),
            (
                InventoryLimits {
                    retained_bytes: nz(1),
                    ..exact
                },
                E::Bytes,
            ),
        ] {
            assert_eq!(
                collect_table_rows(third, T::External, cap).unwrap_err(),
                error
            );
            assert_eq!(third.nodes(), before);
        }
    });
}

#[test]
fn tight_small_work_witnesses_include_empty_external_scratch() {
    for (input, work, output_counts) in [
        (vec![node(None, D::TableRow)], 12, (1, 1, 0)),
        (
            vec![node(None, D::TableRow), node(Some(0), D::TableCell)],
            19,
            (1, 1, 1),
        ),
        (vec![node(None, D::TableRowGroup)], 10, (1, 0, 0)),
        (vec![], 2, (0, 0, 0)),
    ] {
        with_tree(D::Table, &input, |third| {
            let view = collect_table_rows(
                third,
                T::External,
                InventoryLimits {
                    work: nz(work),
                    ..limits()
                },
            )
            .unwrap();
            assert_eq!(
                (view.bands().len(), view.rows().len(), view.cells().len()),
                output_counts
            );
            assert_eq!(
                collect_table_rows(
                    third,
                    T::External,
                    InventoryLimits {
                        work: nz(work - 1),
                        ..limits()
                    }
                )
                .unwrap_err(),
                E::Work
            );
            if input.is_empty() {
                assert_eq!(view.retained_bytes(), 0);
                assert!(view.peak_retained_bytes() > 0);
            }
        });
    }
}
