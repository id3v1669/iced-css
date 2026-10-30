//! OAH (slice G4d-3b-P): under an O whose computed `height` is `auto`, P's own `top` or
//! `bottom` percentage resolves against O's used content height (CSS2.2 9.3.2, 9.4.3:
//! offsets apply after layout). O is the captured root, whose margins do not collapse
//! (CSS2.2 8.3.1), so its tentative content height runs from its content top to P's
//! bottom margin edge (10.6.3, one f32 subtraction); O's `min-height` and `max-height`
//! then constrain it (10.7). A percentage limit of O (its basis, the initial containing
//! block's height, which the old entries do not supply; VPH, the user's U2=A) keeps
//! `IndefiniteOffsetBasis` at P's step, a negative tentative height takes O's
//! `min-height` (MFZ, the user's U3=A; before MFZ it was `IndefiniteOffsetBasis` after
//! P's bottom), and a host value is not read under an `auto` O.
//!
//! A child module of `decorated_auto_normals` (OAH design section 6): it reuses
//! `assert_pct_row`, `assert_pct_view`, `pct_css`, `pct_layout`, `pct_layout_in`, `pct_lit`,
//! `pct_with`, `pht_error_in`, `pht_refusal`, `ptb_assert_indefinite`, `och_layout`, `eml_row`,
//! `PCT_OUTER`, `PCT_F32_MAX_PX`, `layout_limits` and `nz` through `use super::*`. The
//! RED part (`oah_error`, A1 and the guards AG1-AG5) compiles against the DAZ commit
//! ed6b3dba: it names no new item and reads only published geometry and errors as text.
//! Fixture: `baseline_css()` (O content left 0, width 100, top 10; P `hidden` with
//! padding-top and padding-bottom 1: border top 10, content top 11, content bottom 22,
//! margin bottom 23, so O's used content height is 13, and 18 at N6), N5 unless stated.
//! Each admitted row compares the listed literals and every other published number with
//! its control (the same fragment without the percentage side), and every number with
//! its absolute twin, bit for bit.

use super::*;

/// The `Debug` text of the N5 layout error of `fragment` with O's content geometry
/// `PCT_OUTER`.
fn oah_error(fragment: &str) -> String {
    pht_error_in(&pct_css(5, fragment), PCT_OUTER)
}

/// A1 (RED; MO1, MO8, MO12, MO13, MO16 witness): under the Auto O, P's `top:50%` of O's
/// used content height 13 is 6.5, so `bottom` is -6.5; every other number is the
/// control's. Predecessor (DAZ commit ed6b3dba): the layout fails with
/// `Captured(Unsupported { projected: 1, property: "top", reason: IndefiniteOffsetBasis })`.
#[test]
fn oah_red_a1_parent_top_percentage_of_an_auto_outer_height() {
    let expected = [
        pct_lit("p.offset.top", 6.5),
        pct_lit("p.offset.bottom", -6.5),
    ];
    assert_pct_row(
        "#p { position:relative; top:50%; }",
        "#p { position:relative; }",
        "#p { position:relative; top:6.5px; }",
        &expected,
    );
}

/// AG1 (RED guard; MO3 witness): O's `min-height:10%` refers to the initial containing
/// block's height, which the old entries do not supply (VPH, the user's U2=A), so P's
/// `top:50%` stays `IndefiniteOffsetBasis` at P's step. Passes before and after OAH.
#[test]
fn oah_red_guard_ag1_a_percentage_outer_min_height_keeps_the_refusal() {
    ptb_assert_indefinite(&[(
        "#o { min-height:10%; } #p { position:relative; top:50%; }",
        1,
        "top",
    )]);
}

/// AG2 (RED guard; MO4 witness): O's `max-height:10%` keeps P's `bottom:50%`
/// `IndefiniteOffsetBasis` at P's step. Passes before and after OAH.
#[test]
fn oah_red_guard_ag2_a_percentage_outer_max_height_keeps_the_refusal() {
    ptb_assert_indefinite(&[(
        "#o { max-height:10%; } #p { position:relative; bottom:50%; }",
        1,
        "bottom",
    )]);
}

/// AG3 (RED guard): the refusal for O's percentage limit stays at P's step, before every
/// child error (B1's `overflow:scroll`). Passes before and after OAH.
#[test]
fn oah_red_guard_ag3_the_limit_refusal_precedes_the_children() {
    ptb_assert_indefinite(&[(
        "#o { min-height:10%; } #p { position:relative; top:50%; } #b1 { overflow:scroll; }",
        1,
        "top",
    )]);
}

