//! CBR: nonzero child borders (B1, F and the tails) and the unisolated parent in the
//! Sequence facade.
//!
//! These rows read `PrincipalChildBox::padding()`, `border()` and `content()` and the
//! core's published spans and records. Since CBR (ed315c83) `padding()` is the
//! child's padding box, which differs from `border()` exactly when the child has
//! borders. Every literal below is derived from `baseline_css()` (O content left 0,
//! width 100, top 10; P `hidden`, padding-top 1 and padding-bottom 1) by the CBR
//! plan's exact-rational model; twin comparisons are additional, never the only
//! oracle.

use super::*;

/// Child selectors by projected source, starting at source 2.
const CBR_CHILDREN: [&str; 5] = ["b1", "f", "b2", "b3", "b4"];

/// Box sides in edge-array order.
const CBR_SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

/// The work fixture (C-9): an unisolated P, a bordered B1, a bordered and padded F
/// and a bordered B2.
const CBR_WORK_FRAGMENT: &str = "#p { padding:0; } #b1 { border:1px solid; } \
                                 #f { border:1px solid; padding:1px; } \
                                 #b2 { border-bottom:2px solid; }";

/// The facade fixture: `baseline_css()`, the N7 leading rule, then the fragment.
fn cbr_css(nodes: usize, fragment: &str) -> String {
    let leading = if nodes == 7 {
        leading_clear_source_rule(7)
    } else {
        ""
    };
    format!("{} {leading} {fragment}", baseline_css())
}

/// A box as (left, right, top, bottom).
type CbrBox = (f32, f32, f32, f32);

fn cbr_box(edges: PrincipalEdges) -> CbrBox {
    (edges.left, edges.right, edges.top, edges.bottom)
}

/// Copied observations of one principal child.
#[derive(Debug, Clone, Copy)]
struct CbrChild {
    source: usize,
    border: CbrBox,
    padding: CbrBox,
    content: CbrBox,
    /// Used margins: top, right, bottom, left.
    margin: [f32; 4],
    used_width: f32,
    used_height: f32,
}

/// Copied observations of one successful layout.
#[derive(Debug, Clone)]
struct CbrView {
    /// P border top, content top, content bottom, padding bottom, border bottom and
    /// margin bottom.
    p_vertical: [f32; 6],
    /// P used width, content left and content right.
    p_horizontal: [f32; 3],
    children: Vec<CbrChild>,
    /// F's margin box.
    float: CbrBox,
    clearances: Vec<Option<f32>>,
    work: usize,
    core_work: usize,
}

fn cbr_bits(name: String, value: f32) -> (String, u64) {
    (name, u64::from(value.to_bits()))
}

fn cbr_count(name: String, value: usize) -> (String, u64) {
    (name, u64::try_from(value).unwrap())
}

fn cbr_box_bits(out: &mut Vec<(String, u64)>, name: &str, value: CbrBox) {
    for (side, coordinate) in [
        ("left", value.0),
        ("right", value.1),
        ("top", value.2),
        ("bottom", value.3),
    ] {
        out.push(cbr_bits(format!("{name}.{side}"), coordinate));
    }
}

impl CbrView {
    /// Every observed number by name, f32 values as bits; each child's padding box
    /// only when `padding` is set.
    fn fields(&self, padding: bool) -> Vec<(String, u64)> {
        let mut out = Vec::new();
        for (index, value) in self.p_vertical.iter().enumerate() {
            out.push(cbr_bits(format!("p.vertical.{index}"), *value));
        }
        for (index, value) in self.p_horizontal.iter().enumerate() {
            out.push(cbr_bits(format!("p.horizontal.{index}"), *value));
        }
        out.push(cbr_count("children".to_owned(), self.children.len()));
        for (index, child) in self.children.iter().enumerate() {
            out.push(cbr_count(format!("c{index}.source"), child.source));
            cbr_box_bits(&mut out, &format!("c{index}.border"), child.border);
            if padding {
                cbr_box_bits(&mut out, &format!("c{index}.padding"), child.padding);
            }
            cbr_box_bits(&mut out, &format!("c{index}.content"), child.content);
            for (side, value) in CBR_SIDES.iter().zip(child.margin) {
                out.push(cbr_bits(format!("c{index}.margin.{side}"), value));
            }
            out.push(cbr_bits(format!("c{index}.used_width"), child.used_width));
            out.push(cbr_bits(format!("c{index}.used_height"), child.used_height));
        }
        cbr_box_bits(&mut out, "float", self.float);
        for (index, clearance) in self.clearances.iter().enumerate() {
            match clearance {
                Some(value) => out.push(cbr_bits(format!("tail{index}.clearance"), *value)),
                None => out.push(cbr_count(format!("tail{index}.no_clearance"), 1)),
            }
        }
        out.push(cbr_count("work".to_owned(), self.work));
        out.push(cbr_count("core_work".to_owned(), self.core_work));
        out
    }
}

