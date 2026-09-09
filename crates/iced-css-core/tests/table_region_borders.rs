use iced_css_core::formatting::{
    block::Direction,
    table_borders::{BorderSegmentLimits, BorderStyle, CollapsedBorder, split_collapsed_borders},
    table_cell_borders::{
        BorderAxis, CellBorderInput, CellBorderSide, CellBorders, placed_cell_borders,
    },
    table_placement::{CellSpan, TablePlacement, TablePlacementLimits, place_table_cells},
    table_region_borders::{
        TableBorderRegion as Region, TableRegionBorderError, placed_region_borders,
    },
};
use std::num::{NonZeroU32, NonZeroUsize};
fn n() -> NonZeroUsize {
    NonZeroUsize::new(10000).unwrap()
}
fn grid() -> TablePlacement {
    let cells: Vec<_> = (0..3)
        .flat_map(|row| {
            (0..4).map(move |_| CellSpan {
                row,
                row_span: NonZeroU32::new(1).unwrap(),
                column_span: NonZeroU32::new(1).unwrap(),
            })
        })
        .collect();
    place_table_cells(
        &[3; 3],
        0,
        &cells,
        TablePlacementLimits {
            rows: n(),
            columns: n(),
            cells: n(),
            retained_bytes: n(),
            work: n(),
        },
    )
    .unwrap()
}
fn borders() -> CellBorders {
    let b = CellBorderInput {
        style: BorderStyle::Solid,
        width: 2.,
    };
    CellBorders {
        top: b,
        right: b,
        bottom: b,
        left: b,
    }
}
fn caps() -> BorderSegmentLimits {
    BorderSegmentLimits {
        edges: n(),
        segments: n(),
        retained_bytes: n(),
        work: n(),
    }
}
#[test]
fn every_region_projects_its_physical_perimeter_in_both_directions() {
    let grid = grid();
    for (region, top, bottom, start, end) in [
        (Region::Table, 0, 3, 0, 4),
        (Region::Row { row: 1 }, 1, 2, 0, 4),
        (Region::RowGroup { start: 1, end: 3 }, 1, 3, 0, 4),
        (Region::Column { start: 0, end: 2 }, 0, 3, 0, 2),
        (Region::ColumnGroup { start: 1, end: 4 }, 0, 3, 1, 4),
    ] {
        for direction in [Direction::Ltr, Direction::Rtl] {
            let (left, right) = if direction == Direction::Ltr {
                (start, end)
            } else {
                (4 - end, 4 - start)
            };
            let output = placed_region_borders(&grid, region, borders(), direction).unwrap();
            for (edge, (side, axis, line, start, end)) in output.iter().zip([
                (
                    CellBorderSide::Top,
                    BorderAxis::Horizontal,
                    top,
                    left,
                    right,
                ),
                (
                    CellBorderSide::Right,
                    BorderAxis::Vertical,
                    right,
                    top,
                    bottom,
                ),
                (
                    CellBorderSide::Bottom,
                    BorderAxis::Horizontal,
                    bottom,
                    left,
                    right,
                ),
                (
                    CellBorderSide::Left,
                    BorderAxis::Vertical,
                    left,
                    top,
                    bottom,
                ),
            ]) {
                assert_eq!(edge.region, region);
                assert_eq!((edge.side, edge.axis, edge.line), (side, axis, line));
                let split = split_collapsed_borders(&[edge.edge], caps()).unwrap();
                assert_eq!((split[0].start, split[0].end), (start, end));
            }
        }
    }
}
#[test]
fn actual_splitter_uses_all_six_origin_priorities() {
    let grid = grid();
    let regions = [
        Region::Table,
        Region::ColumnGroup { start: 0, end: 4 },
        Region::Column { start: 0, end: 4 },
        Region::RowGroup { start: 0, end: 3 },
        Region::Row { row: 0 },
    ];
    let mut edges: Vec<_> = regions
        .iter()
        .map(|r| placed_region_borders(&grid, *r, borders(), Direction::Ltr).unwrap()[0].edge)
        .collect();
    edges.push(placed_cell_borders(&grid, 0, borders(), Direction::Ltr).unwrap()[0].edge);
    for count in 1..=edges.len() {
        let split = split_collapsed_borders(&edges[..count], caps()).unwrap();
        assert_eq!(
            split[0].border,
            CollapsedBorder::Winner {
                index: count - 1,
                paint_style: BorderStyle::Solid
            }
        );
    }
}
#[test]
fn invalid_or_degenerate_regions_are_errors_not_silent_empty_output() {
    let grid = grid();
    let before = grid.cells().to_vec();
    for region in [
        Region::Row { row: 3 },
        Region::Row { row: u32::MAX },
        Region::RowGroup { start: 1, end: 1 },
        Region::RowGroup { start: 0, end: 4 },
        Region::Column { start: 2, end: 1 },
        Region::Column { start: 0, end: 5 },
        Region::ColumnGroup { start: 0, end: 0 },
    ] {
        assert_eq!(
            placed_region_borders(&grid, region, borders(), Direction::Rtl).unwrap_err(),
            TableRegionBorderError::Region
        );
    }
    assert_eq!(grid.cells(), before);
}

