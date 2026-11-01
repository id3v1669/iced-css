//! MFZ (user decisions U3=A and U4=A, 2026-10-29). U3: under an Auto O whose tentative
//! content height (P's bottom margin edge minus O's content top) is negative, CSS2.2 10.7
//! is applied literally: the negative tentative height is not greater than O's
//! `max-height` and is smaller than O's `min-height` (0 by default), so O's used content
//! height, the basis of P's `top` and `bottom` percentages, is O's `min-height`. U4: a
//! new-BFC (`overflow:hidden` or `auto`) normal, B1 or a tail, whose computed `height` is
//! used as `auto` and whose constrained tentative height is +0 has the used height zero
//! (Q3=A extended from F) and goes to the core as an explicit +0, which the ZHN core
//! admits (its own margins are not adjoining, CSS2.2 8.3.1).
//!
//! A child module of `inherited_offsets` (MFZ design section 6): it reuses
//! `assert_pct_row`, `assert_pct_view`, `pct_css`, `pct_lit`, `pct_with`, `pht_refusal`,
//! `eml_row`, `zhn_layout`, `zhn_error`, `oah_error`, `daz_held`, `PCT_OUTER`,
//! `PCT_F32_MAX_PX`, `layout_limits` and `nz` through `use super::*`. The RED part
//! (`mfz_same`, `mfz_offsets`, F1, Z1 and the guards FG1, ZG1 and ZG2) compiles against
//! the ISP commit 888f0769: it names no new item and reads only published geometry and
//! errors as text. Fixture:
//! `baseline_css()` (O content left 0, width 100, top 10; P `hidden` with padding-top and
//! padding-bottom 1; B1 `height:1px; margin-bottom:4px`; F `float:left; width:10px;
//! height:2px`; B2 and B3 cleared), N5 unless stated.

use super::*;

/// Two admitted `nodes`-source rows publish the same numbers, bit for bit.
fn mfz_same(nodes: usize, fragment: &str, twin: &str) {
    assert_pct_view(
        &zhn_layout(nodes, fragment),
        &zhn_layout(nodes, twin),
        &[],
        &pct_css(nodes, fragment),
    );
}

/// The offset literals of P: `top` and `bottom`.
fn mfz_offsets(top: f32, bottom: f32) -> [(String, f32); 2] {
    [
        pct_lit("p.offset.top", top),
        pct_lit("p.offset.bottom", bottom),
    ]
}

/// F1 (RED; MF1, MF2, MF3 witness): P's `margin-top:-20px` puts P's margin bottom at 3,
/// so O's tentative content height is 3 - 10 = -7; it is smaller than O's
/// `min-height:4px`, which is used (10.7): `top:50%` is 2 and `bottom` -2. Predecessor
/// (ISP commit 888f0769): `Captured(Unsupported { projected: 1, property: "top", reason:
/// IndefiniteOffsetBasis })` after P's bottom.
#[test]
fn mfz_red_f1_a_negative_tentative_outer_height_takes_the_min_height() {
    assert_pct_row(
        "#o { min-height:4px; } #p { position:relative; top:50%; margin-top:-20px; }",
        "#o { min-height:4px; } #p { position:relative; margin-top:-20px; }",
        "#o { min-height:4px; } #p { position:relative; top:2px; margin-top:-20px; }",
        &mfz_offsets(2.0, -2.0),
    );
}

/// FG1 (RED guard): without a viewport height (the old entries; VPH, the user's U2=A) a
/// percentage `min-height` of O keeps the refusal at P's step, also with a negative
/// tentative height. Passes before and after MFZ.
#[test]
fn mfz_red_guard_fg1_a_percentage_outer_limit_keeps_the_refusal() {
    assert_eq!(
        oah_error("#o { min-height:10%; } #p { position:relative; top:50%; margin-top:-20px; }"),
        pht_refusal(1, "top", "IndefiniteOffsetBasis")
    );
}

/// Z1 (RED; MN1, MN3 witness; N6): an Auto-height `overflow:hidden` B3, the last tail, is
/// an explicit +0 and equals the ZHN Z2 `height:0` twin: B3 [22, 22], P's content bottom
/// 26 and used height 15. Predecessor (ISP commit 888f0769): the layout fails with
/// `Captured(Unsupported { projected: 5, property: "height", reason: HeightKind })`.
#[test]
fn mfz_red_z1_an_auto_height_new_bfc_tail_is_zero() {
    mfz_same(
        6,
        "#b3 { overflow:hidden; height:auto; }",
        "#b3 { overflow:hidden; height:0; }",
    );
}

