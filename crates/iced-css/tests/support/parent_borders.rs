//! PBR: computed `border-*-width` facts on P in the Sequence facade (parent borders).
//!
//! This module compiles against the predecessor API: it reads only `border_widths()`
//! and records that already exist. Every literal below is derived in the PBR test
//! notes from `baseline_css()` (O content left 0, width 100, top 10; P `hidden`,
//! padding-top 1 and padding-bottom 1); twin comparisons are additional, never the
//! only oracle.

use super::*;

/// f32::MAX as a decimal string (rows X7-X9).
const PBR_M: &str = "340282346638528859811704183484516925440";

/// Child selectors by projected source, starting at source 2.
const PBR_CHILDREN: [&str; 5] = ["b1", "f", "b2", "b3", "b4"];

/// Box sides in edge-array order.
const PBR_SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

/// P matrix widths: CSS text and computed pixels.
const PBR_WIDTHS: [(&str, f32); 5] = [
    ("thin", 1.0),
    ("medium", 3.0),
    ("thick", 5.0),
    ("2px", 2.0),
    ("0.5px", 0.5),
];

/// P's baseline padding, top, right, bottom, left.
const PBR_BASE_PADDING: [f32; 4] = [1.0, 0.0, 1.0, 0.0];

/// P Auto content bottom minus P content top (22, 27 and 31 at content top 11).
fn auto_span(nodes: usize) -> f32 {
    match nodes {
        5 => 11.0,
        6 => 16.0,
        7 => 20.0,
        _ => panic!("no PBR fixture for N{nodes}"),
    }
}

/// The facade fixture: `baseline_css()`, the N7 leading rule, then the fragment.
fn pbr_css(nodes: usize, fragment: &str) -> String {
    let leading = if nodes == 7 {
        leading_clear_source_rule(7)
    } else {
        ""
    };
    format!("{} {leading} {fragment}", baseline_css())
}

/// Copied observations of one principal child.
#[derive(Debug, Clone, Copy)]
struct PbrChild {
    source: usize,
    border: PrincipalEdges,
    content: PrincipalEdges,
    /// Used margins: top, right, bottom, left.
    margin: [f32; 4],
    used_width: f32,
    used_height: f32,
}

/// Copied observations of one successful layout.
#[derive(Debug, Clone)]
struct PbrView {
    origin: FirstChildOriginPlacement,
    horizontal: BlockHorizontalPlacement,
    height: BfcHeightPlacement,
    bottom: BlockBottomPlacement,
    content: PrincipalEdges,
    children: Vec<PbrChild>,
    float: PlacedFloat,
    work: usize,
    core_work: usize,
}

fn push(out: &mut Vec<(String, u64)>, name: &str, value: f32) {
    out.push((name.to_owned(), u64::from(value.to_bits())));
}

fn push_count(out: &mut Vec<(String, u64)>, name: &str, value: usize) {
    out.push((name.to_owned(), u64::try_from(value).unwrap()));
}

fn push_edges(out: &mut Vec<(String, u64)>, name: &str, edges: PrincipalEdges) {
    for (side, value) in [
        ("left", edges.left),
        ("right", edges.right),
        ("top", edges.top),
        ("bottom", edges.bottom),
    ] {
        push(out, &format!("{name}.{side}"), value);
    }
}

impl PbrView {
    /// Every observed number by name: f32 values as bits, counts as integers.
    fn fields(&self) -> Vec<(String, u64)> {
        let mut out = Vec::new();
        push(
            &mut out,
            "p.origin.used_margin_top",
            self.origin.used_margin_top,
        );
        push(&mut out, "p.origin.border_top", self.origin.border_top);
        push(&mut out, "p.origin.content_top", self.origin.content_top);
        push(&mut out, "p.horizontal.width", self.horizontal.used.width);
        push(
            &mut out,
            "p.horizontal.margin_left",
            self.horizontal.used.margin_left,
        );
        push(
            &mut out,
            "p.horizontal.margin_right",
            self.horizontal.used.margin_right,
        );
        out.push((
            "p.horizontal.passes".to_owned(),
            u64::from(self.horizontal.used.passes),
        ));
        push(
            &mut out,
            "p.horizontal.content_left",
            self.horizontal.content.left,
        );
        push(
            &mut out,
            "p.horizontal.content_right",
            self.horizontal.content.right,
        );
        push(&mut out, "p.height.used_height", self.height.used_height);
        push(
            &mut out,
            "p.height.content_bottom",
            self.height.content_bottom,
        );
        match self.height.auto_extent {
            Some(extent) => {
                push(
                    &mut out,
                    "p.height.normal_margin_bottom",
                    extent.normal_margin_bottom,
                );
                push(&mut out, "p.height.selected_bottom", extent.selected_bottom);
                out.push((
                    "p.height.float_extended".to_owned(),
                    u64::from(extent.float_extended),
                ));
            }
            None => out.push(("p.height.explicit".to_owned(), 1)),
        }
        push(
            &mut out,
            "p.bottom.used_margin_bottom",
            self.bottom.used_margin_bottom,
        );
        push(
            &mut out,
            "p.bottom.padding_bottom",
            self.bottom.padding_bottom,
        );
        push(
            &mut out,
            "p.bottom.border_bottom",
            self.bottom.border_bottom,
        );
        push(
            &mut out,
            "p.bottom.margin_bottom",
            self.bottom.margin_bottom,
        );
        push_edges(&mut out, "p.content", self.content);
        push_count(&mut out, "children", self.children.len());
        for (index, child) in self.children.iter().enumerate() {
            push_count(&mut out, &format!("c{index}.source"), child.source);
            push_edges(&mut out, &format!("c{index}.border"), child.border);
            push_edges(&mut out, &format!("c{index}.content"), child.content);
            for (side, value) in PBR_SIDES.iter().zip(child.margin) {
                push(&mut out, &format!("c{index}.margin.{side}"), value);
            }
            push(&mut out, &format!("c{index}.used_width"), child.used_width);
            push(
                &mut out,
                &format!("c{index}.used_height"),
                child.used_height,
            );
        }
        push(&mut out, "float.left", self.float.left);
        push(&mut out, "float.right", self.float.right);
        push(&mut out, "float.top", self.float.top);
        push(&mut out, "float.bottom", self.float.bottom);
        push_count(&mut out, "work", self.work);
        push_count(&mut out, "core_work", self.core_work);
        out
    }
}

