use iced_css_core::formatting::{
    block::Direction,
    table_border_intersection::{
        TableIntersectionError, TableIntersectionLimits, intersect_table_borders,
    },
    table_borders::{
        BorderCandidate, BorderEdge, BorderOrder, BorderOrigin, BorderSegmentLimits, BorderStyle,
        CollapsedBorder, CollapsedBorderSegment, split_collapsed_borders,
    },
    table_cell_borders::BorderAxis,
    table_line_boundaries::{TableBoundaryLimits, table_line_boundaries},
    table_placement::{CellSpan, TablePlacement, TablePlacementLimits, place_table_cells},
};
use std::num::{NonZeroU32, NonZeroUsize};
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn limits() -> TableIntersectionLimits {
    TableIntersectionLimits {
        input_segments: n(100),
        segments: n(100),
        retained_bytes: n(10000),
        work: n(10000),
    }
}
fn grid() -> TablePlacement {
    let cells: Vec<_> = (0..4)
        .flat_map(|row| {
            (0..if row % 2 == 0 { 2 } else { 1 }).map(move |_| CellSpan {
                row,
                row_span: NonZeroU32::new(1).unwrap(),
                column_span: NonZeroU32::new(if row % 2 == 0 { 1 } else { 2 }).unwrap(),
            })
        })
        .collect();
    place_table_cells(
        &[4; 4],
        0,
        &cells,
        TablePlacementLimits {
            rows: n(100),
            columns: n(100),
            cells: n(100),
            retained_bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap()
}
fn edge(start: u32, end: u32, style: BorderStyle, width: f32) -> BorderEdge {
    BorderEdge::new(
        start,
        end,
        BorderCandidate::new(
            style,
            width,
            BorderOrigin::Column,
            BorderOrder {
                inline_start: 0,
                block_start: 0,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn splitter_winners_survive_interior_exclusion_in_both_directions() {
    let grid = grid();
    let resolved = split_collapsed_borders(
        &[
            edge(2, 3, BorderStyle::Solid, 4.),
            edge(0, 4, BorderStyle::Solid, 2.),
        ],
        BorderSegmentLimits {
            edges: n(100),
            segments: n(100),
            retained_bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap();
    for direction in [Direction::Ltr, Direction::Rtl] {
        let boundaries = table_line_boundaries(
            &grid,
            BorderAxis::Vertical,
            1,
            direction,
            TableBoundaryLimits {
                segments: n(100),
                retained_bytes: n(10000),
                work: n(10000),
            },
        )
        .unwrap();
        let result = intersect_table_borders(&boundaries, &resolved, limits()).unwrap();
        assert_eq!(
            result.segments(),
            [
                CollapsedBorderSegment {
                    start: 0,
                    end: 1,
                    border: CollapsedBorder::Winner {
                        index: 1,
                        paint_style: BorderStyle::Solid
                    }
                },
                CollapsedBorderSegment {
                    start: 2,
                    end: 3,
                    border: CollapsedBorder::Winner {
                        index: 0,
                        paint_style: BorderStyle::Solid
                    }
                },
            ]
        );
        assert!(std::ptr::eq(result.boundaries(), &boundaries));
    }
}
#[test]
fn invalid_ranges_are_rejected_before_publication() {
    let grid = grid();
    let boundaries = table_line_boundaries(
        &grid,
        BorderAxis::Vertical,
        1,
        Direction::Ltr,
        TableBoundaryLimits {
            segments: n(100),
            retained_bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap();
    let segment = |start, end| CollapsedBorderSegment {
        start,
        end,
        border: CollapsedBorder::Absent,
    };
    for input in [
        vec![segment(1, 1)],
        vec![segment(3, 2)],
        vec![segment(0, 5)],
        vec![segment(2, 3), segment(0, 1)],
        vec![segment(0, 3), segment(2, 4)],
    ] {
        assert_eq!(
            intersect_table_borders(&boundaries, &input, limits()).unwrap_err(),
            TableIntersectionError::Range
        );
    }
}

#[test]
fn limits_fail_atomically_and_absent_and_hidden_values_survive() {
    let grid = grid();
    let boundaries = table_line_boundaries(
        &grid,
        BorderAxis::Vertical,
        1,
        Direction::Ltr,
        TableBoundaryLimits {
            segments: n(100),
            retained_bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap();
    let input = [
        CollapsedBorderSegment {
            start: 0,
            end: 2,
            border: CollapsedBorder::Absent,
        },
        CollapsedBorderSegment {
            start: 2,
            end: 4,
            border: CollapsedBorder::Suppressed,
        },
    ];
    let result = intersect_table_borders(&boundaries, &input, limits()).unwrap();
    assert_eq!(
        result.segments(),
        [
            CollapsedBorderSegment {
                start: 0,
                end: 1,
                border: CollapsedBorder::Absent
            },
            CollapsedBorderSegment {
                start: 2,
                end: 3,
                border: CollapsedBorder::Suppressed
            }
        ]
    );
    let bytes = result.retained_bytes();
    let mut exact = limits();
    exact.work = n(8);
    exact.retained_bytes = n(bytes);
    assert_eq!(
        intersect_table_borders(&boundaries, &input, exact)
            .unwrap()
            .segments(),
        result.segments()
    );
    for (limit, error) in [
        (
            TableIntersectionLimits {
                work: n(7),
                ..limits()
            },
            TableIntersectionError::Work,
        ),
        (
            TableIntersectionLimits {
                input_segments: n(1),
                ..limits()
            },
            TableIntersectionError::InputSegments,
        ),
        (
            TableIntersectionLimits {
                segments: n(1),
                ..limits()
            },
            TableIntersectionError::Segments,
        ),
        (
            TableIntersectionLimits {
                retained_bytes: n(bytes - 1),
                ..limits()
            },
            TableIntersectionError::Bytes,
        ),
    ] {
        assert_eq!(
            intersect_table_borders(&boundaries, &input, limit).unwrap_err(),
            error
        );
    }
    assert!(
        intersect_table_borders(
            &boundaries,
            &[],
            TableIntersectionLimits {
                work: n(1),
                ..limits()
            }
        )
        .unwrap()
        .segments()
        .is_empty()
    );
    assert_eq!(input[1].border, CollapsedBorder::Suppressed);
}
#[test]
fn all_small_input_ranges_match_per_track_membership() {
    let grid = grid();
    for direction in [Direction::Ltr, Direction::Rtl] {
        for line in 0..=2 {
            let boundaries = table_line_boundaries(
                &grid,
                BorderAxis::Vertical,
                line,
                direction,
                TableBoundaryLimits {
                    segments: n(100),
                    retained_bytes: n(10000),
                    work: n(10000),
                },
            )
            .unwrap();
            for start in 0..4 {
                for end in start + 1..=4 {
                    for border in [
                        CollapsedBorder::Absent,
                        CollapsedBorder::Suppressed,
                        CollapsedBorder::Winner {
                            index: 17,
                            paint_style: BorderStyle::Double,
                        },
                    ] {
                        let result = intersect_table_borders(
                            &boundaries,
                            &[CollapsedBorderSegment { start, end, border }],
                            limits(),
                        )
                        .unwrap();
                        for row in 0..4 {
                            let expected = (start <= row
                                && row < end
                                && (line == 0 || line == 2 || row % 2 == 0))
                                .then_some(border);
                            let actual = result
                                .segments()
                                .iter()
                                .find(|s| s.start <= row && row < s.end)
                                .map(|s| s.border);
                            assert_eq!(actual, expected);
                        }
                        assert!(result.segments().windows(2).all(|s| s[0].end <= s[1].start));
                    }
                }
            }
        }
    }
}