/// ZG1 (RED guard): a new-BFC Auto B1 with a positive `min-height` keeps that used height
/// (MMX). Passes before and after MFZ.
#[test]
fn mfz_red_guard_zg1_a_positive_min_height_is_used() {
    mfz_same(
        5,
        "#b1 { overflow:hidden; height:auto; min-height:2px; }",
        "#b1 { overflow:hidden; height:2px; }",
    );
}

/// ZG2 (RED guard): `overflow:scroll` stays `ChildOverflow` before the height category.
/// Passes before and after MFZ.
#[test]
fn mfz_red_guard_zg2_scroll_stays_refused() {
    assert_eq!(
        zhn_error(5, "#b1 { overflow:scroll; height:auto; }"),
        pht_refusal(2, "overflow", "ChildOverflow")
    );
}

// MFZ FLIP rows: they need the product change (P1-P3): the U3 floor and the U4 term.

/// F2 (MF5 witness): O's `max-height:5px` does not apply to -7; the default `min-height` 0
/// is used: `top:50%` is +0.
#[test]
fn mfz_f2_the_outer_max_height_does_not_apply() {
    assert_pct_row(
        "#o { max-height:5px; } #p { position:relative; top:50%; margin-top:-20px; }",
        "#o { max-height:5px; } #p { position:relative; margin-top:-20px; }",
        "#o { max-height:5px; } #p { position:relative; top:0px; margin-top:-20px; }",
        &mfz_offsets(0.0, 0.0),
    );
}

/// F3 (MF4 witness): with `min-height:6px; max-height:2px` the min wins (10.7 order):
/// `top:50%` is 3.
#[test]
fn mfz_f3_the_min_height_wins_over_the_max_height() {
    let outer = "#o { min-height:6px; max-height:2px; }";
    assert_pct_row(
        &format!("{outer} #p {{ position:relative; top:50%; margin-top:-20px; }}"),
        &format!("{outer} #p {{ position:relative; margin-top:-20px; }}"),
        &format!("{outer} #p {{ position:relative; top:3px; margin-top:-20px; }}"),
        &mfz_offsets(3.0, -3.0),
    );
}

/// F4: P's `bottom:50%` of O's `min-height:4px` is 2, so `top` is -2.
#[test]
fn mfz_f4_a_bottom_percentage_takes_the_min_height() {
    assert_pct_row(
        "#o { min-height:4px; } #p { position:relative; bottom:50%; margin-top:-20px; }",
        "#o { min-height:4px; } #p { position:relative; margin-top:-20px; }",
        "#o { min-height:4px; } #p { position:relative; bottom:2px; margin-top:-20px; }",
        &mfz_offsets(-2.0, 2.0),
    );
}

/// F5 (EML): O's `min-height:0.5em` with O's `font-size:10px` is 5 in the font entry's
/// facts: the layout equals the `min-height:5px; top:2.5px` twin.
#[test]
fn mfz_f5_an_em_outer_min_height_is_used() {
    eml_row(
        "#o { font-size:10px; min-height:0.5em; } \
         #p { position:relative; top:50%; margin-top:-20px; }",
        "#o { min-height:5px; } #p { position:relative; top:2.5px; margin-top:-20px; }",
        &[],
    );
}

/// F6 (error order): O's `min-height:<f32::MAX>px` is the basis, and P's `top:200%` of it
/// is `Percentage` (predecessor: `IndefiniteOffsetBasis` after P's bottom).
#[test]
fn mfz_f6_a_min_height_basis_reaches_the_percentage_resolution() {
    assert_eq!(
        oah_error(&format!(
            "#o {{ min-height:{PCT_F32_MAX_PX}; }} \
             #p {{ position:relative; top:200%; margin-top:-20px; }}"
        )),
        "Percentage { source: 1, property: \"top\", error: Unrepresentable }"
    );
}

