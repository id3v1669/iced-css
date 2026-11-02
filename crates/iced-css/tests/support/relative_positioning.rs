//! REL: computed `position: relative` on P and on every Sequence child (B1, F and the
//! tails), the offset facts and the shifted `auto` containment.
//!
//! The RED part of this module (the helpers, R1, R2, the Legacy guard and E10b)
//! compiles against the predecessor API at ed315c83: it reads only normal-flow
//! accessors. The rows appended at FLIP also read `relative_offset()` and the
//! `offset` fact. Every literal is derived from `baseline_css()` (O content left 0,
//! width 100, top 10; P `hidden`, padding-top 1 and padding-bottom 1) by the REL
//! plan's exact-rational model; twin comparisons with the static control are
//! additional, never the only oracle.

use super::*;

/// The facade fixture: `baseline_css()`, the N7 leading rule, then the fragment.
fn rel_css(nodes: usize, fragment: &str) -> String {
    let leading = if nodes == 7 {
        leading_clear_source_rule(7)
    } else {
        ""
    };
    format!("{} {leading} {fragment}", baseline_css())
}

/// A box or an offset quadruple as (left, right, top, bottom).
type RelBox = (f32, f32, f32, f32);

fn rel_box(edges: PrincipalEdges) -> RelBox {
    (edges.left, edges.right, edges.top, edges.bottom)
}

fn rel_push(out: &mut Vec<(String, u64)>, name: &str, values: &[f32]) {
    for (index, value) in values.iter().enumerate() {
        out.push((format!("{name}.{index}"), u64::from(value.to_bits())));
    }
}

fn rel_push_box(out: &mut Vec<(String, u64)>, name: &str, value: RelBox) {
    rel_push(out, name, &[value.0, value.1, value.2, value.3]);
}

/// The normal-flow view of one layout: every published number, f32 values as bits.
/// Indices: `p.vertical` is P's border top, content top, content bottom, padding
/// bottom, border bottom and margin bottom; `p.horizontal` its used width, content
/// left and content right; each child box is (left, right, top, bottom) and `margin`
/// is (top, right, bottom, left).
fn rel_view(layout: &CapturedSiblingLayout<'_>) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let parent = layout.parent();
    let origin = parent.origin();
    let horizontal = parent.horizontal();
    let height = parent.height();
    let bottom = parent.bottom();
    rel_push(
        &mut out,
        "p.vertical",
        &[
            origin.border_top,
            origin.content_top,
            height.content_bottom,
            bottom.padding_bottom,
            bottom.border_bottom,
            bottom.margin_bottom,
        ],
    );
    rel_push(
        &mut out,
        "p.horizontal",
        &[
            horizontal.used.width,
            horizontal.content.left,
            horizontal.content.right,
        ],
    );
    for (index, child) in layout.children().iter().enumerate() {
        out.push((
            format!("c{index}.source"),
            u64::try_from(child.source_index()).unwrap(),
        ));
        rel_push_box(
            &mut out,
            &format!("c{index}.border"),
            rel_box(child.border()),
        );
        rel_push_box(
            &mut out,
            &format!("c{index}.padding"),
            rel_box(child.padding()),
        );
        rel_push_box(
            &mut out,
            &format!("c{index}.content"),
            rel_box(child.content()),
        );
        rel_push(
            &mut out,
            &format!("c{index}.margin"),
            &[
                child.used_margin_top(),
                child.used_margin_right(),
                child.used_margin_bottom(),
                child.used_margin_left(),
            ],
        );
        rel_push(
            &mut out,
            &format!("c{index}.used"),
            &[child.used_width(), child.used_height()],
        );
    }
    let placed = layout.sequence().placed_float();
    rel_push(
        &mut out,
        "float",
        &[placed.left, placed.right, placed.top, placed.bottom],
    );
    for (index, tail) in layout.sequence().tail().iter().enumerate() {
        match tail.clearance() {
            Some(value) => rel_push(&mut out, &format!("tail{index}.clearance"), &[value]),
            None => out.push((format!("tail{index}.no_clearance"), 1)),
        }
    }
    out.push((
        "work".to_owned(),
        u64::try_from(layout.receipt().work).unwrap(),
    ));
    out.push((
        "core_work".to_owned(),
        u64::try_from(layout.receipt().core.work).unwrap(),
    ));
    out
}

/// Runs the facts (which must succeed) and the facade under `limits`, then `observe`.
fn rel_with<T>(
    nodes: usize,
    css: &str,
    limits: SiblingLayoutLimits,
    observe: impl FnOnce(&CapturedSiblingLayout<'_>) -> T,
) -> Result<T, SiblingLayoutError> {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits())
            .unwrap_or_else(|error| panic!("{css}: producer error {error:?}"));
        out = Some(
            default_outer_layout(&facts, &projection.nodes()[0], 0.0, 100.0, 10.0, limits)
                .map(|layout| observe(&layout)),
        );
    });
    out.unwrap()
}

/// Success-asserting normal-flow view: panics with the typed error otherwise.
fn rel_layout(nodes: usize, css: &str) -> Vec<(String, u64)> {
    rel_with(nodes, css, layout_limits(), rel_view)
        .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"))
}