fn pbr_view(layout: &CapturedSiblingLayout<'_>) -> PbrView {
    let parent = layout.parent();
    PbrView {
        origin: parent.origin(),
        horizontal: parent.horizontal(),
        height: parent.height(),
        bottom: parent.bottom(),
        content: parent.content(),
        children: layout
            .children()
            .iter()
            .map(|child| PbrChild {
                source: child.source_index(),
                border: child.border(),
                content: child.content(),
                margin: [
                    child.used_margin_top(),
                    child.used_margin_right(),
                    child.used_margin_bottom(),
                    child.used_margin_left(),
                ],
                used_width: child.used_width(),
                used_height: child.used_height(),
            })
            .collect(),
        float: layout.sequence().placed_float(),
        work: layout.receipt().work,
        core_work: layout.receipt().core.work,
    }
}

/// The producer result, then (only on producer success) the layout result.
enum PbrRun {
    Producer(SiblingBoxFactsError),
    Built {
        rows: Vec<ClearanceBoxFact>,
        result: Result<PbrView, SiblingLayoutError>,
    },
}

/// Never unwraps the producer: a producer error is returned as `PbrRun::Producer`.
fn pbr_run(nodes: usize, css: &str) -> PbrRun {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        out = Some(
            match evaluate_sibling_box_facts(projection, display, sibling_fact_limits()) {
                Err(error) => PbrRun::Producer(error),
                Ok(facts) => PbrRun::Built {
                    rows: (0..nodes)
                        .map(|source| *facts.row(source).unwrap())
                        .collect(),
                    result: default_outer_layout(
                        &facts,
                        &projection.nodes()[0],
                        0.0,
                        100.0,
                        10.0,
                        layout_limits(),
                    )
                    .map(|layout| pbr_view(&layout)),
                },
            },
        );
    });
    out.unwrap()
}

/// A producer error with its property named by the registry.
fn describe(error: &SiblingBoxFactsError) -> String {
    match error {
        SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Unsupported {
            original,
            property,
            target,
            reason,
        }) => format!(
            "Evaluation(Unsupported {{ original: {original}, property: {}, target: {target:?}, \
             reason: {reason:?} }})",
            property.descriptor().name
        ),
        other => format!("{other:?}"),
    }
}

/// Success-asserting helper: panics with the typed error otherwise.
fn pbr_layout_rows(nodes: usize, css: &str) -> (Vec<ClearanceBoxFact>, PbrView) {
    match pbr_run(nodes, css) {
        PbrRun::Producer(error) => panic!(
            "{css}: expected a layout, got producer error {} ({error:?})",
            describe(&error)
        ),
        PbrRun::Built {
            result: Err(error), ..
        } => panic!("{css}: expected a layout, got {error:?}"),
        PbrRun::Built {
            rows,
            result: Ok(view),
        } => (rows, view),
    }
}

fn pbr_layout(nodes: usize, css: &str) -> PbrView {
    pbr_layout_rows(nodes, css).1
}

/// Layout-error-asserting helper: panics on a producer error or a layout.
fn pbr_error(nodes: usize, css: &str) -> SiblingLayoutError {
    match pbr_run(nodes, css) {
        PbrRun::Producer(error) => panic!(
            "{css}: expected a layout error, got producer error {} ({error:?})",
            describe(&error)
        ),
        PbrRun::Built {
            result: Ok(view), ..
        } => panic!("{css}: expected a layout error, got {view:?}"),
        PbrRun::Built {
            result: Err(error), ..
        } => error,
    }
}

/// Producer-error-asserting helper: panics when the producer succeeds.
fn pbr_facts_error(nodes: usize, css: &str) -> SiblingBoxFactsError {
    match pbr_run(nodes, css) {
        PbrRun::Producer(error) => error,
        PbrRun::Built { result, .. } => {
            panic!("{css}: expected a producer error, got {result:?}")
        }
    }
}

