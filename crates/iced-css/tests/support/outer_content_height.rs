//! OCH: P's percentage `height`, `min-height`, `max-height`, `top` and `bottom` under an O
//! whose computed `height` is not `auto` resolve against O's used content height supplied
//! by the host through `build_captured_sibling_layout_with_outer_height` (CSS2.2 10.5,
//! 10.7, 9.3.2).
//!
//! A child module of `percentage_offsets` (design section 6): it reuses the `pct`, `ppm`,
//! `pht` and `ptb` helpers (`pct_css`, `pct_view`, `assert_pct_view`, `pct_lit`, `pct_y`,
//! `pht_refusal`, `pht_error_in`, `PCT_OUTER`, ...) through `use super::*`. The RED part
//! (the helpers, OC0, OC1, I2 and the Legacy row) compiles against the PTA commit 0d51bef3
//! with the RED stub S0: it reads only published geometry and errors as `Debug` text.
//! Fixture: `baseline_css()` (O content left 0, width 100, top 10, O `height` auto unless
//! a row sets it; P `hidden` with padding-top and padding-bottom 1, so P's content top is
//! 11; B1 [11, 12] with margin-bottom 4; F [16, 18]; B2 [18, 19] with margins 3). Under
//! an explicit P of used height `v` P's content bottom is f32(11 + v) and its padding,
//! border and margin bottoms are one more, +0 and +0. Each admitted row compares the
//! listed literals and every other published number with its twin (each percentage
//! replaced by the value it resolves to, under the same O and host value), bit for bit.

use super::*;
use iced_css::evaluation::sibling_layout::build_captured_sibling_layout_with_outer_height;

/// `pct_with` through the new entry with O's content geometry `outer` and O's used
/// content height `height` from the host (the only `ClearanceOuterContentGeometry`
/// literal of this module).
fn och_with<T>(
    nodes: usize,
    css: &str,
    outer: (f32, f32, f32),
    height: Option<f32>,
    limits: SiblingLayoutLimits,
    observe: impl FnOnce(&CapturedSiblingLayout<'_>) -> T,
) -> Result<T, SiblingLayoutError> {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits())
            .unwrap_or_else(|error| {
                panic!("{css}: producer error {}", pct_sequence_describe(&error))
            });
        let (content_left, content_width, content_top) = outer;
        out = Some(
            build_captured_sibling_layout_with_outer_height(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left,
                    content_width,
                    content_top,
                },
                height,
                limits,
            )
            .map(|layout| observe(&layout)),
        );
    });
    out.unwrap()
}

/// The N5 view of `fragment` through the new entry; a layout is required.
fn och_layout(fragment: &str, height: Option<f32>) -> Vec<(String, f32)> {
    let css = pct_css(5, fragment);
    och_with(5, &css, PCT_OUTER, height, layout_limits(), pct_view)
        .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"))
}

/// The `Debug` text of the N5 layout error of `fragment` through the new entry with O's
/// content geometry `outer`; a layout is a failure.
fn och_error_in(fragment: &str, outer: (f32, f32, f32), height: Option<f32>) -> String {
    let css = pct_css(5, fragment);
    match och_with(5, &css, outer, height, layout_limits(), |_| ()) {
        Ok(()) => panic!("{css}: expected a layout error, got a layout"),
        Err(error) => format!("{error:?}"),
    }
}

/// `och_error_in` with `PCT_OUTER`.
fn och_error(fragment: &str, height: Option<f32>) -> String {
    och_error_in(fragment, PCT_OUTER, height)
}

/// An admitted N5 row through the new entry: `fragment` with the host value `height`
/// gives the `expected` literals and every other number of its `twin` (the same O and
/// `height`), bit for bit.
fn och_row(fragment: &str, twin: &str, height: Option<f32>, expected: &[(String, f32)]) {
    let view = och_layout(fragment, height);
    assert_pct_view(
        &view,
        &och_layout(twin, height),
        expected,
        &pct_css(5, fragment),
    );
}