fn cbr_view(layout: &CapturedSiblingLayout<'_>) -> CbrView {
    let parent = layout.parent();
    let origin = parent.origin();
    let horizontal = parent.horizontal();
    let height = parent.height();
    let bottom = parent.bottom();
    let placed = layout.sequence().placed_float();
    CbrView {
        p_vertical: [
            origin.border_top,
            origin.content_top,
            height.content_bottom,
            bottom.padding_bottom,
            bottom.border_bottom,
            bottom.margin_bottom,
        ],
        p_horizontal: [
            horizontal.used.width,
            horizontal.content.left,
            horizontal.content.right,
        ],
        children: layout
            .children()
            .iter()
            .map(|child| CbrChild {
                source: child.source_index(),
                border: cbr_box(child.border()),
                padding: cbr_box(child.padding()),
                content: cbr_box(child.content()),
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
        float: (placed.left, placed.right, placed.top, placed.bottom),
        clearances: layout
            .sequence()
            .tail()
            .iter()
            .map(|tail| tail.clearance())
            .collect(),
        work: layout.receipt().work,
        core_work: layout.receipt().core.work,
    }
}

fn cbr_limits(work: usize, core_work: usize) -> SiblingLayoutLimits {
    let mut limits = layout_limits();
    limits.work = nz(work);
    limits.core.work = nz(core_work);
    limits
}

/// The layout result under `limits`; the producer must succeed.
fn cbr_run_with(
    nodes: usize,
    css: &str,
    limits: SiblingLayoutLimits,
) -> Result<CbrView, SiblingLayoutError> {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits())
            .unwrap_or_else(|error| panic!("{css}: producer error {error:?}"));
        out = Some(
            default_outer_layout(&facts, &projection.nodes()[0], 0.0, 100.0, 10.0, limits)
                .map(|layout| cbr_view(&layout)),
        );
    });
    out.unwrap()
}

/// Success-asserting helper: panics with the typed error otherwise.
fn cbr_layout(nodes: usize, css: &str) -> CbrView {
    cbr_run_with(nodes, css, layout_limits())
        .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"))
}

/// Layout-error-asserting helper: panics on a layout.
fn cbr_error(nodes: usize, css: &str) -> SiblingLayoutError {
    match cbr_run_with(nodes, css, layout_limits()) {
        Ok(view) => panic!("{css}: expected a layout error, got {view:?}"),
        Err(error) => error,
    }
}

/// P (border top, content top, content bottom, padding bottom, border bottom); P's
/// margins are zero, so its margin bottom equals its border bottom.
fn assert_p(view: &CbrView, expected: [f32; 5], context: &str) {
    let actual = [
        view.p_vertical[0],
        view.p_vertical[1],
        view.p_vertical[2],
        view.p_vertical[3],
        view.p_vertical[4],
    ];
    assert_eq!(
        actual.map(f32::to_bits),
        expected.map(f32::to_bits),
        "{context}: P {actual:?}"
    );
    assert_eq!(
        view.p_vertical[5].to_bits(),
        view.p_vertical[4].to_bits(),
        "{context}: P margin bottom"
    );
}

/// One child's border, padding and content boxes, in that order.
fn assert_child(view: &CbrView, source: usize, expected: [CbrBox; 3], context: &str) {
    let child = &view.children[source - 2];
    assert_eq!(child.source, source, "{context}");
    assert_eq!(
        [child.border, child.padding, child.content],
        expected,
        "{context}: source {source} border, padding and content boxes"
    );
}

/// Border spans (top, bottom) of every child in projected order.
fn assert_spans(view: &CbrView, expected: &[(f32, f32)], context: &str) {
    let actual: Vec<(f32, f32)> = view
        .children
        .iter()
        .map(|child| (child.border.2, child.border.3))
        .collect();
    assert_eq!(actual, expected, "{context}: border spans");
}