fn assert_facts_unsupported(
    error: &SiblingBoxFactsError,
    expected: (usize, &str, BoxFactTarget, BoxFactUnsupported),
    context: &str,
) {
    let (original, name, target, reason) = expected;
    assert!(
        matches!(
            error,
            SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Unsupported {
                original: actual,
                property,
                target: actual_target,
                reason: actual_reason,
            }) if *actual == original
                && property.descriptor().name == name
                && *actual_target == target
                && *actual_reason == reason
        ),
        "{context}: expected Unsupported at original {original} on {name}, got {}",
        describe(error)
    );
}

fn assert_profile(
    error: &SiblingLayoutError,
    source: usize,
    reason: OwnerProfileReason,
    context: &str,
) {
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Profile(
                ClearanceOwnerError::Unsupported {
                    source: actual,
                    reason: actual_reason,
                }
            )) if *actual == source && *actual_reason == reason
        ),
        "{context}: expected {reason:?} at source {source}, got {error:?}"
    );
}

fn assert_captured(error: &SiblingLayoutError, expected: CapturedClearanceError, context: &str) {
    assert!(
        matches!(error, SiblingLayoutError::Captured(actual) if *actual == expected),
        "{context}: expected {expected:?}, got {error:?}"
    );
}

/// Field-by-field bit equality except the named fields.
fn assert_same_view(actual: &PbrView, expected: &PbrView, skip: &[&str], context: &str) {
    let actual_fields = actual.fields();
    let expected_fields = expected.fields();
    assert_eq!(
        actual_fields.len(),
        expected_fields.len(),
        "{context}: field count"
    );
    let mut compared = 0;
    for ((name, value), (other, expected)) in actual_fields.iter().zip(&expected_fields) {
        assert_eq!(name, other, "{context}");
        if skip.contains(&name.as_str()) {
            continue;
        }
        assert_eq!(value, expected, "{context}: {name}");
        compared += 1;
    }
    assert_eq!(
        compared + skip.len(),
        actual_fields.len(),
        "{context}: skip list"
    );
}

/// P (border_top, content_top, content_bottom, padding_bottom, border_bottom); P's
/// margins are zero, so margin_bottom equals border_bottom.
fn assert_p_vertical(view: &PbrView, expected: [f32; 5], context: &str) {
    let actual = [
        view.origin.border_top,
        view.origin.content_top,
        view.height.content_bottom,
        view.bottom.padding_bottom,
        view.bottom.border_bottom,
    ];
    assert_eq!(
        actual.map(f32::to_bits),
        expected.map(f32::to_bits),
        "{context}: {actual:?}"
    );
    assert_eq!(
        view.bottom.margin_bottom.to_bits(),
        view.bottom.border_bottom.to_bits(),
        "{context}: margin_bottom"
    );
}

/// Child border-box spans (top, bottom) in projected order; the float's border box is
/// its margin box (zero margins), and both are checked.
fn assert_child_spans(view: &PbrView, expected: &[(f32, f32)], context: &str) {
    let actual: Vec<(f32, f32)> = view
        .children
        .iter()
        .map(|child| (child.border.top, child.border.bottom))
        .collect();
    assert_eq!(actual, expected, "{context}");
    assert_eq!(
        (view.float.top, view.float.bottom),
        expected[1],
        "{context}: float margin box"
    );
}

/// P's used width and content band.
fn assert_p_band(view: &PbrView, width: f32, band: (f32, f32), context: &str) {
    assert_eq!(view.horizontal.used.width, width, "{context}: used width");
    assert_eq!(
        (view.horizontal.content.left, view.horizontal.content.right),
        band,
        "{context}: content band"
    );
    assert_eq!(
        (view.content.left, view.content.right),
        band,
        "{context}: parent().content()"
    );
}

// RED (plan Task 1, step 1.2).

/// R1 / B-T: P's top border is admitted and moves the first-child origin.
#[test]
fn pbr_red_parent_top_border_is_admitted() {
    let css = pbr_css(5, "#p { border-top:2px solid; padding-top:0; }");
    let view = pbr_layout(5, &css);
    assert_p_vertical(&view, [10.0, 12.0, 23.0, 24.0, 24.0], &css);
    assert_child_spans(&view, &[(12.0, 13.0), (17.0, 19.0), (19.0, 20.0)], &css);
}

/// Former R2 (CBR): a child with a nonzero computed border width is admitted; B1's
/// content starts below its 1px top border.
#[test]
fn pbr_child_top_border_is_admitted() {
    let css = pbr_css(5, "#b1 { border-top:1px solid; }");
    let view = pbr_layout(5, &css);
    assert_p_vertical(&view, [10.0, 11.0, 23.0, 24.0, 24.0], &css);
    assert_child_spans(&view, &[(11.0, 13.0), (17.0, 19.0), (19.0, 20.0)], &css);
    assert_eq!(
        (
            view.children[0].content.top,
            view.children[0].content.bottom
        ),
        (12.0, 13.0),
        "{css}: B1 content"
    );
}

// Literal rows: P borders (facade).

/// B-T2 (M5 witness): the top border adds to the kept padding-top.
#[test]
fn pbr_b_t2_top_border_adds_to_padding_top() {
    let css = pbr_css(5, "#p { border-top-width:2px; border-top-style:solid; }");
    let view = pbr_layout(5, &css);
    assert_p_vertical(&view, [10.0, 13.0, 24.0, 25.0, 25.0], &css);
    assert_child_spans(&view, &[(13.0, 14.0), (18.0, 20.0), (20.0, 21.0)], &css);
}