/// OC0 (RED guard, review C-7; M14, M17 witness): with `None` the new entry equals the
/// old one and both give the literal texts: under O's `height:200px`, P's `height:50%`
/// and P's ignored `bottom:10%` are `OuterHeightBasis` at P (`height`, `bottom`: the H10
/// and PB9 texts), and so is P's inherited `height:50%` under O's `height:50%` (O's
/// percentage is explicit: O is the captured root); the baseline layout is bit-equal (P's
/// used content height 11). Passes at RED (the stub forwards) and after OCH.
#[test]
fn och_red_guard_oc0_none_keeps_the_old_entry() {
    for (fragment, property) in [
        ("#o { height:200px; } #p { height:50%; }", "height"),
        (
            "#o { height:200px; } #p { position:relative; top:1px; bottom:10%; }",
            "bottom",
        ),
        ("#o { height:50%; } #p { height:inherit; }", "height"),
    ] {
        let css = pct_css(5, fragment);
        let expected = pht_refusal(1, property, "OuterHeightBasis");
        assert_eq!(pht_error_in(&css, PCT_OUTER), expected, "{css}: old entry");
        assert_eq!(och_error(fragment, None), expected, "{css}: new entry");
    }
    let css = pct_css(5, "");
    assert_pct_view(
        &och_layout("", None),
        &pct_layout(5, &css),
        &[pct_lit("p.used_height", 11.0)],
        &css,
    );
}

/// OC1 (RED; M1, M2, M3 witness): under O's `height:200px; max-height:150px` and the
/// host's 150, P's `height:50%` is 75: P content [11, 86], padding, border and margin
/// bottom 87 (O's row 200 gives 100, O's width 100 gives 50, O's content top 10 gives 5);
/// every other number is the `height:75px` twin's. Predecessor (the RED stub):
/// `Captured(Unsupported { projected: 1, property: "height", reason: OuterHeightBasis })`.
#[test]
fn och_red_oc1_parent_height_percentage_of_the_host_height() {
    let expected = [
        pct_lit("p.used_height", 75.0),
        pct_lit("p.content_bottom", 86.0),
        pct_lit("p.padding_bottom", 87.0),
        pct_lit("p.border_bottom", 87.0),
        pct_lit("p.margin_bottom", 87.0),
    ];
    och_row(
        "#o { height:200px; max-height:150px; } #p { height:50%; }",
        "#o { height:200px; max-height:150px; } #p { height:75px; }",
        Some(150.0),
        &expected,
    );
}

/// I2 (PTA code review I-2, design Q-12): under an Auto P a later child's admission error
/// precedes an earlier child's deferred unrepresentable `top`: under P's
/// `min-height:<f32::MAX>px`, B1's `top:200%` and B2's `overflow:scroll` give
/// `ChildOverflow` at projected 4 (old entry). Passes at RED and after OCH.
#[test]
fn och_red_guard_i2_later_child_admission_precedes_a_deferred_top() {
    let fragment = format!(
        "#p {{ min-height:{PCT_F32_MAX_PX}; }} #b1 {{ position:relative; top:200%; }} \
         #b2 {{ overflow:scroll; }}"
    );
    let css = pct_css(5, &fragment);
    assert_eq!(
        pht_error_in(&css, PCT_OUTER),
        pht_refusal(4, "overflow", "ChildOverflow"),
        "{css}"
    );
}

/// Legacy (design section 6; no Legacy entry takes the host value): the Legacy facts
/// refuse P's `height:50%` and `max-height:50%` under O's `height:200px` with
/// `PercentageLength` at P's original 3, as before OCH. Passes at RED and after OCH.
#[test]
fn och_red_guard_legacy_refuses_the_parent_percentages() {
    for (fragment, name) in [
        ("#o { height:200px; } #p { height:50%; }", "height"),
        ("#o { height:200px; } #p { max-height:50%; }", "max-height"),
    ] {
        let css = pct_css(5, fragment);
        let (error, originals) = pct_legacy_error(&css);
        let error = error.unwrap_or_else(|| panic!("{css}: Legacy facts Ok"));
        assert_eq!(
            pct_describe(&error),
            pct_refusal(originals[1], name, BoxFactUnsupported::PercentageLength),
            "{css}"
        );
        assert_eq!(originals, vec![4, 3, 0, 1, 2], "{css}: Legacy originals");
    }
}

// OCH FLIP rows: they need P's resolution against the host value.

/// The `Debug` text of `Percentage { source: 1, property, error }`.
fn och_percentage(property: &str, error: &str) -> String {
    format!("Percentage {{ source: 1, property: \"{property}\", error: {error} }}")
}

