//! ZHN: zero-height Sequence normals whose own top and bottom margins are not adjoining
//! (CSS2.2 8.3.1): a normal (B1 or a tail) with used height +0 is admitted by the core when
//! it establishes a new block formatting context (`overflow:hidden` or `auto`) or when a
//! positive top or bottom border or padding separates its margins, so it cannot collapse
//! through; a zero-height new-BFC border box strictly inside F's margin interval is
//! `NewBfcFloatContact` (CSS2.2 9.5, Q-3). Every other zero-height normal stays
//! `NonpositiveBlockHeight`, and every undecorated or undecidable ordinary Auto-height
//! normal whose constrained tentative height is +0 stays `HeightKind` (a decorated
//! ordinary Auto normal is admitted since DAZ, G6b, and a new-BFC one since MFZ, the
//! user's U4=A; undecidable: its only separation is a vertical padding percentage without
//! P's solve, one that does not resolve or one that rounds to +0, DAZ Q-2).
//!
//! A child module of `em_lengths` (ZHN design section 6): it reuses the `pct`, `pht`, `och`
//! and `eml` helpers (`pct_css`, `pct_with`, `pct_view`, `assert_pct_view`, `pct_lit`,
//! `pct_x`, `pht_refusal`, `eml_row`, `PCT_OUTER`, ...) through `use super::*`. The RED part
//! (the helpers, Z1 and the guards Z7a-Z7e) compiles against the EML commit d695cd9a: it
//! names no new item and reads only published geometry and errors as text. Fixture:
//! `baseline_css()` (O content left 0, width 100, top 10; P `hidden` with padding-top and
//! padding-bottom 1, content top 11; B1 `height:1px; margin-bottom:4px`; F
//! `float:left; width:10px; height:2px`; B2 and B3 cleared), N5 unless stated. Each
//! admitted row compares the listed literals and every other published number with the
//! baseline (or a twin), bit for bit.

use super::*;

/// The facade refusal of a zero-height B1 whose own margins may adjoin.
const ZHN_B1_REFUSAL: &str =
    "Core(Profile(Unsupported { source: 2, reason: NonpositiveBlockHeight }))";

/// The `nodes`-source layout view of `fragment` on `baseline_css()` (`pct_css`) with O's
/// content geometry `PCT_OUTER`, or the `Debug` text of the layout error.
fn zhn_run(nodes: usize, fragment: &str) -> Result<Vec<(String, f32)>, String> {
    let css = pct_css(nodes, fragment);
    pct_with(nodes, &css, PCT_OUTER, layout_limits(), pct_view)
        .map_err(|error| format!("{error:?}"))
}

/// `zhn_run` that must lay out.
fn zhn_layout(nodes: usize, fragment: &str) -> Vec<(String, f32)> {
    zhn_run(nodes, fragment).unwrap_or_else(|error| {
        panic!(
            "{}: expected a layout, got {error}",
            pct_css(nodes, fragment)
        )
    })
}

/// The `Debug` text of the layout error of `fragment`; a layout is a failure.
fn zhn_error(nodes: usize, fragment: &str) -> String {
    match zhn_run(nodes, fragment) {
        Ok(_) => panic!(
            "{}: expected a layout error, got a layout",
            pct_css(nodes, fragment)
        ),
        Err(error) => error,
    }
}

/// An admitted row: `fragment` gives the `expected` literals and every other number of
/// the baseline `pct_css(nodes, "")`, bit for bit.
fn zhn_row(nodes: usize, fragment: &str, expected: &[(String, f32)]) {
    assert_pct_view(
        &zhn_layout(nodes, fragment),
        &zhn_layout(nodes, ""),
        expected,
        &pct_css(nodes, fragment),
    );
}