/// B-B (M6 witness): the bottom border is published after the padding.
#[test]
fn pbr_b_b_bottom_border_follows_padding_bottom() {
    let css = pbr_css(
        5,
        "#p { border-bottom-width:2px; border-bottom-style:solid; }",
    );
    let view = pbr_layout(5, &css);
    assert_p_vertical(&view, [10.0, 11.0, 22.0, 23.0, 25.0], &css);
    assert_child_spans(&view, &[(11.0, 12.0), (16.0, 18.0), (18.0, 19.0)], &css);
}

/// B-L: a left border offsets and narrows P's `auto`-width content box.
#[test]
fn pbr_b_l_left_border_narrows_the_content_band() {
    let css = pbr_css(5, "#p { border-left-width:2px; border-left-style:solid; }");
    let view = pbr_layout(5, &css);
    assert_p_band(&view, 98.0, (2.0, 100.0), &css);
    assert_eq!(
        (view.children[0].border.left, view.children[0].border.right),
        (2.0, 100.0),
        "{css}: B1 border box"
    );
    assert_eq!((view.float.left, view.float.right), (2.0, 12.0), "{css}");
    assert_p_vertical(&view, [10.0, 11.0, 22.0, 23.0, 23.0], &css);
}

/// B-R: a right border narrows P's content band on the right.
#[test]
fn pbr_b_r_right_border_narrows_the_content_band() {
    let css = pbr_css(
        5,
        "#p { border-right-width:2px; border-right-style:solid; }",
    );
    let view = pbr_layout(5, &css);
    assert_p_band(&view, 98.0, (0.0, 98.0), &css);
    assert_eq!(
        (view.children[0].border.left, view.children[0].border.right),
        (0.0, 98.0),
        "{css}: B1 border box"
    );
    assert_eq!((view.float.left, view.float.right), (0.0, 10.0), "{css}");
    assert_p_vertical(&view, [10.0, 11.0, 22.0, 23.0, 23.0], &css);
}

/// B-A: all four borders.
#[test]
fn pbr_b_a_all_four_borders() {
    let css = pbr_css(5, "#p { border-width:2px; border-style:solid; }");
    let view = pbr_layout(5, &css);
    assert_p_band(&view, 96.0, (2.0, 98.0), &css);
    assert_p_vertical(&view, [10.0, 13.0, 24.0, 25.0, 27.0], &css);
    assert_child_spans(&view, &[(13.0, 14.0), (18.0, 20.0), (20.0, 21.0)], &css);
}

/// X13: a visible style without a width declaration gives the initial `medium` (3).
#[test]
fn pbr_x13_visible_style_uses_initial_medium_width() {
    let css = pbr_css(5, "#p { border-style:solid; padding:0; }");
    let (rows, view) = pbr_layout_rows(5, &css);
    assert_eq!(rows[1].border_widths(), [3.0; 4], "{css}");
    assert_p_band(&view, 94.0, (3.0, 97.0), &css);
    assert_p_vertical(&view, [10.0, 13.0, 24.0, 24.0, 27.0], &css);
    assert_child_spans(&view, &[(13.0, 14.0), (18.0, 20.0), (20.0, 21.0)], &css);
}

/// One A1 row: N, the shared P and F declarations, P's explicit height, and the
/// bordered sides in edge-array order.
type PbrMatrixRow = (usize, String, Option<f32>, Vec<usize>);

fn pbr_matrix_rows() -> Vec<PbrMatrixRow> {
    let mut rows = Vec::new();
    for nodes in [5, 6, 7] {
        for f_side in ["left", "right"] {
            for direction in ["ltr", "rtl"] {
                for overflow in ["hidden", "auto"] {
                    for height in [None, Some(40.0_f32)] {
                        let mut common = format!(
                            "#f {{ float:{f_side}; }} #p {{ direction:{direction}; \
                             overflow:{overflow};"
                        );
                        if let Some(height) = height {
                            common.push_str(&format!(" height:{height}px;"));
                        }
                        for pattern in 0..5 {
                            let sides = if pattern == 4 {
                                vec![0, 1, 2, 3]
                            } else {
                                vec![pattern]
                            };
                            rows.push((nodes, common.clone(), height, sides));
                        }
                    }
                }
            }
        }
    }
    rows
}