/// OC2: under O's `height:200px` and the host's 200, P's `min-height:50%` is 100: the
/// 10.7 rerun over the tentative 11 gives P's used content height 100, content bottom 111.
#[test]
fn och_oc2_parent_min_height_percentage() {
    let expected = [
        pct_lit("p.used_height", 100.0),
        pct_lit("p.content_bottom", 111.0),
    ];
    och_row(
        "#o { height:200px; } #p { min-height:50%; }",
        "#o { height:200px; } #p { min-height:100px; }",
        Some(200.0),
        &expected,
    );
}

/// OC3: P's `max-height:5%` of the host's 200 is 10 and clamps P's `height:20px`: used
/// content height 10, content bottom 21.
#[test]
fn och_oc3_parent_max_height_percentage() {
    let expected = [
        pct_lit("p.used_height", 10.0),
        pct_lit("p.content_bottom", 21.0),
    ];
    och_row(
        "#o { height:200px; } #p { height:20px; max-height:5%; }",
        "#o { height:200px; } #p { height:20px; max-height:10px; }",
        Some(200.0),
        &expected,
    );
}

/// OC4 (M8, M9, M13 witness): P's own offsets against the host's 200 (O's direction,
/// `top` wins): `top:10%` gives 20/-20; `bottom:10%` gives -20/20; with `top:1px` the
/// ignored `bottom:10%` is resolved (20) and `top` wins, 1/-1; with P's own
/// `height:20px`, `top:10%` is still 20 (P's height would give 2).
#[test]
fn och_oc4_parent_offset_percentages() {
    for (fragment, twin, top, bottom) in [
        (
            "#o { height:200px; } #p { position:relative; top:10%; }",
            "#o { height:200px; } #p { position:relative; top:20px; }",
            20.0,
            -20.0,
        ),
        (
            "#o { height:200px; } #p { position:relative; bottom:10%; }",
            "#o { height:200px; } #p { position:relative; bottom:20px; }",
            -20.0,
            20.0,
        ),
        (
            "#o { height:200px; } #p { position:relative; top:1px; bottom:10%; }",
            "#o { height:200px; } #p { position:relative; top:1px; bottom:20px; }",
            1.0,
            -1.0,
        ),
        (
            "#o { height:200px; } #p { height:20px; position:relative; top:10%; }",
            "#o { height:200px; } #p { height:20px; position:relative; top:20px; }",
            20.0,
            -20.0,
        ),
    ] {
        let expected = [
            pct_lit("p.offset.top", top),
            pct_lit("p.offset.bottom", bottom),
        ];
        och_row(fragment, twin, Some(200.0), &expected);
    }
}

/// OC4e (review C-1; M18 witness): only P's `top` and `bottom` take the host basis: P's
/// `left:10%` of O's content width 100 is 10 (`right` -10) and P's `top:10%` of the
/// host's 200 is 20 (`bottom` -20).
#[test]
fn och_oc4e_parent_left_keeps_the_width_basis() {
    let expected = [
        pct_lit("p.offset.left", 10.0),
        pct_lit("p.offset.right", -10.0),
        pct_lit("p.offset.top", 20.0),
        pct_lit("p.offset.bottom", -20.0),
    ];
    och_row(
        "#o { height:200px; } #p { position:relative; left:10%; top:10%; }",
        "#o { height:200px; } #p { position:relative; left:10px; top:20px; }",
        Some(200.0),
        &expected,
    );
}

/// OC5: the chain reaches the children: P's `height:50%` of 200 is 100 (content bottom
/// 111); B1's `top:10%` of P's 100 is 10 (`bottom` -10); F's `height:10%` is 10, F
/// [16, 26]; B2 clears F: [26, 27] (O's 200 as the children's basis would give 20).
#[test]
fn och_oc5_parent_resolution_reaches_the_children() {
    let mut expected = vec![
        pct_lit("p.used_height", 100.0),
        pct_lit("p.content_bottom", 111.0),
        pct_lit("c0.offset.top", 10.0),
        pct_lit("c0.offset.bottom", -10.0),
        pct_lit("c1.used_height", 10.0),
        pct_lit("float.top", 16.0),
        pct_lit("float.bottom", 26.0),
    ];
    expected.extend(pct_y(1, 16.0, 26.0));
    expected.extend(pct_y(2, 26.0, 27.0));
    och_row(
        "#o { height:200px; } #p { height:50%; } #b1 { position:relative; top:10%; } \
         #f { height:10%; }",
        "#o { height:200px; } #p { height:100px; } #b1 { position:relative; top:10px; } \
         #f { height:10px; }",
        Some(200.0),
        &expected,
    );
}