/// The view of `css` equals the view of its static control, bit for bit.
fn assert_rel_same(view: &[(String, u64)], control: &[(String, u64)], context: &str) {
    assert_eq!(
        view, control,
        "{context}: normal-flow view differs from the control"
    );
}

/// The N5 baseline literals (design section 6): P [10, 11, 22, 23, 23] with margin
/// bottom 23; B1 border (11, 12); F margin box [0, 10] x [16, 18]; B2 border (18, 19)
/// with clearance -1.
fn assert_rel_baseline(view: &[(String, u64)], context: &str) {
    for (name, value) in [
        ("p.vertical.0", 10.0_f32),
        ("p.vertical.1", 11.0),
        ("p.vertical.2", 22.0),
        ("p.vertical.3", 23.0),
        ("p.vertical.4", 23.0),
        ("p.vertical.5", 23.0),
        ("p.horizontal.1", 0.0),
        ("p.horizontal.2", 100.0),
        ("c0.border.2", 11.0),
        ("c0.border.3", 12.0),
        ("float.0", 0.0),
        ("float.1", 10.0),
        ("float.2", 16.0),
        ("float.3", 18.0),
        ("c2.border.2", 18.0),
        ("c2.border.3", 19.0),
        ("tail0.clearance.0", -1.0),
    ] {
        let actual = view
            .iter()
            .find(|(key, _)| key == name)
            .unwrap_or_else(|| panic!("{context}: no {name}"))
            .1;
        assert_eq!(
            actual,
            u64::from(value.to_bits()),
            "{context}: {name} is {}",
            f32::from_bits(u32::try_from(actual).unwrap())
        );
    }
}

/// The fact producer's error and every source's original index; panics, naming the
/// facade result, when the facts succeed.
fn rel_fact_error(nodes: usize, css: &str) -> (SiblingBoxFactsError, Vec<usize>) {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let originals = (0..nodes)
            .map(|source| projection.original_index(source).unwrap())
            .collect();
        match evaluate_sibling_box_facts(projection, display, sibling_fact_limits()) {
            Err(error) => out = Some((error, originals)),
            Ok(facts) => {
                let layout = default_outer_layout(
                    &facts,
                    &projection.nodes()[0],
                    0.0,
                    100.0,
                    10.0,
                    layout_limits(),
                )
                .map(|_| ());
                panic!("{css}: expected a fact error, got facts Ok and layout {layout:?}");
            }
        }
    });
    out.unwrap()
}

/// `error` is the producer's `Unsupported` refusal of the element longhand `name` at
/// `original`, with `reason`.
fn assert_rel_refusal(
    error: &SiblingBoxFactsError,
    expected: (usize, &str, BoxFactUnsupported),
    context: &str,
) {
    let (original, name, reason) = expected;
    assert!(
        matches!(
            error,
            SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Unsupported {
                original: actual,
                property,
                target: BoxFactTarget::Element,
                reason: actual_reason,
            }) if *actual == original
                && property.descriptor().name == name
                && *actual_reason == reason
        ),
        "{context}: expected {reason:?} at original {original} on {name}, got {error:?}"
    );
}

/// R1 (RED; M1 witness): a relatively positioned B1 is admitted and keeps the static
/// control's normal-flow view. Predecessor at ed315c83: `Captured(Unsupported {
/// projected: 2, property: "display/position/float", reason: DisplayOrFlow })`.
#[test]
fn rel_red_r1_relative_b1_is_admitted() {
    let css = rel_css(5, "#b1 { position:relative; top:5px; left:7px; }");
    let view = rel_layout(5, &css);
    assert_rel_same(&view, &rel_layout(5, &rel_css(5, "")), &css);
    assert_rel_baseline(&view, &css);
}

/// R2 (RED): a relatively positioned P is admitted and keeps the static control's
/// normal-flow view. Predecessor at ed315c83: the R1 error with `projected: 1`.
#[test]
fn rel_red_r2_relative_p_is_admitted() {
    let css = rel_css(5, "#p { position:relative; top:2px; right:3px; }");
    let view = rel_layout(5, &css);
    assert_rel_same(&view, &rel_layout(5, &rel_css(5, "")), &css);
    assert_rel_baseline(&view, &css);
}

/// Legacy guard (C-3; M28 witness): the Legacy entry never resolves offsets. A
/// Legacy batch with `#b1 { position:relative; top:10%; }` keeps its facts and the
/// Legacy bridge error `DisplayOrFlow` at projected 2, exactly as at ed315c83.
#[test]
fn rel_legacy_guard_relative_b1_keeps_display_or_flow() {
    let css = rel_css(5, "#b1 { position:relative; top:10%; }");
    sibling_fixture(5, &css, |projection, display| {
        let legacy_projection = prepare_clearance_source_projection(
            projection.host(),
            projection.snapshot(),
            ClearanceProjectionLimits {
                work: nz(36),
                retained_bytes: bound(),
            },
        )
        .unwrap();
        let legacy_facts = evaluate_clearance_box_facts(&legacy_projection, display, fact_limits())
            .unwrap_or_else(|error| panic!("{css}: Legacy facts error {error:?}"));
        let error = build_captured_clearance_with_parent_bottom(
            &legacy_facts,
            ClearanceOuterContentGeometry {
                outer: &legacy_projection.nodes()[0],
                content_left: 0.0,
                content_width: 100.0,
                content_top: 10.0,
            },
        )
        .map(|_| ())
        .unwrap_err();
        assert_eq!(
            error,
            ParentBottomError::Parent(ParentHeightError::Parent(ParentClearanceError::Captured(
                CapturedClearanceError::Unsupported {
                    projected: 2,
                    property: "display/position/float",
                    reason: CapturedClearanceUnsupported::DisplayOrFlow,
                }
            ))),
            "{css}"
        );
    });
}