#[test]
fn adjacent_regions_tie_by_direction_start_and_top_not_input_order() {
    let grid = grid();
    for direction in [Direction::Ltr, Direction::Rtl] {
        for column_group in [false, true] {
            let region = |start, end| {
                if column_group {
                    Region::ColumnGroup { start, end }
                } else {
                    Region::Column { start, end }
                }
            };
            let a = placed_region_borders(&grid, region(0, 2), borders(), direction).unwrap();
            let b = placed_region_borders(&grid, region(2, 4), borders(), direction).unwrap();
            let (a, b) = if direction == Direction::Ltr {
                (a[1], b[3])
            } else {
                (a[3], b[1])
            };
            assert_eq!((a.axis, a.line), (b.axis, b.line));
            let split = split_collapsed_borders(&[b.edge, a.edge], caps()).unwrap();
            assert_eq!(
                split[0].border,
                CollapsedBorder::Winner {
                    index: 1,
                    paint_style: BorderStyle::Solid
                }
            );
        }
        for row_group in [false, true] {
            let region = |row| {
                if row_group {
                    Region::RowGroup {
                        start: row,
                        end: row + 1,
                    }
                } else {
                    Region::Row { row }
                }
            };
            let a = placed_region_borders(&grid, region(0), borders(), direction).unwrap()[2];
            let b = placed_region_borders(&grid, region(1), borders(), direction).unwrap()[0];
            assert_eq!((a.axis, a.line), (b.axis, b.line));
            let split = split_collapsed_borders(&[b.edge, a.edge], caps()).unwrap();
            assert_eq!(
                split[0].border,
                CollapsedBorder::Winner {
                    index: 1,
                    paint_style: BorderStyle::Solid
                }
            );
        }
    }
}
#[test]
fn invalid_width_on_any_side_and_empty_grid_return_errors() {
    let grid = grid();
    for value in [f32::NAN, f32::INFINITY, -1.] {
        for side in 0..4 {
            let mut borders = borders();
            let selected = match side {
                0 => &mut borders.top,
                1 => &mut borders.right,
                2 => &mut borders.bottom,
                _ => &mut borders.left,
            };
            selected.width = value;
            assert!(matches!(
                placed_region_borders(&grid, Region::Table, borders, Direction::Ltr),
                Err(TableRegionBorderError::Candidate(_))
            ));
        }
    }
    for rows in [vec![], vec![1]] {
        let grid = place_table_cells(
            &rows,
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
        assert_eq!(
            placed_region_borders(&grid, Region::Table, borders(), Direction::Ltr).unwrap_err(),
            TableRegionBorderError::Region
        );
    }
}
#[test]
fn column_contender_is_clipped_where_it_crosses_a_spanning_cell() {
    use iced_css_core::formatting::{
        table_border_intersection::{TableIntersectionLimits, intersect_table_borders},
        table_line_boundaries::{TableBoundaryLimits, table_line_boundaries},
    };
    let cell = |row, columns| CellSpan {
        row,
        row_span: NonZeroU32::new(1).unwrap(),
        column_span: NonZeroU32::new(columns).unwrap(),
    };
    let grid = place_table_cells(
        &[2; 2],
        0,
        &[cell(0, 1), cell(0, 1), cell(1, 2)],
        TablePlacementLimits {
            rows: n(),
            columns: n(),
            cells: n(),
            retained_bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    for direction in [Direction::Ltr, Direction::Rtl] {
        let edges = placed_region_borders(
            &grid,
            Region::Column { start: 0, end: 1 },
            borders(),
            direction,
        )
        .unwrap();
        let edge = if direction == Direction::Ltr {
            edges[1]
        } else {
            edges[3]
        };
        let resolved = split_collapsed_borders(&[edge.edge], caps()).unwrap();
        assert_eq!((resolved[0].start, resolved[0].end), (0, 2));
        let boundaries = table_line_boundaries(
            &grid,
            edge.axis,
            edge.line,
            direction,
            TableBoundaryLimits {
                segments: n(),
                retained_bytes: n(),
                work: n(),
            },
        )
        .unwrap();
        let clipped = intersect_table_borders(
            &boundaries,
            &resolved,
            TableIntersectionLimits {
                input_segments: n(),
                segments: n(),
                retained_bytes: n(),
                work: n(),
            },
        )
        .unwrap();
        assert_eq!(clipped.segments().len(), 1);
        assert_eq!(
            (clipped.segments()[0].start, clipped.segments()[0].end),
            (0, 1)
        );
        assert_eq!(clipped.segments()[0].border, resolved[0].border);
    }
}

#[test]
fn rtl_mirrors_geometry_without_swapping_physical_side_styles() {
    let grid = grid();
    let style = |style| CellBorderInput { style, width: 2. };
    let borders = CellBorders {
        top: style(BorderStyle::Dotted),
        right: style(BorderStyle::Double),
        bottom: style(BorderStyle::Dashed),
        left: style(BorderStyle::Solid),
    };
    for direction in [Direction::Ltr, Direction::Rtl] {
        let output = placed_region_borders(
            &grid,
            Region::Column { start: 0, end: 1 },
            borders,
            direction,
        )
        .unwrap();
        for (edge, expected) in output.iter().zip([
            BorderStyle::Dotted,
            BorderStyle::Double,
            BorderStyle::Dashed,
            BorderStyle::Solid,
        ]) {
            let split = split_collapsed_borders(&[edge.edge], caps()).unwrap();
            assert_eq!(
                split[0].border,
                CollapsedBorder::Winner {
                    index: 0,
                    paint_style: expected
                }
            );
        }
    }
}