/// Z1 (RED; MA1, MA3 witness): a zero-height `overflow:hidden` B1 is admitted: its border,
/// padding and content boxes are [11, 11]; F's hypothetical position after B1's bottom
/// margin 4 is 15, so F is [15, 17]; B2's hypothetical top 15 and zero-clearance top 18
/// give border top 17 (clearance -1), B2 [17, 18]; P's content bottom 21 and used height
/// 10. Predecessor (EML commit d695cd9a): the layout fails with
/// `Core(Profile(Unsupported { source: 2, reason: NonpositiveBlockHeight }))`.
#[test]
fn zhn_red_z1_a_zero_height_new_bfc_b1_is_admitted() {
    let mut expected = pct_x(0, "bottom", 11.0);
    expected.push(pct_lit("c0.used_height", 0.0));
    for (side, value) in [("top", 15.0), ("bottom", 17.0)] {
        expected.extend(pct_x(1, side, value));
        expected.push(pct_lit(&format!("float.{side}"), value));
    }
    expected.extend(pct_x(2, "top", 17.0));
    expected.extend(pct_x(2, "bottom", 18.0));
    expected.extend([
        pct_lit("p.content_bottom", 21.0),
        pct_lit("p.padding_bottom", 22.0),
        pct_lit("p.border_bottom", 22.0),
        pct_lit("p.margin_bottom", 22.0),
        pct_lit("p.used_height", 10.0),
        pct_lit("p.extent.normal_margin_bottom", 21.0),
        pct_lit("p.extent.selected_bottom", 21.0),
    ]);
    zhn_row(5, "#b1 { overflow:hidden; height:0; }", &expected);
}

/// Z7a (RED guard; MA8 witness): an ordinary zero-height B1 without top or bottom border
/// or padding may collapse through (G6) and stays `NonpositiveBlockHeight`. Passes before
/// and after ZHN.
#[test]
fn zhn_red_guard_z7a_an_undecorated_ordinary_zero_height_stays_refused() {
    assert_eq!(zhn_error(5, "#b1 { height:0; }"), ZHN_B1_REFUSAL);
}

/// Z7b (RED guard; MA9 witness): left and right borders and paddings do not separate the
/// vertical margins. Passes before and after ZHN.
#[test]
fn zhn_red_guard_z7b_horizontal_decoration_does_not_separate() {
    assert_eq!(
        zhn_error(
            5,
            "#b1 { height:0; border-left:1px solid; border-right:1px solid; \
             padding-left:1px; padding-right:1px; }"
        ),
        ZHN_B1_REFUSAL
    );
}

/// Z7c (RED guard): a top border width with style `none` is a used 0 and does not
/// separate. Passes before and after ZHN.
#[test]
fn zhn_red_guard_z7c_a_borderless_top_width_does_not_separate() {
    assert_eq!(
        zhn_error(5, "#b1 { height:0; border-top-width:1px; }"),
        ZHN_B1_REFUSAL
    );
}

/// Z7d (migrated by MFZ; MN1 witness): a new-BFC Auto-height normal, undecorated or
/// decorated, is an explicit +0 since MFZ (the user's U4=A extends Q3=A; 10.6.7 defines no
/// childless height) and equals its `height:0` twin (before MFZ: `HeightKind`, G6c).
#[test]
fn zhn_z7d_new_bfc_auto_heights_equal_their_zero_twins() {
    for (fragment, twin) in [
        (
            "#b1 { overflow:hidden; height:auto; }",
            "#b1 { overflow:hidden; height:0; }",
        ),
        (
            "#b1 { overflow:auto; height:auto; padding-top:1px; }",
            "#b1 { overflow:auto; height:0; padding-top:1px; }",
        ),
    ] {
        zhn_same(fragment, twin);
    }
}

/// Z7e (RED guard; ZH-4): `height:10px; max-height:0` on an ordinary undecorated B1 stays
/// refused. This is a scope choice for G6/G6b, not a CSS2.2 8.3.1 requirement (the
/// normative list reads the computed height, which is not zero here). Passes before and
/// after ZHN.
#[test]
fn zhn_red_guard_z7e_a_max_height_zero_ordinary_b1_stays_refused() {
    assert_eq!(
        zhn_error(5, "#b1 { height:10px; max-height:0; }"),
        ZHN_B1_REFUSAL
    );
}

// ZHN FLIP rows: they need the widened core admission and guard (and the G6a work).

/// Two admitted N5 rows publish the same numbers, bit for bit.
fn zhn_same(fragment: &str, twin: &str) {
    assert_pct_view(
        &zhn_layout(5, fragment),
        &zhn_layout(5, twin),
        &[],
        &pct_css(5, fragment),
    );
}

/// One published number of `view` by name.
fn zhn_pick(view: &[(String, f32)], name: &str) -> f32 {
    view.iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("{name} not published"))
        .1
}