fn assert_cbr_profile(
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

/// Field-by-field equality of two named field lists.
fn assert_same_fields(actual: &[(String, u64)], expected: &[(String, u64)], context: &str) {
    assert_eq!(actual.len(), expected.len(), "{context}: field count");
    for ((name, value), (other, wanted)) in actual.iter().zip(expected) {
        assert_eq!(name, other, "{context}");
        assert_eq!(value, wanted, "{context}: {name}");
    }
}

// RED (plan section 4).

/// R1 / C1 (M1, M5, M7, M9 and M17 witness): B1's top border is admitted; B1's
/// padding and content start below it and every later box moves down by 2.
#[test]
fn cbr_red_c1_b1_top_border_is_admitted() {
    let css = cbr_css(5, "#b1 { border-top:2px solid; }");
    let view = cbr_layout(5, &css);
    assert_child(
        &view,
        2,
        [
            (0.0, 100.0, 11.0, 14.0),
            (0.0, 100.0, 13.0, 14.0),
            (0.0, 100.0, 13.0, 14.0),
        ],
        &css,
    );
    assert_spans(&view, &[(11.0, 14.0), (18.0, 20.0), (20.0, 21.0)], &css);
    assert_eq!(view.float, (0.0, 10.0, 18.0, 20.0), "{css}");
    assert_eq!(view.clearances, vec![Some(-1.0)], "{css}");
    assert_p(&view, [10.0, 11.0, 24.0, 25.0, 25.0], &css);
}

/// R2 / C5 (M3 witness): a P without border and padding is admitted; its content top
/// is its border top.
#[test]
fn cbr_red_c5_unisolated_parent_is_admitted() {
    let css = cbr_css(5, "#p { padding:0; }");
    let view = cbr_layout(5, &css);
    assert_p(&view, [10.0, 10.0, 21.0, 21.0, 21.0], &css);
    assert_spans(&view, &[(10.0, 11.0), (15.0, 17.0), (17.0, 18.0)], &css);
    assert_eq!(view.clearances, vec![Some(-1.0)], "{css}");
}

// Literal rows (plan section 3.2).

/// C2 (M15 and M16 witness): B1's side borders narrow its content band; its padding
/// band is the content band and its border band is P's content band.
#[test]
fn cbr_c2_b1_side_borders_narrow_the_content_band() {
    let css = cbr_css(5, "#b1 { border-left:3px solid; border-right:5px solid; }");
    let view = cbr_layout(5, &css);
    assert_child(
        &view,
        2,
        [
            (0.0, 100.0, 11.0, 12.0),
            (3.0, 95.0, 11.0, 12.0),
            (3.0, 95.0, 11.0, 12.0),
        ],
        &css,
    );
    assert_eq!(view.children[0].used_width, 92.0, "{css}");
    assert_spans(&view, &[(11.0, 12.0), (16.0, 18.0), (18.0, 19.0)], &css);
    assert_p(&view, [10.0, 11.0, 22.0, 23.0, 23.0], &css);
}

/// C2b (M23 witness): border and padding on both horizontal sides of B1.
#[test]
fn cbr_c2b_b1_side_borders_and_padding_stage_the_edges() {
    let css = cbr_css(
        5,
        "#b1 { border-left:3px solid; padding-left:2px; border-right:5px solid; \
         padding-right:1px; }",
    );
    let view = cbr_layout(5, &css);
    assert_child(
        &view,
        2,
        [
            (0.0, 100.0, 11.0, 12.0),
            (3.0, 95.0, 11.0, 12.0),
            (5.0, 94.0, 11.0, 12.0),
        ],
        &css,
    );
    assert_eq!(view.children[0].used_width, 89.0, "{css}");
    assert_p(&view, [10.0, 11.0, 22.0, 23.0, 23.0], &css);
}

/// C3 (M10, M11 and M12 witness): F's border grows its margin box on both axes.
#[test]
fn cbr_c3_left_float_border_grows_the_margin_box() {
    let css = cbr_css(5, "#f { border:1px solid; }");
    let view = cbr_layout(5, &css);
    assert_eq!(view.float, (0.0, 12.0, 16.0, 20.0), "{css}: margin box");
    assert_child(
        &view,
        3,
        [
            (0.0, 12.0, 16.0, 20.0),
            (1.0, 11.0, 17.0, 19.0),
            (1.0, 11.0, 17.0, 19.0),
        ],
        &css,
    );
    assert_eq!(view.children[1].used_width, 10.0, "{css}");
    assert_spans(&view, &[(11.0, 12.0), (16.0, 20.0), (20.0, 21.0)], &css);
    assert_eq!(view.clearances, vec![Some(1.0)], "{css}");
    assert_p(&view, [10.0, 11.0, 24.0, 25.0, 25.0], &css);
}

/// C3R (M13 witness): the right-float path.
#[test]
fn cbr_c3r_right_float_border_grows_the_margin_box() {
    let css = cbr_css(5, "#f { float:right; border:1px solid; }");
    let view = cbr_layout(5, &css);
    assert_eq!(view.float, (88.0, 100.0, 16.0, 20.0), "{css}: margin box");
    assert_child(
        &view,
        3,
        [
            (88.0, 100.0, 16.0, 20.0),
            (89.0, 99.0, 17.0, 19.0),
            (89.0, 99.0, 17.0, 19.0),
        ],
        &css,
    );
    assert_spans(&view, &[(11.0, 12.0), (16.0, 20.0), (20.0, 21.0)], &css);
    assert_p(&view, [10.0, 11.0, 24.0, 25.0, 25.0], &css);
}

/// C3b (M24 and M25 witness): F with border and padding on every side.
#[test]
fn cbr_c3b_left_float_border_and_padding() {
    let css = cbr_css(5, "#f { border:1px solid; padding:2px; }");
    let view = cbr_layout(5, &css);
    assert_eq!(view.float, (0.0, 16.0, 16.0, 24.0), "{css}: margin box");
    assert_child(
        &view,
        3,
        [
            (0.0, 16.0, 16.0, 24.0),
            (1.0, 15.0, 17.0, 23.0),
            (3.0, 13.0, 19.0, 21.0),
        ],
        &css,
    );
    assert_spans(&view, &[(11.0, 12.0), (16.0, 24.0), (24.0, 25.0)], &css);
    assert_eq!(view.clearances, vec![Some(5.0)], "{css}");
    assert_p(&view, [10.0, 11.0, 28.0, 29.0, 29.0], &css);
}

/// C3bR (M26 witness): the right-float path with border and padding.
#[test]
fn cbr_c3br_right_float_border_and_padding() {
    let css = cbr_css(5, "#f { float:right; border:1px solid; padding:2px; }");
    let view = cbr_layout(5, &css);
    assert_eq!(view.float, (84.0, 100.0, 16.0, 24.0), "{css}: margin box");
    assert_child(
        &view,
        3,
        [
            (84.0, 100.0, 16.0, 24.0),
            (85.0, 99.0, 17.0, 23.0),
            (87.0, 97.0, 19.0, 21.0),
        ],
        &css,
    );
    assert_spans(&view, &[(11.0, 12.0), (16.0, 24.0), (24.0, 25.0)], &css);
    assert_p(&view, [10.0, 11.0, 28.0, 29.0, 29.0], &css);
}

/// C4 (M6 and M9 witness): B2's bottom border extends its border box below its
/// content and padding.
#[test]
fn cbr_c4_b2_bottom_border_extends_the_border_box() {
    let css = cbr_css(5, "#b2 { border-bottom:4px solid; }");
    let view = cbr_layout(5, &css);
    assert_child(
        &view,
        4,
        [
            (0.0, 100.0, 18.0, 23.0),
            (0.0, 100.0, 18.0, 19.0),
            (0.0, 100.0, 18.0, 19.0),
        ],
        &css,
    );
    assert_spans(&view, &[(11.0, 12.0), (16.0, 18.0), (18.0, 23.0)], &css);
    assert_p(&view, [10.0, 11.0, 26.0, 27.0, 27.0], &css);
}

/// C4b (M27 witness): border and padding at B2's bottom.
#[test]
fn cbr_c4b_b2_bottom_border_and_padding() {
    let css = cbr_css(5, "#b2 { border-bottom:2px solid; padding-bottom:1px; }");
    let view = cbr_layout(5, &css);
    assert_child(
        &view,
        4,
        [
            (0.0, 100.0, 18.0, 22.0),
            (0.0, 100.0, 18.0, 20.0),
            (0.0, 100.0, 18.0, 19.0),
        ],
        &css,
    );
    assert_p(&view, [10.0, 11.0, 25.0, 26.0, 26.0], &css);
}

/// C6: a P without top isolation is admitted.
#[test]
fn cbr_c6_parent_without_top_isolation_is_admitted() {
    let css = cbr_css(5, "#p { padding-top:0; }");
    let view = cbr_layout(5, &css);
    assert_p(&view, [10.0, 10.0, 21.0, 22.0, 22.0], &css);
    assert_spans(&view, &[(10.0, 11.0), (15.0, 17.0), (17.0, 18.0)], &css);
}

/// C7a and C7b (M18 witness): the Q1 contact check reads the new-BFC tail's border
/// box, which includes its bottom border: contact passes, overlap is rejected.
#[test]
fn cbr_c7_new_bfc_tail_border_contact_and_overlap() {
    let css = cbr_css(
        5,
        "#b2 { clear:none; overflow:hidden; margin-top:-5px; border-bottom:4px solid; }",
    );
    let view = cbr_layout(5, &css);
    assert_child(
        &view,
        4,
        [
            (0.0, 100.0, 11.0, 16.0),
            (0.0, 100.0, 11.0, 12.0),
            (0.0, 100.0, 11.0, 12.0),
        ],
        &css,
    );
    assert_spans(&view, &[(11.0, 12.0), (16.0, 18.0), (11.0, 16.0)], &css);
    assert_eq!(view.clearances, vec![None], "{css}");
    assert_p(&view, [10.0, 11.0, 19.0, 20.0, 20.0], &css);
    let css = cbr_css(
        5,
        "#b2 { clear:none; overflow:hidden; margin-top:-5px; border-bottom:5px solid; }",
    );
    let error = cbr_error(5, &css);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 4,
                error: ClearanceOwnerError::NewBfcFloatContact,
            })
        ),
        "{css}: {error:?}"
    );
}

