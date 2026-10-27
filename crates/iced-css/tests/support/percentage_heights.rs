//! PHT: percentages of the containing block's height in `height`, `min-height` and
//! `max-height` on every Sequence row (CSS2.2 10.5, 10.7), with the carried PPM rows
//! K9, K9b (PM-9, PM-16) and KF (PM-16).
//!
//! A child module of `padding_min_max_percentages` (design section 6): it reuses the
//! `pct` and `ppm` private helpers (`pct_css`, `pct_view`, `assert_pct_row`,
//! `PPM_WORK_FRAGMENT`, ...) through `use super::*`. The RED part of this module (K9,
//! K9b, KF, the guard rows and H1-H3) compiles against the predecessor API at db5db111:
//! it reads only published geometry and errors. The rows appended at FLIP also read the
//! new `min_height` fact type and the `OuterHeightBasis` reason. Every literal is derived
//! from `baseline_css()` (O content left 0, width 100, top 10, O `height` auto; P
//! `hidden` with padding-top and padding-bottom of 1); each admitted row also compares
//! every other published number with a control and with its twin (the same CSS with
//! each percentage replaced by the value it resolves to, or by `auto`, 0 or `none` for
//! a percentage that takes the indefinite rule), bit for bit.

use super::*;

/// The E20 control: P's explicit `height:20px`. P content [11, 31], padding and border
/// bottom 32, used height 20; the children as in the baseline (B1 [11, 12], F [16, 18],
/// B2 [18, 19] with clearance -1).
const PHT_E20: &str = "#p { height:20px; }";

/// H1 (RED; M1 witness): B1's `height:10%` of P's explicit used content height 20 is 2
/// (the width basis would give 10): B1 [11, 13], used height 2; F [17, 19]; B2 [19, 20]
/// with clearance max(19 - 20, 17 - 20) = -1; P as E20. Predecessor at db5db111:
/// `Evaluation(Unsupported { original: 0, property: height, target: Element, reason:
/// PercentageLength })`.
#[test]
fn pht_red_h1_child_height_percentage_of_an_explicit_parent() {
    let mut expected = vec![
        pct_lit("c0.used_height", 2.0),
        pct_lit("float.top", 17.0),
        pct_lit("float.bottom", 19.0),
        pct_lit("tail0.clearance", -1.0),
    ];
    expected.extend(pct_y(0, 11.0, 13.0));
    expected.extend(pct_y(1, 17.0, 19.0));
    expected.extend(pct_y(2, 19.0, 20.0));
    assert_pct_row(
        "#p { height:20px; } #b1 { height:10%; }",
        PHT_E20,
        "#p { height:20px; } #b1 { height:2px; }",
        &expected,
    );
}

/// H2 (RED; M2 witness): B1's `min-height:50%` of 20 is 10 (the width basis would give
/// 50): B1's height 1 is raised to 10, B1 [11, 21]; F [25, 27]; B2 [27, 28]. Predecessor
/// at db5db111: the same refusal with `property: min-height`.
#[test]
fn pht_red_h2_child_min_height_percentage_of_an_explicit_parent() {
    let mut expected = vec![
        pct_lit("c0.used_height", 10.0),
        pct_lit("float.top", 25.0),
        pct_lit("float.bottom", 27.0),
    ];
    expected.extend(pct_y(0, 11.0, 21.0));
    expected.extend(pct_y(1, 25.0, 27.0));
    expected.extend(pct_y(2, 27.0, 28.0));
    assert_pct_row(
        "#p { height:20px; } #b1 { min-height:50%; }",
        PHT_E20,
        "#p { height:20px; } #b1 { min-height:10px; }",
        &expected,
    );
}

/// H3 (RED; M3, M29 witness): F's `max-height:5%` of 20 is 1 (the width basis would
/// give 5, no clamp): F's height 2 is clamped to 1, F [16, 17]; B2's clearance
/// max(17 - 19, 16 - 19) = -2, B2 [17, 18]. Predecessor at db5db111: `original: 1`,
/// `property: max-height`.
#[test]
fn pht_red_h3_float_max_height_percentage_of_an_explicit_parent() {
    let mut expected = vec![
        pct_lit("c1.used_height", 1.0),
        pct_lit("float.bottom", 17.0),
        pct_lit("tail0.clearance", -2.0),
    ];
    expected.extend(pct_y(1, 16.0, 17.0));
    expected.extend(pct_y(2, 17.0, 18.0));
    assert_pct_row(
        "#p { height:20px; } #f { max-height:5%; }",
        PHT_E20,
        "#p { height:20px; } #f { max-height:1px; }",
        &expected,
    );
}

/// The layout error of each N5 `fragment` under P's `width:<f32::MAX>px` must be
/// `Percentage { source: 2, property, error: Unrepresentable }`.
fn pht_assert_unrepresentable(cases: &[(&str, &str)], parent: &str) {
    for &(fragment, property) in cases {
        let css = pct_css(5, &format!("{parent} {fragment}"));
        assert_eq!(
            format!("{:?}", pct_error_in(&css, PCT_OUTER)),
            format!("Percentage {{ source: 2, property: \"{property}\", error: Unrepresentable }}"),
            "{css}"
        );
    }
}