/// OC6 (Q-4; M4 witness): under an `auto` O the host's value is not read: P's
/// `height:50%` is used as `auto` (P's used content height 11; reading 200 would give
/// 100), also under O's `min-height:200px` (H10c), and P's `top:10%` is
/// `IndefiniteOffsetBasis`.
#[test]
fn och_oc6_auto_outer_height_ignores_the_host_height() {
    for fragment in [
        "#p { height:50%; }",
        "#o { min-height:200px; } #p { height:50%; }",
    ] {
        assert_pct_view(
            &och_layout(fragment, Some(200.0)),
            &och_layout(fragment, None),
            &[pct_lit("p.used_height", 11.0)],
            &pct_css(5, fragment),
        );
    }
    assert_eq!(
        och_error("#p { position:relative; top:10%; }", Some(200.0)),
        pht_refusal(1, "top", "IndefiniteOffsetBasis")
    );
}

/// OC7 (PH-11 (2), Q-5): O's `height:50%` is explicit (O is the captured root), so P's
/// `height:50%` of the host's 300 is 150: content bottom 161, padding bottom 162.
#[test]
fn och_oc7_outer_percentage_height_is_explicit() {
    let expected = [
        pct_lit("p.used_height", 150.0),
        pct_lit("p.content_bottom", 161.0),
        pct_lit("p.padding_bottom", 162.0),
    ];
    och_row(
        "#o { height:50%; } #p { height:50%; }",
        "#o { height:50%; } #p { height:150px; }",
        Some(300.0),
        &expected,
    );
}

/// OC8 (Q-6; M5, M6, M7, M19, M20 witness): a host value that is not finite and
/// nonnegative is `Percentage { source: 1, property, error: InvalidContext }` with the
/// first of P's percentages (NaN, infinity and -1 for `height`; `max-height` before
/// `bottom`; `bottom`); -0 is valid (P's `height:50%` is +0, content bottom 11); without
/// a P percentage the value is not read (equal to `None`).
#[test]
fn och_oc8_an_invalid_host_height() {
    let height = "#o { height:200px; } #p { height:50%; }";
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        assert_eq!(
            och_error(height, Some(value)),
            och_percentage("height", "InvalidContext"),
            "{value}"
        );
    }
    assert_eq!(
        och_error(
            "#o { height:200px; } #p { max-height:50%; position:relative; bottom:10%; }",
            Some(-1.0)
        ),
        och_percentage("max-height", "InvalidContext")
    );
    assert_eq!(
        och_error(
            "#o { height:200px; } #p { position:relative; bottom:10%; }",
            Some(f32::NAN)
        ),
        och_percentage("bottom", "InvalidContext")
    );
    let expected = [
        pct_lit("p.used_height", 0.0),
        pct_lit("p.content_bottom", 11.0),
    ];
    och_row(
        height,
        "#o { height:200px; } #p { height:0px; }",
        Some(-0.0),
        &expected,
    );
    let fragment = "#o { height:200px; }";
    assert_pct_view(
        &och_layout(fragment, Some(f32::NAN)),
        &och_layout(fragment, None),
        &[],
        &pct_css(5, fragment),
    );
}

/// OC9 (Q-9; M10, M12 witness): under the host's `f32::MAX` each `200%` is
/// unrepresentable; the first failing property in the order `height`, `min-height`,
/// `max-height`, `top`, `bottom` is reported, an ignored `bottom` included (PB-2).
#[test]
fn och_oc9_unrepresentable_parent_percentages_and_their_order() {
    for (fragment, property) in [
        ("#p { height:200%; }", "height"),
        ("#p { height:200%; min-height:200%; }", "height"),
        ("#p { min-height:200%; max-height:200%; }", "min-height"),
        (
            "#p { max-height:200%; position:relative; top:200%; }",
            "max-height",
        ),
        ("#p { position:relative; top:0px; bottom:200%; }", "bottom"),
        ("#p { position:relative; top:200%; bottom:200%; }", "top"),
    ] {
        let fragment = format!("#o {{ height:200px; }} {fragment}");
        assert_eq!(
            och_error(&fragment, Some(f32::MAX)),
            och_percentage(property, "Unrepresentable"),
            "{fragment}"
        );
    }
}