/// The facade error of a zero-height new-BFC border box strictly inside F's margin
/// interval at `source`.
fn zhn_contact(source: usize) -> String {
    format!("Core(Placement {{ source: {source}, error: NewBfcFloatContact }})")
}

/// Z2 (N6; the TNB C1 flip): a zero-height `overflow:hidden` B3 is admitted at [22, 22]
/// (uncleared, after B2's bottom margin 3 collapsed with its top margin 2); P's content
/// bottom 26 is B3's margin bottom 22 + 4 on the nonnegative route, used height 15.
#[test]
fn zhn_z2_a_zero_height_new_bfc_last_tail_is_admitted() {
    let mut expected = pct_x(3, "bottom", 22.0);
    expected.extend([
        pct_lit("c3.used_height", 0.0),
        pct_lit("p.content_bottom", 26.0),
        pct_lit("p.padding_bottom", 27.0),
        pct_lit("p.border_bottom", 27.0),
        pct_lit("p.margin_bottom", 27.0),
        pct_lit("p.used_height", 15.0),
        pct_lit("p.extent.normal_margin_bottom", 26.0),
        pct_lit("p.extent.selected_bottom", 26.0),
    ]);
    zhn_row(6, "#b3 { overflow:hidden; height:0; }", &expected);
}

/// Z3 (MG6 witness): a zero-height `overflow:hidden` B2 cleared to [18, 18] touches F's
/// margin bottom 18 (contact, Q1=A) and is admitted; clearance stays -1.
#[test]
fn zhn_z3_contact_at_the_float_bottom_is_admitted() {
    let mut expected = pct_x(2, "bottom", 18.0);
    expected.extend([
        pct_lit("c2.used_height", 0.0),
        pct_lit("p.content_bottom", 21.0),
        pct_lit("p.padding_bottom", 22.0),
        pct_lit("p.border_bottom", 22.0),
        pct_lit("p.margin_bottom", 22.0),
        pct_lit("p.used_height", 10.0),
        pct_lit("p.extent.normal_margin_bottom", 21.0),
        pct_lit("p.extent.selected_bottom", 21.0),
    ]);
    zhn_row(5, "#b2 { overflow:hidden; height:0; }", &expected);
}

/// Z4 (MG1, MG7 witness): an uncleared zero-height `overflow:hidden` B2 at 17 lies strictly
/// inside F's margin interval [16, 26] and is `NewBfcFloatContact` at source 4 (the
/// unwidened guard admits its empty interval).
#[test]
fn zhn_z4_a_zero_height_new_bfc_tail_inside_the_float_is_refused() {
    assert_eq!(
        zhn_error(
            5,
            "#f { height:10px; } #b2 { overflow:hidden; height:0; clear:none; margin-top:5px; }"
        ),
        zhn_contact(4)
    );
}

/// Z4b (MG4 witness): a zero-height `overflow:hidden` B1 at 12 (margin-top 1) with
/// margin-bottom -5 puts F's hypothetical top 7 above P's content top, so F is clamped to
/// [11, 13] and B1 lies strictly inside it: `NewBfcFloatContact` at source 2.
#[test]
fn zhn_z4b_a_zero_height_new_bfc_b1_inside_the_float_is_refused() {
    assert_eq!(
        zhn_error(
            5,
            "#b1 { overflow:hidden; height:0; margin-top:1px; margin-bottom:-5px; }"
        ),
        zhn_contact(2)
    );
}

/// Z5 (MG5 witness): an uncleared zero-height `overflow:hidden` B2 at [16, 16] touches F's
/// margin top 16 (contact, Q1=A) and is admitted; F [16, 26] extends P's Auto height.
#[test]
fn zhn_z5_contact_at_the_float_top_is_admitted() {
    let mut expected = pct_x(1, "bottom", 26.0);
    expected.extend(pct_x(2, "top", 16.0));
    expected.extend(pct_x(2, "bottom", 16.0));
    expected.extend([
        pct_lit("c1.used_height", 10.0),
        pct_lit("float.bottom", 26.0),
        pct_lit("c2.used_height", 0.0),
        pct_lit("tail0.cleared", 0.0),
        pct_lit("tail0.clearance", 0.0),
        pct_lit("p.content_bottom", 26.0),
        pct_lit("p.padding_bottom", 27.0),
        pct_lit("p.border_bottom", 27.0),
        pct_lit("p.margin_bottom", 27.0),
        pct_lit("p.used_height", 15.0),
        pct_lit("p.extent.normal_margin_bottom", 19.0),
        pct_lit("p.extent.selected_bottom", 26.0),
        pct_lit("p.extent.float_extended", 1.0),
    ]);
    zhn_row(
        5,
        "#f { height:10px; } #b2 { overflow:hidden; height:0; clear:none; }",
        &expected,
    );
}