/// AG4 (migrated by MFZ; MF1, MF3 witness): with P's `margin-top:-20px`, P's border top
/// is -10, its content top -9, content bottom 2 and margin bottom 3, so O's tentative
/// content height is -7; MFZ (the user's U3=A) applies CSS2.2 10.7 literally and uses O's
/// default `min-height` 0, so P's `top:50%` is +0 and `bottom` +0 (before MFZ:
/// `IndefiniteOffsetBasis` after P's bottom).
#[test]
fn oah_ag4_a_negative_tentative_outer_height_takes_the_default_min_height() {
    assert_pct_row(
        "#p { position:relative; top:50%; margin-top:-20px; }",
        "#p { position:relative; margin-top:-20px; }",
        "#p { position:relative; top:0px; margin-top:-20px; }",
        &oah_offsets(0.0, 0.0),
    );
}

/// AG5 (RED guard): a static P never reads its offsets: `#p { top:50%; }` lays out equal
/// to the baseline. Passes before and after OAH.
#[test]
fn oah_red_guard_ag5_a_static_parent_reads_no_offset() {
    let fragment = "#p { top:50%; }";
    assert_pct_view(
        &pct_layout(5, &pct_css(5, fragment)),
        &pct_layout(5, &pct_css(5, "")),
        &[],
        fragment,
    );
}

// OAH FLIP rows: they need the pass after P's bottom, the narrowed P step and the OAH work.

/// The offset literals of P: `top` and `bottom`.
fn oah_offsets(top: f32, bottom: f32) -> [(String, f32); 2] {
    [
        pct_lit("p.offset.top", top),
        pct_lit("p.offset.bottom", bottom),
    ]
}

/// A2: P's `bottom:50%` of 13 is 6.5, so `top` is -6.5.
#[test]
fn oah_a2_parent_bottom_percentage_of_an_auto_outer_height() {
    assert_pct_row(
        "#p { position:relative; bottom:50%; }",
        "#p { position:relative; }",
        "#p { position:relative; bottom:6.5px; }",
        &oah_offsets(-6.5, 6.5),
    );
}

/// A3 (PB-2): with `top:50%` the ignored `bottom:200%` is still resolved (26) and `top`
/// wins (9.4.3): (6.5, -6.5).
#[test]
fn oah_a3_an_ignored_bottom_percentage_is_resolved() {
    assert_pct_row(
        "#p { position:relative; top:50%; bottom:200%; }",
        "#p { position:relative; }",
        "#p { position:relative; top:6.5px; bottom:26px; }",
        &oah_offsets(6.5, -6.5),
    );
}

/// A4 (N6): P's margin bottom is 28, so O's used content height is 18 and `top:50%` is 9.
#[test]
fn oah_a4_six_sources_give_the_taller_outer_height() {
    let fragment = "#p { position:relative; top:50%; }";
    let view = |fragment: &str| pct_layout(6, &pct_css(6, fragment));
    let actual = view(fragment);
    let expected = [
        pct_lit("p.margin_bottom", 28.0),
        pct_lit("p.offset.top", 9.0),
        pct_lit("p.offset.bottom", -9.0),
    ];
    assert_pct_view(
        &actual,
        &view("#p { position:relative; }"),
        &expected,
        fragment,
    );
    assert_pct_view(
        &actual,
        &view("#p { position:relative; top:9px; }"),
        &[],
        fragment,
    );
}

/// A5 (MO7, MO9 witness): P's margins count: border top 14, content top 15, content
/// bottom 26, margin bottom 33, so O's used content height is 23 and `top:50%` is 11.5
/// (P's border bottom 27 would give 8.5; P's border top 14 as O's content top 9.5).
#[test]
fn oah_a5_parent_margins_count_in_the_outer_height() {
    let expected = [
        pct_lit("p.border_top", 14.0),
        pct_lit("p.margin_bottom", 33.0),
        pct_lit("p.offset.top", 11.5),
        pct_lit("p.offset.bottom", -11.5),
    ];
    assert_pct_row(
        "#p { position:relative; top:50%; margin-top:4px; margin-bottom:6px; }",
        "#p { position:relative; margin-top:4px; margin-bottom:6px; }",
        "#p { position:relative; top:11.5px; margin-top:4px; margin-bottom:6px; }",
        &expected,
    );
}