/// OC10 (PH-11 (3); M11 witness): P's step follows P's `overflow` test, and precedes P's
/// held horizontal error (O's content width NaN) and every child error; a successful
/// resolution lets the child error through.
#[test]
fn och_oc10_the_failure_position() {
    assert_eq!(
        och_error(
            "#o { height:200px; } #p { overflow:visible; height:50%; }",
            Some(f32::NAN)
        ),
        pht_refusal(1, "overflow", "ParentOverflow")
    );
    assert_eq!(
        och_error_in(
            "#o { height:200px; } #p { height:200%; }",
            (0.0, f32::NAN, 10.0),
            Some(f32::MAX)
        ),
        och_percentage("height", "Unrepresentable")
    );
    assert_eq!(
        och_error(
            "#o { height:200px; } #p { position:relative; top:200%; } #b1 { overflow:scroll; }",
            Some(f32::MAX)
        ),
        och_percentage("top", "Unrepresentable")
    );
    assert_eq!(
        och_error(
            "#o { height:200px; } #p { height:50%; } #b1 { overflow:scroll; }",
            Some(200.0)
        ),
        pht_refusal(2, "overflow", "ChildOverflow")
    );
}

/// OC11 (M15 witness): one rounding: P's `height:0.001%` of the host's 11 is the nearest
/// f32 of 0.00011, bits `38e6afcd` (the pipeline f32(f32(f32(p) * 11) / 100) gives
/// `38e6afce`); content bottom bits `41300073`, padding bottom `41400073`; equal to the
/// `0.00011px` twin.
#[test]
fn och_oc11_one_rounding_parent_height() {
    let expected = [
        pct_lit("p.used_height", f32::from_bits(0x38e6afcd)),
        pct_lit("p.content_bottom", f32::from_bits(0x41300073)),
        pct_lit("p.padding_bottom", f32::from_bits(0x41400073)),
    ];
    och_row(
        "#o { height:11px; } #p { height:0.001%; }",
        "#o { height:11px; } #p { height:0.00011px; }",
        Some(11.0),
        &expected,
    );
}

/// OC12 (PA-13): the children's basis is P's constrained used height: P's `height:50%`
/// of 200 is 100, clamped by `max-height:30px` to 30, so B1's `top:50%` is 15 (`bottom`
/// -15; the unconstrained 100 would give 50).
#[test]
fn och_oc12_children_use_the_constrained_parent_height() {
    let expected = [
        pct_lit("p.used_height", 30.0),
        pct_lit("c0.offset.top", 15.0),
        pct_lit("c0.offset.bottom", -15.0),
    ];
    och_row(
        "#o { height:200px; } #p { height:50%; max-height:30px; } \
         #b1 { position:relative; top:50%; }",
        "#o { height:200px; } #p { height:30px; } #b1 { position:relative; top:15px; }",
        Some(200.0),
        &expected,
    );
}

/// OC13 (Q4=C): under P `auto` the resolved height bounds the containment: `height:5%`
/// of 200 gives P content [11, 21] and B2's margin bottom 22 leaves it
/// (`ParentAutoOverflow { source: 4 }`); `height:6%` gives content bottom 23 and a
/// layout equal to the `height:12px` twin.
#[test]
fn och_oc13_auto_parent_containment_with_a_resolved_height() {
    assert_eq!(
        och_error(
            "#o { height:200px; } #p { overflow:auto; height:5%; }",
            Some(200.0)
        ),
        "ParentAutoOverflow { source: 4 }"
    );
    let expected = [
        pct_lit("p.used_height", 12.0),
        pct_lit("p.content_bottom", 23.0),
    ];
    och_row(
        "#o { height:200px; } #p { overflow:auto; height:6%; }",
        "#o { height:200px; } #p { overflow:auto; height:12px; }",
        Some(200.0),
        &expected,
    );
}

/// The OCH work fragment: P's five percentages under O's `height:200px` with the host's
/// 200 (P's `height:10%` is 20, its limits 0 and 200, its offsets 0).
const OCH_WORK_FRAGMENT: &str = "#o { height:200px; } #p { height:10%; min-height:0%; \
                                 max-height:100%; position:relative; top:0%; bottom:0%; }";