/// C7c (M28 witness; plan review PR-1): the Q1 contact check for a new-BFC B1 reads
/// its border box, which includes its bottom border: contact passes, overlap is
/// rejected.
#[test]
fn cbr_c7c_new_bfc_b1_border_contact_and_overlap() {
    let css = cbr_css(
        5,
        "#b1 { overflow:hidden; border-bottom:1px solid; margin-bottom:0; }",
    );
    let view = cbr_layout(5, &css);
    assert_eq!(view.float, (0.0, 10.0, 13.0, 15.0), "{css}: F margin box");
    let css = cbr_css(
        5,
        "#b1 { overflow:hidden; border-bottom:1px solid; margin-bottom:-1px; }",
    );
    let error = cbr_error(5, &css);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Core(ClearanceSequenceError::Placement {
                source: 2,
                error: ClearanceOwnerError::NewBfcFloatContact,
            })
        ),
        "{css}: {error:?}"
    );
}

/// C8 (M14 witness): F's left border offsets a negative left margin in the outer
/// extent; without the border the extent is negative.
#[test]
fn cbr_c8_float_border_offsets_a_negative_margin() {
    let css = cbr_css(
        5,
        "#f { width:0; margin-left:-1px; border-left:1px solid; }",
    );
    let view = cbr_layout(5, &css);
    assert_eq!(view.float, (0.0, 0.0, 16.0, 18.0), "{css}: margin box");
    assert_child(
        &view,
        3,
        [
            (-1.0, 0.0, 16.0, 18.0),
            (0.0, 0.0, 16.0, 18.0),
            (0.0, 0.0, 16.0, 18.0),
        ],
        &css,
    );
    assert_spans(&view, &[(11.0, 12.0), (16.0, 18.0), (18.0, 19.0)], &css);
    assert_p(&view, [10.0, 11.0, 22.0, 23.0, 23.0], &css);
    let control = cbr_css(5, "#f { width:0; margin-left:-1px; }");
    assert_cbr_profile(
        &cbr_error(5, &control),
        3,
        OwnerProfileReason::NegativeFloatOuterExtent,
        &control,
    );
}