/// K9 (carried PPM plan review K-1, PM-9, PM-16; M30, M31 witness): the six longhands
/// appended by PPM resolve in the fixed order `padding-top`, `padding-right`,
/// `padding-bottom`, `padding-left`, `min-width`, `max-width`: under P's
/// `width:<f32::MAX>px` each `200%` is unrepresentable and the first in that order is
/// reported (the reviewer's validated four cases, verbatim). Passes at RED and after
/// PHT.
#[test]
fn pht_red_guard_k9_order_of_the_appended_longhands() {
    pht_assert_unrepresentable(
        &[
            (
                "#b1 { padding-top:200%; padding-left:200%; min-width:200%; max-width:200%; }",
                "padding-top",
            ),
            (
                "#b1 { padding-left:200%; min-width:200%; max-width:200%; }",
                "padding-left",
            ),
            ("#b1 { min-width:200%; max-width:200%; }", "min-width"),
            ("#b1 { max-width:200%; }", "max-width"),
        ],
        &format!("#p {{ width:{PCT_F32_MAX_PX}; }}"),
    );
}

/// K9b (design Q-8 (b); M32 witness): the two added cases that pin the whole order of
/// the six: `padding-right` before `padding-bottom`, `padding-bottom` before
/// `padding-left`. Passes at RED and after PHT.
#[test]
fn pht_red_guard_k9b_order_of_the_appended_paddings() {
    pht_assert_unrepresentable(
        &[
            (
                "#b1 { padding-right:200%; padding-bottom:200%; padding-left:200%; \
                 min-width:200%; max-width:200%; }",
                "padding-right",
            ),
            (
                "#b1 { padding-bottom:200%; padding-left:200%; min-width:200%; \
                 max-width:200%; }",
                "padding-bottom",
            ),
        ],
        &format!("#p {{ width:{PCT_F32_MAX_PX}; }}"),
    );
}

/// KF (carried PPM code review K-1, PM-16; M33 witness): F's `min-width:30%` resolves
/// against P's used width 50, not O's 100: F's used width 15 (O's basis gives 30),
/// placed [0, 15] x [16, 18]. Passes at RED and after PHT.
#[test]
fn pht_red_guard_kf_float_min_width_basis_is_the_parent_width() {
    let mut expected = vec![pct_lit("c1.used_width", 15.0), pct_lit("float.right", 15.0)];
    expected.extend(pct_x(1, "right", 15.0));
    assert_pct_row(
        "#p { width:50px; } #f { min-width:30%; }",
        "#p { width:50px; }",
        "#p { width:50px; } #f { min-width:15px; }",
        &expected,
    );
}

/// LG3 (Legacy guard; M4 witness): the Legacy entry refuses height-family percentages
/// unchanged: B2's `height` and `min-height` (original 2) and F's `max-height`
/// (original 1), each alone. Passes at RED and after PHT.
#[test]
fn pht_red_guard_lg3_legacy_refuses_height_percentages() {
    for (fragment, projected, name) in [
        ("#b2 { height:10%; }", 4, "height"),
        ("#b2 { min-height:10%; }", 4, "min-height"),
        ("#f { max-height:10%; }", 3, "max-height"),
    ] {
        let css = pct_css(5, fragment);
        let (error, originals) = pct_legacy_error(&css);
        let error = error.unwrap_or_else(|| panic!("{css}: Legacy facts Ok"));
        assert_eq!(
            pct_describe(&error),
            pct_refusal(
                originals[projected],
                name,
                BoxFactUnsupported::PercentageLength
            ),
            "{css}"
        );
        assert_eq!(originals, vec![4, 3, 0, 1, 2], "{css}: Legacy originals");
    }
}

// FLIP rows: they read the PHT facts and the `OuterHeightBasis` reason.

/// The `Debug` text of the N5 layout error of `css` with O's content geometry `outer`.
fn pht_error_in(css: &str, outer: (f32, f32, f32)) -> String {
    format!("{:?}", pct_error_in(css, outer))
}

/// The refusal of `property` at `projected` with `reason`, as a `Debug` text.
fn pht_refusal(projected: usize, property: &str, reason: &str) -> String {
    format!(
        "Captured(Unsupported {{ projected: {projected}, property: \"{property}\", reason: \
         {reason} }})"
    )
}

/// Each N5 `fragment` gives the facade refusal `(projected, property, reason)`.
fn pht_assert_refusals(cases: &[(&str, usize, &str, &str)]) {
    for &(fragment, projected, property, reason) in cases {
        let css = pct_css(5, fragment);
        assert_eq!(
            pht_error_in(&css, PCT_OUTER),
            pht_refusal(projected, property, reason),
            "{css}"
        );
    }
}

