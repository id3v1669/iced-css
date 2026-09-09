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

#[test]
fn disjoint_input_segments_match_explicit_per_track_oracle() {
    let grid = grid();
    let values = [
        CollapsedBorder::Absent,
        CollapsedBorder::Suppressed,
        CollapsedBorder::Winner {
            index: 17,
            paint_style: BorderStyle::Double,
        },
    ];
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
            // The fixture has separate cells in rows 0 and 2 and a single
            // spanning cell in rows 1 and 3. Exterior lines remain eligible.
            let eligible = [true, line != 1, true, line != 1];
            for first_start in 0..4 {
                for first_end in first_start + 1..=4 {
                    for second_start in first_end..4 {
                        for second_end in second_start + 1..=4 {
                            for first_border in values {
                                for second_border in values {
                                    let input = [
                                        CollapsedBorderSegment {
                                            start: first_start,
                                            end: first_end,
                                            border: first_border,
                                        },
                                        CollapsedBorderSegment {
                                            start: second_start,
                                            end: second_end,
                                            border: second_border,
                                        },
                                    ];
                                    let result =
                                        intersect_table_borders(&boundaries, &input, limits())
                                            .unwrap();
                                    for (row, allowed) in eligible.into_iter().enumerate() {
                                        let row = row as u32;
                                        let expected = if !allowed {
                                            None
                                        } else if first_start <= row && row < first_end {
                                            Some(first_border)
                                        } else if second_start <= row && row < second_end {
                                            Some(second_border)
                                        } else {
                                            None
                                        };
                                        let actual = result
                                            .segments()
                                            .iter()
                                            .find(|segment| {
                                                segment.start <= row && row < segment.end
                                            })
                                            .map(|segment| segment.border);
                                        assert_eq!(
                                            actual, expected,
                                            "{direction:?}, line {line}, row {row}, input {input:?}"
                                        );
                                    }
                                    assert!(result.segments().iter().all(|segment| {
                                        segment.start < segment.end && segment.end <= 4
                                    }));
                                    assert!(
                                        result
                                            .segments()
                                            .windows(2)
                                            .all(|pair| pair[0].end <= pair[1].start)
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn fully_excluded_boundary_validates_input_and_accepts_maximum_endpoint() {
    let grid = place_table_cells(
        &[2, 2],
        0,
        &[CellSpan {
            row: 0,
            row_span: NonZeroU32::new(2).unwrap(),
            column_span: NonZeroU32::new(u32::MAX).unwrap(),
        }],
        TablePlacementLimits {
            rows: n(2),
            columns: n(u32::MAX as usize),
            cells: n(1),
            retained_bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap();
    for direction in [Direction::Ltr, Direction::Rtl] {
        let boundaries = table_line_boundaries(
            &grid,
            BorderAxis::Horizontal,
            1,
            direction,
            TableBoundaryLimits {
                segments: n(1),
                retained_bytes: n(1),
                work: n(100),
            },
        )
        .unwrap();
        assert!(boundaries.intervals().is_empty());
        assert_eq!(boundaries.extent(), u32::MAX);
        let segment = |start, end| CollapsedBorderSegment {
            start,
            end,
            border: CollapsedBorder::Winner {
                index: 17,
                paint_style: BorderStyle::Solid,
            },
        };
        for input in [
            vec![segment(u32::MAX, u32::MAX)],
            vec![segment(u32::MAX, u32::MAX - 1)],
            vec![segment(0, u32::MAX), segment(1, 2)],
            vec![segment(2, 3), segment(0, 1)],
        ] {
            assert_eq!(
                intersect_table_borders(&boundaries, &input, limits()).unwrap_err(),
                TableIntersectionError::Range
            );
        }
        let valid = [segment(0, u32::MAX)];
        let exact = TableIntersectionLimits {
            input_segments: n(1),
            segments: n(1),
            retained_bytes: n(1),
            work: n(2),
        };
        let result = intersect_table_borders(&boundaries, &valid, exact).unwrap();
        assert!(result.segments().is_empty());
        assert_eq!(result.retained_bytes(), 0);
        assert_eq!(
            intersect_table_borders(
                &boundaries,
                &valid,
                TableIntersectionLimits {
                    work: n(1),
                    ..exact
                }
            )
            .unwrap_err(),
            TableIntersectionError::Work
        );
    }
}
