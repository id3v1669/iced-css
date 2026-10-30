//! CTB (user decision U1=A, the first slice: a collapse-through B1): an ordinary
//! (`overflow:visible`) B1 whose computed `height` is zero or `auto` and whose computed
//! `min-height` is zero, without a top or bottom border or padding, has adjoining own top
//! and bottom margins (CSS2.2 8.3.1). When no normal (B1 or a tail) has a negative used
//! vertical margin it collapses through: F's hypothetical top is B1's top border edge, P's
//! content top plus B1's top margin (the user's U1=A: "the hypothetical top of B1's border
//! box (8.3.1 for the position)"), and the first tail's top margin collapses with both of
//! B1's margins from P's content top; clearance separates it from B1's collapsed pair
//! (9.5.2). A negative normal margin keeps `NonpositiveBlockHeight` (a scope choice of this
//! slice, root ruling CT-3), and a B1 used as +0 whose computed `height` or `min-height` is
//! a percentage or a length rounded to +0 keeps `HeightKind` unless its margins are
//! separated (CT-2). Carried first: VPH-L6 (rows L1-L3, witnesses of the VPH probes RP9,
//! RP8 and RP10).
//!
//! A child module of `viewport_heights` (CTB design section 6): it reuses `vph_row`,
//! `vph_error`, `vph_offsets`, `vph_with`, `zhn_row`, `zhn_same`, `zhn_error`,
//! `zhn_layout`, `zhn_pick`, `daz_held`, `pct_css`, `pct_with`, `pct_lit`, `pct_x`,
//! `pht_refusal`, `layout_limits`, `nz`, `PCT_OUTER` and `PCT_F32_MAX_PX` through
//! `use super::*`. The RED part (L1-L3, the guards CG1 and CG3, the helper and C1)
//! compiles against the VPH commit e7e63a1e without a stub: it names no new item and reads
//! only published geometry and errors as text. Fixture: `baseline_css()` (O content left 0,
//! width 100, top 10; P `hidden` with padding-top and padding-bottom 1, content top 11; B1
//! `height:1px; margin-bottom:4px`; F `float:left; width:10px; height:2px`; B2
//! `height:1px; margin-top:3px; margin-bottom:3px; clear:both`; B3 `height:1px;
//! margin-top:2px; margin-bottom:4px; clear:both`), N5 unless stated. Each admitted row
//! compares the listed literals and every other published number with the baseline (or a
//! twin), bit for bit; edges are border edges.

use super::*;

/// L1 (VPH-L6, VP-6; probe RP9 witness): P's `min-height:50%` under the `auto` O is 0
/// (CSS2.2 10.7), so with the viewport height 40 O's limit 20 > 13 gives P's `top:50%` 10
/// and `bottom` -10, as the twin without P's limit. RP9 (P's limits against the viewport
/// height) gives P a used height of 20 and a `top` of 11.
#[test]
fn ctb_vph_l6_l1_parent_min_height_percentage_keeps_the_indefinite_rule() {
    vph_row(
        "#o { min-height:50%; } #p { min-height:50%; position:relative; top:50%; }",
        Some(40.0),
        "#o { min-height:50%; } #p { min-height:50%; position:relative; }",
        "#o { min-height:20px; } #p { position:relative; top:10px; }",
        &vph_offsets(10.0, -10.0),
    );
}

/// L2 (VPH-L6, VP-6; probe RP8 witness): B1's `min-height:10%` under the `auto` P is 0
/// (10.7), so B1's used height stays 1 while P's `top:50%` is 10. RP8 (the children's
/// limits against the viewport height) gives B1 a used height of 4.
#[test]
fn ctb_vph_l6_l2_child_min_height_percentage_keeps_its_basis() {
    let mut expected = vph_offsets(10.0, -10.0).to_vec();
    expected.push(pct_lit("c0.used_height", 1.0));
    vph_row(
        "#o { min-height:50%; } #p { position:relative; top:50%; } #b1 { min-height:10%; }",
        Some(40.0),
        "#o { min-height:50%; } #p { position:relative; } #b1 { min-height:10%; }",
        "#o { min-height:20px; } #p { position:relative; top:10px; } #b1 { min-height:10%; }",
        &expected,
    );
}