/// E10b (C-2): offsets are resolved for every relative Sequence row, O included, so a
/// relative O with an `em` offset is a fact error before the facade's O fence (PTB keeps
/// O's percentage offsets; `percentage_offsets` FX12 covers them). Predecessor at
/// ed315c83: facts Ok and `Captured(Unsupported { projected: 0, property:
/// "display/position/float", reason: DisplayOrFlow })`.
#[test]
fn rel_red_e10b_relative_outer_offsets_are_resolved() {
    let css = rel_css(5, "#o { position:relative; top:1em; }");
    let (error, originals) = rel_fact_error(5, &css);
    assert_rel_refusal(
        &error,
        (originals[0], "top", BoxFactUnsupported::RelativeLength),
        &css,
    );
}

// FLIP rows: they read `relative_offset()` and the `offset` fact.

/// Child selectors by projected source, starting at source 2.
const REL_CHILDREN: [&str; 5] = ["b1", "f", "b2", "b3", "b4"];

/// Layout-error-asserting helper: panics on a layout.
fn rel_error(nodes: usize, css: &str) -> SiblingLayoutError {
    match rel_with(nodes, css, layout_limits(), |_| ()) {
        Ok(()) => panic!("{css}: expected a layout error, got a layout"),
        Err(error) => error,
    }
}

const REL_ZERO: RelBox = (0.0, 0.0, 0.0, 0.0);

fn rel_offset_box(offset: iced_css_core::formatting::relative::RelativeOffsets) -> RelBox {
    (offset.left, offset.right, offset.top, offset.bottom)
}

/// Used offsets of P (index 0), then of every child in source order.
fn rel_offsets(layout: &CapturedSiblingLayout<'_>) -> Vec<RelBox> {
    std::iter::once(rel_offset_box(layout.parent().relative_offset()))
        .chain(
            layout
                .children()
                .iter()
                .map(|child| rel_offset_box(child.relative_offset())),
        )
        .collect()
}

/// Success-asserting view and offsets.
fn rel_full(nodes: usize, css: &str) -> (Vec<(String, u64)>, Vec<RelBox>) {
    rel_with(nodes, css, layout_limits(), |layout| {
        (rel_view(layout), rel_offsets(layout))
    })
    .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"))
}

/// Offsets of every box: `value` at `index` (0 = P, 1 = B1, ...) and +0 elsewhere.
fn rel_only(nodes: usize, index: usize, value: RelBox) -> Vec<RelBox> {
    let mut out = vec![REL_ZERO; nodes - 1];
    out[index] = value;
    out
}

fn rel_bits(boxes: &[RelBox]) -> Vec<[u32; 4]> {
    boxes
        .iter()
        .map(|b| [b.0.to_bits(), b.1.to_bits(), b.2.to_bits(), b.3.to_bits()])
        .collect()
}

/// Offsets bit for bit, as (left, right, top, bottom) per box.
fn assert_rel_offsets(actual: &[RelBox], expected: &[RelBox], context: &str) {
    assert_eq!(
        rel_bits(actual),
        rel_bits(expected),
        "{context}: offsets {actual:?}"
    );
}

/// An admitted row: the normal-flow view equals the static control's, and every
/// offset is literal.
fn assert_rel_row(nodes: usize, fragment: &str, control: &str, expected: &[RelBox]) {
    let css = rel_css(nodes, fragment);
    let (view, offsets) = rel_full(nodes, &css);
    let (control_view, control_offsets) = rel_full(nodes, &rel_css(nodes, control));
    assert_rel_same(&view, &control_view, &css);
    assert_rel_offsets(&control_offsets, &vec![REL_ZERO; nodes - 1], control);
    assert_rel_offsets(&offsets, expected, &css);
}

/// The Sequence fact rows, the fact receipt and every source's original index; the
/// producer must succeed.
fn rel_facts(
    nodes: usize,
    css: &str,
) -> (Vec<ClearanceBoxFact>, ClearanceBoxFactsReceipt, Vec<usize>) {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits())
            .unwrap_or_else(|error| panic!("{css}: producer error {error:?}"));
        let rows = (0..nodes)
            .map(|source| *facts.row(source).unwrap())
            .collect();
        let originals = (0..nodes)
            .map(|source| projection.original_index(source).unwrap())
            .collect();
        out = Some((rows, facts.receipt().evaluation, originals));
    });
    out.unwrap()
}