/// Z6a (MA4 witness): an ordinary zero-height B1 with a top border is admitted: border
/// [11, 12], padding and content [12, 12].
#[test]
fn zhn_z6a_a_top_border_separates_the_margins() {
    zhn_row(
        5,
        "#b1 { height:0; border-top:1px solid; }",
        &[
            pct_lit("c0.padding.top", 12.0),
            pct_lit("c0.content.top", 12.0),
            pct_lit("c0.used_height", 0.0),
        ],
    );
}

/// Z6b (MA5 witness): a bottom border: border [11, 12], padding and content [11, 11].
#[test]
fn zhn_z6b_a_bottom_border_separates_the_margins() {
    zhn_row(
        5,
        "#b1 { height:0; border-bottom:1px solid; }",
        &[
            pct_lit("c0.padding.bottom", 11.0),
            pct_lit("c0.content.bottom", 11.0),
            pct_lit("c0.used_height", 0.0),
        ],
    );
}

/// Z6c (MA6 witness): a top padding: border and padding [11, 12], content [12, 12].
#[test]
fn zhn_z6c_a_top_padding_separates_the_margins() {
    zhn_row(
        5,
        "#b1 { height:0; padding-top:1px; }",
        &[
            pct_lit("c0.content.top", 12.0),
            pct_lit("c0.used_height", 0.0),
        ],
    );
}

/// Z6d (MA7 witness): a bottom padding: border and padding [11, 12], content [11, 11].
#[test]
fn zhn_z6d_a_bottom_padding_separates_the_margins() {
    zhn_row(
        5,
        "#b1 { height:0; padding-bottom:1px; }",
        &[
            pct_lit("c0.content.bottom", 11.0),
            pct_lit("c0.used_height", 0.0),
        ],
    );
}

/// Z8a (limit route): `height:10px; max-height:0` on a new-BFC B1 reaches the core as +0
/// and equals Z1.
#[test]
fn zhn_z8a_a_max_height_zero_reaches_the_core_as_zero() {
    zhn_same(
        "#b1 { overflow:hidden; height:10px; max-height:0; }",
        "#b1 { overflow:hidden; height:0; }",
    );
}

/// Z8b (percentage route, PHT): `height:0%` of P's explicit used content height 20 equals
/// `height:0px`.
#[test]
fn zhn_z8b_a_zero_percentage_reaches_the_core_as_zero() {
    zhn_same(
        "#p { height:20px; } #b1 { overflow:hidden; height:0%; }",
        "#p { height:20px; } #b1 { overflow:hidden; height:0px; }",
    );
}

/// Z9 (EML route): through the font entry, `height:1em` against B1's `font-size:0px` is +0
/// and equals the `height:0px` twin through the old entry.
#[test]
fn zhn_z9_an_em_against_a_zero_font_size_reaches_the_core_as_zero() {
    eml_row(
        "#b1 { overflow:hidden; font-size:0px; height:1em; }",
        "#b1 { overflow:hidden; height:0px; }",
        &[],
    );
}