/// A6: P's explicit `height:20px` and `border-bottom:2px`: content bottom 31, border and
/// margin bottom 34, so O's used content height is 24 and `top:50%` is 12.
#[test]
fn oah_a6_parent_height_and_bottom_border_count() {
    let expected = [
        pct_lit("p.content_bottom", 31.0),
        pct_lit("p.margin_bottom", 34.0),
        pct_lit("p.offset.top", 12.0),
        pct_lit("p.offset.bottom", -12.0),
    ];
    assert_pct_row(
        "#p { position:relative; top:50%; height:20px; border-bottom:2px solid; }",
        "#p { position:relative; height:20px; border-bottom:2px solid; }",
        "#p { position:relative; top:12px; height:20px; border-bottom:2px solid; }",
        &expected,
    );
}

/// A7 (MO10 witness): O's `min-height:30px` raises O's used content height to 30:
/// `top:50%` is 15.
#[test]
fn oah_a7_outer_min_height_applies() {
    assert_pct_row(
        "#o { min-height:30px; } #p { position:relative; top:50%; }",
        "#o { min-height:30px; } #p { position:relative; }",
        "#o { min-height:30px; } #p { position:relative; top:15px; }",
        &oah_offsets(15.0, -15.0),
    );
}

/// A8 (MO11 witness): O's `max-height:5px` lowers O's used content height to 5:
/// `top:50%` is 2.5.
#[test]
fn oah_a8_outer_max_height_applies() {
    assert_pct_row(
        "#o { max-height:5px; } #p { position:relative; top:50%; }",
        "#o { max-height:5px; } #p { position:relative; }",
        "#o { max-height:5px; } #p { position:relative; top:2.5px; }",
        &oah_offsets(2.5, -2.5),
    );
}

/// A8b (MA witness): 10.7's order: 13 is above the `max-height` 5, then 5 is below the
/// `min-height` 8, so O's used content height is 8 and `top:50%` is 4 (min first would
/// give 2.5).
#[test]
fn oah_a8b_outer_min_height_wins_over_max_height() {
    assert_pct_row(
        "#o { min-height:8px; max-height:5px; } #p { position:relative; top:50%; }",
        "#o { min-height:8px; max-height:5px; } #p { position:relative; }",
        "#o { min-height:8px; max-height:5px; } #p { position:relative; top:4px; }",
        &oah_offsets(4.0, -4.0),
    );
}

/// A9 (EML): O's `min-height:3em` with O's `font-size:10px` is the absolute 30 in the
/// font entry's facts: the layout equals the `min-height:30px; top:15px` twin.
#[test]
fn oah_a9_an_em_outer_limit_is_absolute() {
    eml_row(
        "#o { font-size:10px; min-height:3em; } #p { position:relative; top:50%; }",
        "#o { min-height:30px; } #p { position:relative; top:15px; }",
        &[],
    );
}

/// A10 (PX6 witness; MO5 retired by MFZ): with P's `margin-top:-13px` P's margin bottom
/// is 10, O's content top, so the tentative height is +0 (not negative): `top:50%` is +0
/// and `bottom` +0.
#[test]
fn oah_a10_a_zero_tentative_outer_height_is_admitted() {
    let expected = [
        pct_lit("p.border_top", -3.0),
        pct_lit("p.margin_bottom", 10.0),
        pct_lit("p.offset.top", 0.0),
        pct_lit("p.offset.bottom", 0.0),
    ];
    assert_pct_row(
        "#p { position:relative; top:50%; margin-top:-13px; }",
        "#p { position:relative; margin-top:-13px; }",
        "#p { position:relative; top:0px; margin-top:-13px; }",
        &expected,
    );
}

/// A11 (MO14 witness): under the Auto O the host's value is not read: with the host's
/// 200 and without a host value `top:50%` is 6.5 (reading 200 would give 100).
#[test]
fn oah_a11_the_host_value_is_not_read() {
    let fragment = "#p { position:relative; top:50%; }";
    let control = och_layout("#p { position:relative; }", None);
    for height in [Some(200.0), None] {
        assert_pct_view(
            &och_layout(fragment, height),
            &control,
            &oah_offsets(6.5, -6.5),
            &format!("{fragment} with {height:?}"),
        );
    }
}

/// A12: an invalid host value is not read either: NaN, infinity and -1 each give 6.5.
#[test]
fn oah_a12_an_invalid_host_value_is_not_read() {
    let fragment = "#p { position:relative; top:50%; }";
    let control = och_layout("#p { position:relative; }", None);
    for height in [f32::NAN, f32::INFINITY, -1.0] {
        assert_pct_view(
            &och_layout(fragment, Some(height)),
            &control,
            &oah_offsets(6.5, -6.5),
            &format!("{fragment} with {height}"),
        );
    }
}