fn rel_px(pixels: f32, origin: BoxFactOrigin) -> BoxLengthFact {
    BoxLengthFact::Absolute(AbsoluteBoxFact {
        pixels,
        rounded_to_zero: false,
        origin,
    })
}

const REL_AUTO: BoxLengthFact = BoxLengthFact::Auto {
    origin: BoxFactOrigin::Initial,
};

/// R1 (M9, M10 witness): B1's offsets are (7, -7, 5, -5); P, F and B2 stay +0.
#[test]
fn rel_r1_b1_offsets_are_published() {
    assert_rel_row(
        5,
        "#b1 { position:relative; top:5px; left:7px; }",
        "",
        &rel_only(5, 1, (7.0, -7.0, 5.0, -5.0)),
    );
}

/// R2 (M22 witness): `left: auto` gives -right; P's offsets are (-3, 3, 2, -2).
#[test]
fn rel_r2_p_offsets_are_published() {
    assert_rel_row(
        5,
        "#p { position:relative; top:2px; right:3px; }",
        "",
        &rel_only(5, 0, (-3.0, 3.0, 2.0, -2.0)),
    );
}

/// E1 (M21 witness): a relative F keeps clearance (-1 on B2) and `placed_float()`
/// [0, 10] x [16, 18]; F's offsets are (-3, 3, 4, -4).
#[test]
fn rel_e1_float_offsets_keep_clearance_and_placement() {
    let css = rel_css(5, "#f { position:relative; left:-3px; top:4px; }");
    let (view, _) = rel_full(5, &css);
    assert_rel_baseline(&view, &css);
    assert_rel_row(
        5,
        "#f { position:relative; left:-3px; top:4px; }",
        "",
        &rel_only(5, 2, (-3.0, 3.0, 4.0, -4.0)),
    );
}

/// E2: under an `ltr` P `left` wins over `right`, and `top` wins over `bottom`.
#[test]
fn rel_e2_ltr_overconstraint_left_and_top_win() {
    assert_rel_row(
        5,
        "#b2 { position:relative; left:4px; right:9px; top:1px; bottom:6px; }",
        "",
        &rel_only(5, 3, (4.0, -4.0, 1.0, -1.0)),
    );
}

/// E3 (M8 witness): an `rtl` P governs its children, so `right` wins.
#[test]
fn rel_e3_rtl_parent_governs_children() {
    assert_rel_row(
        5,
        "#p { direction:rtl; } #b2 { position:relative; left:4px; right:9px; }",
        "#p { direction:rtl; }",
        &rel_only(5, 3, (-9.0, 9.0, 0.0, 0.0)),
    );
}

/// E4 (M7 witness): O's direction governs P.
#[test]
fn rel_e4_outer_direction_governs_p() {
    assert_rel_row(
        5,
        "#o { direction:rtl; } #p { direction:ltr; position:relative; left:4px; right:9px; }",
        "#o { direction:rtl; } #p { direction:ltr; }",
        &rel_only(5, 0, (-9.0, 9.0, 0.0, 0.0)),
    );
}

/// E4b: P's direction, not O's, governs the children.
#[test]
fn rel_e4b_parent_direction_governs_children() {
    assert_rel_row(
        5,
        "#o { direction:rtl; } #p { direction:ltr; } #b1 { position:relative; left:4px; right:9px; }",
        "#o { direction:rtl; } #p { direction:ltr; }",
        &rel_only(5, 1, (4.0, -4.0, 0.0, 0.0)),
    );
}

/// E5 (C-5): `-0px` and `0px` give +0 offsets, and the producer stores +0 facts.
#[test]
fn rel_e5_zero_offsets_are_positive_zero() {
    let fragment = "#b1 { position:relative; top:-0px; left:0px; }";
    assert_rel_row(5, fragment, "", &[REL_ZERO; 4]);
    let css = rel_css(5, fragment);
    let (rows, _, _) = rel_facts(5, &css);
    let offset = rows[2].offset.unwrap();
    for side in [0, 3] {
        let BoxLengthFact::Absolute(fact) = offset[side] else {
            panic!("{css}: offset {side} is {:?}", offset[side]);
        };
        assert_eq!(fact.pixels.to_bits(), 0x00000000, "{css}: offset {side}");
        assert_eq!(
            fact.origin,
            BoxFactOrigin::Specified,
            "{css}: offset {side}"
        );
    }
}

/// E6: the extreme finite offsets stay finite and exact.
#[test]
fn rel_e6_extreme_offsets_stay_finite() {
    let max = f32::from_bits(0x7f7fffff);
    assert_rel_row(
        5,
        "#b1 { position:relative; left:340282346638528859811704183484516925440px; \
         top:-340282346638528859811704183484516925440px; }",
        "",
        &rel_only(5, 1, (max, -max, -max, max)),
    );
}

