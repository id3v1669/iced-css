use iced_css_core::formatting::{
    block::Direction,
    table_cell_borders::BorderAxis,
    table_line_boundaries::{
        GridInterval, TableBoundaryError, TableBoundaryLimits, table_line_boundaries,
    },
    table_placement::{CellSpan, TablePlacement, TablePlacementLimits, place_table_cells},
};
use std::num::{NonZeroU32, NonZeroUsize};
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100_000).unwrap()
}
fn caps() -> TableBoundaryLimits {
    TableBoundaryLimits {
        segments: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn cell(row: u32, rows: u32, columns: u32) -> CellSpan {
    CellSpan {
        row,
        row_span: NonZeroU32::new(rows).unwrap(),
        column_span: NonZeroU32::new(columns).unwrap(),
    }
}
fn grid(groups: &[u32], columns: u32, cells: &[CellSpan]) -> TablePlacement {
    place_table_cells(
        groups,
        columns,
        cells,
        TablePlacementLimits {
            rows: n(),
            columns: NonZeroUsize::new(u32::MAX as usize).unwrap(),
            cells: n(),
            retained_bytes: n(),
            work: n(),
        },
    )
    .unwrap()
}
fn span(start: u32, end: u32) -> GridInterval {
    GridInterval { start, end }
}
fn alternating() -> TablePlacement {
    let mut cells = vec![];
    for row in 0..4 {
        if row % 2 == 0 {
            for _ in 0..4 {
                cells.push(cell(row, 1, 1));
            }
        } else {
            cells.push(cell(row, 1, 4));
        }
    }
    grid(&[4, 4, 4, 4], 0, &cells)
}
#[test]
fn column_crossings_are_excluded_only_in_spanning_rows() {
    let grid = alternating();
    for line in 1..4 {
        let boundaries =
            table_line_boundaries(&grid, BorderAxis::Vertical, line, Direction::Ltr, caps())
                .unwrap();
        assert_eq!(boundaries.intervals(), [span(0, 1), span(2, 3)]);
        assert_eq!(
            (
                boundaries.axis(),
                boundaries.line(),
                boundaries.extent(),
                boundaries.direction()
            ),
            (BorderAxis::Vertical, line, 4, Direction::Ltr)
        );
    }
    for line in [0, 4] {
        assert_eq!(
            table_line_boundaries(&grid, BorderAxis::Vertical, line, Direction::Ltr, caps())
                .unwrap()
                .intervals(),
            [span(0, 4)]
        );
    }
}
#[test]
fn row_crossings_use_physical_columns_and_reverse_for_rtl() {
    let grid = grid(
        &[3, 3, 3],
        0,
        &[cell(0, 3, 1), cell(0, 1, 1), cell(1, 1, 1), cell(2, 1, 1)],
    );
    for line in [1, 2] {
        assert_eq!(
            table_line_boundaries(&grid, BorderAxis::Horizontal, line, Direction::Ltr, caps())
                .unwrap()
                .intervals(),
            [span(1, 2)]
        );
        assert_eq!(
            table_line_boundaries(&grid, BorderAxis::Horizontal, line, Direction::Rtl, caps())
                .unwrap()
                .intervals(),
            [span(0, 1)]
        );
    }
    for line in [0, 3] {
        assert_eq!(
            table_line_boundaries(&grid, BorderAxis::Horizontal, line, Direction::Ltr, caps())
                .unwrap()
                .intervals(),
            [span(0, 2)]
        );
    }
}
#[test]
fn fully_excluded_and_empty_lines_have_no_fictitious_segment() {
    let grid = grid(&[2, 2], 0, &[cell(0, 2, 2)]);
    for axis in [BorderAxis::Horizontal, BorderAxis::Vertical] {
        let boundaries = table_line_boundaries(&grid, axis, 1, Direction::Ltr, caps()).unwrap();
        assert!(boundaries.intervals().is_empty());
        assert_eq!(boundaries.retained_bytes(), 0);
    }
    let empty = place_table_cells(
        &[],
        0,
        &[],
        TablePlacementLimits {
            rows: n(),
            columns: n(),
            cells: n(),
            retained_bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    assert!(
        table_line_boundaries(&empty, BorderAxis::Horizontal, 0, Direction::Ltr, caps())
            .unwrap()
            .intervals()
            .is_empty()
    );
}
#[test]
fn huge_sparse_extent_uses_one_interval() {
    let grid = grid(&[1], u32::MAX, &[cell(0, 1, 1)]);
    let boundaries =
        table_line_boundaries(&grid, BorderAxis::Horizontal, 0, Direction::Ltr, caps()).unwrap();
    assert_eq!(boundaries.intervals(), [span(0, u32::MAX)]);
}
#[test]
fn explicit_limits_and_line_admission_preserve_the_owner() {
    let grid = alternating();
    let before = grid.cells().to_vec();
    let base = caps();
    for (cap, want) in [
        (
            TableBoundaryLimits {
                segments: NonZeroUsize::MIN,
                ..base
            },
            TableBoundaryError::Segments,
        ),
        (
            TableBoundaryLimits {
                retained_bytes: NonZeroUsize::MIN,
                ..base
            },
            TableBoundaryError::Bytes,
        ),
        (
            TableBoundaryLimits {
                work: NonZeroUsize::new(24).unwrap(),
                ..base
            },
            TableBoundaryError::Work,
        ),
    ] {
        assert_eq!(
            table_line_boundaries(&grid, BorderAxis::Vertical, 1, Direction::Ltr, cap).unwrap_err(),
            want
        );
        assert_eq!(grid.cells(), before);
    }
    let boundaries = table_line_boundaries(
        &grid,
        BorderAxis::Vertical,
        1,
        Direction::Ltr,
        TableBoundaryLimits {
            work: NonZeroUsize::new(25).unwrap(),
            ..base
        },
    )
    .unwrap();
    let bytes = boundaries.retained_bytes();
    assert!(
        table_line_boundaries(
            &grid,
            BorderAxis::Vertical,
            1,
            Direction::Ltr,
            TableBoundaryLimits {
                retained_bytes: NonZeroUsize::new(bytes).unwrap(),
                ..base
            }
        )
        .is_ok()
    );
    assert_eq!(
        table_line_boundaries(
            &grid,
            BorderAxis::Vertical,
            1,
            Direction::Ltr,
            TableBoundaryLimits {
                retained_bytes: NonZeroUsize::new(bytes - 1).unwrap(),
                ..base
            }
        )
        .unwrap_err(),
        TableBoundaryError::Bytes
    );
    assert_eq!(
        table_line_boundaries(&grid, BorderAxis::Vertical, 5, Direction::Ltr, base).unwrap_err(),
        TableBoundaryError::Line {
            line: 5,
            maximum: 4
        }
    );
}

#[test]
fn small_topologies_match_dense_neighbor_ownership_on_every_grid_line() {
    for encoded in 0..4u32.pow(4) {
        let mut choices = encoded;
        let mut input = vec![];
        for row in [0, 0, 1, 2] {
            let choice = choices % 4;
            choices /= 4;
            input.push(cell(row, choice / 2 + 1, choice % 2 + 1));
        }
        let grid = grid(&[3, 3, 3], 8, &input);
        assert_eq!(grid.columns(), 8);
        for direction in [Direction::Ltr, Direction::Rtl] {
            let mut owner = [[None; 8]; 3];
            for (index, cell) in grid.cells().iter().enumerate() {
                let columns = grid.physical_columns(index, direction).unwrap();
                for row in owner
                    .iter_mut()
                    .take(cell.row_end as usize)
                    .skip(cell.row_start as usize)
                {
                    for slot in row
                        .iter_mut()
                        .take(columns.end as usize)
                        .skip(columns.start as usize)
                    {
                        assert!(slot.is_none());
                        *slot = Some(index);
                    }
                }
            }
            for axis in [BorderAxis::Horizontal, BorderAxis::Vertical] {
                let (maximum, extent) = match axis {
                    BorderAxis::Horizontal => (3, 8),
                    BorderAxis::Vertical => (8, 3),
                };
                for line in 0..=maximum {
                    // A unit edge is interior only when the same present cell
                    // owns both neighboring slots. Empty slots create no owner.
                    let allowed = (0..extent).map(|at| {
                        if line == 0 || line == maximum {
                            return true;
                        }
                        let (a, b) = match axis {
                            BorderAxis::Horizontal => (owner[line - 1][at], owner[line][at]),
                            BorderAxis::Vertical => (owner[at][line - 1], owner[at][line]),
                        };
                        a.is_none() || a != b
                    });
                    let mut expected = vec![];
                    let mut start = None;
                    for (at, allowed) in allowed.chain(std::iter::once(false)).enumerate() {
                        if allowed {
                            start.get_or_insert(at);
                        } else if let Some(begin) = start.take() {
                            expected.push(span(begin as u32, at as u32));
                        }
                    }
                    let actual =
                        table_line_boundaries(&grid, axis, line as u32, direction, caps()).unwrap();
                    assert_eq!(
                        actual.intervals(),
                        expected,
                        "combination {encoded}, {direction:?}, {axis:?}, line {line}"
                    );
                }
            }
        }
    }
}
