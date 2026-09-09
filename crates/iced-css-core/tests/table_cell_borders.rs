use iced_css_core::formatting::{
    block::Direction,
    table_borders::{
        BorderConflictError, BorderSegmentLimits, BorderStyle, CollapsedBorder,
        split_collapsed_borders,
    },
    table_cell_borders::{
        BorderAxis, CellBorderError, CellBorderInput, CellBorderSide, CellBorders,
        placed_cell_borders,
    },
    table_placement::{CellSpan, TablePlacementLimits, place_table_cells},
};
use std::num::{NonZeroU32, NonZeroUsize};
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100_000).unwrap()
}
fn limits() -> TablePlacementLimits {
    TablePlacementLimits {
        rows: n(),
        columns: n(),
        cells: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn segments() -> BorderSegmentLimits {
    BorderSegmentLimits {
        edges: n(),
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
fn border(style: BorderStyle, width: f32) -> CellBorderInput {
    CellBorderInput { style, width }
}
fn all(style: BorderStyle, width: f32) -> CellBorders {
    let b = border(style, width);
    CellBorders {
        top: b,
        right: b,
        bottom: b,
        left: b,
    }
}
#[test]
fn spanning_rtl_cell_has_only_its_four_physical_perimeter_edges() {
    let grid = place_table_cells(&[2, 2], 0, &[cell(0, 2, 2), cell(0, 1, 1)], limits()).unwrap();
    let edges = placed_cell_borders(&grid, 0, all(BorderStyle::Solid, 2.), Direction::Rtl).unwrap();
    for (edge, (side, axis, line, start, end)) in edges.iter().zip([
        (CellBorderSide::Top, BorderAxis::Horizontal, 0, 1, 3),
        (CellBorderSide::Right, BorderAxis::Vertical, 3, 0, 2),
        (CellBorderSide::Bottom, BorderAxis::Horizontal, 2, 1, 3),
        (CellBorderSide::Left, BorderAxis::Vertical, 1, 0, 2),
    ]) {
        assert_eq!(
            (edge.cell, edge.side, edge.axis, edge.line),
            (0, side, axis, line)
        );
        let split = split_collapsed_borders(&[edge.edge], segments()).unwrap();
        assert_eq!(split.len(), 1);
        assert_eq!((split[0].start, split[0].end), (start, end));
        assert_eq!(
            split[0].border,
            CollapsedBorder::Winner {
                index: 0,
                paint_style: BorderStyle::Solid
            }
        );
    }
}
#[test]
fn shared_edge_ties_follow_table_direction_instead_of_contender_input_order() {
    let grid = place_table_cells(&[1], 0, &[cell(0, 1, 1), cell(0, 1, 1)], limits()).unwrap();
    for direction in [Direction::Ltr, Direction::Rtl] {
        let a = placed_cell_borders(&grid, 0, all(BorderStyle::Solid, 2.), direction).unwrap();
        let b = placed_cell_borders(&grid, 1, all(BorderStyle::Solid, 2.), direction).unwrap();
        let (a, b) = match direction {
            Direction::Ltr => (a[1], b[3]),
            Direction::Rtl => (a[3], b[1]),
        };
        assert_eq!((a.axis, a.line), (BorderAxis::Vertical, 1));
        assert_eq!((b.axis, b.line), (a.axis, a.line));
        let split = split_collapsed_borders(&[b.edge, a.edge], segments()).unwrap();
        assert_eq!(
            split[0].border,
            CollapsedBorder::Winner {
                index: 1,
                paint_style: BorderStyle::Solid
            }
        );
        assert_eq!(a.cell, 0);
    }
}
#[test]
fn rowspan_side_splits_against_neighbor_sides_and_hidden_suppresses_only_its_segment() {
    let grid = place_table_cells(
        &[2, 2],
        0,
        &[cell(0, 2, 1), cell(0, 1, 1), cell(1, 1, 1)],
        limits(),
    )
    .unwrap();
    let a = placed_cell_borders(
        &grid,
        0,
        CellBorders {
            right: border(BorderStyle::Solid, 2.),
            ..Default::default()
        },
        Direction::Ltr,
    )
    .unwrap()[1];
    let b = placed_cell_borders(
        &grid,
        1,
        CellBorders {
            left: border(BorderStyle::Solid, 3.),
            ..Default::default()
        },
        Direction::Ltr,
    )
    .unwrap()[3];
    let c = placed_cell_borders(
        &grid,
        2,
        CellBorders {
            left: border(BorderStyle::Hidden, 0.),
            ..Default::default()
        },
        Direction::Ltr,
    )
    .unwrap()[3];
    assert!(
        [a, b, c]
            .iter()
            .all(|e| e.axis == BorderAxis::Vertical && e.line == 1)
    );
    let split = split_collapsed_borders(&[a.edge, b.edge, c.edge], segments()).unwrap();
    assert_eq!(split.len(), 2);
    assert_eq!(
        (split[0].start, split[0].end, split[0].border),
        (
            0,
            1,
            CollapsedBorder::Winner {
                index: 1,
                paint_style: BorderStyle::Solid
            }
        )
    );
    assert_eq!(
        (split[1].start, split[1].end, split[1].border),
        (1, 2, CollapsedBorder::Suppressed)
    );
    assert_eq!((b.cell, b.side), (1, CellBorderSide::Left));
}
#[test]
fn every_side_is_validated_and_invalid_input_does_not_change_placement() {
    let grid = place_table_cells(&[1], 0, &[cell(0, 1, 1)], limits()).unwrap();
    let before = grid.cells().to_vec();
    for value in [f32::NAN, f32::INFINITY, -1.] {
        for side in 0..4 {
            let mut borders = CellBorders::default();
            let selected = match side {
                0 => &mut borders.top,
                1 => &mut borders.right,
                2 => &mut borders.bottom,
                _ => &mut borders.left,
            };
            *selected = border(BorderStyle::Solid, value);
            assert_eq!(
                placed_cell_borders(&grid, 0, borders, Direction::Ltr).unwrap_err(),
                CellBorderError::Candidate(BorderConflictError::Width)
            );
            assert_eq!(grid.cells(), before);
        }
    }
    assert_eq!(
        placed_cell_borders(&grid, usize::MAX, CellBorders::default(), Direction::Ltr).unwrap_err(),
        CellBorderError::Cell { cell: usize::MAX }
    );
}