/// A1: 240 P rows, each compared with its transfer twin (the border removed and the
/// same side's padding increased by the width) and with the literal P model. Widths
/// rotate over `PBR_WIDTHS` so that every side pattern meets every width.
#[test]
fn pbr_matrix() {
    let cases = pbr_matrix_rows();
    assert_eq!(cases.len(), 240);
    let mut evaluated = Vec::new();
    for (index, (nodes, common, height, sides)) in cases.into_iter().enumerate() {
        let (name, width) = PBR_WIDTHS[(index + index / 5) % 5];
        let mut bordered = common.clone();
        let mut padded = common;
        let mut border = [0.0_f32; 4];
        for side in sides {
            let edge = PBR_SIDES[side];
            bordered.push_str(&format!(
                " border-{edge}-style:solid; border-{edge}-width:{name};"
            ));
            padded.push_str(&format!(
                " padding-{edge}:{}px;",
                PBR_BASE_PADDING[side] + width
            ));
            border[side] = width;
        }
        let bordered = pbr_css(nodes, &format!("{bordered} }}"));
        let padded = pbr_css(nodes, &format!("{padded} }}"));
        let view = pbr_layout(nodes, &bordered);
        let twin = pbr_layout(nodes, &padded);
        assert_same_view(&view, &twin, &["p.bottom.padding_bottom"], &bordered);

        let [top, right, bottom, left] = border;
        let content_top = 10.0 + top + 1.0;
        let content_bottom = content_top + height.unwrap_or_else(|| auto_span(nodes));
        assert_p_band(
            &view,
            100.0 - left - right,
            (left, 100.0 - right),
            &bordered,
        );
        assert_p_vertical(
            &view,
            [
                10.0,
                content_top,
                content_bottom,
                content_bottom + 1.0,
                content_bottom + 1.0 + bottom,
            ],
            &bordered,
        );
        assert_eq!(
            twin.bottom.padding_bottom,
            content_bottom + 1.0 + bottom,
            "{padded}"
        );
        assert_eq!(view.children[0].border.top, content_top, "{bordered}");
        evaluated.push(index);
    }
    assert_eq!(evaluated.len(), 240);
}

/// The plan's twin pair: `bordered` has a 2px solid border and padding 1 on `side`;
/// `padded` has padding 3 there (b = 2, p = 1, q = 3).
fn pbr_twin_css(nodes: usize, side: &str, f_side: &str) -> (String, String) {
    let float = format!("#f {{ float:{f_side}; }}");
    (
        pbr_css(
            nodes,
            &format!(
                "{float} #p {{ border-{side}-style:solid; border-{side}-width:2px; \
                 padding-{side}:1px; }}"
            ),
        ),
        pbr_css(nodes, &format!("{float} #p {{ padding-{side}:3px; }}")),
    )
}

/// TW-E (24 exact pairs), TW-D1 to TW-D3 (double rounding) and TW-C (exact control).
#[test]
fn pbr_twin_matrix() {
    let mut rows = 0;
    for nodes in [5, 6, 7] {
        for f_side in ["left", "right"] {
            for side in PBR_SIDES {
                let (bordered, padded) = pbr_twin_css(nodes, side, f_side);
                let view = pbr_layout(nodes, &bordered);
                let twin = pbr_layout(nodes, &padded);
                assert_same_view(&view, &twin, &["p.bottom.padding_bottom"], &bordered);
                let content_top = if side == "top" { 13.0 } else { 11.0 };
                let content_bottom = content_top + auto_span(nodes);
                let (padding, border) = if side == "bottom" {
                    (content_bottom + 3.0, content_bottom + 3.0)
                } else {
                    (content_bottom + 1.0, content_bottom + 1.0)
                };
                assert_p_vertical(
                    &view,
                    [
                        10.0,
                        content_top,
                        content_bottom,
                        content_bottom + 1.0,
                        border,
                    ],
                    &bordered,
                );
                assert_p_vertical(
                    &twin,
                    [10.0, content_top, content_bottom, padding, border],
                    &padded,
                );
                rows += 1;
            }
        }
    }
    let edges = [
        "p.bottom.padding_bottom",
        "p.bottom.border_bottom",
        "p.bottom.margin_bottom",
    ];
    for nodes in [5, 6, 7] {
        let bordered = pbr_css(
            nodes,
            "#p { height:16777205px; border-bottom:1px solid; padding-bottom:1px; }",
        );
        let padded = pbr_css(nodes, "#p { height:16777205px; padding-bottom:2px; }");
        let view = pbr_layout(nodes, &bordered);
        let twin = pbr_layout(nodes, &padded);
        assert_same_view(&view, &twin, &edges, &bordered);
        assert_eq!(
            view.height.content_bottom.to_bits(),
            0x4b80_0000,
            "{bordered}"
        );
        assert_eq!(
            [
                view.bottom.padding_bottom,
                view.bottom.border_bottom,
                view.bottom.margin_bottom,
            ]
            .map(f32::to_bits),
            [0x4b80_0000; 3],
            "{bordered}"
        );
        assert_eq!(
            twin.height.content_bottom.to_bits(),
            0x4b80_0000,
            "{padded}"
        );
        assert_eq!(
            [
                twin.bottom.padding_bottom,
                twin.bottom.border_bottom,
                twin.bottom.margin_bottom,
            ]
            .map(f32::to_bits),
            [0x4b80_0001; 3],
            "{padded}"
        );
        assert_ne!(
            view.bottom.border_bottom.to_bits(),
            twin.bottom.border_bottom.to_bits()
        );
        assert_ne!(
            view.bottom.margin_bottom.to_bits(),
            twin.bottom.margin_bottom.to_bits()
        );
        rows += 1;
    }
    let bordered = pbr_css(
        5,
        "#p { height:7px; border-bottom:2px solid; padding-bottom:1px; }",
    );
    let padded = pbr_css(5, "#p { height:7px; padding-bottom:3px; }");
    let view = pbr_layout(5, &bordered);
    let twin = pbr_layout(5, &padded);
    assert_same_view(&view, &twin, &["p.bottom.padding_bottom"], &bordered);
    assert_p_vertical(&view, [10.0, 11.0, 18.0, 19.0, 21.0], &bordered);
    assert_p_vertical(&twin, [10.0, 11.0, 18.0, 21.0, 21.0], &padded);
    rows += 1;
    assert_eq!(rows, 28);
}