/// A13 (PHT): P's `height:50%` under the Auto O is used as `auto` and never reads O, so
/// there is no cycle: `top:50%` is 6.5.
#[test]
fn oah_a13_a_parent_height_percentage_is_used_as_auto() {
    assert_pct_row(
        "#p { height:50%; position:relative; top:50%; }",
        "#p { height:50%; position:relative; }",
        "#p { height:50%; position:relative; top:6.5px; }",
        &oah_offsets(6.5, -6.5),
    );
}

/// A14 (the mask): P's `left:5px` is solved with the vertical percentages counted as
/// `auto`: offsets left 5, right -5, top 6.5, bottom -6.5.
#[test]
fn oah_a14_the_horizontal_axis_keeps_its_solve() {
    let expected = [
        pct_lit("p.offset.left", 5.0),
        pct_lit("p.offset.right", -5.0),
        pct_lit("p.offset.top", 6.5),
        pct_lit("p.offset.bottom", -6.5),
    ];
    assert_pct_row(
        "#p { position:relative; top:50%; left:5px; }",
        "#p { position:relative; }",
        "#p { position:relative; top:6.5px; left:5px; }",
        &expected,
    );
}

/// A15 (PTA): P's `top:10%` of 13 is 1.3, and B1's inherited `top:10%` of P's used content
/// height 11 is 1.1. Both values are the same in either pass order (the children's basis
/// is P's used content height), so this row does not pin the order of the passes; OX2 does.
#[test]
fn oah_a15_parent_and_child_percentages_both_resolve() {
    let expected = [
        pct_lit("p.offset.top", 1.3),
        pct_lit("p.offset.bottom", -1.3),
        pct_lit("c0.offset.top", 1.1),
        pct_lit("c0.offset.bottom", -1.1),
    ];
    assert_pct_row(
        "#p { position:relative; top:10%; } #b1 { position:relative; top:inherit; }",
        "#p { position:relative; } #b1 { position:relative; }",
        "#p { position:relative; top:1.3px; } #b1 { position:relative; top:1.1px; }",
        &expected,
    );
}

/// AE1 (late failure): with P's `height:<f32::MAX>px` P's margin bottom and O's used
/// content height round to f32::MAX, and `top:200%` is unrepresentable at P (predecessor:
/// `IndefiniteOffsetBasis`).
#[test]
fn oah_ae1_an_unrepresentable_top_fails_at_the_parent() {
    assert_eq!(
        oah_error(&format!(
            "#p {{ height:{PCT_F32_MAX_PX}; position:relative; top:200%; }}"
        )),
        "Percentage { source: 1, property: \"top\", error: Unrepresentable }"
    );
}

/// AE2 (PB-2): an ignored `bottom:200%` fails the same way, and with both sides
/// unrepresentable `top` fails first (predecessor: `IndefiniteOffsetBasis`).
#[test]
fn oah_ae2_an_unrepresentable_ignored_bottom_fails_at_the_parent() {
    for (offsets, property) in [
        ("top:0px; bottom:200%;", "bottom"),
        ("top:200%; bottom:200%;", "top"),
    ] {
        assert_eq!(
            oah_error(&format!(
                "#p {{ height:{PCT_F32_MAX_PX}; position:relative; {offsets} }}"
            )),
            format!("Percentage {{ source: 1, property: {property:?}, error: Unrepresentable }}")
        );
    }
}

/// AE3 (error order): a child's resolution in the child loop precedes P's pass: B1's
/// `top:200%` of P's `<f32::MAX>px` is unrepresentable at B1 (predecessor:
/// `IndefiniteOffsetBasis` at P).
#[test]
fn oah_ae3_a_child_resolution_precedes_the_parent_pass() {
    assert_eq!(
        oah_error(&format!(
            "#p {{ height:{PCT_F32_MAX_PX}; position:relative; top:200%; }} \
             #b1 {{ position:relative; top:200%; }}"
        )),
        "Percentage { source: 2, property: \"top\", error: Unrepresentable }"
    );
}

/// AE4 (error order): a child refusal precedes OAH's late refusal: B1's `overflow:scroll`
/// with the AG4 geometry (predecessor: `IndefiniteOffsetBasis`).
#[test]
fn oah_ae4_a_child_refusal_precedes_the_late_refusal() {
    assert_eq!(
        oah_error("#p { position:relative; top:50%; margin-top:-20px; } #b1 { overflow:scroll; }"),
        pht_refusal(2, "overflow", "ChildOverflow")
    );
}

