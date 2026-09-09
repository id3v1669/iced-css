use iced_css_core::formatting::{
    block::Direction,
    table_placement::{
        CellPlacement, CellSpan, TablePlacementError, TablePlacementLimits, place_table_cells,
    },
};
use std::num::{NonZeroU32, NonZeroUsize};
fn limits() -> TablePlacementLimits {
    let n = NonZeroUsize::new(100_000).unwrap();
    TablePlacementLimits {
        rows: n,
        columns: n,
        cells: n,
        retained_bytes: n,
        work: n,
    }
}
fn cell(row: u32, rows: u32, columns: u32) -> CellSpan {
    CellSpan {
        row,
        row_span: NonZeroU32::new(rows).unwrap(),
        column_span: NonZeroU32::new(columns).unwrap(),
    }
}
fn rect(row_start: u32, row_end: u32, column_start: u32, column_end: u32) -> CellPlacement {
    CellPlacement {
        row_start,
        row_end,
        column_start,
        column_end,
    }
}
#[test]
fn crossing_span_shifts_to_first_nonoverlapping_position_and_rtl_projects_once() {
    let input = [cell(0, 1, 1), cell(0, 2, 1), cell(1, 1, 2)];
    let grid = place_table_cells(&[2, 2], 0, &input, limits()).unwrap();
    assert_eq!((grid.rows(), grid.columns()), (2, 4));
    assert_eq!(
        grid.cells(),
        [rect(0, 1, 0, 1), rect(0, 2, 1, 2), rect(1, 2, 2, 4)]
    );
    assert_eq!(grid.physical_columns(0, Direction::Ltr), Some(0..1));
    assert_eq!(grid.physical_columns(0, Direction::Rtl), Some(3..4));
    assert_eq!(grid.physical_columns(1, Direction::Rtl), Some(2..3));
    assert_eq!(grid.physical_columns(2, Direction::Rtl), Some(0..2));
    assert_eq!(grid.physical_columns(3, Direction::Ltr), None);
}
#[test]
fn first_column_occupancy_and_group_clipping_do_not_create_extra_rows() {
    let input = [cell(0, u32::MAX, 2), cell(1, 1, 1), cell(2, u32::MAX, 1)];
    let grid = place_table_cells(&[2, 2, 4, 4], 0, &input, limits()).unwrap();
    assert_eq!((grid.rows(), grid.columns()), (4, 3));
    assert_eq!(
        grid.cells(),
        [rect(0, 2, 0, 2), rect(1, 2, 2, 3), rect(2, 4, 0, 1)]
    );
}
#[test]
fn empty_rows_and_declared_columns_remain_without_dense_storage() {
    let grid = place_table_cells(&[], 7, &[], limits()).unwrap();
    assert_eq!(
        (grid.rows(), grid.columns(), grid.retained_bytes()),
        (0, 7, 0)
    );
    let grid = place_table_cells(&[1, 3, 3], 5, &[cell(2, 7, 1)], limits()).unwrap();
    assert_eq!((grid.rows(), grid.columns()), (3, 5));
    assert_eq!(grid.cells(), [rect(2, 3, 0, 1)]);
    assert_eq!(grid.physical_columns(0, Direction::Rtl), Some(4..5));
}
#[test]
fn invalid_groups_rows_and_source_order_are_rejected() {
    for groups in [&[0][..], &[2][..], &[2, 1][..], &[2, 3, 3][..]] {
        assert!(matches!(
            place_table_cells(groups, 0, &[], limits()),
            Err(TablePlacementError::Group { .. })
        ));
    }
    assert!(matches!(
        place_table_cells(&[1], 0, &[cell(1, 1, 1)], limits()),
        Err(TablePlacementError::Row { cell: 0, row: 1 })
    ));
    assert!(matches!(
        place_table_cells(&[2, 2], 0, &[cell(1, 1, 1), cell(0, 1, 1)], limits()),
        Err(TablePlacementError::Order { cell: 1 })
    ));
}
#[test]
fn limits_and_exact_search_work_are_checked_without_mutating_input() {
    let input = [cell(0, 1, 1), cell(0, 2, 1), cell(1, 1, 2)];
    let before = input;
    let base = limits();
    for (cap, want) in [
        (
            TablePlacementLimits {
                rows: NonZeroUsize::MIN,
                ..base
            },
            TablePlacementError::Rows,
        ),
        (
            TablePlacementLimits {
                cells: NonZeroUsize::MIN,
                ..base
            },
            TablePlacementError::Cells,
        ),
        (
            TablePlacementLimits {
                columns: NonZeroUsize::new(3).unwrap(),
                ..base
            },
            TablePlacementError::Columns,
        ),
        (
            TablePlacementLimits {
                retained_bytes: NonZeroUsize::MIN,
                ..base
            },
            TablePlacementError::Bytes,
        ),
        (
            TablePlacementLimits {
                work: NonZeroUsize::new(14).unwrap(),
                ..base
            },
            TablePlacementError::Work,
        ),
    ] {
        assert_eq!(
            place_table_cells(&[2, 2], 0, &input, cap).unwrap_err(),
            want
        );
        assert_eq!(input, before);
    }
    let grid = place_table_cells(
        &[2, 2],
        0,
        &input,
        TablePlacementLimits {
            work: NonZeroUsize::new(15).unwrap(),
            ..base
        },
    )
    .unwrap();
    let bytes = grid.retained_bytes();
    assert!(
        place_table_cells(
            &[2, 2],
            0,
            &input,
            TablePlacementLimits {
                retained_bytes: NonZeroUsize::new(bytes).unwrap(),
                ..base
            }
        )
        .is_ok()
    );
    assert_eq!(
        place_table_cells(
            &[2, 2],
            0,
            &input,
            TablePlacementLimits {
                retained_bytes: NonZeroUsize::new(bytes - 1).unwrap(),
                ..base
            }
        )
        .unwrap_err(),
        TablePlacementError::Bytes
    );
    assert_eq!(
        place_table_cells(
            &[],
            4,
            &[],
            TablePlacementLimits {
                columns: NonZeroUsize::new(3).unwrap(),
                ..base
            }
        )
        .unwrap_err(),
        TablePlacementError::Columns
    );
}
#[test]
fn huge_columns_use_rectangles_and_checked_coordinate_ends() {
    let cap = TablePlacementLimits {
        columns: NonZeroUsize::new(u32::MAX as usize).unwrap(),
        ..limits()
    };
    let grid = place_table_cells(&[1], 0, &[cell(0, 1, u32::MAX)], cap).unwrap();
    assert_eq!(grid.columns(), u32::MAX);
    assert_eq!(grid.cells().len(), 1);
    assert_eq!(grid.physical_columns(0, Direction::Rtl), Some(0..u32::MAX));
    assert_eq!(
        place_table_cells(&[1], 0, &[cell(0, 1, u32::MAX), cell(0, 1, 1)], cap).unwrap_err(),
        TablePlacementError::Columns
    );
}

