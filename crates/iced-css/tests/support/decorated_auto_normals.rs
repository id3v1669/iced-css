//! DAZ (slice G6b): an ordinary (`overflow:visible`) Sequence normal, B1 or a tail, whose
//! computed `height` is used as `auto` and whose constrained tentative height is +0 has
//! no in-flow children or line boxes, so its height is zero (CSS2.2 10.6.3 "zero,
//! otherwise"); when a positive top or bottom border or padding separates its own top and
//! bottom margins (CSS2.2 8.3.1) it cannot collapse through, and the facade passes it to
//! the core as an explicit +0 (the ZHN admission, G6a). A vertical padding percentage
//! counts with its value against P's used content width (CSS2.2 8.4), read from P's
//! solve; without P's solve, when it does not resolve, or when it rounds to +0 it does
//! not separate by itself (Q-2), and a normal whose only separation it is keeps
//! `HeightKind` when its min-height is nonliteral zero. CTB/CTT (the user's U1=A)
//! admits ordinary Auto B1 and tails with literal min-height zero in unsigned scope.
//! The old nonseparation guards below therefore use B2 with min-height:0%; the new
//! CTT cases separately cover literal admission and the later errors it exposes.
//!
//! A child module of `zero_height_normals` (DAZ design section 6): it reuses `zhn_row`,
//! `zhn_same`, `zhn_error`, `zhn_layout`, `zhn_pick`, `eml_row`, `pct_css`, `pct_with`,
//! `pct_lit`, `pht_error_in`, `pht_refusal`, `PCT_OUTER`, `PCT_F32_MAX_PX`,
//! `layout_limits` and `nz` through `use super::*`. The RED part (the helpers, D1 and the
//! guards DG0-DG8) compiles against the ZHN commit c5aefadb: it names no new item and
//! reads only published geometry and errors as text. Fixture: `baseline_css()` (O content
//! left 0, width 100, top 10; P `hidden` with padding-top and padding-bottom 1, content
//! top 11, used content width 100; B1 `height:1px; margin-bottom:4px`; F
//! `float:left; width:10px; height:2px`; B2 and B3 cleared), N5 unless stated. Each
//! admitted row compares the listed literals and every other published number with the
//! baseline (or its ZHN `height:0` twin), bit for bit.

use super::*;

/// The facade refusal of an Auto-height B2 with nonliteral zero min-height (HK2/CTT).
const DAZ_B2_REFUSAL: &str =
    "Captured(Unsupported { projected: 4, property: \"height\", reason: HeightKind })";

/// The `Debug` text of the N5 layout error of `fragment` with O's content width NaN: P's
/// horizontal solve fails and is held (the `pht` HP precedent), so the height category
/// sees no P solve.
fn daz_held(fragment: &str) -> String {
    pht_error_in(&pct_css(5, fragment), (0.0, f32::NAN, 10.0))
}

/// D1 (RED; MD1, MD3, MD7 witness): an Auto-height B1 with a top border is admitted as an
/// explicit +0: border [11, 12], padding and content [12, 12]; the rest equals the
/// baseline (the ZHN Z6a twin). Predecessor (ZHN commit c5aefadb): the layout fails with
/// `Captured(Unsupported { projected: 2, property: "height", reason: HeightKind })`.
#[test]
fn daz_red_d1_an_auto_height_b1_with_a_top_border_is_admitted() {
    zhn_row(
        5,
        "#b1 { height:auto; border-top:1px solid; }",
        &[
            pct_lit("c0.padding.top", 12.0),
            pct_lit("c0.content.top", 12.0),
            pct_lit("c0.used_height", 0.0),
        ],
    );
}

/// DG0 (RED guard; MD9, MD14 witness): an undecorated ordinary Auto tail may collapse
/// through only with literal min-height zero. CTT keeps these guards outside that
/// admission with min-height:0%, so the lack of separation still yields HeightKind.
#[test]
fn daz_red_guard_dg0_an_undecorated_auto_b1_stays_height_kind() {
    assert_eq!(
        zhn_error(5, "#b2 { height:auto; min-height:0%; }"),
        DAZ_B2_REFUSAL
    );
}