/// H4 (M7 witness): the children's basis is P's used content height: P's computed 20 is
/// clamped by `max-height:10px` to 10, so B1's `height:50%` is 5 (the computed 20 gives
/// 10, the width 50): B1 [11, 16]; F [20, 22]; B2 [22, 23]; P content bottom 21.
#[test]
fn pht_h4_child_basis_is_the_constrained_parent_height() {
    let mut expected = vec![
        pct_lit("c0.used_height", 5.0),
        pct_lit("float.top", 20.0),
        pct_lit("float.bottom", 22.0),
        pct_lit("p.content_bottom", 21.0),
    ];
    expected.extend(pct_y(0, 11.0, 16.0));
    expected.extend(pct_y(1, 20.0, 22.0));
    expected.extend(pct_y(2, 22.0, 23.0));
    assert_pct_row(
        "#p { height:20px; max-height:10px; } #b1 { height:50%; }",
        "#p { height:20px; max-height:10px; }",
        "#p { height:20px; max-height:10px; } #b1 { height:5px; }",
        &expected,
    );
}

/// H4b (review F-2; M35 witness): P's `min-height:20px` raises its computed 10 to 20,
/// the children's basis: B1's `height:50%` is 10 (the basis without P's minimum gives
/// 5): B1 [11, 21]; F [25, 27]; B2 [27, 28]; P content bottom 31, padding and border
/// bottom 32.
#[test]
fn pht_h4b_child_basis_includes_the_parent_minimum() {
    let mut expected = vec![
        pct_lit("c0.used_height", 10.0),
        pct_lit("float.top", 25.0),
        pct_lit("float.bottom", 27.0),
        pct_lit("p.content_bottom", 31.0),
        pct_lit("p.border_bottom", 32.0),
    ];
    expected.extend(pct_y(0, 11.0, 21.0));
    expected.extend(pct_y(1, 25.0, 27.0));
    expected.extend(pct_y(2, 27.0, 28.0));
    assert_pct_row(
        "#p { height:10px; min-height:20px; } #b1 { height:50%; }",
        "#p { height:10px; min-height:20px; }",
        "#p { height:10px; min-height:20px; } #b1 { height:10px; }",
        &expected,
    );
}

/// H5: a tail's `height:50%` of 20 is 10: B2 [18, 28]; P as E20.
#[test]
fn pht_h5_tail_height_percentage() {
    let mut expected = vec![pct_lit("c2.used_height", 10.0)];
    expected.extend(pct_y(2, 18.0, 28.0));
    assert_pct_row(
        "#p { height:20px; } #b2 { height:50%; }",
        PHT_E20,
        "#p { height:20px; } #b2 { height:10px; }",
        &expected,
    );
}

/// H6 (M11 witness): without a basis (P's height is `auto`) B1's `height:50%` is used as
/// `auto`, so B1 is an Auto normal without a positive minimum: `HeightKind` at projected
/// 2 (a basis of 0 gives the core's `NonpositiveBlockHeight`, a basis of 100 a layout).
#[test]
fn pht_h6_child_height_without_a_basis_is_auto() {
    pht_assert_refusals(&[("#b1 { height:50%; }", 2, "height", "HeightKind")]);
}

/// H7 (M12 witness): without a basis B1's `min-height:50%` is 0, so its `height:auto`
/// stays `HeightKind` (P's width basis would give 50 and admit B1).
#[test]
fn pht_h7_child_min_height_without_a_basis_is_zero() {
    pht_assert_refusals(&[(
        "#b1 { height:auto; min-height:50%; }",
        2,
        "height",
        "HeightKind",
    )]);
}

/// H9d (M28 witness): P's `height:50%` used as `auto` depends on content, so it gives its
/// children no basis: B1's `height:10%` is used as `auto`, `HeightKind` (counting P's
/// percentage as explicit gives a layout).
#[test]
fn pht_h9d_children_of_an_auto_used_parent_have_no_basis() {
    pht_assert_refusals(&[(
        "#p { height:50%; } #b1 { height:10%; }",
        2,
        "height",
        "HeightKind",
    )]);
}

/// H9e (review F-2; M36 witness): an Auto P with a length `min-height` still depends on
/// content and gives no basis: B1's `height:50%` is `HeightKind` (the basis
/// `constrain(0, 20, none)` = 20 would give B1 10 and a layout).
#[test]
fn pht_h9e_auto_parent_with_a_minimum_gives_no_basis() {
    pht_assert_refusals(&[(
        "#p { min-height:20px; } #b1 { height:50%; }",
        2,
        "height",
        "HeightKind",
    )]);
}

/// H6b: B1's `height:50%` is used as `auto` and its `min-height:3px` gives the used
/// height 3: B1 [11, 14]; F [18, 20]; B2 [20, 21] with clearance -1; P content bottom
/// max(21 + 3, 20) = 24, padding and border bottom 25.
#[test]
fn pht_h6b_auto_used_child_with_a_minimum() {
    let mut expected = pht_b1_three_expected();
    expected.push(pct_lit("c0.used_height", 3.0));
    assert_pct_row(
        "#b1 { height:50%; min-height:3px; }",
        "",
        "#b1 { height:auto; min-height:3px; }",
        &expected,
    );
}