/// F7 (error order; design review m1): O's tentative height -7 takes the default
/// `min-height` 0, so P's `top:50%` is +0; B1's `top:50px` then moves its margin box out of
/// P's `auto` content box: `ParentAutoOverflow` at B1 (predecessor: `IndefiniteOffsetBasis`
/// after P's bottom).
#[test]
fn mfz_f7_a_parent_auto_overflow_follows() {
    assert_eq!(
        oah_error(
            "#p { position:relative; top:50%; margin-top:-20px; overflow:auto; } \
             #b1 { position:relative; top:50px; }"
        ),
        "ParentAutoOverflow { source: 2 }"
    );
}

/// Z2 (MN2 witness): an undecorated `overflow:auto` B1 with `max-height:0`.
#[test]
fn mfz_z2_an_auto_overflow_b1_is_zero() {
    mfz_same(
        5,
        "#b1 { overflow:auto; height:auto; max-height:0; }",
        "#b1 { overflow:auto; height:0; }",
    );
}

/// Z3 (N7): the middle tail B3 and the last tail B4 (`pct_css` gives B4 `height:2px`) are
/// both Auto-height new-BFC boxes: B3 [22, 22], B4 [26, 26], P's content bottom 28.
#[test]
fn mfz_z3_two_auto_height_new_bfc_tails_are_zero() {
    mfz_same(
        7,
        "#b3, #b4 { overflow:hidden; height:auto; }",
        "#b3, #b4 { overflow:hidden; height:0; }",
    );
}

/// Z4: a cleared B2 at [18, 18] touches F's margin bottom (the ZHN Z3 twin).
#[test]
fn mfz_z4_contact_at_the_float_bottom() {
    mfz_same(
        5,
        "#b2 { overflow:hidden; height:auto; }",
        "#b2 { overflow:hidden; height:0; }",
    );
}

/// Z5: an uncleared B2 at [16, 16] touches F's margin top (the ZHN Z5 twin).
#[test]
fn mfz_z5_contact_at_the_float_top() {
    mfz_same(
        5,
        "#f { height:10px; } #b2 { overflow:hidden; height:auto; clear:none; }",
        "#f { height:10px; } #b2 { overflow:hidden; height:0; clear:none; }",
    );
}

/// Z6: an uncleared B2 at 17 strictly inside F [16, 26] is `NewBfcFloatContact` (the ZHN
/// Z4 twin).
#[test]
fn mfz_z6_a_tail_inside_the_float_is_refused() {
    assert_eq!(
        zhn_error(
            5,
            "#f { height:10px; } #b2 { overflow:hidden; height:auto; clear:none; margin-top:5px; }"
        ),
        "Core(Placement { source: 4, error: NewBfcFloatContact })"
    );
}

/// Z7: B1 at 12 strictly inside the clamped F [11, 13] is `NewBfcFloatContact` (the ZHN
/// Z4b twin).
#[test]
fn mfz_z7_a_b1_inside_the_float_is_refused() {
    assert_eq!(
        zhn_error(
            5,
            "#b1 { overflow:hidden; height:auto; margin-top:1px; margin-bottom:-5px; }"
        ),
        "Core(Placement { source: 2, error: NewBfcFloatContact })"
    );
}

/// Z8 (signed route, Q2=B): B2 [18, 18] with `margin-bottom:-1px` (the ZHN Z11 twin).
#[test]
fn mfz_z8_the_signed_route() {
    mfz_same(
        5,
        "#b2 { overflow:hidden; height:auto; margin-bottom:-1px; }",
        "#b2 { overflow:hidden; height:0; margin-bottom:-1px; }",
    );
}

/// Z9 (PHT): `height:50%` and `min-height:50%` under P's Auto height are `auto` and 0.
#[test]
fn mfz_z9_percentages_under_an_auto_parent() {
    for fragment in [
        "#b1 { overflow:hidden; height:50%; }",
        "#b1 { overflow:hidden; height:auto; min-height:50%; }",
    ] {
        mfz_same(5, fragment, "#b1 { overflow:hidden; height:0; }");
    }
}

/// Z10 (P `auto`): B1's margin box [11, 15] lies in P's content box [11, 21].
#[test]
fn mfz_z10_parent_auto_contains_the_zero_b1() {
    mfz_same(
        5,
        "#p { overflow:auto; } #b1 { overflow:hidden; height:auto; }",
        "#p { overflow:auto; } #b1 { overflow:hidden; height:0; }",
    );
}

