//! PPM: percentages of the containing block's width in the four paddings, `min-width`
//! and `max-width` on every Sequence row (CSS2.2 8.4, 10.4), with the carried PCT
//! code-review rows KA-KE (PL-17).
//!
//! A child module of `percentage_lengths` (review C-3, ruling PM-3): it reuses that
//! module's private helpers (`pct_css`, `pct_view`, `assert_pct_row`, ...) through
//! `use super::*`. The RED part of this module (the helpers, R1-R3 and the guard rows)
//! compiles against the predecessor API at f2f688c7: it reads only published geometry,
//! errors and the PCT `offset` fact. The rows appended at FLIP also read the new
//! padding and constraint facts. Every literal is derived from `baseline_css()` (O
//! content left 0, width 100, top 10; P `hidden` with padding-top and padding-bottom of
//! 1); each admitted row also compares every other published number with a control and
//! with its absolute twin (the same CSS with each percentage replaced by the value it
//! resolves to), bit for bit.

use super::*;

/// R1 (RED; M1 witness): B1's `padding-left:10%` of P's used width 100 is 10: used
/// width 90, margins (0, 0), `passes` 1, content [10, 100], padding and border [0, 100];
/// every other number is the baseline's and every number is the `padding-left:10px`
/// twin's. Predecessor at f2f688c7: `Evaluation(Unsupported { original: 0, property:
/// padding-left, target: Element, reason: PercentageLength })`.
#[test]
fn ppm_red_r1_padding_left_percentage_narrows_b1() {
    let expected = [
        pct_lit("c0.used_width", 90.0),
        pct_lit("c0.margin.left", 0.0),
        pct_lit("c0.margin.right", 0.0),
        pct_lit("c0.passes", 1.0),
        pct_lit("c0.content.left", 10.0),
        pct_lit("c0.padding.left", 0.0),
        pct_lit("c0.border.left", 0.0),
    ];
    assert_pct_row(
        "#b1 { padding-left:10%; }",
        "",
        "#b1 { padding-left:10px; }",
        &expected,
    );
}

/// R2 (RED; M3 witness): B1's `max-width:50%` of 100 is 50: the tentative width 100 is
/// above it, so the rerun gives width 50, margin-right 50, `passes` 2, border [0, 50].
/// Predecessor at f2f688c7: the same refusal with `property: max-width`.
#[test]
fn ppm_red_r2_max_width_percentage_narrows_b1() {
    let mut expected = vec![
        pct_lit("c0.used_width", 50.0),
        pct_lit("c0.margin.right", 50.0),
        pct_lit("c0.passes", 2.0),
    ];
    expected.extend(pct_x(0, "right", 50.0));
    assert_pct_row(
        "#b1 { max-width:50%; }",
        "",
        "#b1 { max-width:50px; }",
        &expected,
    );
}

/// R3 (RED; M2 witness): P's `min-width:120%` of O's 100 is 120: P width 120, margins
/// (0, -20) (ltr overconstraint), `passes` 2, content [0, 120]; B1 and B2 [0, 120]; F
/// keeps [0, 10]; the vertical numbers stay the baseline. Predecessor at f2f688c7:
/// `original: 3`, `property: min-width`.
#[test]
fn ppm_red_r3_parent_min_width_percentage_widens_p() {
    let mut expected = vec![
        pct_lit("p.width", 120.0),
        pct_lit("p.margin_left", 0.0),
        pct_lit("p.margin_right", -20.0),
        pct_lit("p.passes", 2.0),
        pct_lit("p.content_right", 120.0),
        pct_lit("c0.used_width", 120.0),
        pct_lit("c2.used_width", 120.0),
    ];
    expected.extend(pct_x(0, "right", 120.0));
    expected.extend(pct_x(2, "right", 120.0));
    assert_pct_row(
        "#p { min-width:120%; }",
        "",
        "#p { min-width:120px; }",
        &expected,
    );
}

/// FX5 (guard; M5 witness): `top` and `bottom` percentages keep `PercentageLength` at
/// B1's own index on a Sequence row (PHT admits `height`, `min-height` and
/// `max-height`). Passes at RED and after PPM and PHT.
#[test]
fn ppm_red_guard_fx5_height_constraint_and_vertical_offset_percentages_stay_refused() {
    for (fragment, name) in [
        ("#b1 { position:relative; top:10%; }", "top"),
        ("#b1 { position:relative; bottom:10%; }", "bottom"),
    ] {
        let css = pct_css(5, fragment);
        let (error, originals) = pct_fact_error(5, &css);
        assert_eq!(
            pct_sequence_describe(&error),
            format!(
                "Evaluation({})",
                pct_refusal(originals[2], name, BoxFactUnsupported::PercentageLength)
            ),
            "{css}"
        );
    }
}