/// Z10 (MG2 witness; PR-5): near 2^24 the f32 spacing is 2, so B2's decorated zero-height
/// border box at 16777216 (content top 16777217 ties to 16777216) has a rounded-empty
/// padding interval strictly inside F [16777212, 16777220]; it is not a zero-height border
/// box (top padding 1), so it is admitted. Without the padding it is `NewBfcFloatContact`.
#[test]
fn zhn_z10_a_decorated_rounded_empty_tail_inside_the_float_is_admitted() {
    let fragment = "#b1 { margin-top:16777200px; margin-bottom:0; } #f { height:8px; } \
                    #b2 { overflow:hidden; clear:none; height:0; padding-top:1px; \
                    margin-top:4px; }";
    let view = zhn_layout(5, fragment);
    for (name, value) in [
        ("c0.border.top", 16777211.0),
        ("c0.border.bottom", 16777212.0),
        ("float.top", 16777212.0),
        ("float.bottom", 16777220.0),
        ("c2.border.top", 16777216.0),
        ("c2.border.bottom", 16777216.0),
        ("c2.padding.top", 16777216.0),
        ("c2.padding.bottom", 16777216.0),
        ("c2.content.top", 16777216.0),
        ("c2.content.bottom", 16777216.0),
        ("c2.used_height", 0.0),
        ("p.content_bottom", 16777220.0),
        ("p.used_height", 16777209.0),
        ("p.extent.float_extended", 0.0),
    ] {
        let actual: f32 = zhn_pick(&view, name);
        assert_eq!(
            actual.to_bits(),
            f32::to_bits(value),
            "{fragment}: {name} {actual}"
        );
    }
    assert_eq!(
        zhn_error(5, &fragment.replace(" padding-top:1px;", "")),
        zhn_contact(4)
    );
}

/// Z11 (signed route, Q2=B): a zero-height `overflow:hidden` B2 [18, 18] with
/// margin-bottom -1: the signed extrema give the normal bottom 17, F extends to 18; P's
/// content bottom 18, used height 7.
#[test]
fn zhn_z11_a_zero_height_tail_with_a_negative_margin_takes_the_signed_route() {
    let mut expected = pct_x(2, "bottom", 18.0);
    expected.extend([
        pct_lit("c2.used_height", 0.0),
        pct_lit("c2.margin.bottom", -1.0),
        pct_lit("p.content_bottom", 18.0),
        pct_lit("p.padding_bottom", 19.0),
        pct_lit("p.border_bottom", 19.0),
        pct_lit("p.margin_bottom", 19.0),
        pct_lit("p.used_height", 7.0),
        pct_lit("p.extent.normal_margin_bottom", 17.0),
        pct_lit("p.extent.selected_bottom", 18.0),
        pct_lit("p.extent.float_extended", 1.0),
    ]);
    zhn_row(
        5,
        "#b2 { overflow:hidden; height:0; margin-bottom:-1px; }",
        &expected,
    );
}

/// Z12 (MW1, MW2 witness): core work `702 + 12N + N(N-1)/2 + 54(N-4) + 61(N-2) +
/// 182(N-3)` (G6a: 62 per normal on every path) is exact at N5, N6 and N7 and one short
/// is `Core(Work)`; the facade work is unchanged.
#[test]
fn zhn_z12_core_work_charges_62_per_normal() {
    for (nodes, facade, core) in [(5, 3150, 1373), (6, 4079, 1687), (7, 5008, 2002)] {
        assert_eq!(
            core,
            702 + 12 * nodes
                + nodes * (nodes - 1) / 2
                + 54 * (nodes - 4)
                + 61 * (nodes - 2)
                + 182 * (nodes - 3)
        );
        let css = pct_css(nodes, "");
        let limits = |core_work: usize| {
            let mut limits = layout_limits();
            limits.work = nz(facade);
            limits.core.work = nz(core_work);
            limits
        };
        let receipt = pct_with(nodes, &css, PCT_OUTER, limits(core), |layout| {
            (layout.receipt().work, layout.receipt().core.work)
        })
        .unwrap_or_else(|error| panic!("N{nodes}: expected a layout, got {error:?}"));
        assert_eq!(receipt, (facade, core), "N{nodes}");
        assert_eq!(
            pct_with(nodes, &css, PCT_OUTER, limits(core - 1), |_| ())
                .map_err(|error| format!("{error:?}"))
                .unwrap_err(),
            "Core(Work)",
            "N{nodes} core one short"
        );
    }
}

/// Z15 (P `auto`): B1's border [11, 11] and margin box [11, 15] lie in P's content box
/// [11, 21], so P `auto` presents no overflow and the layout equals Z1.
#[test]
fn zhn_z15_parent_auto_contains_a_zero_height_b1() {
    zhn_same(
        "#p { overflow:auto; } #b1 { overflow:hidden; height:0; }",
        "#b1 { overflow:hidden; height:0; }",
    );
}

// DAZ: decorated Auto-height normals used as zero (G6b) reuse the `pct`, `pht`, `eml` and
// `zhn` helpers as a child module of this one (DAZ design section 6).
#[path = "decorated_auto_normals.rs"]
mod decorated_auto_normals;