// Interplay rows.

/// I1: a bordered P on the signed Auto route equals its transfer twin; the float
/// bottom (18) extends past the signed normal extent (17).
#[test]
fn pbr_i1_signed_auto_route_twin() {
    let bordered = pbr_css(
        5,
        "#b2 { margin-bottom:-2px; } #p { border-bottom:2px solid; }",
    );
    let padded = pbr_css(5, "#b2 { margin-bottom:-2px; } #p { padding-bottom:3px; }");
    let view = pbr_layout(5, &bordered);
    let twin = pbr_layout(5, &padded);
    assert_same_view(&view, &twin, &["p.bottom.padding_bottom"], &bordered);
    assert_p_vertical(&view, [10.0, 11.0, 18.0, 19.0, 21.0], &bordered);
    assert_p_vertical(&twin, [10.0, 11.0, 18.0, 21.0, 21.0], &padded);
    assert_eq!(view.children[2].margin[2], -2.0, "{bordered}");
}

/// I2 (M7 witness): the `auto` containment area is P's content box, not its border
/// area; B2's margin bottom 22 lies in the border area below content bottom 21.
#[test]
fn pbr_i2_auto_containment_excludes_the_border_area() {
    let css = pbr_css(
        5,
        "#p { overflow:auto; height:10px; padding-bottom:0; border-bottom:20px solid; }",
    );
    let error = pbr_error(5, &css);
    assert!(
        matches!(error, SiblingLayoutError::ParentAutoOverflow { source: 4 }),
        "{css}: {error:?}"
    );
    let hidden = pbr_css(
        5,
        "#p { overflow:hidden; height:10px; padding-bottom:0; border-bottom:20px solid; }",
    );
    let view = pbr_layout(5, &hidden);
    assert_p_vertical(&view, [10.0, 11.0, 21.0, 21.0, 41.0], &hidden);
}

/// I3: a new-BFC tail under a bordered P keeps contact; every interval moves by +2.
#[test]
fn pbr_i3_new_bfc_tail_moves_with_the_top_border() {
    let control = pbr_css(5, "#b2 { overflow:hidden; }");
    let css = pbr_css(5, "#b2 { overflow:hidden; } #p { border-top:2px solid; }");
    let base = pbr_layout(5, &control);
    let view = pbr_layout(5, &css);
    assert_child_spans(&base, &[(11.0, 12.0), (16.0, 18.0), (18.0, 19.0)], &control);
    assert_child_spans(&view, &[(13.0, 14.0), (18.0, 20.0), (20.0, 21.0)], &css);
    assert_p_vertical(&base, [10.0, 11.0, 22.0, 23.0, 23.0], &control);
    assert_p_vertical(&view, [10.0, 13.0, 24.0, 25.0, 25.0], &css);
    for (child, twin) in view.children.iter().zip(&base.children) {
        assert_eq!(child.content.top, twin.content.top + 2.0, "{css}");
        assert_eq!(child.content.bottom, twin.content.bottom + 2.0, "{css}");
        assert_eq!(
            (child.border.left, child.border.right),
            (twin.border.left, twin.border.right),
            "{css}"
        );
    }
}

// New pins.

/// N1 (CBR): 48 rows; every child source and side admits a 1px solid border, and
/// that child's border box exceeds its content box by exactly 1 on that side only
/// (twins and literal anchors: `child_borders`).
#[test]
fn pbr_n1_child_borders_are_admitted() {
    let mut rows = 0;
    for nodes in [5, 6, 7] {
        for source in 2..nodes {
            for (index, side) in PBR_SIDES.iter().enumerate() {
                let css = pbr_css(
                    nodes,
                    &format!(
                        "#{} {{ border-{side}:1px solid; }}",
                        PBR_CHILDREN[source - 2]
                    ),
                );
                let child = pbr_layout(nodes, &css).children[source - 2];
                let inset = [
                    child.content.top - child.border.top,
                    child.border.right - child.content.right,
                    child.border.bottom - child.content.bottom,
                    child.content.left - child.border.left,
                ];
                let mut expected = [0.0_f32; 4];
                expected[index] = 1.0;
                assert_eq!(inset, expected, "{css}");
                rows += 1;
            }
        }
    }
    assert_eq!(rows, 48);
}

/// N2 (M8 witness): a nonzero border alone isolates P on its side.
#[test]
fn pbr_n2_border_alone_isolates_the_parent() {
    for (fragment, expected) in [
        (
            "#p { border-top:1px solid; border-bottom:1px solid; padding:0; }",
            [10.0, 11.0, 22.0, 22.0, 23.0],
        ),
        (
            "#p { border-top:1px solid; padding-top:0; }",
            [10.0, 11.0, 22.0, 23.0, 23.0],
        ),
        (
            "#p { border-bottom:1px solid; padding-bottom:0; }",
            [10.0, 11.0, 22.0, 22.0, 23.0],
        ),
    ] {
        let css = pbr_css(5, fragment);
        let view = pbr_layout(5, &css);
        assert_p_vertical(&view, expected, &css);
        assert_child_spans(&view, &[(11.0, 12.0), (16.0, 18.0), (18.0, 19.0)], &css);
    }
}