/// E7 (M14 witness): `inherit` copies a relative parent's resolved offset.
#[test]
fn rel_e7_inherit_copies_a_relative_parent_offset() {
    let fragment = "#p { position:relative; top:3px; } #b1 { position:relative; top:inherit; }";
    let css = rel_css(5, fragment);
    let (rows, _, originals) = rel_facts(5, &css);
    let inherited = BoxFactOrigin::Inherited {
        original_parent: originals[1],
    };
    assert_eq!(
        rows[2].offset,
        Some([rel_px(3.0, inherited), REL_AUTO, REL_AUTO, REL_AUTO]),
        "{css}: offset[0] inherited, offset[1], offset[2] and offset[3] initial"
    );
    let mut expected = rel_only(5, 0, (0.0, 0.0, 3.0, -3.0));
    expected[1] = (0.0, 0.0, 3.0, -3.0);
    assert_rel_row(5, fragment, "", &expected);
}

/// E8 (M11 witness): a static row publishes and converts no offset (ISP reads a static
/// O's or P's specified offsets only for a relative child's `inherit`), so `%`, `em` and
/// lengths are ignored: facts Ok with no offset row and no extra conversion.
#[test]
fn rel_e8_static_rows_never_read_offsets() {
    let fragment = "#b1 { top:10%; left:1em; right:3px; }";
    let css = rel_css(5, fragment);
    let (rows, receipt, _) = rel_facts(5, &css);
    let (_, baseline, _) = rel_facts(5, &rel_css(5, ""));
    assert!(rows.iter().all(|row| row.offset.is_none()), "{css}");
    assert_eq!(receipt.numeric_calls, baseline.numeric_calls, "{css}");
    let (view, _) = rel_full(5, &css);
    assert_rel_baseline(&view, &css);
    assert_rel_row(5, fragment, "", &[REL_ZERO; 4]);
}

/// E9 (M12 witness): an absolutely positioned row never reads its offsets; the facade
/// refuses it as before.
#[test]
fn rel_e9_absolute_rows_never_read_offsets() {
    let css = rel_css(5, "#b1 { position:absolute; top:10%; }");
    let (rows, _, _) = rel_facts(5, &css);
    assert!(rows.iter().all(|row| row.offset.is_none()), "{css}");
    let error = rel_error(5, &css);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Captured(CapturedClearanceError::Unsupported {
                projected: 2,
                property: "display/position/float",
                reason: CapturedClearanceUnsupported::DisplayOrFlow,
            })
        ),
        "{css}: {error:?}"
    );
}

/// E10 (M3 witness): O stays static in the facade.
#[test]
fn rel_e10_relative_outer_stays_refused() {
    let css = rel_css(5, "#o { position:relative; top:1px; }");
    let error = rel_error(5, &css);
    assert!(
        matches!(
            error,
            SiblingLayoutError::Captured(CapturedClearanceError::Unsupported {
                projected: 0,
                property: "display/position/float",
                reason: CapturedClearanceUnsupported::DisplayOrFlow,
            })
        ),
        "{css}: {error:?}"
    );
}

/// E11 (a refusal until OAH; it witnessed PTB M11, REL M15 while the producer refused
/// every offset percentage, and PTB M5 while a child's was refused): P's `top:10%` under
/// the Auto O resolves against O's used content height 13 (OAH, the case of `ptb` V9):
/// P's offsets (0, 0, 1.3, -1.3); a child's resolves after P's bottom (PTA, `ptb` A1).
#[test]
fn rel_e11_parent_percentage_offset_resolves() {
    assert_rel_row(
        5,
        "#p { position:relative; top:10%; }",
        "",
        &rel_only(5, 0, (0.0, 0.0, 1.3, -1.3)),
    );
}

/// E12: an `em` offset on a relative F is refused.
#[test]
fn rel_e12_relative_length_offset_is_refused() {
    let css = rel_css(5, "#f { position:relative; left:1em; }");
    let (error, originals) = rel_fact_error(5, &css);
    assert_rel_refusal(
        &error,
        (originals[3], "left", BoxFactUnsupported::RelativeLength),
        &css,
    );
}

/// E13 (M13 witness until ISP): `inherit` from a static parent copies its computed offset
/// (CSS2.2 6.2.1): B1's `top:inherit` under the static P's `top:3px` is 3px, so B1's
/// offsets are (0, 0, 3, -3) and the normal flow is the static control's.
#[test]
fn rel_e13_inherit_from_a_static_parent_copies_its_computed_offset() {
    assert_rel_row(
        5,
        "#p { top:3px; } #b1 { position:relative; top:inherit; }",
        "#p { top:3px; }",
        &rel_only(5, 1, (0.0, 0.0, 3.0, -3.0)),
    );
}

/// E14: an unrepresentable offset gives the `margin-top` twin's conversion error.
#[test]
fn rel_e14_unrepresentable_offset_is_the_margin_error() {
    let value = "400000000000000000000000000000000000000px";
    let css = rel_css(5, &format!("#b1 {{ position:relative; top:{value}; }}"));
    let twin = rel_css(5, &format!("#b1 {{ margin-top:{value}; }}"));
    let (error, originals) = rel_fact_error(5, &css);
    let (twin_error, _) = rel_fact_error(5, &twin);
    let (
        SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Length {
            original,
            property,
            error: length_error,
        }),
        SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Length {
            original: twin_original,
            property: twin_property,
            error: twin_length_error,
        }),
    ) = (&error, &twin_error)
    else {
        panic!("{css}: {error:?} against {twin_error:?}");
    };
    assert_eq!(*original, originals[2], "{css}");
    assert_eq!(twin_original, original, "{css}");
    assert_eq!(property.descriptor().name, "top", "{css}");
    assert_eq!(twin_property.descriptor().name, "margin-top", "{twin}");
    assert_eq!(length_error, twin_length_error, "{css}");
}