/// The numbers that differ from the baseline when B1's used height is 3 under an Auto
/// P: B1 [11, 14]; F [18, 20]; B2 [20, 21]; P content bottom 24, padding, border and
/// margin bottom 25, used height 13, Auto extent 24.
fn pht_b1_three_expected() -> Vec<(String, f32)> {
    let mut expected = vec![
        pct_lit("float.top", 18.0),
        pct_lit("float.bottom", 20.0),
        pct_lit("p.content_bottom", 24.0),
        pct_lit("p.padding_bottom", 25.0),
        pct_lit("p.border_bottom", 25.0),
        pct_lit("p.margin_bottom", 25.0),
        pct_lit("p.used_height", 13.0),
        pct_lit("p.extent.normal_margin_bottom", 24.0),
        pct_lit("p.extent.selected_bottom", 24.0),
    ];
    expected.extend(pct_y(0, 11.0, 14.0));
    expected.extend(pct_y(1, 18.0, 20.0));
    expected.extend(pct_y(2, 20.0, 21.0));
    expected
}

/// H6c (CFH): F's `height:50%` without a basis is used as `auto`; with no minimum its
/// constrained height is +0 and F keeps the Auto category: every number equals the
/// `#f { height:auto; }` twin's, F's used height +0. B2 does not clear here (as in the
/// CFH rows): with `clear:both` the baseline's B2 meets a zero-height F at the core's
/// `UnsupportedEquality`, for the twin as well.
#[test]
fn pht_h6c_auto_used_float_height() {
    let twin = "#b2 { clear:none; } #f { height:auto; }";
    let expected = [pct_lit("c1.used_height", 0.0)];
    assert_pct_row(
        "#b2 { clear:none; } #f { height:50%; }",
        twin,
        twin,
        &expected,
    );
    assert_eq!(
        pht_result("#f { height:50%; }"),
        pht_result("#f { height:auto; }")
    );
}

/// The layout of `fragment` equals the baseline (Auto extent present, P content bottom
/// 22) and its `twin`.
fn pht_assert_baseline(fragment: &str, twin: &str) {
    let expected = [pct_lit("p.extent", 1.0), pct_lit("p.content_bottom", 22.0)];
    assert_pct_row(fragment, "", twin, &expected);
}

/// H7b (M9 witness): under an Auto O, P's `min-height:50%` is 0: the baseline (an O-width
/// basis gives P content bottom 61).
#[test]
fn pht_h7b_parent_min_height_percentage_is_zero() {
    pht_assert_baseline("#p { min-height:50%; }", "#p { min-height:0px; }");
}

/// H8 (M10 witness): under an Auto O, P's `max-height:5%` is `none`: the baseline with
/// its Auto extent (an O-width basis gives 5: P content bottom 16, MMX P2).
#[test]
fn pht_h8_parent_max_height_percentage_is_none() {
    pht_assert_baseline("#p { max-height:5%; }", "#p { max-height:none; }");
}

/// H9 (M8 witness): under an Auto O, P's `height:50%` is used as `auto`: the baseline (an
/// O-width basis gives P content bottom 61); P's `computed_height()` publishes
/// `Percentage(50, Specified)`.
#[test]
fn pht_h9_parent_height_percentage_is_auto() {
    pht_assert_baseline("#p { height:50%; }", "#p { height:auto; }");
    let css = pct_css(5, "#p { height:50%; }");
    let computed = pct_with(5, &css, PCT_OUTER, layout_limits(), |layout| {
        layout.parent().computed_height()
    })
    .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"));
    assert_eq!(computed, pct_fact("50", BoxFactOrigin::Specified), "{css}");
}

/// H8b (M13 witness): B1's `max-height:1%` without a basis is `none`, so its
/// `height:3px` stays 3 (a width basis gives 1, a 0 maximum the core's
/// `NonpositiveBlockHeight`): the numbers of H6b.
#[test]
fn pht_h8b_child_max_height_without_a_basis_is_none() {
    let mut expected = pht_b1_three_expected();
    expected.push(pct_lit("c0.used_height", 3.0));
    assert_pct_row(
        "#b1 { height:3px; max-height:1%; }",
        "",
        "#b1 { height:3px; max-height:none; }",
        &expected,
    );
}

/// H9b (M26 witness): P's `height:50%` used as `auto` with a negative child margin takes
/// the signed Auto route: equal to the `#b2 { margin-bottom:-3px; }` control (MMX P6).
#[test]
fn pht_h9b_auto_used_parent_takes_the_signed_route() {
    let control = "#b2 { margin-bottom:-3px; }";
    assert_pct_row(
        "#p { height:50%; } #b2 { margin-bottom:-3px; }",
        control,
        control,
        &[],
    );
}

/// H9c (M27 witness): P's `height:50%` used as `auto` keeps its `max-height:5px` through
/// `constrain_bfc_height`: equal to `#p { max-height:5px; }` (MMX P2): P content bottom
/// 16, the explicit rerun, extent absent (skipping the step gives 22).
#[test]
fn pht_h9c_auto_used_parent_keeps_its_limits() {
    let control = "#p { max-height:5px; }";
    let expected = [pct_lit("p.content_bottom", 16.0), pct_lit("p.extent", 0.0)];
    assert_pct_row(
        "#p { height:50%; max-height:5px; }",
        control,
        control,
        &expected,
    );
}

