use iced_css_core::formatting::{
    display::Display as D,
    table_repair_stage1::{RepairKind as K, RepairLimits, RepairNode, suppress_table_boxes},
    table_repair_stage2::{Stage2Limits, Stage2Origin, wrap_table_children},
    table_repair_stage3::{
        Stage3Error, Stage3Limits, Stage3Node, Stage3Origin as O, TableWrapperKind as T,
        wrap_table_parents,
    },
};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn first_limits() -> RepairLimits {
    RepairLimits {
        nodes: nz(20000),
        retained_bytes: nz(4_000_000),
        work: nz(4_000_000),
    }
}
fn second_limits() -> Stage2Limits {
    Stage2Limits {
        input_nodes: nz(20000),
        output_nodes: nz(60000),
        retained_bytes: nz(8_000_000),
        work: nz(4_000_000),
    }
}
fn limits() -> Stage3Limits {
    Stage3Limits {
        input_nodes: nz(60000),
        output_nodes: nz(180000),
        retained_bytes: nz(16_000_000),
        work: nz(4_000_000),
    }
}
fn node(parent: Option<u32>, role: D) -> RepairNode {
    RepairNode {
        parent,
        kind: K::Box(role),
    }
}
fn special(parent: Option<u32>, kind: K) -> RepairNode {
    RepairNode { parent, kind }
}
fn out(parent: Option<u32>, origin: O) -> Stage3Node {
    Stage3Node { parent, origin }
}
fn check(parent: D, input: &[RepairNode], expected: &[Stage3Node]) {
    let first = suppress_table_boxes(parent, input, first_limits()).unwrap();
    let second = wrap_table_children(&first, second_limits()).unwrap();
    let third = wrap_table_parents(&second, limits()).unwrap();
    assert_eq!(
        third.nodes(),
        expected,
        "external {parent:?}; input {input:?}"
    );
    assert!(std::ptr::eq(third.stage2(), &second));
    assert!(std::ptr::eq(third.stage2().stage1().input(), input));
    assert_eq!(third.external_parent(), parent);
    for (index, mapped) in third.previous_mapping().iter().copied().enumerate() {
        assert_eq!(
            third.nodes()[mapped as usize].origin,
            O::Existing {
                index: index as u32
            }
        );
        assert_eq!(third.kind(mapped), second.kind(index as u32));
    }
    for (original, previous) in second.original_mapping().iter().enumerate() {
        assert_eq!(
            third.original_output(original as u32),
            previous.map(|p| third.previous_mapping()[p as usize])
        );
    }
    assert_eq!(third.original_output(u32::MAX), None);
    assert_eq!(third.kind(u32::MAX), None);
}
#[test]
fn proper_parent_contexts_preserve_existing_nodes() {
    let proper = [
        D::TableRow,
        D::TableRowGroup,
        D::TableHeaderGroup,
        D::TableFooterGroup,
        D::TableColumn,
        D::TableColumnGroup,
        D::TableCaption,
    ];
    for parent in [D::Table, D::InlineTable] {
        for child in proper {
            check(
                parent,
                &[node(None, child)],
                &[out(None, O::Existing { index: 0 })],
            );
        }
    }
    for parent in [D::TableRowGroup, D::TableHeaderGroup, D::TableFooterGroup] {
        check(
            parent,
            &[node(None, D::TableRow), node(None, D::TableRow)],
            &[
                out(None, O::Existing { index: 0 }),
                out(None, O::Existing { index: 1 }),
            ],
        );
    }
    check(
        D::TableRow,
        &[node(None, D::TableCell), node(None, D::TableCell)],
        &[
            out(None, O::Existing { index: 0 }),
            out(None, O::Existing { index: 1 }),
        ],
    );
    check(
        D::TableColumnGroup,
        &[node(None, D::TableColumn), node(None, D::TableColumn)],
        &[
            out(None, O::Existing { index: 0 }),
            out(None, O::Existing { index: 1 }),
        ],
    );
    check(D::TableColumn, &[node(None, D::Block)], &[]);
}
#[test]
fn generic_parents_and_each_proper_role_choose_actual_table_kind() {
    for parent in [
        D::Inline,
        D::Block,
        D::InlineBlock,
        D::ListItem,
        D::TableCell,
        D::TableCaption,
    ] {
        let table = if parent == D::Inline {
            T::InlineTable
        } else {
            T::Table
        };
        for child in [
            D::TableRow,
            D::TableRowGroup,
            D::TableHeaderGroup,
            D::TableFooterGroup,
            D::TableColumn,
            D::TableColumnGroup,
            D::TableCaption,
        ] {
            check(
                parent,
                &[node(None, child)],
                &[
                    out(None, O::GeneratedTable(table)),
                    out(Some(0), O::Existing { index: 0 }),
                ],
            );
        }
        check(
            parent,
            &[node(None, D::TableCell)],
            &[
                out(None, O::GeneratedTable(table)),
                out(Some(0), O::GeneratedRow),
                out(Some(1), O::Existing { index: 0 }),
            ],
        );
    }
}
#[test]
fn mixed_cell_and_proper_runs_share_only_the_required_wrappers() {
    let roles = [
        D::TableCell,
        D::TableCell,
        D::TableRow,
        D::TableColumn,
        D::TableRowGroup,
        D::TableHeaderGroup,
        D::TableFooterGroup,
        D::TableColumnGroup,
        D::TableCaption,
        D::TableCell,
    ];
    let input: Vec<_> = roles.into_iter().map(|d| node(None, d)).collect();
    let mut expected = vec![
        out(None, O::GeneratedTable(T::InlineTable)),
        out(Some(0), O::GeneratedRow),
        out(Some(1), O::Existing { index: 0 }),
        out(Some(1), O::Existing { index: 1 }),
    ];
    for index in 2..9 {
        expected.push(out(Some(0), O::Existing { index }));
    }
    expected.push(out(Some(0), O::GeneratedRow));
    expected.push(out(Some(11), O::Existing { index: 9 }));
    check(D::Inline, &input, &expected);
    check(
        D::Block,
        &[
            node(None, D::TableCell),
            node(None, D::TableCaption),
            node(None, D::TableCell),
        ],
        &[
            out(None, O::GeneratedTable(T::Table)),
            out(Some(0), O::GeneratedRow),
            out(Some(1), O::Existing { index: 0 }),
            out(Some(0), O::Existing { index: 1 }),
            out(Some(0), O::GeneratedRow),
            out(Some(4), O::Existing { index: 2 }),
        ],
    );
}
#[test]
fn whitespace_edges_placeholders_and_nonqualifiers_break_runs() {
    check(
        D::Inline,
        &[
            special(None, K::AnonymousWhitespace),
            node(None, D::TableCell),
            special(None, K::AnonymousWhitespace),
        ],
        &[
            out(None, O::Existing { index: 0 }),
            out(None, O::GeneratedTable(T::InlineTable)),
            out(Some(1), O::GeneratedRow),
            out(Some(2), O::Existing { index: 1 }),
            out(None, O::Existing { index: 2 }),
        ],
    );
    for kind in [
        K::OutOfFlowPlaceholder,
        K::Box(D::Inline),
        K::Box(D::Block),
        K::Box(D::Table),
        K::Box(D::InlineTable),
        K::Box(D::InlineBlock),
        K::Box(D::ListItem),
    ] {
        check(
            D::Block,
            &[
                node(None, D::TableCell),
                special(None, kind),
                node(None, D::TableCell),
            ],
            &[
                out(None, O::GeneratedTable(T::Table)),
                out(Some(0), O::GeneratedRow),
                out(Some(1), O::Existing { index: 0 }),
                out(None, O::Existing { index: 1 }),
                out(None, O::GeneratedTable(T::Table)),
                out(Some(4), O::GeneratedRow),
                out(Some(5), O::Existing { index: 2 }),
            ],
        );
    }
    check(
        D::Block,
        &[
            node(None, D::TableCell),
            special(None, K::AnonymousWhitespace),
            node(None, D::Inline),
        ],
        &[
            out(None, O::GeneratedTable(T::Table)),
            out(Some(0), O::GeneratedRow),
            out(Some(1), O::Existing { index: 0 }),
            out(None, O::Existing { index: 1 }),
            out(None, O::Existing { index: 2 }),
        ],
    );
}
#[test]
fn stage2_generated_cells_supply_real_parent_and_keep_provenance() {
    // Stage2: external group -> GeneratedRow -> GeneratedCell -> Original Caption.
    for parent in [D::TableRowGroup, D::TableHeaderGroup, D::TableFooterGroup] {
        check(
            parent,
            &[node(None, D::TableCaption)],
            &[
                out(None, O::Existing { index: 0 }),
                out(Some(0), O::Existing { index: 1 }),
                out(Some(1), O::GeneratedTable(T::Table)),
                out(Some(2), O::Existing { index: 2 }),
            ],
        );
    }
    // Source ancestry includes Inline, but actual parent of Caption is generated Cell.
    let input = [node(None, D::TableRow), node(Some(0), D::TableCaption)];
    check(
        D::Inline,
        &input,
        &[
            out(None, O::GeneratedTable(T::InlineTable)),
            out(Some(0), O::Existing { index: 0 }),
            out(Some(1), O::Existing { index: 1 }),
            out(Some(2), O::GeneratedTable(T::Table)),
            out(Some(3), O::Existing { index: 2 }),
        ],
    );
    let input = [node(None, D::Inline)];
    let first = suppress_table_boxes(D::TableRow, &input, first_limits()).unwrap();
    let second = wrap_table_children(&first, second_limits()).unwrap();
    let third = wrap_table_parents(&second, limits()).unwrap();
    assert_eq!(third.nodes()[0].origin, O::Existing { index: 0 });
    assert_eq!(
        third.stage2().nodes()[0].origin,
        Stage2Origin::GeneratedCell
    );
}
#[test]
fn interleaved_descendants_keep_independent_parent_runs() {
    let input = [
        node(None, D::TableCell),
        node(Some(0), D::TableCaption),
        node(None, D::TableCell),
    ];
    check(
        D::Block,
        &input,
        &[
            out(None, O::GeneratedTable(T::Table)),
            out(Some(0), O::GeneratedRow),
            out(Some(1), O::Existing { index: 0 }),
            out(Some(2), O::GeneratedTable(T::Table)),
            out(Some(3), O::Existing { index: 1 }),
            out(Some(1), O::Existing { index: 2 }),
        ],
    );
    let input = [
        node(None, D::Inline),
        node(None, D::Block),
        node(Some(0), D::TableCell),
        node(Some(1), D::TableCell),
        node(Some(0), D::TableCell),
        node(Some(1), D::TableCaption),
    ];
    check(
        D::Block,
        &input,
        &[
            out(None, O::Existing { index: 0 }),
            out(None, O::Existing { index: 1 }),
            out(Some(0), O::GeneratedTable(T::InlineTable)),
            out(Some(2), O::GeneratedRow),
            out(Some(3), O::Existing { index: 2 }),
            out(Some(1), O::GeneratedTable(T::Table)),
            out(Some(5), O::GeneratedRow),
            out(Some(6), O::Existing { index: 3 }),
            out(Some(3), O::Existing { index: 4 }),
            out(Some(5), O::Existing { index: 5 }),
        ],
    );
}
#[test]
fn suppression_maps_and_exact_resource_admission() {
    let bridged = [
        node(None, D::TableCell),
        special(None, K::AnonymousWhitespace),
        node(None, D::TableCell),
    ];
    let first = suppress_table_boxes(D::Block, &bridged, first_limits()).unwrap();
    let second = wrap_table_children(&first, second_limits()).unwrap();
    let third = wrap_table_parents(&second, limits()).unwrap();
    assert_eq!(
        third.nodes(),
        [
            out(None, O::GeneratedTable(T::Table)),
            out(Some(0), O::GeneratedRow),
            out(Some(1), O::Existing { index: 0 }),
            out(Some(1), O::Existing { index: 1 })
        ]
    );
    assert_eq!(
        (
            third.original_output(0),
            third.original_output(1),
            third.original_output(2)
        ),
        (Some(2), None, Some(3))
    );

    let input = [
        node(None, D::TableColumn),
        node(Some(0), D::TableCell),
        node(Some(1), D::Inline),
    ];
    let first = suppress_table_boxes(D::Table, &input, first_limits()).unwrap();
    let second = wrap_table_children(&first, second_limits()).unwrap();
    let third = wrap_table_parents(&second, limits()).unwrap();
    assert_eq!(third.original_output(0), Some(0));
    assert_eq!(third.original_output(1), None);
    assert_eq!(third.original_output(2), None);
    let mut cap = limits();
    cap.output_nodes = nz(1);
    cap.work = nz(10); // 5*1+3+2*1
    assert!(wrap_table_parents(&second, cap).is_ok());
    cap.work = nz(9);
    assert_eq!(
        wrap_table_parents(&second, cap).unwrap_err(),
        Stage3Error::Work
    );
    let input = [node(None, D::TableCell), node(None, D::TableCell)];
    let first = suppress_table_boxes(D::Block, &input, first_limits()).unwrap();
    let second = wrap_table_children(&first, second_limits()).unwrap();
    cap = limits();
    cap.input_nodes = nz(1);
    assert_eq!(
        wrap_table_parents(&second, cap).unwrap_err(),
        Stage3Error::InputNodes
    );
    cap = limits();
    cap.output_nodes = nz(3);
    assert_eq!(
        wrap_table_parents(&second, cap).unwrap_err(),
        Stage3Error::OutputNodes
    );
    cap.output_nodes = nz(4);
    cap.work = nz(21);
    let output = wrap_table_parents(&second, cap).unwrap(); // 5*2+3+2*4
    assert_eq!(output.nodes().len(), 4);
    cap.work = nz(20);
    assert_eq!(
        wrap_table_parents(&second, cap).unwrap_err(),
        Stage3Error::Work
    );
    cap.work = nz(21);
    cap.retained_bytes = nz(output.peak_retained_bytes());
    assert!(wrap_table_parents(&second, cap).is_ok());
    cap.retained_bytes = nz(output.peak_retained_bytes() - 1);
    assert_eq!(
        wrap_table_parents(&second, cap).unwrap_err(),
        Stage3Error::Bytes
    );
    cap.retained_bytes = nz(1);
    assert_eq!(
        wrap_table_parents(&second, cap).unwrap_err(),
        Stage3Error::Bytes
    );
    cap.retained_bytes = nz(usize::MAX);
    assert_eq!(
        wrap_table_parents(&second, cap).unwrap_err(),
        Stage3Error::Bytes
    );
    assert_eq!(second.nodes().len(), 2);
    assert_eq!(second.original_mapping(), [Some(0), Some(1)]);
    let first = suppress_table_boxes(D::TableColumn, &input, first_limits()).unwrap();
    let second = wrap_table_children(&first, second_limits()).unwrap();
    cap = limits();
    cap.work = nz(1);
    cap.retained_bytes = nz(1);
    let empty = wrap_table_parents(&second, cap).unwrap();
    assert!(empty.nodes().is_empty());
    assert!(empty.previous_mapping().is_empty());
    assert_eq!(empty.original_output(0), None);
    assert_eq!(empty.retained_bytes(), 0);
    assert_eq!(empty.peak_retained_bytes(), 0);
    check(D::Inline, &[], &[]);
}
#[test]
fn deep_and_wide_forests_preserve_identity_without_recursion() {
    let deep: Vec<_> = (0..2000)
        .map(|i| node((i != 0).then(|| i - 1), D::TableCell))
        .collect();
    let first = suppress_table_boxes(D::Block, &deep, first_limits()).unwrap();
    let second = wrap_table_children(&first, second_limits()).unwrap();
    let third = wrap_table_parents(&second, limits()).unwrap();
    assert_eq!(third.nodes().len(), 6000);
    for i in 0..2000u32 {
        let base = i * 3;
        assert_eq!(
            third.nodes()[base as usize],
            out((i != 0).then(|| base - 1), O::GeneratedTable(T::Table))
        );
        assert_eq!(
            third.nodes()[base as usize + 1],
            out(Some(base), O::GeneratedRow)
        );
        assert_eq!(
            third.nodes()[base as usize + 2],
            out(Some(base + 1), O::Existing { index: i })
        );
        assert_eq!(third.original_output(i), Some(base + 2));
    }
    let wide: Vec<_> = (0..2000).map(|_| node(None, D::TableCell)).collect();
    let first = suppress_table_boxes(D::Inline, &wide, first_limits()).unwrap();
    let second = wrap_table_children(&first, second_limits()).unwrap();
    let third = wrap_table_parents(&second, limits()).unwrap();
    assert_eq!(third.nodes().len(), 2002);
    assert_eq!(
        third.nodes()[0],
        out(None, O::GeneratedTable(T::InlineTable))
    );
    assert_eq!(third.nodes()[1], out(Some(0), O::GeneratedRow));
    for i in 0..2000u32 {
        assert_eq!(
            third.nodes()[i as usize + 2],
            out(Some(1), O::Existing { index: i })
        );
    }
}