/// C1: under P `auto` with height 20 a shifted B2 border (20, 21) and margin box
/// (17, 24) stay inside P's content box [11, 31].
#[test]
fn rel_c1_auto_parent_contains_a_shifted_b2() {
    let fragment = "#p { overflow:auto; height:20px; } #b2 { position:relative; top:2px; }";
    let css = rel_css(5, fragment);
    let (view, _) = rel_full(5, &css);
    for (name, value) in [
        ("p.vertical.0", 10.0_f32),
        ("p.vertical.1", 11.0),
        ("p.vertical.2", 31.0),
        ("p.vertical.3", 32.0),
        ("p.vertical.4", 32.0),
        ("c2.border.2", 18.0),
        ("c2.border.3", 19.0),
    ] {
        let actual = view.iter().find(|(key, _)| key == name).unwrap().1;
        assert_eq!(actual, u64::from(value.to_bits()), "{css}: {name}");
    }
    assert_rel_row(
        5,
        fragment,
        "#p { overflow:auto; height:20px; }",
        &rel_only(5, 3, (0.0, 0.0, 2.0, -2.0)),
    );
}

fn assert_rel_overflow(fragment: &str, source: usize) {
    let css = rel_css(5, fragment);
    let error = rel_error(5, &css);
    assert!(
        matches!(error, SiblingLayoutError::ParentAutoOverflow { source: actual } if actual == source),
        "{css}: {error:?}"
    );
}

/// C2a (M16, M17 witness): B2's shifted margin bottom 22 + 1 = 23 leaves P's content
/// box (bottom 22) while its shifted border box (19, 20) stays inside.
#[test]
fn rel_c2a_shifted_margin_box_overflows() {
    assert_rel_overflow(
        "#p { overflow:auto; } #b2 { position:relative; top:1px; }",
        4,
    );
}

/// C2b: B2 shifted up by 1 has margin box (14, 21), inside.
#[test]
fn rel_c2b_shifted_margin_box_inside() {
    assert_rel_row(
        5,
        "#p { overflow:auto; } #b2 { position:relative; top:-1px; }",
        "#p { overflow:auto; }",
        &rel_only(5, 3, (0.0, 0.0, -1.0, 1.0)),
    );
}

/// C3 (M16 witness): B1's shifted border right 101 > 100.
#[test]
fn rel_c3_horizontal_shift_overflows() {
    assert_rel_overflow(
        "#p { overflow:auto; } #b1 { position:relative; left:1px; }",
        2,
    );
}

/// C4a: F's shifted margin box [-1, 9] leaves P's content box.
#[test]
fn rel_c4a_float_shift_overflows() {
    assert_rel_overflow(
        "#p { overflow:auto; } #f { position:relative; left:-1px; }",
        3,
    );
}

/// C4b: F's shifted margin box [1, 11] stays inside.
#[test]
fn rel_c4b_float_shift_inside() {
    assert_rel_row(
        5,
        "#p { overflow:auto; } #f { position:relative; left:1px; }",
        "#p { overflow:auto; }",
        &rel_only(5, 2, (1.0, -1.0, 0.0, 0.0)),
    );
}

/// C4c (C-1; M18 witness): F with margin-left 2 has border [2, 12] and margin box
/// [0, 12]; shifted by -1 the border [1, 11] stays inside and the margin box [-1, 11]
/// leaves P's content box.
#[test]
fn rel_c4c_float_margin_box_shift_overflows() {
    let css = rel_css(5, "#p { overflow:auto; } #f { margin-left:2px; }");
    let (view, _) = rel_full(5, &css);
    for (name, value) in [
        ("c1.border.0", 2.0_f32),
        ("c1.border.1", 12.0),
        ("float.0", 0.0),
        ("float.1", 12.0),
    ] {
        let actual = view.iter().find(|(key, _)| key == name).unwrap().1;
        assert_eq!(actual, u64::from(value.to_bits()), "{css}: {name}");
    }
    assert_rel_overflow(
        "#p { overflow:auto; } #f { margin-left:2px; position:relative; left:-1px; }",
        3,
    );
}

/// C5 (M19 witness): P's own offset is not applied to the containment test.
#[test]
fn rel_c5_parent_offset_moves_its_content_box_too() {
    assert_rel_row(
        5,
        "#p { overflow:auto; position:relative; top:50px; left:50px; }",
        "#p { overflow:auto; }",
        &rel_only(5, 0, (50.0, -50.0, 50.0, -50.0)),
    );
}