/// N2 / CBR M-B (PBR M2 and M3 witnesses): zero computed widths leave P unisolated,
/// whatever the specified width under `none` or `hidden`, and such a P is admitted:
/// its content top is its border top.
#[test]
fn pbr_n2_zero_width_borders_leave_an_admitted_unisolated_parent() {
    for (fragment, expected) in [
        ("#p { padding:0; }", [10.0, 10.0, 21.0, 21.0, 21.0]),
        ("#p { padding-top:0; }", [10.0, 10.0, 21.0, 22.0, 22.0]),
        (
            "#p { border:0 solid; padding:0; }",
            [10.0, 10.0, 21.0, 21.0, 21.0],
        ),
        (
            "#p { border:5px hidden; padding:0; }",
            [10.0, 10.0, 21.0, 21.0, 21.0],
        ),
        (
            "#p { border:5px none; padding:0; }",
            [10.0, 10.0, 21.0, 21.0, 21.0],
        ),
    ] {
        let css = pbr_css(5, fragment);
        let view = pbr_layout(5, &css);
        assert_p_vertical(&view, expected, &css);
        assert_child_spans(&view, &[(10.0, 11.0), (15.0, 17.0), (17.0, 18.0)], &css);
    }
}

/// N3 (M12 witness): a zero-width styled child has the geometry of `border-style:none`.
#[test]
fn pbr_n3_zero_width_styled_children_equal_the_none_control() {
    let mut rows = 0;
    for (nodes, child) in [(5, "b1"), (5, "f"), (5, "b2"), (6, "b3")] {
        let control = pbr_css(nodes, &format!("#{child} {{ border-style:none; }}"));
        let expected = pbr_layout(nodes, &control);
        for declarations in [
            "border:0 solid;",
            "border:3px hidden;",
            "border-style:dotted; border-width:0;",
        ] {
            let css = pbr_css(nodes, &format!("#{child} {{ {declarations} }}"));
            assert_same_view(&pbr_layout(nodes, &css), &expected, &[], &css);
            rows += 1;
        }
    }
    assert_eq!(rows, 12);
}

/// N4: O's borders are admitted and unused; a relative O width is the producer's
/// `RelativeLength` at O (original 4).
#[test]
fn pbr_n4_outer_borders_are_admitted_and_unused() {
    let control = pbr_layout(5, &pbr_css(5, ""));
    let css = pbr_css(5, "#o { border:7px solid; }");
    let (rows, view) = pbr_layout_rows(5, &css);
    assert_eq!(rows[0].border_widths(), [7.0; 4], "{css}");
    assert_same_view(&view, &control, &[], &css);
    let css = pbr_css(5, "#o { border:1em solid; }");
    assert_facts_unsupported(
        &pbr_facts_error(5, &css),
        (
            4,
            "border-top-width",
            BoxFactTarget::Element,
            BoxFactUnsupported::RelativeLength,
        ),
        &css,
    );
}

// Error-order pins (N6 baseline unless stated).

/// X1: a later tail's `HeightKind` (facade loop) is reported beside B1's admitted
/// border.
#[test]
fn pbr_x01_height_kind_with_an_admitted_child_border() {
    for fragment in [
        "#b1 { border-top:1px solid; } #b3 { height:auto; }",
        "#b3 { height:auto; }",
    ] {
        let css = pbr_css(6, fragment);
        assert_captured(
            &pbr_error(6, &css),
            CapturedClearanceError::Unsupported {
                projected: 5,
                property: "height",
                reason: CapturedClearanceUnsupported::HeightKind,
            },
            &css,
        );
    }
}

/// X2: F's `FloatWidth` (after the facade loop) is reported beside B1's admitted
/// border.
#[test]
fn pbr_x02_float_width_with_an_admitted_child_border() {
    for fragment in [
        "#b1 { border:1px solid; } #f { width:auto; }",
        "#f { width:auto; }",
    ] {
        let css = pbr_css(5, fragment);
        assert_captured(
            &pbr_error(5, &css),
            CapturedClearanceError::Unsupported {
                projected: 3,
                property: "width",
                reason: CapturedClearanceUnsupported::FloatWidth,
            },
            &css,
        );
    }
}

/// X3: P's `ParentOverflow` is reported beside B1's admitted border.
#[test]
fn pbr_x03_parent_overflow_with_an_admitted_child_border() {
    for fragment in [
        "#b1 { border:1px solid; } #p { overflow:visible; }",
        "#p { overflow:visible; }",
    ] {
        let css = pbr_css(5, fragment);
        assert_captured(
            &pbr_error(5, &css),
            CapturedClearanceError::Unsupported {
                projected: 1,
                property: "overflow",
                reason: CapturedClearanceUnsupported::ParentOverflow,
            },
            &css,
        );
    }
}

/// X4: an earlier child's `NonpositiveBlockHeight` is reported beside a later child's
/// admitted border (B1's negative top margin keeps it, CTB).
#[test]
fn pbr_x04_earlier_height_with_a_later_child_border() {
    for fragment in [
        "#b2 { border-left:1px solid; } #b1 { height:0; margin-top:-1px; }",
        "#b1 { height:0; margin-top:-1px; }",
    ] {
        let css = pbr_css(5, fragment);
        assert_profile(
            &pbr_error(5, &css),
            2,
            OwnerProfileReason::NonpositiveBlockHeight,
            &css,
        );
    }
}