/// L3 (VPH-L6, VP-6; probe RP10 witness): the viewport height -0.5 is negative, so O's
/// `min-height:10%` is `Percentage { source: 0, .., InvalidContext }`; RP10
/// (`viewport >= -0.5`) lays out.
#[test]
fn ctb_vph_l6_l3_a_viewport_height_of_minus_one_half() {
    assert_eq!(
        vph_error(
            "#o { min-height:10%; } #p { position:relative; top:50%; }",
            Some(-0.5)
        ),
        "Percentage { source: 0, property: \"min-height\", error: InvalidContext }"
    );
}

/// The core refusal of a zero-height B1 whose own margins may adjoin.
const CTB_B1_REFUSAL: &str =
    "Core(Profile(Unsupported { source: 2, reason: NonpositiveBlockHeight }))";

/// CG1 (RED guard; CM3, CM4 witness): a negative used vertical margin on any normal (B1's
/// own top margin, B2's bottom margin, or at N6 B3's top margin) keeps the zero-height B1
/// `NonpositiveBlockHeight` (CT-3). Passes before and after CTB.
#[test]
fn ctb_red_guard_cg1_a_negative_normal_margin_keeps_the_refusal() {
    for (nodes, fragment) in [
        (5, "#b1 { height:0; margin-top:-1px; }"),
        (5, "#b1 { height:0; } #b2 { margin-bottom:-1px; }"),
        (6, "#b1 { height:0; } #b3 { margin-top:-1px; }"),
    ] {
        assert_eq!(zhn_error(nodes, fragment), CTB_B1_REFUSAL, "{fragment}");
    }
}

/// CG3 (RED guard; CM18, CM19, CM25 witness): a percentage `height` used as `auto`, a
/// percentage `min-height` used as 0 and a `min-height` length rounded to +0 are not
/// computed zeros (CSS2.2 10.5, 10.7; CT-2), so the Auto B1 stays `HeightKind`. Passes
/// before and after CTB.
#[test]
fn ctb_red_guard_cg3_percentages_and_rounded_lengths_are_not_computed_zeros() {
    for fragment in [
        "#b1 { height:50%; }",
        "#b1 { height:auto; min-height:5%; }",
        "#b1 { height:auto; min-height:0.0000000000000000000000000000000000000000000001px; }",
    ] {
        assert_eq!(
            zhn_error(5, fragment),
            pht_refusal(2, "height", "HeightKind"),
            "{fragment}"
        );
    }
}

/// The literals of an N5 collapse-through row: B1's border edges `b1`, F's margin box
/// `float`, B2's border edges `b2` and its clearance (`None`: no clearance), and P's
/// content bottom (P's padding, border and margin bottom one more, its used height the
/// content bottom less 11, and both extent bottoms the content bottom).
fn ctb_expected(
    b1: [f32; 2],
    float: [f32; 2],
    b2: [f32; 2],
    clearance: Option<f32>,
    content_bottom: f32,
) -> Vec<(String, f32)> {
    let mut expected = Vec::new();
    for (index, [top, bottom]) in [(0, b1), (1, float), (2, b2)] {
        expected.extend(pct_x(index, "top", top));
        expected.extend(pct_x(index, "bottom", bottom));
    }
    expected.extend([
        pct_lit("c0.used_height", 0.0),
        pct_lit("float.top", float[0]),
        pct_lit("float.bottom", float[1]),
        pct_lit("tail0.cleared", if clearance.is_some() { 1.0 } else { 0.0 }),
        pct_lit("tail0.clearance", clearance.unwrap_or(0.0)),
        pct_lit("p.content_bottom", content_bottom),
        pct_lit("p.padding_bottom", content_bottom + 1.0),
        pct_lit("p.border_bottom", content_bottom + 1.0),
        pct_lit("p.margin_bottom", content_bottom + 1.0),
        pct_lit("p.used_height", content_bottom - 11.0),
        pct_lit("p.extent.normal_margin_bottom", content_bottom),
        pct_lit("p.extent.selected_bottom", content_bottom),
    ]);
    expected
}