/// C9a and C9b (M19 witness): P `auto` containment tests the margin box derived from
/// the bordered border box; the margin bottom edge on P's content bottom passes.
#[test]
fn cbr_c9_auto_containment_reads_the_bordered_margin_box() {
    let css = cbr_css(
        5,
        "#p { overflow:auto; height:12px; } #b2 { border-bottom:1px solid; }",
    );
    let view = cbr_layout(5, &css);
    assert_child(
        &view,
        4,
        [
            (0.0, 100.0, 18.0, 20.0),
            (0.0, 100.0, 18.0, 19.0),
            (0.0, 100.0, 18.0, 19.0),
        ],
        &css,
    );
    assert_p(&view, [10.0, 11.0, 23.0, 24.0, 24.0], &css);
    let css = cbr_css(
        5,
        "#p { overflow:auto; height:12px; } #b2 { border-bottom:2px solid; }",
    );
    let error = cbr_error(5, &css);
    assert!(
        matches!(error, SiblingLayoutError::ParentAutoOverflow { source: 4 }),
        "{css}: {error:?}"
    );
}

/// C10: an N6 tail (B3) with a top border and padding.
#[test]
fn cbr_c10_n6_tail_top_border_and_padding() {
    let css = cbr_css(6, "#b3 { border-top:2px solid; padding-top:1px; }");
    let view = cbr_layout(6, &css);
    assert_child(
        &view,
        5,
        [
            (0.0, 100.0, 22.0, 26.0),
            (0.0, 100.0, 24.0, 26.0),
            (0.0, 100.0, 25.0, 26.0),
        ],
        &css,
    );
    assert_spans(
        &view,
        &[(11.0, 12.0), (16.0, 18.0), (18.0, 19.0), (22.0, 26.0)],
        &css,
    );
    assert_eq!(view.clearances, vec![Some(-1.0), None], "{css}");
    assert_p(&view, [10.0, 11.0, 30.0, 31.0, 31.0], &css);
}

