#[path = "support/clearance_fixture.rs"]
mod support;
use iced_css::{
    capture::{CapturedElement, adapters},
    evaluation::{
        display_rules::DisplayRuleBundle,
        source_display::evaluate_source_display,
        tree_layout::{TreeLayoutError, layout_captured_tree, prepare_tree_source_projection},
    },
    host::source_publication::InitialSourceHost,
};
use iced_css_core::formatting::block_flow::{FlowContent, FlowGeometry, FlowLimits, FlowViewport};
use support::*;

type Element = CapturedElement<'static, (), iced::Theme, ()>;

fn node(id: &str, children: Vec<Element>) -> Element {
    adapters::column_from_vec(children, capture_limits())
        .unwrap()
        .style_id(id, attrs())
        .unwrap()
        .erase()
}
fn hosted(root: Element) -> InitialSourceHost {
    let (_, construction) = root.into_parts();
    host(construction, None, None)
}
/// root > [a > [a1, a2 > [deep]], b, c]
fn sample() -> InitialSourceHost {
    let deep = node("deep", vec![]);
    let a1 = node("a1", vec![]);
    let a2 = node("a2", vec![deep]);
    let a = node("a", vec![a1, a2]);
    let root = node("root", vec![a, node("b", vec![]), node("c", vec![])]);
    hosted(root)
}
fn run(
    css: &str,
    measure: impl FnMut(usize) -> Option<FlowContent>,
) -> Result<Vec<Option<FlowGeometry>>, TreeLayoutError> {
    let host = sample();
    let snapshot = host.snapshot().unwrap();
    let projection = prepare_tree_source_projection(&host, snapshot, bound()).unwrap();
    let ua = catalog();
    let graph = graph(css, context(100, 100));
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(snapshot, &bundle, display_limits()).unwrap();
    layout_captured_tree(
        &projection,
        &display,
        fact_limits(),
        FlowViewport {
            left: 0.0,
            top: 0.0,
            width: 200.0,
            height: Some(400.0),
        },
        FlowLimits {
            boxes: bound(),
            depth: nz(32),
        },
        measure,
    )
}
// Preorder: root0 a1 a1_2 a2_3 deep4 b5 c6.
const ROOT: usize = 0;
const A: usize = 1;
const A1: usize = 2;
const A2: usize = 3;
const DEEP: usize = 4;
const B: usize = 5;
const C: usize = 6;

fn solved(css: &str) -> Vec<FlowGeometry> {
    run(css, |_| None)
        .unwrap()
        .into_iter()
        .map(|g| g.unwrap())
        .collect()
}

#[test]
fn nested_margins_padding_and_percentages_from_real_css() {
    let g = solved(
        "#a { margin-top:10px; width:50%; padding:0 5px; }
         #a1 { margin-top:20px; height:8px; margin-bottom:6px; }
         #a2 { margin-top:4px; }
         #deep { height:10%; margin-bottom:3px; }
         #b { height:2px; margin-top:1px; }
         #c { height:1px; }",
    );
    // a and a1 top margins collapse to 20.
    assert_eq!(g[A].border.top, 20.0);
    assert_eq!(g[A1].border.top, 20.0);
    assert_eq!(g[A].border.right, 110.0);
    assert_eq!(g[A1].border.left, 5.0);
    assert_eq!(g[A1].border.right, 105.0);
    // 28 + max(6,4) = 34; the height percentage has an auto basis, so it computes auto.
    assert_eq!(g[A2].border.top, 34.0);
    assert!(g[DEEP].collapsed_through);
    // a's bottom has no separator, so its last margins run on to b: max(6,4,3,1)
    // already applied at 34, b's own top margin joins the same open run.
    assert_eq!(g[B].border.top, 34.0);
    assert_eq!(g[C].border.top, 36.0);
    assert_eq!(g[ROOT].content.bottom, 37.0);
}

#[test]
fn floats_escape_nested_blocks_and_clear_sees_them() {
    let g = solved(
        "#a1 { height:10px; }
         #deep { float:right; width:60px; height:50px; }
         #b { clear:right; height:5px; }
         #c { overflow:hidden; float:none; height:5px; }",
    );
    assert_eq!(g[DEEP].border.left, 140.0);
    assert_eq!(g[DEEP].border.top, 10.0);
    assert_eq!(g[B].border.top, 60.0);
    assert_eq!(g[B].clearance, Some(50.0));
    assert_eq!(g[C].border.top, 65.0);
}

#[test]
fn display_none_removes_a_subtree_and_relative_moves_one() {
    let result = run(
        "#a2 { display:none; }
         #a { position:relative; top:7px; left:3px; }
         #a1 { height:10px; }
         #b { height:1px; }",
        |_| None,
    )
    .unwrap();
    assert!(result[A2].is_none());
    assert!(result[DEEP].is_none());
    let a1 = result[A1].unwrap();
    assert_eq!((a1.border.left, a1.border.top), (3.0, 7.0));
    // Later flow does not react to the offset.
    assert_eq!(result[B].unwrap().border.top, 10.0);
}

#[test]
fn leaf_content_measures_by_source_node_and_shrinks_floats() {
    let mut seen = Vec::new();
    let result = run("#c { float:left; }", |original| {
        seen.push(original);
        Some(FlowContent {
            height: 9.0,
            preferred_width: 33.0,
            preferred_min_width: 5.0,
        })
    })
    .unwrap();
    assert_eq!(seen.len(), 4);
    let c = result[C].unwrap();
    assert_eq!((c.border.left, c.border.right), (0.0, 33.0));
    assert_eq!(result[A1].unwrap().border.bottom, 9.0);
}

#[test]
fn outside_block_flow_is_refused_by_source() {
    assert!(matches!(
        run("#b { position:absolute; }", |_| None),
        Err(TreeLayoutError::Unsupported {
            projected: B,
            property: "position"
        })
    ));
    assert!(matches!(
        run("#c { display:inline; }", |_| None),
        Err(TreeLayoutError::Unsupported {
            projected: C,
            property: "display"
        })
    ));
}

#[test]
fn overflow_establishes_a_bfc_that_avoids_floats() {
    let g = solved(
        "#a1 { float:left; width:50px; height:20px; }
         #c { overflow:hidden; height:1px; }
         #b { height:1px; }",
    );
    assert_eq!(g[B].border.left, 0.0);
    assert_eq!(g[C].border.left, 50.0);
}