/// Facade and core work with the host's value (M16 witness): exact and one short at N5,
/// N6 and N7 with PCT's, PPM's, PHT's and PTA's percentages and P's five percentages
/// resolved against the host's 200; the facade charge is `546 + 32N + 526(N-2) +
/// 371(N-3)`, as through the old entry; the layout equals the control with P's
/// percentages replaced by their values and the children relative without offsets.
#[test]
fn och_work_thresholds_with_the_host_height() {
    for (nodes, facade, core) in [(5, 3026, 1373), (6, 3955, 1687), (7, 4884, 2002)] {
        assert_eq!(
            facade,
            546 + 32 * nodes + 526 * (nodes - 2) + 371 * (nodes - 3)
        );
        let earlier = format!("{PCT_WORK_FRAGMENT} {PPM_WORK_FRAGMENT} {PHT_WORK_FRAGMENT}");
        let css = pct_css(
            nodes,
            &format!("{earlier} {PTA_WORK_FRAGMENT} {OCH_WORK_FRAGMENT}"),
        );
        let control = pct_css(
            nodes,
            &format!(
                "{earlier} #b1, #f, #b2, #b3, #b4 {{ position:relative; }} \
                 #o {{ height:200px; }} #p {{ height:20px; min-height:0px; \
                 max-height:200px; position:relative; top:0px; bottom:0px; }}"
            ),
        );
        let view = |css: &str| {
            och_with(
                nodes,
                css,
                PCT_OUTER,
                Some(200.0),
                layout_limits(),
                pct_view,
            )
            .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"))
        };
        assert_pct_view(&view(&css), &view(&control), &[], &css);
        let limits = |work: usize, core_work: usize| {
            let mut limits = layout_limits();
            limits.work = nz(work);
            limits.core.work = nz(core_work);
            limits
        };
        let receipt = och_with(
            nodes,
            &css,
            PCT_OUTER,
            Some(200.0),
            limits(facade, core),
            |layout| (layout.receipt().work, layout.receipt().core.work),
        )
        .unwrap_or_else(|error| panic!("N{nodes}: expected a layout, got {error:?}"));
        assert_eq!(receipt, (facade, core), "N{nodes}");
        assert!(
            matches!(
                och_with(
                    nodes,
                    &css,
                    PCT_OUTER,
                    Some(200.0),
                    limits(facade - 1, core),
                    |_| ()
                ),
                Err(SiblingLayoutError::Work)
            ),
            "N{nodes} facade one short"
        );
        assert!(
            matches!(
                och_with(
                    nodes,
                    &css,
                    PCT_OUTER,
                    Some(200.0),
                    limits(facade, core - 1),
                    |_| ()
                ),
                Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
            ),
            "N{nodes} core one short"
        );
    }
}

// EML: the carried OCH rows (OC-10) first; then the `em` length rows as a child module of
// this one (EML design section 6).

/// OCX1 (OC-10, OCH plan review F-1; MV witness): under an `auto` O the host value is not
/// read, so `Some(NAN)`, `Some(INFINITY)` and `Some(-1.0)` give the `None` view for P's
/// `height:50%` and for P's `min-height:50%; max-height:50%` under O's `min-height:200px`
/// (P's used content height 11 in both). Passes before and after EML.
#[test]
fn eml_red_guard_ocx1_an_invalid_host_height_under_an_auto_outer_is_not_read() {
    for fragment in [
        "#p { height:50%; }",
        "#o { min-height:200px; } #p { min-height:50%; max-height:50%; }",
    ] {
        let control = och_layout(fragment, None);
        for height in [f32::NAN, f32::INFINITY, -1.0] {
            assert_pct_view(
                &och_layout(fragment, Some(height)),
                &control,
                &[pct_lit("p.used_height", 11.0)],
                &format!("{fragment} with {height}"),
            );
        }
    }
}

/// OCX2 (OC-10): under an `auto` O, P's `top:10%` stays `IndefiniteOffsetBasis` with each
/// of the three invalid host values. Passes before and after EML.
#[test]
fn eml_red_guard_ocx2_parent_top_percentage_under_an_auto_outer_stays_refused() {
    for height in [f32::NAN, f32::INFINITY, -1.0] {
        assert_eq!(
            och_error("#p { position:relative; top:10%; }", Some(height)),
            pht_refusal(1, "top", "IndefiniteOffsetBasis"),
            "{height}"
        );
    }
}

// EML: `em` lengths in the Sequence facts against a host-supplied medium reuse the `pct`,
// `ppm`, `pht`, `ptb` and `och` helpers as a child module of this one (EML design
// section 6).
#[path = "em_lengths.rs"]
mod em_lengths;