/// H10 (M14, M16 witness): P's height percentages under an O with an explicit height
/// (a length, or O's own percentage, explicit for the root) are refused with
/// `OuterHeightBasis` at P, property by property, including an inherited percentage.
#[test]
fn pht_h10_parent_percentages_under_an_explicit_outer_height_are_refused() {
    pht_assert_refusals(&[
        (
            "#o { height:200px; } #p { height:50%; }",
            1,
            "height",
            "OuterHeightBasis",
        ),
        (
            "#o { height:200px; } #p { min-height:50%; }",
            1,
            "min-height",
            "OuterHeightBasis",
        ),
        (
            "#o { height:200px; } #p { max-height:50%; }",
            1,
            "max-height",
            "OuterHeightBasis",
        ),
        (
            "#o { height:50%; } #p { max-height:50%; }",
            1,
            "max-height",
            "OuterHeightBasis",
        ),
        (
            "#o { height:50%; } #p { height:inherit; }",
            1,
            "height",
            "OuterHeightBasis",
        ),
    ]);
}

/// H10b (M15 witness): O's explicit height matters for P's percentages only: with P's
/// `height:20px`, B1's `height:10%` gives the H1 geometry.
#[test]
fn pht_h10b_outer_height_does_not_refuse_child_percentages() {
    let mut expected = vec![
        pct_lit("c0.used_height", 2.0),
        pct_lit("float.top", 17.0),
        pct_lit("float.bottom", 19.0),
    ];
    expected.extend(pct_y(0, 11.0, 13.0));
    expected.extend(pct_y(1, 17.0, 19.0));
    expected.extend(pct_y(2, 19.0, 20.0));
    assert_pct_row(
        "#o { height:200px; } #p { height:20px; } #b1 { height:10%; }",
        "#o { height:200px; } #p { height:20px; }",
        "#o { height:200px; } #p { height:20px; } #b1 { height:2px; }",
        &expected,
    );
}

/// H10c (review F-8): the refusal reads O's computed `height` only: an O with `auto`
/// height and a length `min-height` still depends on content, so P's `height:50%` is
/// used as `auto`: equal to the `#o { min-height:200px; }` control.
#[test]
fn pht_h10c_outer_minimum_does_not_make_its_height_explicit() {
    let control = "#o { min-height:200px; }";
    assert_pct_row(
        "#o { min-height:200px; } #p { height:50%; }",
        control,
        control,
        &[],
    );
}

/// H11 (M17 witness): the refusal names the first of `height`, `min-height`,
/// `max-height`.
#[test]
fn pht_h11_outer_height_refusal_order() {
    pht_assert_refusals(&[
        (
            "#o { height:200px; } #p { height:50%; min-height:50%; max-height:50%; }",
            1,
            "height",
            "OuterHeightBasis",
        ),
        (
            "#o { height:200px; } #p { min-height:50%; max-height:50%; }",
            1,
            "min-height",
            "OuterHeightBasis",
        ),
        (
            "#o { height:200px; } #p { max-height:50%; }",
            1,
            "max-height",
            "OuterHeightBasis",
        ),
    ]);
}

/// H11b (M18 witness): the refusal precedes every child error: B1's `overflow:scroll`
/// is not reached.
#[test]
fn pht_h11b_outer_height_refusal_precedes_the_children() {
    pht_assert_refusals(&[(
        "#o { height:200px; } #p { height:50%; } #b1 { overflow:scroll; }",
        1,
        "height",
        "OuterHeightBasis",
    )]);
}

/// H11c: the refusal follows P's `overflow` test.
#[test]
fn pht_h11c_parent_overflow_precedes_the_outer_height_refusal() {
    pht_assert_refusals(&[(
        "#o { height:200px; } #p { overflow:visible; height:50%; }",
        1,
        "overflow",
        "ParentOverflow",
    )]);
}

/// H12 and H12b (M19 witness): under P's `height:<f32::MAX>px` each child `200%` is
/// unrepresentable: `Percentage { source: 2, property, error: Unrepresentable }` with
/// the first of `height`, `min-height`, `max-height`.
#[test]
fn pht_h12_unrepresentable_child_heights_and_their_order() {
    pht_assert_unrepresentable(
        &[
            ("#b1 { height:200%; }", "height"),
            (
                "#b1 { height:200%; min-height:200%; max-height:200%; }",
                "height",
            ),
            ("#b1 { min-height:200%; max-height:200%; }", "min-height"),
            ("#b1 { max-height:200%; }", "max-height"),
        ],
        &format!("#p {{ height:{PCT_F32_MAX_PX}; }}"),
    );
}