/// C11 (transfer twin): border 2 and padding 1 against padding 3 at B2's top; every
/// field but the padding boxes is equal.
#[test]
fn cbr_c11_border_and_padding_transfer_twin() {
    let bordered = cbr_css(5, "#b2 { border-top:2px solid; padding-top:1px; }");
    let padded = cbr_css(5, "#b2 { padding-top:3px; }");
    let view = cbr_layout(5, &bordered);
    let twin = cbr_layout(5, &padded);
    assert_same_fields(&view.fields(false), &twin.fields(false), &bordered);
    assert_child(
        &view,
        4,
        [
            (0.0, 100.0, 18.0, 22.0),
            (0.0, 100.0, 20.0, 22.0),
            (0.0, 100.0, 21.0, 22.0),
        ],
        &bordered,
    );
    assert_child(
        &twin,
        4,
        [
            (0.0, 100.0, 18.0, 22.0),
            (0.0, 100.0, 18.0, 22.0),
            (0.0, 100.0, 21.0, 22.0),
        ],
        &padded,
    );
    assert_p(&view, [10.0, 11.0, 25.0, 26.0, 26.0], &bordered);
}

/// C12 (M8 witness): B1's content top is one exact sum from its border top 2^24; a
/// staged sum would give 16777216.
#[test]
fn cbr_c12_vertical_joint_sum_witness() {
    let css = cbr_css(
        5,
        "#b1 { margin-top:16777205px; border-top:1px solid; padding-top:1px; }",
    );
    let view = cbr_run_with(5, &css, layout_limits())
        .unwrap_or_else(|error| panic!("{css}: expected Ok, got {error:?}"));
    let b1 = view.children[0];
    assert_eq!(
        [b1.border.2, b1.border.3].map(f32::to_bits),
        [0x4b80_0000, 0x4b80_0002],
        "{css}: border span"
    );
    assert_eq!(
        [b1.padding.2, b1.padding.3].map(f32::to_bits),
        [0x4b80_0000, 0x4b80_0002],
        "{css}: padding span"
    );
    assert_eq!(
        [b1.content.2, b1.content.3].map(f32::to_bits),
        [0x4b80_0001, 0x4b80_0002],
        "{css}: content span"
    );
    assert_eq!(
        view.p_vertical[2].to_bits(),
        0x4b80_0008,
        "{css}: P content bottom"
    );
}

/// C13 (M22 witness): the normal's border edge is staged from its padding edge; a
/// single exact sum, or `content - (p + b)` in f32, would give 3f7fffff.
#[test]
fn cbr_c13_horizontal_staged_witness() {
    let css = cbr_css(
        5,
        "#b1 { margin-left:0.999999940395355224609375px; \
         border-left:0.0000000298023223876953125px solid; \
         padding-left:0.0000000298023223876953125px; }",
    );
    let view = cbr_layout(5, &css);
    let b1 = view.children[0];
    assert_eq!(b1.used_width, 99.0, "{css}");
    assert_eq!(
        [b1.content.0, b1.content.1].map(f32::to_bits),
        [0x3f80_0000, 0x42c8_0000],
        "{css}: content band"
    );
    assert_eq!(
        [b1.padding.0, b1.border.0].map(f32::to_bits),
        [0x3f80_0000, 0x3f80_0000],
        "{css}: padding and border left"
    );
}