/// C1 (RED; CM1, CM6, CM7, CM8 witness): a zero-height B1 collapses through: B1 [11, 11];
/// F's hypothetical top is B1's top border edge 11 (U1=A), so F is [11, 13]; B2's top
/// margin 3 collapses with B1's margins 0 and 4 from P's content top 11, so B2's
/// hypothetical top 15 lies below F and B2 is [15, 16] without clearance; P's content
/// bottom 19, used height 8. (F after B1's bottom margin, the predecessor reading, and
/// the anonymous parent of 9.5.1 both give F [15, 17].) Predecessor (VPH commit e7e63a1e):
/// `Core(Profile(Unsupported { source: 2, reason: NonpositiveBlockHeight }))`.
#[test]
fn ctb_red_c1_a_zero_height_b1_collapses_through() {
    zhn_row(
        5,
        "#b1 { height:0; }",
        &ctb_expected([11.0, 11.0], [11.0, 13.0], [15.0, 16.0], None, 19.0),
    );
}

// CTB FLIP rows: they need the collapse-through admission (K1-K8), the height category
// (F1-F3) and the CTB work.

/// C2 (CM8, CM9, CM10 witness): with B1's `margin-top:1px` F's hypothetical top is B1's top
/// border edge 12 (U1=A), so F is [12, 14]; B2's hypothetical top is P's content top 11
/// plus the collapse of 1, 4 and 3, so 15 (B2 [15, 16], no clearance). Pairwise from B1's
/// border bottom gives B2 [16, 17]; the anonymous parent of 9.5.1 gives F [15, 17].
#[test]
fn ctb_c2_f_takes_the_top_border_edge() {
    let mut expected = ctb_expected([12.0, 12.0], [12.0, 14.0], [15.0, 16.0], None, 19.0);
    expected.push(pct_lit("c0.margin.top", 1.0));
    zhn_row(5, "#b1 { height:0; margin-top:1px; }", &expected);
}

/// C3 (CM11, CM12 witness): with F 10 high, F is [12, 22]; B2's hypothetical top 15 lies
/// above F's bottom, so clearance places B2 at 22: its zero-clearance top is P's content top
/// plus B1's collapsed pair (4) plus B2's top margin 3, 18, so clearance 4 (9.5.2). B1's
/// top margin alone before the barrier gives clearance 7; all four terms summed give 3.
#[test]
fn ctb_c3_clearance_after_the_collapsed_pair() {
    let mut expected = ctb_expected([12.0, 12.0], [12.0, 22.0], [22.0, 23.0], Some(4.0), 26.0);
    expected.extend([
        pct_lit("c0.margin.top", 1.0),
        pct_lit("c1.used_height", 10.0),
    ]);
    zhn_row(
        5,
        "#b1 { height:0; margin-top:1px; } #f { height:10px; }",
        &expected,
    );
}

/// C4 (CM9 witness): B1's top margin 6 exceeds its bottom margin 2: B1 and F at 17, B2's
/// hypothetical top 11 + 6 = 17 is F's top, and clearance -1 puts B2 at F's bottom 19.
/// Pairwise from B1's border bottom gives B2 [20, 21] without clearance.
#[test]
fn ctb_c4_a_larger_top_margin() {
    let mut expected = ctb_expected([17.0, 17.0], [17.0, 19.0], [19.0, 20.0], Some(-1.0), 23.0);
    expected.extend([
        pct_lit("c0.margin.top", 6.0),
        pct_lit("c0.margin.bottom", 2.0),
    ]);
    zhn_row(
        5,
        "#b1 { height:0; margin-top:6px; margin-bottom:2px; }",
        &expected,
    );
}

/// C5 (CM17 witness): an Auto B1 (also with `max-height:5px`) whose computed
/// `min-height` is zero is passed as +0 and equals its `height:0` twin (F3).
#[test]
fn ctb_c5_an_auto_b1_equals_its_zero_twin() {
    for fragment in [
        "#b1 { height:auto; }",
        "#b1 { height:auto; max-height:5px; }",
    ] {
        zhn_same(fragment, "#b1 { height:0; }");
    }
}

/// C6: a top border width with style `none` is a used 0 and does not separate (the
/// layout equals the `height:0` twin); left and right borders do not separate either:
/// F's top is 11 and B2's 15.
#[test]
fn ctb_c6_undecorated_by_used_edges() {
    zhn_same(
        "#b1 { height:0; border-top-width:1px; }",
        "#b1 { height:0; }",
    );
    let view = zhn_layout(
        5,
        "#b1 { height:0; border-left:1px solid; border-right:1px solid; }",
    );
    assert_eq!(
        (
            zhn_pick(&view, "float.top"),
            zhn_pick(&view, "c2.border.top")
        ),
        (11.0, 15.0)
    );
}