/// C6a (M20 witness): 100 + 2^-18 is a tie and rounds to even 100 (`42c80000`), so
/// B1's shifted border box stays inside; the exact sum would not.
#[test]
fn rel_c6a_tie_rounds_into_the_content_box() {
    let offset = f32::from_bits(0x36800000);
    assert_eq!(f64::from(offset), 0.000003814697265625);
    assert_eq!((100.0_f32 + offset).to_bits(), 0x42c80000);
    assert_rel_row(
        5,
        "#p { overflow:auto; } #b1 { position:relative; left:0.000003814697265625px; }",
        "#p { overflow:auto; }",
        &rel_only(5, 1, (offset, -offset, 0.0, 0.0)),
    );
}

/// C6b: 100 + 2^-17 is `42c80001` > 100.
#[test]
fn rel_c6b_next_float_overflows() {
    assert_eq!(
        (100.0_f32 + f32::from_bits(0x37000000)).to_bits(),
        0x42c80001
    );
    assert_rel_overflow(
        "#p { overflow:auto; } #b1 { position:relative; left:0.00000762939453125px; }",
        2,
    );
}

/// C7: under P `hidden` a large shift is admitted.
#[test]
fn rel_c7_hidden_parent_admits_any_shift() {
    assert_rel_row(
        5,
        "#b2 { position:relative; top:100px; }",
        "",
        &rel_only(5, 3, (0.0, 0.0, 100.0, -100.0)),
    );
}

/// CR-L1 (M27 witness): the border step, not the padding step, overflows. Outer
/// content left -MAX; B1's content left is exactly -MAX, its zero padding copies it,
/// and -MAX - MAX overflows to -inf at the left border step.
#[test]
fn rel_cr_l1_border_step_publication_witness() {
    let max = "340282346638528859811704183484516925440px";
    let css = rel_css(
        5,
        &format!("#b1 {{ width:1px; margin-left:-{max}; border-left:{max} solid; }}"),
    );
    sibling_fixture(5, &css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits())
            .unwrap_or_else(|error| panic!("{css}: producer error {error:?}"));
        assert_eq!(
            facts.row(2).unwrap().padding[3]
                .absolute()
                .unwrap()
                .pixels
                .to_bits(),
            0
        );
        let error = default_outer_layout(
            &facts,
            &projection.nodes()[0],
            f32::from_bits(0xff7fffff),
            100.0,
            10.0,
            layout_limits(),
        )
        .map(|_| ())
        .unwrap_err();
        assert!(
            matches!(
                error,
                SiblingLayoutError::ChildBorderPublication {
                    source: 2,
                    stage: iced_css::evaluation::sibling_layout::ChildBorderStage::Left,
                }
            ),
            "{css}: {error:?}"
        );
    });
}

/// M-A: N5, N6 and N7, every source 1..N-1 and each side alone at 3px (60 rows).
#[test]
fn rel_m_a_every_source_and_side() {
    let mut rows = 0;
    for nodes in [5, 6, 7] {
        for index in 0..nodes - 1 {
            let selector = if index == 0 {
                "p"
            } else {
                REL_CHILDREN[index - 1]
            };
            for (side, value) in [
                ("top", (0.0, 0.0, 3.0, -3.0)),
                ("right", (-3.0, 3.0, 0.0, 0.0)),
                ("bottom", (0.0, 0.0, -3.0, 3.0)),
                ("left", (3.0, -3.0, 0.0, 0.0)),
            ] {
                assert_rel_row(
                    nodes,
                    &format!("#{selector} {{ position:relative; {side}:3px; }}"),
                    "",
                    &rel_only(nodes, index, value),
                );
                rows += 1;
            }
        }
    }
    assert_eq!(rows, 60);
}

/// M-B: under P `auto`, an overflowing and a contained shift for each N5 child.
#[test]
fn rel_m_b_containment_pairs() {
    for (fragment, source) in [
        ("#b1 { position:relative; top:-1px; }", 2),
        ("#b1 { position:relative; left:-1px; }", 2),
        ("#f { position:relative; top:5px; }", 3),
        ("#b2 { position:relative; top:1px; }", 4),
        ("#b2 { position:relative; left:1px; }", 4),
    ] {
        assert_rel_overflow(&format!("#p {{ overflow:auto; }} {fragment}"), source);
    }
    for (fragment, index, value) in [
        (
            "#b1 { position:relative; top:1px; }",
            1,
            (0.0, 0.0, 1.0, -1.0),
        ),
        (
            "#f { position:relative; top:-5px; }",
            2,
            (0.0, 0.0, -5.0, 5.0),
        ),
        (
            "#b2 { position:relative; top:-1px; }",
            3,
            (0.0, 0.0, -1.0, 1.0),
        ),
    ] {
        assert_rel_row(
            5,
            &format!("#p {{ overflow:auto; }} {fragment}"),
            "#p { overflow:auto; }",
            &rel_only(5, index, value),
        );
    }
}