/// LG2 (Legacy guard; M4 witness): the Legacy entry refuses padding and width-constraint
/// percentages unchanged: B2's `padding-left` and `min-width` (original 2) and F's
/// `max-width` (original 1), each alone. Passes at RED and after PPM.
#[test]
fn ppm_red_guard_lg2_legacy_refuses_padding_and_width_constraint_percentages() {
    for (fragment, projected, name) in [
        ("#b2 { padding-left:10%; }", 4, "padding-left"),
        ("#b2 { min-width:10%; }", 4, "min-width"),
        ("#f { max-width:10%; }", 3, "max-width"),
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

/// KA (PCT K-1, PL-17; M20, M21 witness): P's `margin-top:5%` and `margin-bottom:10%` of
/// O's 100: P border top 15, content top 16; B1 [16, 17]; F [21, 23]; B2 [23, 24] with
/// clearance -1; P content bottom 27, padding and border bottom 28 and its bottom margin
/// edge 38 (reading `margin-top` there would give 33). Passes at RED and after PPM.
#[test]
fn ppm_red_guard_ka_parent_vertical_margin_percentages() {
    let mut expected = vec![
        pct_lit("p.border_top", 15.0),
        pct_lit("p.content_top", 16.0),
        pct_lit("p.content_bottom", 27.0),
        pct_lit("p.padding_bottom", 28.0),
        pct_lit("p.border_bottom", 28.0),
        pct_lit("p.margin_bottom", 38.0),
        pct_lit("p.extent.normal_margin_bottom", 27.0),
        pct_lit("p.extent.selected_bottom", 27.0),
        pct_lit("float.top", 21.0),
        pct_lit("float.bottom", 23.0),
        pct_lit("tail0.clearance", -1.0),
    ];
    expected.extend(pct_y(0, 16.0, 17.0));
    expected.extend(pct_y(1, 21.0, 23.0));
    expected.extend(pct_y(2, 23.0, 24.0));
    assert_pct_row(
        "#p { margin-top:5%; margin-bottom:10%; }",
        "",
        "#p { margin-top:5px; margin-bottom:10px; }",
        &expected,
    );
}

/// KB (PCT K-1, PL-17; M22 witness): B1's `left:inherit` copies P's computed `left:10%`
/// with origin `Inherited`, and each resolves against its own containing block: P's
/// offsets (10, -10, +0, +0) on O's 100, B1's (5, -5, +0, +0) on P's 50. Passes at RED
/// and after PPM.
#[test]
fn ppm_red_guard_kb_inherited_left_percentage() {
    let fragment = "#p { width:50px; position:relative; left:10%; } \
                    #b1 { position:relative; left:inherit; }";
    let css = pct_css(5, fragment);
    let (rows, _, originals) = pct_facts(5, &css);
    let inherited = BoxFactOrigin::Inherited {
        original_parent: originals[1],
    };
    assert_eq!(
        rows[2].offset.map(|offset| offset[3]),
        Some(pct_fact("10", inherited)),
        "{css}"
    );
    let expected = [
        pct_lit("p.offset.left", 10.0),
        pct_lit("p.offset.right", -10.0),
        pct_lit("c0.offset.left", 5.0),
        pct_lit("c0.offset.right", -5.0),
    ];
    assert_pct_row(
        fragment,
        "#p { width:50px; }",
        "#p { width:50px; position:relative; left:10px; } \
         #b1 { position:relative; left:5px; }",
        &expected,
    );
}

/// KC (PCT K-1, PL-17): a negative `left` percentage: B1's offsets (-10, 10, +0, +0).
/// Passes at RED and after PPM.
#[test]
fn ppm_red_guard_kc_negative_left_percentage() {
    let expected = [
        pct_lit("c0.offset.left", -10.0),
        pct_lit("c0.offset.right", 10.0),
    ];
    assert_pct_row(
        "#b1 { position:relative; left:-10%; }",
        "",
        "#b1 { position:relative; left:-10px; }",
        &expected,
    );
}

/// KD (PCT K-1, PL-17; M23 witness): under an `rtl` P, B1's `left:5%` and `right:10%`
/// are overconstrained and `right` wins (CSS2.2 9.4.3): offsets (-10, 10, +0, +0); with
/// O's `ltr` they would be (5, -5). Passes at RED and after PPM.
#[test]
fn ppm_red_guard_kd_rtl_child_offsets() {
    let expected = [
        pct_lit("c0.offset.left", -10.0),
        pct_lit("c0.offset.right", 10.0),
    ];
    assert_pct_row(
        "#p { direction:rtl; } #b1 { position:relative; left:5%; right:10%; }",
        "#p { direction:rtl; }",
        "#p { direction:rtl; } #b1 { position:relative; left:5px; right:10px; }",
        &expected,
    );
}

/// KE (PCT K-1, optional): F's `margin-top:5%` of P's 100: F's margin box [16, 23] and
/// border [21, 23]; B2's clearance max(23 - 19, 16 - 19) = 4, border [23, 24]; P [10,
/// 11, 27, 28, 28]. Passes at RED and after PPM.
#[test]
fn ppm_red_guard_ke_float_margin_top_percentage() {
    let mut expected = vec![
        pct_lit("c1.margin.top", 5.0),
        pct_lit("float.bottom", 23.0),
        pct_lit("tail0.clearance", 4.0),
        pct_lit("p.content_bottom", 27.0),
        pct_lit("p.padding_bottom", 28.0),
        pct_lit("p.border_bottom", 28.0),
        pct_lit("p.margin_bottom", 28.0),
        pct_lit("p.used_height", 16.0),
        pct_lit("p.extent.normal_margin_bottom", 27.0),
        pct_lit("p.extent.selected_bottom", 27.0),
    ];
    expected.extend(pct_y(1, 21.0, 23.0));
    expected.extend(pct_y(2, 23.0, 24.0));
    assert_pct_row(
        "#f { margin-top:5%; }",
        "",
        "#f { margin-top:5px; }",
        &expected,
    );
}

// FLIP rows: they read the padding and width-constraint facts and the PPM error names.

/// A computed padding or `min-width` percentage fact.
fn ppm_lp(text: &str, origin: BoxFactOrigin) -> LengthPercentageBoxFact {
    LengthPercentageBoxFact::Percentage(PercentageBoxFact {
        value: pct_value(text),
        origin,
    })
}

/// A computed `max-width` percentage fact.
fn ppm_max(text: &str, origin: BoxFactOrigin) -> BoxMaxFact {
    BoxMaxFact::Percentage(PercentageBoxFact {
        value: pct_value(text),
        origin,
    })
}

/// The initial padding or `min-width` fact (+0, `Initial`).
const PPM_ZERO: LengthPercentageBoxFact = LengthPercentageBoxFact::Absolute(AbsoluteBoxFact {
    pixels: 0.0,
    rounded_to_zero: false,
    origin: BoxFactOrigin::Initial,
});

/// An admitted N5 row with O's content geometry `outer`: `fragment` gives the
/// `expected` literals and every other number of `control`, and every number of its
/// absolute `twin`, all three laid out with the same `outer`.
fn assert_ppm_row_in(
    fragment: &str,
    control: &str,
    twin: &str,
    expected: &[(String, f32)],
    outer: (f32, f32, f32),
) {
    let css = pct_css(5, fragment);
    let view = pct_layout_in(5, &css, outer);
    let control = pct_layout_in(5, &pct_css(5, control), outer);
    assert_pct_view(&view, &control, expected, &css);
    let twin = pct_layout_in(5, &pct_css(5, twin), outer);
    assert_pct_view(&view, &twin, &[], &css);
}

/// R4: P's `padding-left:10%` and `padding-right:5%` of O's 100 narrow its Auto width to
/// 85, content [10, 95]; B1 and B2 [10, 95]; F placed [10, 20].
#[test]
fn ppm_r4_parent_horizontal_padding_percentages() {
    let mut expected = vec![
        pct_lit("p.width", 85.0),
        pct_lit("p.content_left", 10.0),
        pct_lit("p.content_right", 95.0),
        pct_lit("c0.used_width", 85.0),
        pct_lit("c2.used_width", 85.0),
        pct_lit("float.left", 10.0),
        pct_lit("float.right", 20.0),
    ];
    for child in [0, 2] {
        expected.extend(pct_x(child, "left", 10.0));
        expected.extend(pct_x(child, "right", 95.0));
    }
    expected.extend(pct_x(1, "left", 10.0));
    expected.extend(pct_x(1, "right", 20.0));
    assert_pct_row(
        "#p { padding-left:10%; padding-right:5%; }",
        "",
        "#p { padding-left:10px; padding-right:5px; }",
        &expected,
    );
}

/// R5 (M7 witness): with O's content width 200, P's `padding-left:10%` is 20 (a
/// constant-100 basis gives 10, P's own width 18): P width 180, content [20, 200]; B1
/// and B2 [20, 200]; F [20, 30]. Control and twin use the same O.
#[test]
fn ppm_r5_parent_padding_basis_is_the_outer_width() {
    let mut expected = vec![
        pct_lit("p.width", 180.0),
        pct_lit("p.content_left", 20.0),
        pct_lit("c0.used_width", 180.0),
        pct_lit("c2.used_width", 180.0),
        pct_lit("float.left", 20.0),
        pct_lit("float.right", 30.0),
    ];
    for child in [0, 1, 2] {
        expected.extend(pct_x(child, "left", 20.0));
    }
    expected.extend(pct_x(1, "right", 30.0));
    assert_ppm_row_in(
        "#p { padding-left:10%; }",
        "",
        "#p { padding-left:20px; }",
        &expected,
        (0.0, 200.0, 10.0),
    );
}

/// R6 (M6 witness): a child's padding basis is P's used width 50, not O's 100: B1's
/// `padding-left:10%` is 5, width 45, content [5, 50], border [0, 50].
#[test]
fn ppm_r6_child_padding_basis_is_the_parent_width() {
    let expected = [
        pct_lit("c0.used_width", 45.0),
        pct_lit("c0.content.left", 5.0),
    ];
    assert_pct_row(
        "#p { width:50px; } #b1 { padding-left:10%; }",
        "#p { width:50px; }",
        "#p { width:50px; } #b1 { padding-left:5px; }",
        &expected,
    );
}

/// R7 (M10, M17 witness): vertical padding percentages use the width basis (CSS2.2 8.4):
/// under a 50px P, B1's `padding-top:2%` and `padding-bottom:3%` are 1 and 1.5; B1
/// border [11, 14.5], content [12, 13]; F [18.5, 20.5]; B2 [20.5, 21.5] with clearance
/// max(20.5 - 21.5, 18.5 - 21.5) = -1; P content bottom 24.5, padding and border bottom
/// 25.5.
#[test]
fn ppm_r7_vertical_padding_percentages_use_the_width_basis() {
    let mut expected = vec![
        pct_lit("c0.border.bottom", 14.5),
        pct_lit("c0.padding.bottom", 14.5),
        pct_lit("c0.content.top", 12.0),
        pct_lit("c0.content.bottom", 13.0),
        pct_lit("float.top", 18.5),
        pct_lit("float.bottom", 20.5),
        pct_lit("tail0.clearance", -1.0),
        pct_lit("p.content_bottom", 24.5),
        pct_lit("p.padding_bottom", 25.5),
        pct_lit("p.border_bottom", 25.5),
        pct_lit("p.margin_bottom", 25.5),
        pct_lit("p.used_height", 13.5),
        pct_lit("p.extent.normal_margin_bottom", 24.5),
        pct_lit("p.extent.selected_bottom", 24.5),
    ];
    expected.extend(pct_y(1, 18.5, 20.5));
    expected.extend(pct_y(2, 20.5, 21.5));
    assert_pct_row(
        "#p { width:50px; } #b1 { padding-top:2%; padding-bottom:3%; }",
        "#p { width:50px; }",
        "#p { width:50px; } #b1 { padding-top:1px; padding-bottom:1.5px; }",
        &expected,
    );
}

/// R8 (M8, M9 witness): P's `padding-top:5%` and `padding-bottom:10%` of O's 100: P
/// [10, 15, 26, 36, 36] (reading `padding-top` at the bottom would give 31); B1 [15, 16];
/// F [20, 22]; B2 [22, 23].
#[test]
fn ppm_r8_parent_vertical_padding_percentages() {
    let mut expected = vec![
        pct_lit("p.content_top", 15.0),
        pct_lit("p.content_bottom", 26.0),
        pct_lit("p.padding_bottom", 36.0),
        pct_lit("p.border_bottom", 36.0),
        pct_lit("p.margin_bottom", 36.0),
        pct_lit("p.extent.normal_margin_bottom", 26.0),
        pct_lit("p.extent.selected_bottom", 26.0),
        pct_lit("float.top", 20.0),
        pct_lit("float.bottom", 22.0),
    ];
    expected.extend(pct_y(0, 15.0, 16.0));
    expected.extend(pct_y(1, 20.0, 22.0));
    expected.extend(pct_y(2, 22.0, 23.0));
    assert_pct_row(
        "#p { padding-top:5%; padding-bottom:10%; }",
        "",
        "#p { padding-top:5px; padding-bottom:10px; }",
        &expected,
    );
}

/// R9 (M24 witness): under a 50px P, F's `padding-left:10%` and `padding-top:5%` are 5
/// and 2.5 (O's basis would give 10 and 5): F's margin and border box [0, 15] x [16,
/// 20.5], content [5, 15] x [18.5, 20.5], used height 2; B2's clearance max(20.5 - 19,
/// 16 - 19) = 1.5, border [20.5, 21.5]; P content bottom 24.5, padding and border bottom
/// 25.5.
#[test]
fn ppm_r9_float_padding_percentages() {
    let expected = [
        pct_lit("c1.border.right", 15.0),
        pct_lit("c1.border.bottom", 20.5),
        pct_lit("c1.padding.right", 15.0),
        pct_lit("c1.padding.bottom", 20.5),
        pct_lit("c1.content.left", 5.0),
        pct_lit("c1.content.right", 15.0),
        pct_lit("c1.content.top", 18.5),
        pct_lit("c1.content.bottom", 20.5),
        pct_lit("float.right", 15.0),
        pct_lit("float.bottom", 20.5),
        pct_lit("c2.border.top", 20.5),
        pct_lit("c2.border.bottom", 21.5),
        pct_lit("c2.padding.top", 20.5),
        pct_lit("c2.padding.bottom", 21.5),
        pct_lit("c2.content.top", 20.5),
        pct_lit("c2.content.bottom", 21.5),
        pct_lit("tail0.clearance", 1.5),
        pct_lit("p.content_bottom", 24.5),
        pct_lit("p.padding_bottom", 25.5),
        pct_lit("p.border_bottom", 25.5),
        pct_lit("p.margin_bottom", 25.5),
        pct_lit("p.used_height", 13.5),
        pct_lit("p.extent.normal_margin_bottom", 24.5),
        pct_lit("p.extent.selected_bottom", 24.5),
    ];
    assert_pct_row(
        "#p { width:50px; } #f { padding-left:10%; padding-top:5%; }",
        "#p { width:50px; }",
        "#p { width:50px; } #f { padding-left:5px; padding-top:2.5px; }",
        &expected,
    );
}

/// R10 (M11 witness): under a 50px P, F's `max-width:10%` is 5 (O's basis would give
/// 10): `constrain(10, 0, 5)` = 5, placed [0, 5] x [16, 18].
#[test]
fn ppm_r10_float_max_width_percentage() {
    let mut expected = vec![pct_lit("c1.used_width", 5.0), pct_lit("float.right", 5.0)];
    expected.extend(pct_x(1, "right", 5.0));
    assert_pct_row(
        "#p { width:50px; } #f { max-width:10%; }",
        "#p { width:50px; }",
        "#p { width:50px; } #f { max-width:5px; }",
        &expected,
    );
}

/// R11 (M28 witness): F's `min-width:15%` of 100 is 15: used width 15, placed [0, 15] x
/// [16, 18].
#[test]
fn ppm_r11_float_min_width_percentage() {
    let mut expected = vec![pct_lit("c1.used_width", 15.0), pct_lit("float.right", 15.0)];
    expected.extend(pct_x(1, "right", 15.0));
    assert_pct_row(
        "#f { min-width:15%; }",
        "",
        "#f { min-width:15px; }",
        &expected,
    );
}

/// R12: B1's `max-width:20%` and `min-width:30%`: the max rerun gives 20, below the min
/// 30, so the min wins: width 30, margin-right 70, `passes` 3.
#[test]
fn ppm_r12_min_width_percentage_wins_over_max() {
    let mut expected = vec![
        pct_lit("c0.used_width", 30.0),
        pct_lit("c0.margin.right", 70.0),
        pct_lit("c0.passes", 3.0),
    ];
    expected.extend(pct_x(0, "right", 30.0));
    assert_pct_row(
        "#b1 { max-width:20%; min-width:30%; }",
        "",
        "#b1 { max-width:20px; min-width:30px; }",
        &expected,
    );
}

/// R13 (M15 witness): P's `max-width:50%` of O's 100: P width 50, margin-right 50,
/// `passes` 2, content [0, 50]; B1 and B2 [0, 50]; F keeps [0, 10].
#[test]
fn ppm_r13_parent_max_width_percentage() {
    let mut expected = vec![
        pct_lit("p.width", 50.0),
        pct_lit("p.margin_right", 50.0),
        pct_lit("p.passes", 2.0),
        pct_lit("p.content_right", 50.0),
        pct_lit("c0.used_width", 50.0),
        pct_lit("c2.used_width", 50.0),
    ];
    expected.extend(pct_x(0, "right", 50.0));
    expected.extend(pct_x(2, "right", 50.0));
    assert_pct_row(
        "#p { max-width:50%; }",
        "",
        "#p { max-width:50px; }",
        &expected,
    );
}

/// R13b (review C-8; M27 witness): P's `max-width:50%` resolves against O's content
/// width 200, not a constant 100: P width 100, margin-right 100, `passes` 2 (the wrong
/// basis gives 50). Control and twin use the same O.
#[test]
fn ppm_r13b_parent_constraint_basis_is_the_outer_width() {
    let mut expected = vec![
        pct_lit("p.width", 100.0),
        pct_lit("p.margin_right", 100.0),
        pct_lit("p.passes", 2.0),
        pct_lit("p.content_right", 100.0),
        pct_lit("c0.used_width", 100.0),
        pct_lit("c2.used_width", 100.0),
    ];
    expected.extend(pct_x(0, "right", 100.0));
    expected.extend(pct_x(2, "right", 100.0));
    assert_ppm_row_in(
        "#p { max-width:50%; }",
        "",
        "#p { max-width:100px; }",
        &expected,
        (0.0, 200.0, 10.0),
    );
}

/// R14 (M13 witness): `padding-left:inherit` copies P's computed `padding-left:10%`
/// with origin `Inherited`, which B1 resolves against P's used width 90: P content [10,
/// 100]; B1 padding-left 9, width 81, content [19, 100], padding and border [10, 100].
#[test]
fn ppm_r14_inherit_keeps_the_padding_percentage() {
    let fragment = "#p { padding-left:10%; } #b1 { padding-left:inherit; }";
    let css = pct_css(5, fragment);
    let (rows, _, originals) = pct_facts(5, &css);
    let inherited = BoxFactOrigin::Inherited {
        original_parent: originals[1],
    };
    assert_eq!(rows[2].padding[3], ppm_lp("10", inherited), "{css}");
    let mut expected = vec![
        pct_lit("p.width", 90.0),
        pct_lit("p.content_left", 10.0),
        pct_lit("c0.used_width", 81.0),
        pct_lit("c0.border.left", 10.0),
        pct_lit("c0.padding.left", 10.0),
        pct_lit("c0.content.left", 19.0),
        pct_lit("c2.used_width", 90.0),
        pct_lit("float.left", 10.0),
        pct_lit("float.right", 20.0),
    ];
    expected.extend(pct_x(1, "left", 10.0));
    expected.extend(pct_x(1, "right", 20.0));
    expected.extend(pct_x(2, "left", 10.0));
    assert_pct_row(
        fragment,
        "",
        "#p { padding-left:10px; } #b1 { padding-left:9px; }",
        &expected,
    );
}

/// R15 (M14 witness): `max-width:inherit` copies P's computed `max-width:80%`, which B1
/// resolves against P's used width 80: P width 80, margin-right 20, `passes` 2; B1's
/// maximum 64: width 64, margin-right 16, `passes` 2.
#[test]
fn ppm_r15_inherit_keeps_the_max_width_percentage() {
    let fragment = "#p { max-width:80%; } #b1 { max-width:inherit; }";
    let css = pct_css(5, fragment);
    let (rows, _, originals) = pct_facts(5, &css);
    let inherited = BoxFactOrigin::Inherited {
        original_parent: originals[1],
    };
    assert_eq!(rows[2].max_width, ppm_max("80", inherited), "{css}");
    let mut expected = vec![
        pct_lit("p.width", 80.0),
        pct_lit("p.margin_right", 20.0),
        pct_lit("p.passes", 2.0),
        pct_lit("p.content_right", 80.0),
        pct_lit("c0.used_width", 64.0),
        pct_lit("c0.margin.right", 16.0),
        pct_lit("c0.passes", 2.0),
        pct_lit("c2.used_width", 80.0),
    ];
    expected.extend(pct_x(0, "right", 64.0));
    expected.extend(pct_x(2, "right", 80.0));
    assert_pct_row(
        fragment,
        "",
        "#p { max-width:80px; } #b1 { max-width:64px; }",
        &expected,
    );
}

/// R16: the bounded representation: `18446744073709551616%` (2^64) is
/// `PercentagePrecision` at B1's `padding-left`, `min-width` and `max-width`; the
/// largest stored coefficient `18446744073709551615%` is kept exactly.
#[test]
fn ppm_r16_precision_boundary() {
    for name in ["padding-left", "min-width", "max-width"] {
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
    let css = pct_css(5, "#b1 { padding-left:18446744073709551615%; }");
    let (rows, _, _) = pct_facts(5, &css);
    assert_eq!(
        rows[2].padding[3],
        ppm_lp("18446744073709551615", BoxFactOrigin::Specified),
        "{css}"
    );
}

/// R17 and R17b (M16 witness): under a P of width `f32::MAX`, B1's `padding-left:200%`
/// exceeds `f32::MAX`: `Percentage { source: 2, property: "padding-left", error:
/// Unrepresentable }`; with an equally unrepresentable `margin-left:200%` the margin is
/// reported, because the paddings resolve after the margins.
#[test]
fn ppm_r17_unrepresentable_padding_and_the_resolution_order() {
    for (fragment, property) in [
        ("#b1 { padding-left:200%; }", "padding-left"),
        (
            "#b1 { margin-left:200%; padding-left:200%; }",
            "margin-left",
        ),
    ] {
        let css = pct_css(5, &format!("#p {{ width:{PCT_F32_MAX_PX}; }} {fragment}"));
        let error = pct_error_in(&css, PCT_OUTER);
        assert!(
            matches!(
                error,
                SiblingLayoutError::Percentage {
                    source: 2,
                    property: actual,
                    error: ConversionError::Unrepresentable,
                } if actual == property
            ),
            "{css}: {error:?}"
        );
    }
}

/// R18 (review C-2; M12 witness): an invalid O basis keeps the P solver's own error with
/// P's padding and width-constraint percentages: a NaN width is `containing_width`, a
/// NaN left `InvalidOrigin`. In T18's capture whose lengths are all zero, with the
/// facts' digit limit 2 below the four digits of P's `padding-left:12.25%`, the
/// primitive is never called for an invalid basis, while a valid basis reaches it and
/// fails with `Numeric(DigitLimit)`.
#[test]
fn ppm_r18_invalid_outer_basis_keeps_the_solver_error() {
    let width = (
        (0.0, f32::NAN, 10.0),
        "ParentHorizontal(Width(InvalidInput(\"containing_width\")))",
    );
    let left = ((f32::NAN, 100.0, 10.0), "ParentHorizontal(InvalidOrigin)");
    let css = pct_css(5, "#p { padding-left:10%; min-width:10%; max-width:90%; }");
    for (outer, text) in [width, left] {
        assert_eq!(format!("{:?}", pct_error_in(&css, outer)), text, "{css}");
    }
    let css = "#p { overflow:hidden; padding-left:12.25%; } #b1 { height:0px; } #f { \
               float:left; width:10%; } #b2 { height:0px; clear:both; }";
    let digits = (
        PCT_OUTER,
        "Percentage { source: 1, property: \"padding-left\", error: Numeric(DigitLimit) }",
    );
    for (outer, text) in [width, left, digits] {
        assert_eq!(
            pct_error_with_digits(css, outer, 2),
            text,
            "{css}: digits 2"
        );
    }
}

/// R19 (M25 witness): a child's admission tests precede its resolution: B1's
/// `overflow:scroll` is refused before its unrepresentable `padding-left:200%`.
#[test]
fn ppm_r19_child_admission_precedes_padding_resolution() {
    let css = pct_css(
        5,
        &format!("#p {{ width:{PCT_F32_MAX_PX}; }} #b1 {{ padding-left:200%; overflow:scroll; }}"),
    );
    assert_eq!(
        format!("{:?}", pct_error_in(&css, PCT_OUTER)),
        "Captured(Unsupported { projected: 2, property: \"overflow\", reason: ChildOverflow \
         })",
        "{css}"
    );
}

/// R20 (review C-8; M26 witness): a child's `min-width` basis is P's used width 50:
/// B1's `min-width:120%` is 60 (O's or a constant 100 basis gives 120): width 60,
/// margins (0, -10) (ltr overconstraint), `passes` 2.
#[test]
fn ppm_r20_child_min_width_basis_is_the_parent_width() {
    let mut expected = vec![
        pct_lit("c0.used_width", 60.0),
        pct_lit("c0.margin.right", -10.0),
        pct_lit("c0.passes", 2.0),
    ];
    expected.extend(pct_x(0, "right", 60.0));
    assert_pct_row(
        "#p { width:50px; } #b1 { min-width:120%; }",
        "#p { width:50px; }",
        "#p { width:50px; } #b1 { min-width:60px; }",
        &expected,
    );
}

/// RW4 (review C-4; M19 witness): B1's `padding-left:0.049%` of 100 is one rounding of
/// 0.049 (`3d48b439`), published as B1's content left; the f32 pipelines
/// f32(f32(f32(p) * 100) / 100) and f32(f32(p / 100) * 100) give `3d48b43a`. B1's used
/// width is the exact 100 minus the padding, rounded (`42c7e6e9`); its content right
/// stays 100.
#[test]
fn ppm_rw4_one_rounding_padding() {
    let exact = f32::from_bits(0x3d48b439);
    for pipeline in [0.049_f32 * 100.0 / 100.0, 0.000_49_f32 * 100.0] {
        assert_eq!(pipeline.to_bits(), 0x3d48b43a);
    }
    let expected = [
        pct_lit("c0.content.left", exact),
        pct_lit("c0.content.right", f32::from_bits(0x42c80000)),
        pct_lit("c0.used_width", f32::from_bits(0x42c7e6e9)),
    ];
    assert_pct_row(
        "#b1 { padding-left:0.049%; }",
        "",
        "#b1 { padding-left:0.049px; }",
        &expected,
    );
}

/// FX3: the facts keep `padding-top:2.5%`, `min-width:0%` and `max-width:100%` exactly,
/// `Specified`, with no counted conversion: `numeric_calls` equals the baseline's.
#[test]
fn ppm_fx3_percentage_facts_are_exact_and_unconverted() {
    let css = pct_css(5, "#b1 { padding-top:2.5%; min-width:0%; max-width:100%; }");
    let (rows, receipt, _) = pct_facts(5, &css);
    let (_, baseline, _) = pct_facts(5, &pct_css(5, ""));
    let specified = BoxFactOrigin::Specified;
    assert_eq!(rows[2].padding[0], ppm_lp("2.5", specified), "{css}");
    assert_eq!(rows[2].min_width, ppm_lp("0", specified), "{css}");
    assert_eq!(rows[2].max_width, ppm_max("100", specified), "{css}");
    assert_eq!(rows[2].padding[0].absolute(), None, "{css}");
    assert_eq!(receipt.numeric_calls, baseline.numeric_calls, "{css}");
}

/// FX4: the property grammar drops a negative padding, `min-width` or `max-width`
/// percentage, so B1 keeps the initial facts; the producer's `InvalidValue` test is
/// defensive.
#[test]
fn ppm_fx4_negative_percentages_are_dropped_by_the_parser() {
    for fragment in [
        "#b1 { padding-left:-10%; }",
        "#b1 { min-width:-10%; }",
        "#b1 { max-width:-10%; }",
    ] {
        let css = pct_css(5, fragment);
        let (rows, _, _) = pct_facts(5, &css);
        assert_eq!(rows[2].padding[3], PPM_ZERO, "{css}");
        assert_eq!(rows[2].min_width, PPM_ZERO, "{css}");
        assert_eq!(
            rows[2].max_width,
            BoxMaxFact::None {
                origin: BoxFactOrigin::Initial,
            },
            "{css}"
        );
    }
}

/// The work fixture: PCT's percentages plus padding and width-constraint percentages on
/// every source that resolve to the baseline values.
const PPM_WORK_FRAGMENT: &str = "#p { padding-top:1%; padding-bottom:1%; min-width:0%; \
                                 max-width:100%; } \
                                 #b1, #b2, #b3, #b4 { padding-left:0%; min-width:0%; \
                                 max-width:100%; } \
                                 #f { max-width:100%; }";

/// Facade and core work (M18 witness): exact and one short at N5, N6 and N7 with
/// percentages on every source; the facade charge is `461 + 32N + 449(N-2) +
/// 313(N-3)`; the layout equals the baseline's.
#[test]
fn ppm_work_thresholds_with_percentages() {
    for (nodes, facade, core) in [(5, 2594, 1249), (6, 3388, 1501), (7, 4182, 1754)] {
        assert_eq!(
            facade,
            461 + 32 * nodes + 449 * (nodes - 2) + 313 * (nodes - 3)
        );
        let css = pct_css(nodes, &format!("{PCT_WORK_FRAGMENT} {PPM_WORK_FRAGMENT}"));
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

/// D4 measurement (x86_64; measured at FLIP): the new fact type, the widened maximum
/// and the fact row.
#[test]
fn ppm_representation_measurement() {
    let measured = [
        (
            "LengthPercentageBoxFact",
            std::mem::size_of::<LengthPercentageBoxFact>(),
            std::mem::align_of::<LengthPercentageBoxFact>(),
        ),
        (
            "BoxMaxFact",
            std::mem::size_of::<BoxMaxFact>(),
            std::mem::align_of::<BoxMaxFact>(),
        ),
        (
            "ClearanceBoxFact",
            std::mem::size_of::<ClearanceBoxFact>(),
            std::mem::align_of::<ClearanceBoxFact>(),
        ),
    ];
    for (name, size, align) in measured {
        println!("PPM_REPRESENTATION {name} {size} {align}");
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    assert_eq!(
        measured.map(|(_, size, align)| (size, align)),
        [(32, 8), (32, 8), (848, 8)]
    );
}

// PHT: the height, `min-height` and `max-height` percentage rows reuse the `pct` and
// `ppm` helpers as a child module of this one (design section 6).
#[path = "percentage_heights.rs"]
mod percentage_heights;