/// C7 (CM20, CM21, CM22, CM23, CM24, CM25 witness): a B1 used as +0 whose computed
/// `height` or `min-height` is not a literal zero (`height:10px; max-height:0`,
/// `min-height:0%`, `height:0%` of P's explicit 20, and lengths rounded to +0, CT-2) is
/// `HeightKind` at B1's height step (ZH-4 scope, Q-4); with a top padding its margins are
/// separated and it equals its `height:0` twin.
#[test]
fn ctb_c7_non_literal_zeros_keep_a_refusal() {
    for fragment in [
        "#b1 { height:10px; max-height:0; }",
        "#b1 { height:0; min-height:0%; }",
        "#p { height:20px; } #b1 { height:0%; }",
        "#b1 { height:0.0000000000000000000000000000000000000000000001px; }",
        "#b1 { height:0; min-height:0.0000000000000000000000000000000000000000000001px; }",
    ] {
        assert_eq!(
            zhn_error(5, fragment),
            pht_refusal(2, "height", "HeightKind"),
            "{fragment}"
        );
    }
    zhn_same(
        "#b1 { height:10px; max-height:0; padding-top:1px; }",
        "#b1 { height:0; padding-top:1px; }",
    );
}

/// C8 (CM5 witness): F's negative bottom margin does not count: B1 collapses through, F's
/// border box is [11, 13] and its margin box [11, 12]; B2 as in C1.
#[test]
fn ctb_c8_the_float_margins_do_not_count() {
    let mut expected = ctb_expected([11.0, 11.0], [11.0, 12.0], [15.0, 16.0], None, 19.0);
    expected.retain(|(name, _)| !name.starts_with("c1."));
    expected.extend(pct_x(1, "top", 11.0));
    expected.extend(pct_x(1, "bottom", 13.0));
    expected.push(pct_lit("c1.margin.bottom", -1.0));
    zhn_row(5, "#b1 { height:0; } #f { margin-bottom:-1px; }", &expected);
}

/// C9 (CM2 witness): a zero-height ordinary tail cannot collapse through in this slice: B2
/// stays `NonpositiveBlockHeight` at source 4 after B1 collapses through.
#[test]
fn ctb_c9_a_zero_height_tail_stays_refused() {
    assert_eq!(
        zhn_error(5, "#b1 { height:0; } #b2 { height:0; }"),
        "Core(Profile(Unsupported { source: 4, reason: NonpositiveBlockHeight }))"
    );
}

/// C10 (CM27 witness): at N6 the later tail keeps the pairwise group: B3's hypothetical
/// top is B2's border bottom 16 plus the collapse of 3 and 2, so 19 (B3 [19, 20], no
/// clearance); P's content bottom 24, used height 13. With B1's `margin-top:6px`, B1 and F
/// are at 17, B2 is [19, 20] (clearance -1) and B3's group still leaves B1's top margin
/// out: B3 [23, 24] (with it, 26); P's content bottom 28.
#[test]
fn ctb_c10_later_tails_stay_pairwise() {
    let mut expected = ctb_expected([11.0, 11.0], [11.0, 13.0], [15.0, 16.0], None, 24.0);
    expected.extend(pct_x(3, "top", 19.0));
    expected.extend(pct_x(3, "bottom", 20.0));
    zhn_row(6, "#b1 { height:0; }", &expected);
    let mut expected = ctb_expected([17.0, 17.0], [17.0, 19.0], [19.0, 20.0], Some(-1.0), 28.0);
    expected.extend(pct_x(3, "top", 23.0));
    expected.extend(pct_x(3, "bottom", 24.0));
    expected.push(pct_lit("c0.margin.top", 6.0));
    zhn_row(6, "#b1 { height:0; margin-top:6px; }", &expected);
}

/// C11: B1's `margin-bottom:auto` is used 0: B2's hypothetical top is 11 plus the collapse
/// of 0, 0 and 3, so 14 (B2 [14, 15], no clearance); P's content bottom 18.
#[test]
fn ctb_c11_auto_vertical_margins() {
    let mut expected = ctb_expected([11.0, 11.0], [11.0, 13.0], [14.0, 15.0], None, 18.0);
    expected.push(pct_lit("c0.margin.bottom", 0.0));
    zhn_row(5, "#b1 { height:0; margin-bottom:auto; }", &expected);
}