/// AE5 (held P, PCT Q-6): with O's content width NaN P's horizontal solve fails and is
/// held; it is returned at F's width step, before P's bottom (predecessor:
/// `IndefiniteOffsetBasis`).
#[test]
fn oah_ae5_the_held_parent_error_precedes_the_parent_pass() {
    assert_eq!(
        pht_error_in(
            &pct_css(5, "#p { position:relative; top:50%; }"),
            (0.0, f32::NAN, 10.0)
        ),
        "ParentHorizontal(Width(InvalidInput(\"containing_width\")))"
    );
}

/// AE6: with O's content top -1e38 and P's `margin-top` 1e38 (both exact in f32), P's
/// content top is 1 and P's `height:<f32::MAX>px` puts its margin bottom at f32::MAX, so
/// O's tentative content height overflows to +infinity: `top:50%` of it is
/// `InvalidFiniteContext` at P, unless O's `max-height:10px` caps it (offsets 5 and -5).
#[test]
fn oah_ae6_an_infinite_tentative_outer_height_fails_unless_capped() {
    let outer = (0.0, 100.0, -1e38);
    let parent = format!(
        "#p {{ margin-top:99999996802856924650656260769173209088px; \
         height:{PCT_F32_MAX_PX}; position:relative; }}"
    );
    assert_eq!(
        pht_error_in(&pct_css(5, &format!("{parent} #p {{ top:50%; }}")), outer),
        "Percentage { source: 1, property: \"top\", error: InvalidFiniteContext }"
    );
    let capped = format!("#o {{ max-height:10px; }} {parent}");
    assert_pct_view(
        &pct_layout_in(
            5,
            &pct_css(5, &format!("{capped} #p {{ top:50%; }}")),
            outer,
        ),
        &pct_layout_in(5, &pct_css(5, &capped), outer),
        &oah_offsets(5.0, -5.0),
        &capped,
    );
}

/// OW (MW1, MW2 witness): facade work `695 + 32N + 534(N-2) + 373(N-3)` (OAH: 79 fixed on
/// every path) is exact at N5, N6 and N7 and one short is `Work`; the core work is
/// unchanged by OAH (the formula also carries VPH's 45 and CTB's 25 + 8(N-2) + 2(N-3)).
#[test]
fn oah_ow_facade_work_charges_79_fixed() {
    for (nodes, facade, core) in [(5, 3203, 1484), (6, 4142, 1836), (7, 5081, 2189)] {
        assert_eq!(
            facade,
            695 + 32 * nodes + 534 * (nodes - 2) + 373 * (nodes - 3),
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

// Carried OAH rows (OA-6; OAH plan review M-1, M-2): they pin existing OAH code and pass
// at the ISP RED stage.

/// OX1 (carried OAH plan review M-1, ruling OA-6; MX1 witness): P's own relative offset
/// does not enter O's used content height. With `top:50%` and `bottom:5px` (ignored,
/// 9.4.3) P's offsets are (6.5, -6.5), 50% of 13: the basis is P's bottom margin edge
/// before the offset (an edge moved by P's offset of -5 would give 8 and 4).
#[test]
fn oah_ox1_parent_relative_offset_does_not_enter_the_outer_height() {
    let expected = [
        pct_lit("p.offset.top", 6.5),
        pct_lit("p.offset.bottom", -6.5),
    ];
    assert_pct_row(
        "#p { position:relative; top:50%; bottom:5px; }",
        "#p { position:relative; }",
        "#p { position:relative; top:6.5px; bottom:5px; }",
        &expected,
    );
}

/// OX2 (carried OAH plan review M-2, ruling OA-6; MX2 witness): P's pass precedes the
/// children's. With B2's `height:<f32::MAX>px`, B2's bottom edges, P's used height and
/// O's tentative height all round to f32::MAX, so P's `top:200%` fails first, at source 1;
/// the children's pass first would fail at B2 (source 4).
#[test]
fn oah_ox2_the_parent_pass_precedes_the_children_pass() {
    assert_eq!(
        oah_error(&format!(
            "#p {{ position:relative; top:200%; }} #b2 {{ height:{PCT_F32_MAX_PX}; \
             position:relative; top:200%; }}"
        )),
        "Percentage { source: 1, property: \"top\", error: Unrepresentable }"
    );
}

// ISP: offsets inherited from a static parent row (gap G10) reuse the `pct`, `pht`, `ptb`
// and `eml` helpers as a child module of this one (ISP design section 6).
#[path = "inherited_offsets.rs"]
mod inherited_offsets;