/// DG1 (migrated by MFZ): a decorated new-BFC Auto B1 is an explicit +0 since MFZ (the
/// user's U4=A) and equals its `height:0` twin (before MFZ: `HeightKind`, G6c).
#[test]
fn daz_dg1_a_decorated_new_bfc_auto_b1_equals_its_zero_twin() {
    zhn_same(
        "#b1 { overflow:hidden; height:auto; padding-top:1px; }",
        "#b1 { overflow:hidden; height:0; padding-top:1px; }",
    );
}

/// DG2 (RED guard): left and right borders and paddings do not separate the vertical
/// margins (B2 since CTB). Passes before and after DAZ.
#[test]
fn daz_red_guard_dg2_horizontal_decoration_does_not_separate() {
    assert_eq!(
        zhn_error(
            5,
            "#b2 { height:auto; min-height:0%; border-left:1px solid; border-right:1px solid; \
             padding-left:1px; padding-right:1px; }"
        ),
        DAZ_B2_REFUSAL
    );
}

/// DG3 (RED guard): a top border width with style `none` is a used 0 and does not
/// separate (B2 since CTB). Passes before and after DAZ.
#[test]
fn daz_red_guard_dg3_a_borderless_top_width_does_not_separate() {
    assert_eq!(
        zhn_error(
            5,
            "#b2 { height:auto; min-height:0%; border-top-width:1px; }"
        ),
        DAZ_B2_REFUSAL
    );
}

/// DG4 (RED guard; MD13 witness): `padding-top:0%` resolves to +0 and does not separate.
/// B2 since CTB. Passes before and after DAZ.
#[test]
fn daz_red_guard_dg4_a_zero_percentage_padding_does_not_separate() {
    assert_eq!(
        zhn_error(5, "#b2 { height:auto; min-height:0%; padding-top:0%; }"),
        DAZ_B2_REFUSAL
    );
}

/// DG5 (RED guard; MD12 witness): `padding-top:1%` of P's used content width 0 is +0 and
/// does not separate. P's solve with `width:0` is Ok: the `height:0` twin reaches the
/// core; its negative top margin keeps the explicit zero outside CTT unsigned scope
/// (`NonpositiveBlockHeight` at 4). The Auto arm uses nonliteral min-height zero.
#[test]
fn daz_red_guard_dg5_a_percentage_of_a_zero_width_does_not_separate() {
    assert_eq!(
        zhn_error(
            5,
            "#p { width:0; } #b2 { height:auto; min-height:0%; padding-top:1%; }"
        ),
        DAZ_B2_REFUSAL
    );
    assert_eq!(
        zhn_error(
            5,
            "#p { width:0; } #b2 { height:0; margin-top:-1px; padding-top:1%; }"
        ),
        "Core(Profile(Unsupported { source: 4, reason: NonpositiveBlockHeight }))"
    );
}

/// DG6 (RED guard; MD10 witness): without P's solve (held), a padding percentage cannot
/// be decided and does not separate (Q-2): `HeightKind`, not the held error. Passes
/// before and after DAZ (B2 since CTB).
#[test]
fn daz_red_guard_dg6_a_percentage_without_the_parent_solve_does_not_separate() {
    assert_eq!(
        daz_held("#b2 { height:auto; min-height:0%; padding-top:1%; }"),
        DAZ_B2_REFUSAL
    );
}

/// DG7 (RED guard; MD11 witness): `padding-top:200%` of P's `<f32::MAX>px` does not
/// resolve and does not separate (Q-2): `HeightKind`, and `resolved_row` is not reached.
/// P's solve with that width is Ok: the `height:1px` twin fails in `resolved_row` (B2
/// since CTB).
#[test]
fn daz_red_guard_dg7_an_unresolvable_percentage_does_not_separate() {
    let parent = format!("#p {{ width:{PCT_F32_MAX_PX}; }}");
    assert_eq!(
        zhn_error(
            5,
            &format!("{parent} #b2 {{ height:auto; min-height:0%; padding-top:200%; }}")
        ),
        DAZ_B2_REFUSAL
    );
    assert_eq!(
        zhn_error(
            5,
            &format!("{parent} #b2 {{ height:1px; padding-top:200%; }}")
        ),
        "Percentage { source: 4, property: \"padding-top\", error: Unrepresentable }"
    );
}