/// Legacy rows never carry offsets (C-3).
#[test]
fn rel_legacy_rows_have_no_offsets() {
    let css = rel_css(
        5,
        "#p { position:relative; top:1px; } #b1 { position:relative; top:10%; }",
    );
    sibling_fixture(5, &css, |projection, display| {
        let legacy_projection = prepare_clearance_source_projection(
            projection.host(),
            projection.snapshot(),
            ClearanceProjectionLimits {
                work: nz(36),
                retained_bytes: bound(),
            },
        )
        .unwrap();
        let legacy = evaluate_clearance_box_facts(&legacy_projection, display, fact_limits())
            .unwrap_or_else(|error| panic!("{css}: Legacy facts error {error:?}"));
        for projected in 0..5 {
            assert_eq!(
                legacy.row(projected).unwrap().offset,
                None,
                "{css}: {projected}"
            );
        }
    });
}

/// The work fixture: a relative P and relative children (B1, F, B2).
const REL_WORK_FRAGMENT: &str = "#p { position:relative; top:1px; } \
                                 #b1 { position:relative; left:2px; } \
                                 #f { position:relative; top:-1px; } \
                                 #b2 { position:relative; bottom:1px; }";

fn rel_fact_run(
    nodes: usize,
    css: &str,
    limits: SiblingBoxFactsLimits,
) -> (
    Result<ClearanceBoxFactsReceipt, SiblingBoxFactsError>,
    usize,
) {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        out = Some((
            evaluate_sibling_box_facts(projection, display, limits)
                .map(|facts| facts.receipt().evaluation),
            display.bundle().rule_count(),
        ));
    });
    out.unwrap()
}

/// Facts work (M23 witness): `35 * n + 2 + r`, exact and one short.
#[test]
fn rel_facts_work_is_exact_with_relative_rows() {
    for nodes in [5, 6, 7] {
        let css = rel_css(nodes, REL_WORK_FRAGMENT);
        let (receipt, rules) = rel_fact_run(nodes, &css, sibling_fact_limits());
        let work = 35 * nodes + 2 + rules;
        assert_eq!(receipt.unwrap().work, work, "N{nodes}");
        let mut exact = sibling_fact_limits();
        exact.evaluation.work = nz(work);
        assert!(rel_fact_run(nodes, &css, exact).0.is_ok(), "N{nodes} exact");
        let mut short = exact;
        short.evaluation.work = nz(work - 1);
        assert!(
            matches!(
                rel_fact_run(nodes, &css, short).0,
                Err(SiblingBoxFactsError::Evaluation(
                    ClearanceBoxFactsError::Work
                ))
            ),
            "N{nodes} one short"
        );
    }
}

/// Facts numeric allowance (M24 witness): `22 * n * numeric.work`, exact and one
/// short.
#[test]
fn rel_facts_numeric_allowance_is_exact_with_relative_rows() {
    for nodes in [5, 6, 7] {
        let css = rel_css(nodes, REL_WORK_FRAGMENT);
        let allowance = 22 * nodes * 100_000;
        let (receipt, _) = rel_fact_run(nodes, &css, sibling_fact_limits());
        assert_eq!(receipt.unwrap().numeric_work, allowance, "N{nodes}");
        let mut exact = sibling_fact_limits();
        exact.evaluation.numeric_work = nz(allowance);
        assert!(rel_fact_run(nodes, &css, exact).0.is_ok(), "N{nodes} exact");
        let mut short = exact;
        short.evaluation.numeric_work = nz(allowance - 1);
        assert!(
            matches!(
                rel_fact_run(nodes, &css, short).0,
                Err(SiblingBoxFactsError::Evaluation(
                    ClearanceBoxFactsError::NumericWork
                ))
            ),
            "N{nodes} one short"
        );
    }
}

/// Core and facade work (M25, M26 witness): exact and one short at N5, N6 and N7 with
/// a relative P and relative children.
#[test]
fn rel_work_thresholds_with_relative_boxes() {
    for (nodes, facade, core) in [(5, 3284, 2141), (6, 4231, 2791), (7, 5178, 3442)] {
        assert_eq!(
            facade,
            752 + 32 * nodes + 542 * (nodes - 2) + 373 * (nodes - 3)
        );
        assert_eq!(
            core,
            1062 + 12 * nodes
                + nodes * (nodes - 1) / 2
                + 333 * (nodes - 4)
                + 76 * (nodes - 2)
                + 224 * (nodes - 3)
        );
        let css = rel_css(nodes, REL_WORK_FRAGMENT);
        let limits = |work: usize, core_work: usize| {
            let mut limits = layout_limits();
            limits.work = nz(work);
            limits.core.work = nz(core_work);
            limits
        };
        let receipt = rel_with(nodes, &css, limits(facade, core), |layout| {
            (layout.receipt().work, layout.receipt().core.work)
        })
        .unwrap_or_else(|error| panic!("N{nodes}: expected a layout, got {error:?}"));
        assert_eq!(receipt, (facade, core), "N{nodes}");
        assert!(
            matches!(
                rel_with(nodes, &css, limits(facade - 1, core), |_| ()),
                Err(SiblingLayoutError::Work)
            ),
            "N{nodes} facade one short"
        );
        assert!(
            matches!(
                rel_with(nodes, &css, limits(facade, core - 1), |_| ()),
                Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
            ),
            "N{nodes} core one short"
        );
    }
}