/// H13 (M20 witness): a child's admission tests precede its height resolution: B1's
/// `overflow:scroll` is refused before its unrepresentable `height:200%`.
#[test]
fn pht_h13_child_admission_precedes_height_resolution() {
    let css = pct_css(
        5,
        &format!("#p {{ height:{PCT_F32_MAX_PX}; }} #b1 {{ height:200%; overflow:scroll; }}"),
    );
    assert_eq!(
        pht_error_in(&css, PCT_OUTER),
        pht_refusal(2, "overflow", "ChildOverflow"),
        "{css}"
    );
}

/// HP (review F-1, ruling PH-2; M34 witness): a child's height resolution precedes P's
/// held horizontal error. With O's width NaN (an invalid O basis), P's solve fails and
/// is held; B1's `height:200%` of P's `<f32::MAX>px` is still reported first
/// (`Percentage`, not `ParentHorizontal`), while B1's `height:2px` twin returns the held
/// error.
#[test]
fn pht_hp_child_height_resolution_precedes_the_held_parent_error() {
    let outer = (0.0, f32::NAN, 10.0);
    for (fragment, text) in [
        (
            "#b1 { height:200%; }",
            "Percentage { source: 2, property: \"height\", error: Unrepresentable }",
        ),
        (
            "#b1 { height:2px; }",
            "ParentHorizontal(Width(InvalidInput(\"containing_width\")))",
        ),
    ] {
        let css = pct_css(5, &format!("#p {{ height:{PCT_F32_MAX_PX}; }} {fragment}"));
        assert_eq!(pht_error_in(&css, outer), text, "{css}");
    }
}

/// H14 (M22 witness): `max-height:inherit` copies P's computed `max-height:50%` with
/// origin `Inherited`; P's own 50% is `none` (O `auto`), so P is E20; B1's copy resolves
/// against P's 20: B1 `height:15px` is clamped to 10, B1 [11, 21]; F [25, 27]; B2
/// [27, 28] (a dropped inherit gives B1 [11, 26]).
#[test]
fn pht_h14_inherit_keeps_the_max_height_percentage() {
    let fragment = "#p { height:20px; max-height:50%; } #b1 { height:15px; max-height:inherit; }";
    let css = pct_css(5, fragment);
    let (rows, _, originals) = pct_facts(5, &css);
    let inherited = BoxFactOrigin::Inherited {
        original_parent: originals[1],
    };
    assert_eq!(rows[2].max_height, ppm_max("50", inherited), "{css}");
    let mut expected = vec![
        pct_lit("c0.used_height", 10.0),
        pct_lit("float.top", 25.0),
        pct_lit("float.bottom", 27.0),
    ];
    expected.extend(pct_y(0, 11.0, 21.0));
    expected.extend(pct_y(1, 25.0, 27.0));
    expected.extend(pct_y(2, 27.0, 28.0));
    assert_pct_row(
        fragment,
        PHT_E20,
        "#p { height:20px; } #b1 { height:15px; max-height:10px; }",
        &expected,
    );
}

/// H14b (M21 witness): `min-height:inherit` copies P's computed `min-height:10%` with
/// origin `Inherited`; P's own 10% is 0; B1's copy is 2 of P's 20: the H1 geometry.
#[test]
fn pht_h14b_inherit_keeps_the_min_height_percentage() {
    let fragment = "#p { height:20px; min-height:10%; } #b1 { min-height:inherit; }";
    let css = pct_css(5, fragment);
    let (rows, _, originals) = pct_facts(5, &css);
    let inherited = BoxFactOrigin::Inherited {
        original_parent: originals[1],
    };
    assert_eq!(rows[2].min_height, ppm_lp("10", inherited), "{css}");
    let mut expected = vec![
        pct_lit("c0.used_height", 2.0),
        pct_lit("float.top", 17.0),
        pct_lit("float.bottom", 19.0),
    ];
    expected.extend(pct_y(0, 11.0, 13.0));
    expected.extend(pct_y(1, 17.0, 19.0));
    expected.extend(pct_y(2, 19.0, 20.0));
    assert_pct_row(
        fragment,
        PHT_E20,
        "#p { height:20px; } #b1 { min-height:2px; }",
        &expected,
    );
}

/// H15: F's `height:25%` of 20 is 5: F [16, 21]; B2's clearance max(21 - 19, 16 - 19) =
/// 2, B2 [21, 22].
#[test]
fn pht_h15_float_height_percentage() {
    let mut expected = vec![
        pct_lit("c1.used_height", 5.0),
        pct_lit("float.bottom", 21.0),
        pct_lit("tail0.clearance", 2.0),
    ];
    expected.extend(pct_y(1, 16.0, 21.0));
    expected.extend(pct_y(2, 21.0, 22.0));
    assert_pct_row(
        "#p { height:20px; } #f { height:25%; }",
        PHT_E20,
        "#p { height:20px; } #f { height:5px; }",
        &expected,
    );
}