/// DG8 (RED guard; N-3): a padding percentage that rounds to +0 (1e-46% of 100) does not
/// separate, as the core's `> 0.` test: `HeightKind` (B2 since CTB). Passes before and
/// after DAZ.
#[test]
fn daz_red_guard_dg8_a_percentage_rounded_to_zero_does_not_separate() {
    assert_eq!(
        zhn_error(
            5,
            "#b2 { height:auto; min-height:0%; \
             padding-top:0.0000000000000000000000000000000000000000000001%; }"
        ),
        DAZ_B2_REFUSAL
    );
}

// DAZ FLIP rows: they need the DAZ arm, the separation test and the DAZ work.

/// D2 (MD4 witness): a bottom border: border [11, 12], padding and content [11, 11].
#[test]
fn daz_d2_a_bottom_border_separates_the_margins() {
    zhn_row(
        5,
        "#b1 { height:auto; border-bottom:1px solid; }",
        &[
            pct_lit("c0.padding.bottom", 11.0),
            pct_lit("c0.content.bottom", 11.0),
            pct_lit("c0.used_height", 0.0),
        ],
    );
}

/// D3 (MD5, MD8 witness; Z7d's former second fragment): a top padding: border and padding
/// [11, 12], content [12, 12].
#[test]
fn daz_d3_a_top_padding_separates_the_margins() {
    zhn_row(
        5,
        "#b1 { height:auto; padding-top:1px; }",
        &[
            pct_lit("c0.content.top", 12.0),
            pct_lit("c0.used_height", 0.0),
        ],
    );
}

/// D4 (MD6 witness): a bottom padding: border and padding [11, 12], content [11, 11].
#[test]
fn daz_d4_a_bottom_padding_separates_the_margins() {
    zhn_row(
        5,
        "#b1 { height:auto; padding-bottom:1px; }",
        &[
            pct_lit("c0.content.bottom", 11.0),
            pct_lit("c0.used_height", 0.0),
        ],
    );
}

/// D5: `padding-top:1%` of P's used content width 100 is 1 and separates; the layout
/// equals the ZHN `height:0; padding-top:1px` twin.
#[test]
fn daz_d5_a_percentage_padding_separates_the_margins() {
    zhn_same(
        "#b1 { height:auto; padding-top:1%; }",
        "#b1 { height:0; padding-top:1px; }",
    );
}

/// D5b: the basis is P's used content width 50, not O's 100: `padding-top:1%` is 0.5, so
/// B1's border is [11, 11.5] and content [11.5, 11.5]; F [15.5, 17.5]; B2 [17.5, 18.5]
/// (clearance -1); P's content bottom 21.5, used height 10.5.
#[test]
fn daz_d5b_a_percentage_padding_reads_the_parent_used_width() {
    let fragment = "#p { width:50px; } #b1 { height:auto; padding-top:1%; }";
    zhn_same(
        fragment,
        "#p { width:50px; } #b1 { height:0; padding-top:0.5px; }",
    );
    let view = zhn_layout(5, fragment);
    for (name, value) in [
        ("c0.border.top", 11.0),
        ("c0.border.bottom", 11.5),
        ("c0.content.top", 11.5),
        ("c0.content.bottom", 11.5),
        ("c0.used_height", 0.0),
        ("float.top", 15.5),
        ("float.bottom", 17.5),
        ("c2.border.top", 17.5),
        ("c2.border.bottom", 18.5),
        ("tail0.clearance", -1.0),
        ("p.content_bottom", 21.5),
        ("p.used_height", 10.5),
    ] {
        let actual: f32 = zhn_pick(&view, name);
        assert_eq!(
            actual.to_bits(),
            f32::to_bits(value),
            "{fragment}: {name} {actual}"
        );
    }
}

/// D6 (PHT): a `height:50%` under P's Auto height is used as `auto`; with a top padding
/// it equals the ZHN `height:0` twin.
#[test]
fn daz_d6_a_percentage_height_used_as_auto_is_admitted() {
    zhn_same(
        "#b1 { height:50%; padding-top:1px; }",
        "#b1 { height:0; padding-top:1px; }",
    );
}