#[test]
fn small_span_combinations_match_an_independent_dense_occupancy_oracle() {
    // Each of four source cells has nine row-span/column-span combinations.
    // The oracle tries every column one at a time in a test-only boolean grid;
    // production instead searches prior rectangles and jumps to blocking ends.
    for encoded in 0..9u32.pow(4) {
        let mut choices = encoded;
        let mut input = Vec::new();
        for row in [0, 0, 1, 2] {
            let choice = choices % 9;
            choices /= 9;
            input.push(cell(row, choice / 3 + 1, choice % 3 + 1));
        }
        let mut occupied = [[false; 32]; 3];
        let mut expected = Vec::new();
        let mut previous_row = 0;
        let mut cursor = 0usize;
        let mut columns = 5;
        for cell in &input {
            let row = cell.row as usize;
            if row != previous_row {
                cursor = 0;
                previous_row = row;
            }
            let row_end = (row + cell.row_span.get() as usize).min(3);
            let span = cell.column_span.get() as usize;
            while occupied[row..row_end]
                .iter()
                .any(|r| r[cursor..cursor + span].iter().any(|v| *v))
            {
                cursor += 1;
            }
            for row in &mut occupied[row..row_end] {
                row[cursor..cursor + span].fill(true);
            }
            expected.push(rect(
                cell.row,
                row_end as u32,
                cursor as u32,
                (cursor + span) as u32,
            ));
            cursor += span;
            columns = columns.max(cursor as u32);
        }
        let grid = place_table_cells(&[3, 3, 3], 5, &input, limits()).unwrap();
        assert_eq!(grid.cells(), expected, "span combination {encoded}");
        assert_eq!(grid.columns(), columns, "span combination {encoded}");
    }
}