/// C12 (design section 5): the error order moves. An Auto B1 is now passed to the core,
/// so its old `HeightKind` gives way to the core's `NonpositiveBlockHeight` (a negative
/// margin), `resolved_row`'s `Percentage` at B1, P's held horizontal error at F's width
/// step, F's `FloatWidth` and a later child's error; an explicit non-literal zero B1 is
/// `HeightKind` at its own height step, before a later child's error and, when its only
/// separation is a percentage padding, before P's held horizontal error (review m-3).
#[test]
fn ctb_c12_the_error_order_moves() {
    let parent = format!("#p {{ width:{PCT_F32_MAX_PX}; }}");
    for (fragment, expected) in [
        (
            "#b1 { height:auto; margin-top:-1px; }".to_owned(),
            CTB_B1_REFUSAL.to_owned(),
        ),
        (
            format!("{parent} #b1 {{ height:auto; padding-top:200%; }}"),
            "Percentage { source: 2, property: \"padding-top\", error: Unrepresentable }"
                .to_owned(),
        ),
        (
            "#b1 { height:auto; } #f { width:auto; }".to_owned(),
            pht_refusal(3, "width", "FloatWidth"),
        ),
        (
            "#b1 { height:auto; } #b2 { overflow:scroll; }".to_owned(),
            pht_refusal(4, "overflow", "ChildOverflow"),
        ),
        (
            "#b1 { height:10px; max-height:0; } #b2 { overflow:scroll; }".to_owned(),
            pht_refusal(2, "height", "HeightKind"),
        ),
    ] {
        assert_eq!(zhn_error(5, &fragment), expected, "{fragment}");
    }
    assert_eq!(
        daz_held("#b1 { height:auto; padding-top:1%; }"),
        "ParentHorizontal(Width(InvalidInput(\"containing_width\")))"
    );
    assert_eq!(
        daz_held("#b1 { height:10px; max-height:0; padding-top:1%; }"),
        pht_refusal(2, "height", "HeightKind")
    );
}

/// The facade and core work of one layout.
fn ctb_receipt(layout: &CapturedSiblingLayout<'_>) -> (usize, usize) {
    (layout.receipt().work, layout.receipt().core.work)
}

/// CW (KW1-KW9 witness): facade `695 + 32N + 534(N-2) + 373(N-3)`, core
/// `751 + 12N + N(N-1)/2 + 71(N-4) + 64(N-2) + 200(N-3)`. Prospective CT-15 charges:
/// facade 25 fixed + 8 per child + 2 per normal; core 49 fixed + 3 per child +
/// 18 per normal + 17 per tail. At N5/N6/N7, a collapse-through B1 and the baseline use
/// exact facade 3203/4142/5081, core 1484/1836/2189; one short is `Work` or `Core(Work)`.
#[test]
fn ctb_cw_work() {
    for (nodes, facade, core) in [(5, 3203, 1484), (6, 4142, 1836), (7, 5081, 2189)] {
        assert_eq!(
            facade,
            695 + 32 * nodes + 534 * (nodes - 2) + 373 * (nodes - 3),
            "N{nodes}"
        );
        assert_eq!(
            core,
            751 + 12 * nodes
                + nodes * (nodes - 1) / 2
                + 71 * (nodes - 4)
                + 64 * (nodes - 2)
                + 200 * (nodes - 3),
            "N{nodes}"
        );
        for fragment in ["#b1 { height:0; }", ""] {
            let css = pct_css(nodes, fragment);
            for (work, core_work, expected) in [
                (facade, core, Ok((facade, core))),
                (facade - 1, core, Err("Work".to_owned())),
                (facade, core - 1, Err("Core(Work)".to_owned())),
            ] {
                let mut limits = layout_limits();
                limits.work = nz(work);
                limits.core.work = nz(core_work);
                let result = pct_with(nodes, &css, PCT_OUTER, limits, ctb_receipt)
                    .map_err(|error| format!("{error:?}"));
                assert_eq!(result, expected, "N{nodes} {fragment} {work} {core_work}");
            }
        }
    }
}