/// D7 (PHT, the H7 twin): a `min-height:50%` under P's Auto height is used as 0, so the
/// constrained tentative height stays +0 and the decorated B1 equals the `height:0` twin.
#[test]
fn daz_d7_a_percentage_min_height_used_as_zero_is_admitted() {
    zhn_same(
        "#b1 { height:auto; min-height:50%; padding-top:1px; }",
        "#b1 { height:0; padding-top:1px; }",
    );
}

/// D8 (cleared tail): an Auto-height B2 with a bottom border: border [18, 19] (clearance
/// -1), padding and content [18, 18]; P unchanged.
#[test]
fn daz_d8_a_cleared_auto_height_tail_is_admitted() {
    zhn_row(
        5,
        "#b2 { height:auto; border-bottom:1px solid; }",
        &[
            pct_lit("c2.padding.bottom", 18.0),
            pct_lit("c2.content.bottom", 18.0),
            pct_lit("c2.used_height", 0.0),
        ],
    );
}

/// D9 (N6, the last tail): an Auto-height B3 with a bottom padding: border and padding
/// [22, 23], content [22, 22]; P unchanged.
#[test]
fn daz_d9_the_last_auto_height_tail_is_admitted() {
    zhn_row(
        6,
        "#b3 { height:auto; padding-bottom:1px; }",
        &[
            pct_lit("c3.content.bottom", 22.0),
            pct_lit("c3.used_height", 0.0),
        ],
    );
}

/// D10 (signed route, Q2=B): an Auto-height B2 with a top padding and margin-bottom -1:
/// border [18, 19], content [19, 19]; the signed extrema give the normal bottom 18 and F
/// does not extend it; P's content bottom 18, used height 7.
#[test]
fn daz_d10_an_auto_height_tail_takes_the_signed_route() {
    zhn_row(
        5,
        "#b2 { height:auto; padding-top:1px; margin-bottom:-1px; }",
        &[
            pct_lit("c2.content.top", 19.0),
            pct_lit("c2.used_height", 0.0),
            pct_lit("c2.margin.bottom", -1.0),
            pct_lit("p.content_bottom", 18.0),
            pct_lit("p.padding_bottom", 19.0),
            pct_lit("p.border_bottom", 19.0),
            pct_lit("p.margin_bottom", 19.0),
            pct_lit("p.used_height", 7.0),
            pct_lit("p.extent.normal_margin_bottom", 18.0),
            pct_lit("p.extent.selected_bottom", 18.0),
        ],
    );
}

/// D11 (P `auto`): B1's border [11, 12] and margin box [11, 16] lie in P's content box
/// [11, 22], so P `auto` presents no overflow and the layout equals D1's.
#[test]
fn daz_d11_parent_auto_contains_a_decorated_auto_b1() {
    zhn_same(
        "#p { overflow:auto; } #b1 { height:auto; border-top:1px solid; }",
        "#b1 { height:auto; border-top:1px solid; }",
    );
}

/// D12 (error order): with P's solve held, an admitted B1 (an absolute top padding)
/// continues past the height category, and the held error is returned at F's width step
/// (predecessor: `HeightKind`).
#[test]
fn daz_d12_the_held_parent_error_follows_an_admitted_b1() {
    assert_eq!(
        daz_held("#b1 { height:auto; padding-top:1px; }"),
        "ParentHorizontal(Width(InvalidInput(\"containing_width\")))"
    );
}

/// D13 (error order): a later child's refusal follows an admitted B1 (predecessor:
/// `HeightKind`).
#[test]
fn daz_d13_a_later_child_refusal_follows_an_admitted_b1() {
    assert_eq!(
        zhn_error(
            5,
            "#b1 { height:auto; padding-top:1px; } #b2 { overflow:scroll; }"
        ),
        pht_refusal(4, "overflow", "ChildOverflow")
    );
}

/// D14 (PTB, the V8 twin): an admitted B1's `top:200%` of P's `<f32::MAX>px` fails after
/// its height category (predecessor: `HeightKind`).
#[test]
fn daz_d14_an_admitted_b1_reaches_its_offset_resolution() {
    let fragment = format!(
        "#p {{ height:{PCT_F32_MAX_PX}; }} #b1 {{ height:auto; padding-top:1px; \
         position:relative; top:200%; }}"
    );
    assert_eq!(
        zhn_error(5, &fragment),
        "Percentage { source: 2, property: \"top\", error: Unrepresentable }"
    );
}