/// M-A: every child source and side (N5, N6, N7; 48 rows) admits a 1px border, and
/// the layout equals its padding twin (padding 1px on that side) field by field,
/// except the padding boxes: the bordered child's padding box is its content box and
/// the twin's is its border box; every other child's padding box is its border box.
#[test]
fn cbr_m_a_child_border_twins() {
    let mut rows = 0;
    for nodes in [5, 6, 7] {
        for source in 2..nodes {
            for side in CBR_SIDES {
                let child = CBR_CHILDREN[source - 2];
                let bordered = cbr_css(nodes, &format!("#{child} {{ border-{side}:1px solid; }}"));
                let padded = cbr_css(nodes, &format!("#{child} {{ padding-{side}:1px; }}"));
                let view = cbr_layout(nodes, &bordered);
                let twin = cbr_layout(nodes, &padded);
                assert_same_fields(&view.fields(false), &twin.fields(false), &bordered);
                for (index, (actual, other)) in view.children.iter().zip(&twin.children).enumerate()
                {
                    if index == source - 2 {
                        assert_eq!(actual.padding, actual.content, "{bordered}: padding box");
                        assert_eq!(other.padding, other.border, "{padded}: padding box");
                        assert_ne!(actual.padding, other.padding, "{bordered}: twin padding");
                    } else {
                        assert_eq!(actual.padding, actual.border, "{bordered}: child {index}");
                        assert_eq!(other.padding, other.border, "{padded}: child {index}");
                    }
                }
                rows += 1;
            }
        }
    }
    assert_eq!(rows, 48);
}

/// One M-C row: F's side, its border declarations, the margin box, the border,
/// padding and content boxes, B2's border span and P.
type CbrFloatRow = (
    &'static str,
    &'static str,
    CbrBox,
    [CbrBox; 3],
    (f32, f32),
    [f32; 5],
);