/// H16 (PM-19; M23 witness): vertical paddings keep the width basis (CSS2.2 8.4) under
/// an explicit-height P: B1's `padding-top:10%` is 10 of P's width 100 (the height basis
/// gives 2): B1 border and padding [11, 22], content [21, 22]; F [26, 28]; B2 [28, 29].
#[test]
fn pht_h16_vertical_padding_keeps_the_width_basis() {
    let mut expected = vec![
        pct_lit("c0.border.bottom", 22.0),
        pct_lit("c0.padding.bottom", 22.0),
        pct_lit("c0.content.top", 21.0),
        pct_lit("c0.content.bottom", 22.0),
        pct_lit("float.top", 26.0),
        pct_lit("float.bottom", 28.0),
    ];
    expected.extend(pct_y(1, 26.0, 28.0));
    expected.extend(pct_y(2, 28.0, 29.0));
    assert_pct_row(
        "#p { height:20px; } #b1 { padding-top:10%; }",
        PHT_E20,
        "#p { height:20px; } #b1 { padding-top:10px; }",
        &expected,
    );
}

/// H17 (PM-19; M6, M29 witness): a `max-height` percentage resolves against the height
/// basis: B1's `max-height:50%` of 20 is 10 and clamps its `height:15px` (the width
/// basis 50 gives 15): B1 [11, 21]; F [25, 27]; B2 [27, 28].
#[test]
fn pht_h17_max_height_percentage_uses_the_height_basis() {
    let mut expected = vec![
        pct_lit("c0.used_height", 10.0),
        pct_lit("float.top", 25.0),
        pct_lit("float.bottom", 27.0),
    ];
    expected.extend(pct_y(0, 11.0, 21.0));
    expected.extend(pct_y(1, 25.0, 27.0));
    expected.extend(pct_y(2, 27.0, 28.0));
    assert_pct_row(
        "#p { height:20px; } #b1 { height:15px; max-height:50%; }",
        PHT_E20,
        "#p { height:20px; } #b1 { height:15px; max-height:10px; }",
        &expected,
    );
}

/// H18: the bounded representation: `18446744073709551616%` (2^64) is
/// `PercentagePrecision` at B1's `height`, `min-height` and `max-height`; the largest
/// stored coefficient `18446744073709551615%` is kept exactly.
#[test]
fn pht_h18_precision_boundary() {
    for name in ["height", "min-height", "max-height"] {
        let css = pct_css(5, &format!("#b1 {{ {name}:18446744073709551616%; }}"));
        let (error, originals) = pct_fact_error(5, &css);
        assert_eq!(
            pct_sequence_describe(&error),
            format!(
                "Evaluation({})",
                pct_refusal(originals[2], name, BoxFactUnsupported::PercentagePrecision)
            ),
            "{css}"
        );
    }
    let css = pct_css(5, "#b1 { height:18446744073709551615%; }");
    let (rows, _, _) = pct_facts(5, &css);
    assert_eq!(
        rows[2].height,
        pct_fact("18446744073709551615", BoxFactOrigin::Specified),
        "{css}"
    );
}

/// The `Debug` text of the N5 layout result of `fragment` (a view or an error).
fn pht_result(fragment: &str) -> String {
    let css = pct_css(5, fragment);
    format!(
        "{:?}",
        pct_with(5, &css, PCT_OUTER, layout_limits(), pct_view)
    )
}

/// H19: a zero basis: P's `height:0px` makes B1's `height:50%` 0, the same result as the
/// `#b1 { height:0px; }` twin (compared by `Debug` text, layout or error).
#[test]
fn pht_h19_zero_basis() {
    assert_eq!(
        pht_result("#p { height:0px; } #b1 { height:50%; }"),
        pht_result("#p { height:0px; } #b1 { height:0px; }")
    );
}

/// H20 (Q4=C): under P `overflow:auto` with `height:20px`, B1's `height:150%` is 30 and
/// leaves P's content box (B1 bottom 41 > 31): `ParentAutoOverflow { source: 2 }`, as
/// the `height:30px` twin.
#[test]
fn pht_h20_auto_parent_overflow_is_refused() {
    for fragment in [
        "#p { overflow:auto; height:20px; } #b1 { height:150%; }",
        "#p { overflow:auto; height:20px; } #b1 { height:30px; }",
    ] {
        let css = pct_css(5, fragment);
        assert_eq!(
            pht_error_in(&css, PCT_OUTER),
            "ParentAutoOverflow { source: 2 }",
            "{css}"
        );
    }
}

/// RW5 (M25 witness): B1's `height:0.049%` of 20 is one nearest-even rounding of the
/// exact 0.0098 (`3c20902e`), equal to the `height:0.0098px` twin in every number; the
/// f32 pipeline f32(f32(f32(p) * 20) / 100) gives `3c20902d`.
#[test]
fn pht_rw5_one_rounding_height() {
    assert_eq!((0.049_f32 * 20.0 / 100.0).to_bits(), 0x3c20902d);
    let twin = "#p { height:20px; } #b1 { height:0.0098px; }";
    let expected = [pct_lit("c0.used_height", f32::from_bits(0x3c20902e))];
    assert_pct_row(
        "#p { height:20px; } #b1 { height:0.049%; }",
        twin,
        twin,
        &expected,
    );
}