/// X5 (CBR): an earlier child's admitted border leaves a later child's height fault
/// as the first error.
#[test]
fn pbr_x05_admitted_child_border_leaves_the_later_height_fault() {
    let css = pbr_css(5, "#b1 { border-bottom:2px solid; } #b2 { height:0; }");
    assert_profile(
        &pbr_error(5, &css),
        4,
        OwnerProfileReason::NonpositiveBlockHeight,
        &css,
    );
    let control = pbr_css(5, "#b2 { height:0; }");
    assert_profile(
        &pbr_error(5, &control),
        4,
        OwnerProfileReason::NonpositiveBlockHeight,
        &control,
    );
}

/// X6: within one child, the height check precedes the border check, which admits
/// any finite nonnegative width. (ZHN) Left and right borders do not separate the
/// margins, so the zero height stays refused (B1's negative top margin, CTB).
#[test]
fn pbr_x06_own_height_with_an_own_border() {
    for fragment in [
        "#b1 { border-left:1px solid; border-right:1px solid; height:0; margin-top:-1px; }",
        "#b1 { height:0; margin-top:-1px; }",
    ] {
        let css = pbr_css(5, fragment);
        assert_profile(
            &pbr_error(5, &css),
            2,
            OwnerProfileReason::NonpositiveBlockHeight,
            &css,
        );
    }
}

/// X7: P's first-child origin cannot publish content top 10 + 2M.
#[test]
fn pbr_x07_parent_origin_content_top_is_unrepresentable() {
    let css = pbr_css(
        5,
        &format!(
            "#b1 {{ border:1px solid; }} #p {{ border-top:{PBR_M}px solid; \
             padding-top:{PBR_M}px; }}"
        ),
    );
    let error = pbr_error(5, &css);
    assert!(
        matches!(
            error,
            SiblingLayoutError::ParentOrigin(FirstChildOriginError::Unrepresentable {
                stage: OriginCoordinateStage::ContentTop,
            })
        ),
        "{css}: {error:?}"
    );
}

/// X8: P's horizontal solve cannot represent the remainder 100 - 2M.
#[test]
fn pbr_x08_parent_horizontal_width_is_unrepresentable() {
    let css = pbr_css(
        5,
        &format!("#p {{ border-left:{PBR_M}px solid; border-right:{PBR_M}px solid; }}"),
    );
    let error = pbr_error(5, &css);
    assert!(
        matches!(
            error,
            SiblingLayoutError::ParentHorizontal(BlockHorizontalError::Width(
                BlockWidthError::Unrepresentable
            ))
        ),
        "{css}: {error:?}"
    );
}

/// X9: P's padding bottom rounds to M; its border bottom M + M is unrepresentable.
#[test]
fn pbr_x09_parent_border_bottom_is_unrepresentable() {
    let css = pbr_css(
        5,
        &format!("#p {{ height:7px; border-bottom:{PBR_M}px solid; padding-bottom:{PBR_M}px; }}"),
    );
    let error = pbr_error(5, &css);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Bottom(BlockBottomError::Unrepresentable {
                stage: BlockBottomStage::BorderBottom,
            })
        ),
        "{css}: {error:?}"
    );
}

/// X10: with B1's border admitted by the producer, a later generated-content fault is
/// the first producer error.
#[test]
fn pbr_x10_generated_content_is_the_first_producer_error() {
    for fragment in [
        "#b1 { border:1px solid; } #b3:after { content:'x'; }",
        "#b3:after { content:'x'; }",
    ] {
        let css = pbr_css(6, fragment);
        assert_facts_unsupported(
            &pbr_facts_error(6, &css),
            (
                3,
                "content",
                BoxFactTarget::After,
                BoxFactUnsupported::GeneratedContent,
            ),
            &css,
        );
    }
}

/// X11: a relative width on a visible P side is `RelativeLength` at the first width.
#[test]
fn pbr_x11_relative_parent_width_is_relative_length() {
    let css = pbr_css(5, "#p { border:1em solid; }");
    assert_facts_unsupported(
        &pbr_facts_error(5, &css),
        (
            3,
            "border-top-width",
            BoxFactTarget::Element,
            BoxFactUnsupported::RelativeLength,
        ),
        &css,
    );
}

/// X12 (CBR M-B): a zero-width solid top border with zero padding-top leaves P
/// unisolated, and such a P is admitted.
#[test]
fn pbr_x12_zero_width_top_border_leaves_an_admitted_unisolated_parent() {
    let css = pbr_css(5, "#p { border-top:0 solid; padding-top:0; }");
    let view = pbr_layout(5, &css);
    assert_p_vertical(&view, [10.0, 10.0, 21.0, 22.0, 22.0], &css);
    assert_child_spans(&view, &[(10.0, 11.0), (15.0, 17.0), (17.0, 18.0)], &css);
}

/// X14: O's border leaves the layout unchanged.
#[test]
fn pbr_x14_outer_border_leaves_the_layout_unchanged() {
    let control = pbr_layout(5, &pbr_css(5, ""));
    let css = pbr_css(5, "#o { border:2px solid; }");
    assert_same_view(&pbr_layout(5, &css), &control, &[], &css);
}