/// M-C (M12 and M13 witness): each decorated float path, left and right, with a 1px
/// border on one side or on all four.
#[test]
fn cbr_m_c_decorated_float_paths() {
    let rows: [CbrFloatRow; 10] = [
        (
            "left",
            "border-top:1px solid;",
            (0.0, 10.0, 16.0, 19.0),
            [
                (0.0, 10.0, 16.0, 19.0),
                (0.0, 10.0, 17.0, 19.0),
                (0.0, 10.0, 17.0, 19.0),
            ],
            (19.0, 20.0),
            [10.0, 11.0, 23.0, 24.0, 24.0],
        ),
        (
            "left",
            "border-right:1px solid;",
            (0.0, 11.0, 16.0, 18.0),
            [
                (0.0, 11.0, 16.0, 18.0),
                (0.0, 10.0, 16.0, 18.0),
                (0.0, 10.0, 16.0, 18.0),
            ],
            (18.0, 19.0),
            [10.0, 11.0, 22.0, 23.0, 23.0],
        ),
        (
            "left",
            "border-bottom:1px solid;",
            (0.0, 10.0, 16.0, 19.0),
            [
                (0.0, 10.0, 16.0, 19.0),
                (0.0, 10.0, 16.0, 18.0),
                (0.0, 10.0, 16.0, 18.0),
            ],
            (19.0, 20.0),
            [10.0, 11.0, 23.0, 24.0, 24.0],
        ),
        (
            "left",
            "border-left:1px solid;",
            (0.0, 11.0, 16.0, 18.0),
            [
                (0.0, 11.0, 16.0, 18.0),
                (1.0, 11.0, 16.0, 18.0),
                (1.0, 11.0, 16.0, 18.0),
            ],
            (18.0, 19.0),
            [10.0, 11.0, 22.0, 23.0, 23.0],
        ),
        (
            "left",
            "border:1px solid;",
            (0.0, 12.0, 16.0, 20.0),
            [
                (0.0, 12.0, 16.0, 20.0),
                (1.0, 11.0, 17.0, 19.0),
                (1.0, 11.0, 17.0, 19.0),
            ],
            (20.0, 21.0),
            [10.0, 11.0, 24.0, 25.0, 25.0],
        ),
        (
            "right",
            "border-top:1px solid;",
            (90.0, 100.0, 16.0, 19.0),
            [
                (90.0, 100.0, 16.0, 19.0),
                (90.0, 100.0, 17.0, 19.0),
                (90.0, 100.0, 17.0, 19.0),
            ],
            (19.0, 20.0),
            [10.0, 11.0, 23.0, 24.0, 24.0],
        ),
        (
            "right",
            "border-right:1px solid;",
            (89.0, 100.0, 16.0, 18.0),
            [
                (89.0, 100.0, 16.0, 18.0),
                (89.0, 99.0, 16.0, 18.0),
                (89.0, 99.0, 16.0, 18.0),
            ],
            (18.0, 19.0),
            [10.0, 11.0, 22.0, 23.0, 23.0],
        ),
        (
            "right",
            "border-bottom:1px solid;",
            (90.0, 100.0, 16.0, 19.0),
            [
                (90.0, 100.0, 16.0, 19.0),
                (90.0, 100.0, 16.0, 18.0),
                (90.0, 100.0, 16.0, 18.0),
            ],
            (19.0, 20.0),
            [10.0, 11.0, 23.0, 24.0, 24.0],
        ),
        (
            "right",
            "border-left:1px solid;",
            (89.0, 100.0, 16.0, 18.0),
            [
                (89.0, 100.0, 16.0, 18.0),
                (90.0, 100.0, 16.0, 18.0),
                (90.0, 100.0, 16.0, 18.0),
            ],
            (18.0, 19.0),
            [10.0, 11.0, 22.0, 23.0, 23.0],
        ),
        (
            "right",
            "border:1px solid;",
            (88.0, 100.0, 16.0, 20.0),
            [
                (88.0, 100.0, 16.0, 20.0),
                (89.0, 99.0, 17.0, 19.0),
                (89.0, 99.0, 17.0, 19.0),
            ],
            (20.0, 21.0),
            [10.0, 11.0, 24.0, 25.0, 25.0],
        ),
    ];
    for (f_side, declarations, margin, boxes, b2, p) in rows {
        let css = cbr_css(5, &format!("#f {{ float:{f_side}; {declarations} }}"));
        let view = cbr_layout(5, &css);
        assert_eq!(view.float, margin, "{css}: margin box");
        assert_child(&view, 3, boxes, &css);
        assert_eq!(
            (view.children[2].border.2, view.children[2].border.3),
            b2,
            "{css}: B2 border span"
        );
        assert_p(&view, p, &css);
    }
}

/// C-9 (M20 and M21 witness): the exact and one-short facade and core work at
/// N5/N6/N7 with bordered children and an unisolated P.
#[test]
fn cbr_work_thresholds_with_child_borders_and_an_unisolated_parent() {
    for (nodes, facade, core, p_bottom) in [
        (5, 2910, 1249, 29.0),
        (6, 3781, 1501, 34.0),
        (7, 4652, 1754, 38.0),
    ] {
        assert_eq!(
            facade,
            546 + 32 * nodes + 526 * (nodes - 2) + 313 * (nodes - 3)
        );
        assert_eq!(
            core,
            702 + 12 * nodes
                + nodes * (nodes - 1) / 2
                + 54 * (nodes - 4)
                + 61 * (nodes - 2)
                + 120 * (nodes - 3)
        );
        let css = cbr_css(nodes, CBR_WORK_FRAGMENT);
        let view = cbr_run_with(nodes, &css, cbr_limits(facade, core))
            .unwrap_or_else(|error| panic!("N{nodes}: expected a layout, got {error:?}"));
        assert_eq!((view.work, view.core_work), (facade, core), "N{nodes}");
        assert_p(&view, [10.0, 10.0, p_bottom, p_bottom, p_bottom], &css);
        assert_eq!(
            view.children[..3]
                .iter()
                .map(|child| (child.border.2, child.border.3))
                .collect::<Vec<_>>(),
            vec![(10.0, 13.0), (17.0, 23.0), (23.0, 26.0)],
            "N{nodes}"
        );
        assert!(
            matches!(
                cbr_run_with(nodes, &css, cbr_limits(facade - 1, core)),
                Err(SiblingLayoutError::Work)
            ),
            "N{nodes} facade one short"
        );
        assert!(
            matches!(
                cbr_run_with(nodes, &css, cbr_limits(facade, core - 1)),
                Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
            ),
            "N{nodes} core one short"
        );
    }
}