/// FX7: the facts keep `height:2.5%`, `min-height:0%` and `max-height:100%` exactly,
/// `Specified`, with no counted conversion: `numeric_calls` equals the
/// `#b1 { height:auto; }` twin's.
#[test]
fn pht_fx7_height_percentage_facts_are_exact_and_unconverted() {
    let css = pct_css(5, "#b1 { height:2.5%; min-height:0%; max-height:100%; }");
    let (rows, receipt, _) = pct_facts(5, &css);
    let (_, twin, _) = pct_facts(5, &pct_css(5, "#b1 { height:auto; }"));
    let specified = BoxFactOrigin::Specified;
    assert_eq!(rows[2].height, pct_fact("2.5", specified), "{css}");
    assert_eq!(rows[2].min_height, ppm_lp("0", specified), "{css}");
    assert_eq!(rows[2].max_height, ppm_max("100", specified), "{css}");
    assert_eq!(receipt.numeric_calls, twin.numeric_calls, "{css}");
}

/// FX8: the property grammar drops a negative `height`, `min-height` or `max-height`
/// percentage, so every row equals the baseline's; the producer's `InvalidValue` test is
/// defensive.
#[test]
fn pht_fx8_negative_height_percentages_are_dropped_by_the_parser() {
    let (baseline, _, _) = pct_facts(5, &pct_css(5, ""));
    for fragment in [
        "#b1 { height:-10%; }",
        "#b1 { min-height:-10%; }",
        "#b1 { max-height:-10%; }",
    ] {
        let css = pct_css(5, fragment);
        let (rows, _, _) = pct_facts(5, &css);
        assert_eq!(rows, baseline, "{css}");
    }
}

/// The work fixture: PCT's and PPM's percentages plus height-constraint percentages on
/// every source that take the indefinite rules (O and P `auto`).
const PHT_WORK_FRAGMENT: &str = "#p { min-height:0%; max-height:100%; } \
                                 #b1, #f, #b2, #b3, #b4 { min-height:0%; max-height:100%; }";

/// Facade and core work (M24 witness): exact and one short at N5, N6 and N7 with
/// percentages on every source; the facade charge is `469 + 32N + 491(N-2) +
/// 313(N-3)`; the layout equals the baseline's.
#[test]
fn pht_work_thresholds_with_percentages() {
    for (nodes, facade, core) in [(5, 2728, 1249), (6, 3564, 1501), (7, 4400, 1754)] {
        assert_eq!(
            facade,
            469 + 32 * nodes + 491 * (nodes - 2) + 313 * (nodes - 3)
        );
        let css = pct_css(
            nodes,
            &format!("{PCT_WORK_FRAGMENT} {PPM_WORK_FRAGMENT} {PHT_WORK_FRAGMENT}"),
        );
        assert_pct_view(
            &pct_layout(nodes, &css),
            &pct_layout(nodes, &pct_css(nodes, "")),
            &[],
            &css,
        );
        let limits = |work: usize, core_work: usize| {
            let mut limits = layout_limits();
            limits.work = nz(work);
            limits.core.work = nz(core_work);
            limits
        };
        let receipt = pct_with(nodes, &css, PCT_OUTER, limits(facade, core), |layout| {
            (layout.receipt().work, layout.receipt().core.work)
        })
        .unwrap_or_else(|error| panic!("N{nodes}: expected a layout, got {error:?}"));
        assert_eq!(receipt, (facade, core), "N{nodes}");
        assert!(
            matches!(
                pct_with(nodes, &css, PCT_OUTER, limits(facade - 1, core), |_| ()),
                Err(SiblingLayoutError::Work)
            ),
            "N{nodes} facade one short"
        );
        assert!(
            matches!(
                pct_with(nodes, &css, PCT_OUTER, limits(facade, core - 1), |_| ()),
                Err(SiblingLayoutError::Core(ClearanceSequenceError::Work))
            ),
            "N{nodes} core one short"
        );
    }
}

/// D4 measurement (x86_64; measured at FLIP): the fact row, whose `min_height` widens
/// from `AbsoluteBoxFact` to `LengthPercentageBoxFact`, and the refusal reason with its
/// new fieldless variant.
#[test]
fn pht_representation_measurement() {
    let measured = [
        (
            "ClearanceBoxFact",
            std::mem::size_of::<ClearanceBoxFact>(),
            std::mem::align_of::<ClearanceBoxFact>(),
        ),
        (
            "CapturedClearanceUnsupported",
            std::mem::size_of::<CapturedClearanceUnsupported>(),
            std::mem::align_of::<CapturedClearanceUnsupported>(),
        ),
    ];
    for (name, size, align) in measured {
        println!("PHT_REPRESENTATION {name} {size} {align}");
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    assert_eq!(
        measured.map(|(_, size, align)| (size, align)),
        [(848, 8), (1, 1)]
    );
}

// PTB: the `top` and `bottom` percentage rows reuse the `pct`, `ppm` and `pht` helpers
// as a child module of this one (design Q-9).
#[path = "percentage_offsets.rs"]
mod percentage_offsets;