/// Z11 (error order; MN4 witness): with P's solve held, the held error follows the
/// admitted B1 (predecessor: `HeightKind`).
#[test]
fn mfz_z11_the_held_parent_error_follows() {
    assert_eq!(
        daz_held("#b1 { overflow:hidden; height:auto; }"),
        "ParentHorizontal(Width(InvalidInput(\"containing_width\")))"
    );
}

/// Z12 (error order): a later child's refusal follows (predecessor: `HeightKind`).
#[test]
fn mfz_z12_a_later_child_refusal_follows() {
    assert_eq!(
        zhn_error(
            5,
            "#b1 { overflow:hidden; height:auto; } #b2 { overflow:scroll; }"
        ),
        pht_refusal(4, "overflow", "ChildOverflow")
    );
}

/// Z13 (error order): B1's unresolvable `padding-top:200%` of P's `<f32::MAX>px` now
/// fails in `resolved_row` (predecessor: `HeightKind`).
#[test]
fn mfz_z13_an_unresolvable_padding_follows() {
    assert_eq!(
        zhn_error(
            5,
            &format!(
                "#p {{ width:{PCT_F32_MAX_PX}; }} \
                 #b1 {{ overflow:hidden; height:auto; padding-top:200%; }}"
            )
        ),
        "Percentage { source: 2, property: \"padding-top\", error: Unrepresentable }"
    );
}

/// Z14 (error order): F's `FloatWidth` follows (predecessor: `HeightKind`).
#[test]
fn mfz_z14_the_float_width_refusal_follows() {
    assert_eq!(
        zhn_error(
            5,
            "#b1 { overflow:hidden; height:auto; } #f { width:auto; }"
        ),
        pht_refusal(3, "width", "FloatWidth")
    );
}

/// Z15 (work): with every normal an Auto-height new-BFC box the facade work stays
/// `752 + 32N + 542(N-2) + 373(N-3)`, exact and one short, and the core work too.
/// (The formula also carries VPH's 45 and CTB's 25 + 8(N-2) + 2(N-3).)
#[test]
fn mfz_z15_the_work_is_unchanged() {
    for (nodes, facade, core) in [(5, 3284, 2117), (6, 4231, 2755), (7, 5178, 3394)] {
        let css = pct_css(
            nodes,
            "#b1, #b2, #b3, #b4 { overflow:hidden; height:auto; } #b2, #b3 { clear:none; }",
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
        assert_eq!(
            pct_with(nodes, &css, PCT_OUTER, limits(facade - 1, core), |_| ())
                .map_err(|error| format!("{error:?}"))
                .unwrap_err(),
            "Work",
            "N{nodes} facade one short"
        );
    }
}

/// Z16 (error order; design review m1): B1's Auto height is admitted, so its own
/// `top:200%` of P's explicit `height:<f32::MAX>px` is resolved and is unrepresentable
/// (predecessor: `HeightKind` at B1).
#[test]
fn mfz_z16_the_same_child_offset_percentage_follows() {
    assert_eq!(
        zhn_error(
            5,
            &format!(
                "#p {{ height:{PCT_F32_MAX_PX}; }} \
                 #b1 {{ overflow:hidden; height:auto; position:relative; top:200%; }}"
            )
        ),
        "Percentage { source: 2, property: \"top\", error: Unrepresentable }"
    );
}

/// Z17 (error order; design review m1): B1's Auto height is admitted, and its `top:50px`
/// moves its margin box out of P's `auto` content box: `ParentAutoOverflow` at B1
/// (predecessor: `HeightKind` at B1).
#[test]
fn mfz_z17_a_parent_auto_overflow_follows() {
    assert_eq!(
        zhn_error(
            5,
            "#p { overflow:auto; } \
             #b1 { overflow:hidden; height:auto; position:relative; top:50px; }"
        ),
        "ParentAutoOverflow { source: 2 }"
    );
}

// EXF: the user's U5=B (`ex` as the 0.5em fallback) reuses the `eml`, `pct`, `zhn` and `isp`
// helpers as a child module of this one (EXF design section 5); MZ-6's Z16b comes first.
#[path = "ex_fallback_lengths.rs"]
mod ex_fallback_lengths;