/// D15 (EML): through the font entry, `padding-top:0.1em` against B1's `font-size:10px` is
/// exactly 1 and the layout equals the `height:0; padding-top:1px` twin through the old
/// entry.
#[test]
fn daz_d15_an_em_padding_separates_the_margins() {
    eml_row(
        "#b1 { font-size:10px; height:auto; padding-top:0.1em; }",
        "#b1 { height:0; padding-top:1px; }",
        &[],
    );
}

/// D16 (MW1, MW2 witness): facade work `752 + 32N + 534(N-2) + 373(N-3)` (DAZ: 58 per
/// normal on every path) is exact at N5, N6 and N7 and one short is `Work`; the core work
/// is unchanged.
#[test]
fn daz_d16_facade_work_charges_58_per_normal() {
    for (nodes, facade, core) in [(5, 3260, 2057), (6, 4199, 2671), (7, 5138, 3286)] {
        assert_eq!(
            facade,
            752 + 32 * nodes + 534 * (nodes - 2) + 373 * (nodes - 3),
            "N{nodes}"
        );
        let css = pct_css(nodes, "");
        let limits = |work: usize| {
            let mut limits = layout_limits();
            limits.work = nz(work);
            limits.core.work = nz(core);
            limits
        };
        let receipt = pct_with(nodes, &css, PCT_OUTER, limits(facade), |layout| {
            (layout.receipt().work, layout.receipt().core.work)
        })
        .unwrap_or_else(|error| panic!("N{nodes}: expected a layout, got {error:?}"));
        assert_eq!(receipt, (facade, core), "N{nodes}");
        assert_eq!(
            pct_with(nodes, &css, PCT_OUTER, limits(facade - 1), |_| ())
                .map_err(|error| format!("{error:?}"))
                .unwrap_err(),
            "Work",
            "N{nodes} facade one short"
        );
    }
}

/// D17 (error order, M-2): F's `FloatWidth` follows an admitted B1 (predecessor:
/// `HeightKind`).
#[test]
fn daz_d17_the_float_width_refusal_follows_an_admitted_b1() {
    assert_eq!(
        zhn_error(
            5,
            "#b1 { height:auto; padding-top:1px; } #f { width:auto; }"
        ),
        pht_refusal(3, "width", "FloatWidth")
    );
}

/// D18 (error order, M-2): an admitted B1's own width-axis resolution fails after its
/// height category: `margin-left:200%` of P's `<f32::MAX>px` (predecessor: `HeightKind`).
#[test]
fn daz_d18_an_admitted_b1_reaches_its_width_axis_resolution() {
    let fragment = format!(
        "#p {{ width:{PCT_F32_MAX_PX}; }} #b1 {{ height:auto; padding-top:1px; \
         margin-left:200%; }}"
    );
    assert_eq!(
        zhn_error(5, &fragment),
        "Percentage { source: 2, property: \"margin-left\", error: Unrepresentable }"
    );
}

/// D19 (mixed Q-2, M-2): a positive `padding-bottom` separates even though `padding-top`
/// does not resolve; the admitted B1 then fails in `resolved_row` at `padding-top`
/// (predecessor: `HeightKind`).
#[test]
fn daz_d19_a_mixed_separation_fails_at_the_unresolvable_padding() {
    let fragment = format!(
        "#p {{ width:{PCT_F32_MAX_PX}; }} #b1 {{ height:auto; padding-top:200%; \
         padding-bottom:1px; }}"
    );
    assert_eq!(
        zhn_error(5, &fragment),
        "Percentage { source: 2, property: \"padding-top\", error: Unrepresentable }"
    );
}

// OAH: P's `top`/`bottom` percentages against an Auto-height O (G4d-3b-P) reuse the `pct`,
// `pht`, `ptb`, `och` and `eml` helpers as a child module of this one (OAH design section 6).
#[path = "outer_auto_heights.rs"]
mod outer_auto_heights;
