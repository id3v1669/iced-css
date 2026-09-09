use iced_css_core::formatting::table_borders::{
    BorderCandidate, BorderEdge, BorderOrder, BorderOrigin, BorderSegmentError,
    BorderSegmentLimits, BorderStyle, CollapsedBorder, split_collapsed_borders,
};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> BorderSegmentLimits {
    BorderSegmentLimits {
        edges: nz(100),
        segments: nz(100),
        retained_bytes: nz(100000),
        work: nz(100000),
    }
}
fn edge(start: u32, end: u32, style: BorderStyle, width: f32) -> BorderEdge {
    BorderEdge::new(
        start,
        end,
        BorderCandidate::new(
            style,
            width,
            BorderOrigin::Cell,
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
fn spanning_hidden_border_suppresses_only_its_covered_segments() {
    use BorderStyle::*;
    let edges = [
        edge(0, 4, Solid, 2.0),
        edge(1, 3, Hidden, 0.0),
        edge(2, 5, Double, 3.0),
    ];
    let segments = split_collapsed_borders(&edges, limits()).unwrap();
    let actual: Vec<_> = segments
        .iter()
        .map(|s| (s.start, s.end, s.border))
        .collect();
    assert_eq!(
        actual,
        [
            (
                0,
                1,
                CollapsedBorder::Winner {
                    index: 0,
                    paint_style: Solid
                }
            ),
            (1, 2, CollapsedBorder::Suppressed),
            (2, 3, CollapsedBorder::Suppressed),
            (
                3,
                4,
                CollapsedBorder::Winner {
                    index: 2,
                    paint_style: Double
                }
            ),
            (
                4,
                5,
                CollapsedBorder::Winner {
                    index: 2,
                    paint_style: Double
                }
            ),
        ]
    );
}
#[test]
fn gaps_and_none_are_absent_without_losing_extreme_logical_endpoints() {
    let edges = [
        edge(0, 1, BorderStyle::Solid, 1.0),
        edge(3, u32::MAX, BorderStyle::None, 0.0),
    ];
    let segments = split_collapsed_borders(&edges, limits()).unwrap();
    assert_eq!(segments.len(), 3);
    assert_eq!(
        (segments[1].start, segments[1].end, segments[1].border),
        (1, 3, CollapsedBorder::Absent)
    );
    assert_eq!(
        (segments[2].start, segments[2].end, segments[2].border),
        (3, u32::MAX, CollapsedBorder::Absent)
    );
    assert!(split_collapsed_borders(&[], limits()).unwrap().is_empty());
}
#[test]
fn selection_indices_refer_to_the_original_unsorted_edges() {
    let edges = [
        edge(2, 6, BorderStyle::Inset, 8.0),
        edge(0, 4, BorderStyle::Double, 3.0),
    ];
    for (entries, want) in [(edges, [1, 0, 0]), ([edges[1], edges[0]], [0, 1, 1])] {
        let segments = split_collapsed_borders(&entries, limits()).unwrap();
        assert_eq!(segments.len(), 3);
        for ((segment, index), paint_style) in segments.iter().zip(want).zip([
            BorderStyle::Double,
            BorderStyle::Ridge,
            BorderStyle::Ridge,
        ]) {
            assert_eq!(
                segment.border,
                CollapsedBorder::Winner { index, paint_style }
            );
        }
    }
}
#[test]
fn invalid_ranges_and_limits_never_return_partial_segments() {
    let c = BorderCandidate::new(
        BorderStyle::Solid,
        1.0,
        BorderOrigin::Cell,
        BorderOrder {
            inline_start: 0,
            block_start: 0,
        },
    )
    .unwrap();
    for (start, end) in [(0, 0), (2, 1), (u32::MAX, u32::MAX)] {
        assert_eq!(
            BorderEdge::new(start, end, c),
            Err(BorderSegmentError::Range)
        );
    }
    let edges = [
        edge(0, 4, BorderStyle::Solid, 1.0),
        edge(1, 3, BorderStyle::Hidden, 0.0),
    ];
    let mut bound = limits();
    bound.edges = nz(1);
    assert_eq!(
        split_collapsed_borders(&edges, bound),
        Err(BorderSegmentError::Edges)
    );
    bound = limits();
    bound.segments = nz(1);
    assert_eq!(
        split_collapsed_borders(&edges, bound),
        Err(BorderSegmentError::Segments)
    );
    bound = limits();
    bound.retained_bytes = nz(1);
    assert_eq!(
        split_collapsed_borders(&edges, bound),
        Err(BorderSegmentError::Bytes)
    );
    bound = limits();
    bound.work = nz(1);
    assert_eq!(
        split_collapsed_borders(&edges, bound),
        Err(BorderSegmentError::Work)
    );
    bound = limits();
    bound.retained_bytes = nz(17); // Endpoints fit; three output records cannot.
    assert_eq!(
        split_collapsed_borders(&edges, bound),
        Err(BorderSegmentError::Bytes)
    );
    bound = limits();
    bound.work = nz(15); // Endpoint preparation fits; segment scans cannot.
    assert_eq!(
        split_collapsed_borders(&edges, bound),
        Err(BorderSegmentError::Work)
    );
    let descending = [
        edge(6, 7, BorderStyle::Solid, 1.0),
        edge(4, 5, BorderStyle::Solid, 1.0),
        edge(2, 3, BorderStyle::Solid, 1.0),
        edge(0, 1, BorderStyle::Solid, 1.0),
    ];
    bound.work = nz(20);
    assert_eq!(
        split_collapsed_borders(&descending, bound),
        Err(BorderSegmentError::Work)
    );
    assert_eq!(split_collapsed_borders(&edges, limits()).unwrap().len(), 3);
}
