use iced_css_core::formatting::line_geometry::{
    InlineBounds, InlineNode, InlineTreeError, InlineTreeLimits, LineAlignment, layout_inline_tree,
};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> InlineTreeLimits {
    InlineTreeLimits {
        nodes: nz(100),
        retained_bytes: nz(100000),
        work: nz(100000),
    }
}
fn strut() -> InlineBounds {
    InlineBounds::font(8.0, 2.0, 10.0).unwrap()
}
fn node(parent: Option<u32>, bounds: InlineBounds, alignment: LineAlignment) -> InlineNode {
    InlineNode {
        parent,
        bounds,
        alignment,
    }
}
fn baseline(raise: f32) -> LineAlignment {
    LineAlignment::Baseline { raise }
}
#[test]
fn nested_baseline_offsets_accumulate_in_parent_coordinates() {
    let nodes = [
        node(None, strut(), baseline(0.0)),
        node(
            Some(0),
            InlineBounds::font(4.0, 1.0, 5.0).unwrap(),
            baseline(3.0),
        ),
        node(
            Some(1),
            InlineBounds::font(2.0, 1.0, 3.0).unwrap(),
            baseline(-2.0),
        ),
    ];
    let line = layout_inline_tree(strut(), &nodes, limits()).unwrap();
    assert_eq!((line.height, line.baseline), (10.0, 8.0));
    let actual: Vec<_> = line
        .placements
        .iter()
        .map(|p| (p.top, p.bottom, p.origin))
        .collect();
    assert_eq!(actual, [(0.0, 10.0, 8.0), (1.0, 6.0, 5.0), (5.0, 8.0, 7.0)]);
}
#[test]
fn top_and_bottom_nodes_align_subtrees_and_exclude_independently_aligned_children() {
    let nodes = [
        node(
            None,
            InlineBounds::edges(-2.0, 2.0).unwrap(),
            LineAlignment::Top,
        ),
        node(Some(0), strut(), baseline(3.0)),
        node(
            Some(1),
            InlineBounds::edges(100.0, 110.0).unwrap(),
            LineAlignment::Bottom,
        ),
    ];
    let line = layout_inline_tree(strut(), &nodes, limits()).unwrap();
    assert_eq!((line.height, line.baseline), (13.0, 8.0));
    let actual: Vec<_> = line
        .placements
        .iter()
        .map(|p| (p.top, p.bottom, p.origin))
        .collect();
    assert_eq!(
        actual,
        [(9.0, 13.0, 11.0), (0.0, 10.0, 8.0), (3.0, 13.0, -97.0)]
    );
}
#[test]
fn oversized_parent_offsets_can_cancel_at_final_group_placement() {
    let max = f32::MAX;
    let zero = InlineBounds::edges(0.0, 0.0).unwrap();
    let far = InlineBounds::edges(max, max).unwrap();
    let nodes = [
        node(None, zero, LineAlignment::Top),
        node(Some(0), far, baseline(max)),
        node(Some(1), far, baseline(max)),
    ];
    let line = layout_inline_tree(zero, &nodes, limits()).unwrap();
    assert_eq!((line.height, line.baseline), (max, 0.0));
    let actual: Vec<_> = line
        .placements
        .iter()
        .map(|p| (p.top, p.bottom, p.origin))
        .collect();
    assert_eq!(actual, [(max, max, max), (max, max, 0.0), (0.0, 0.0, -max)]);
}
#[test]
fn invalid_parents_and_limits_reject_without_partial_output() {
    for parent in [0, 1, u32::MAX] {
        assert!(
            matches!(layout_inline_tree(strut(),&[node(Some(parent),strut(),baseline(0.0))],limits()),Err(InlineTreeError::Parent{node:0,parent:p}) if p==parent)
        );
    }
    let nodes = [
        node(None, strut(), baseline(0.0)),
        node(Some(0), strut(), baseline(0.0)),
    ];
    let mut bound = limits();
    bound.nodes = nz(1);
    assert!(matches!(
        layout_inline_tree(strut(), &nodes, bound),
        Err(InlineTreeError::Nodes)
    ));
    bound = limits();
    bound.retained_bytes = nz(1);
    assert!(matches!(
        layout_inline_tree(strut(), &nodes, bound),
        Err(InlineTreeError::Bytes)
    ));
    bound = limits();
    bound.work = nz(1);
    assert!(matches!(
        layout_inline_tree(strut(), &nodes, bound),
        Err(InlineTreeError::Work)
    ));
    assert!(matches!(
        layout_inline_tree(
            strut(),
            &[node(None, strut(), baseline(f32::NAN))],
            limits()
        ),
        Err(InlineTreeError::Geometry(_))
    ));
}
#[test]
fn empty_forest_keeps_the_parent_strut_and_roots_keep_input_order() {
    let line = layout_inline_tree(strut(), &[], limits()).unwrap();
    assert_eq!((line.height, line.baseline), (10.0, 8.0));
    assert!(line.placements.is_empty());
    let nodes = [
        node(None, strut(), baseline(-4.0)),
        node(None, strut(), baseline(3.0)),
    ];
    let line = layout_inline_tree(strut(), &nodes, limits()).unwrap();
    assert_eq!((line.height, line.baseline), (17.0, 11.0));
    let actual: Vec<_> = line
        .placements
        .iter()
        .map(|p| (p.top, p.bottom, p.origin))
        .collect();
    assert_eq!(actual, [(7.0, 17.0, 15.0), (0.0, 10.0, 8.0)]);
}
