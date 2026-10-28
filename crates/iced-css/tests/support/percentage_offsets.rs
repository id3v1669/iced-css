//! PTB: percentages of the containing block's height in `top` and `bottom` on every
//! relatively positioned Sequence row (CSS2.2 9.3.2, 9.4.3), with the carried PHT rows
//! PC (PH-7) and OB (PH-10, plan review F-4).
//!
//! A child module of `percentage_heights` (design Q-9): it reuses the `pct`, `ppm` and
//! `pht` private helpers (`pct_css`, `pct_view`, `assert_pct_row`, `pht_assert_refusals`,
//! `PHT_WORK_FRAGMENT`, ...) through `use super::*`. The RED part of this module (PC, OB,
//! V1 and V2) compiles against the predecessor API at 9443455d: it reads only published
//! geometry, the computed accessors and errors as `Debug` text. The rows appended at FLIP
//! also read the offset facts and name the `IndefiniteOffsetBasis` reason. Every literal is
//! derived from `baseline_css()` (O content left 0, width 100, top 10, O `height` auto; P
//! `hidden` with padding-top and padding-bottom of 1; B1 [11, 12] with margin-bottom 4);
//! each admitted row also compares every other published number with a control (the same
//! CSS with the box relative and its percentage offsets removed) and every number with its
//! twin (each percentage replaced by the value it resolves to), bit for bit.

use super::*;

/// The E20 control with a relative B1: P's explicit `height:20px` (P content [11, 31]);
/// B1's offsets are all `auto`.
const PTB_B1: &str = "#p { height:20px; } #b1 { position:relative; }";

/// PC (carried PHT plan review F-1, PH-7; M24 witness): every child publishes its computed
/// `height` fact, a percentage included: under P's `height:20px`, B1's `height:10%` and
/// F's `height:25%` are `computed_height()` `Percentage(10, Specified)` and
/// `Percentage(25, Specified)` (the reviewer's validated row). Passes at RED and after
/// PTB.
#[test]
fn ptb_red_guard_pc_child_computed_heights_keep_the_percentages() {
    let css = pct_css(
        5,
        "#p { height:20px; } #b1 { height:10%; } #f { height:25%; }",
    );
    let computed = pct_with(5, &css, PCT_OUTER, layout_limits(), |layout| {
        [
            layout.children()[0].computed_height(),
            layout.children()[1].computed_height(),
        ]
    })
    .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"));
    let specified = BoxFactOrigin::Specified;
    assert_eq!(
        computed,
        [pct_fact("10", specified), pct_fact("25", specified)],
        "{css}"
    );
}

/// OB (carried PHT plan review F-4, PH-10): with O's content width NaN (an invalid O basis,
/// P's horizontal solve held), P's `height:50%` under O's `height:200px` is refused with
/// `OuterHeightBasis` before the held `ParentHorizontal` error. Passes at RED and after PTB.
#[test]
fn ptb_red_guard_ob_outer_height_refusal_precedes_the_held_parent_error() {
    let css = pct_css(5, "#o { height:200px; } #p { height:50%; }");
    assert_eq!(
        pht_error_in(&css, (0.0, f32::NAN, 10.0)),
        pht_refusal(1, "height", "OuterHeightBasis"),
        "{css}"
    );
}

/// V1 (RED; M1, M10 witness): B1's `top:10%` of P's explicit used content height 20 is 2
/// (the width basis would give 10), so `bottom` is -2; every normal-flow number is the
/// control's (9.4.3). Predecessor at 9443455d: `Evaluation(Unsupported { original: 0,
/// property: top, target: Element, reason: PercentageLength })`.
#[test]
fn ptb_red_v1_child_top_percentage_of_an_explicit_parent() {
    let expected = [
        pct_lit("c0.offset.top", 2.0),
        pct_lit("c0.offset.bottom", -2.0),
    ];
    assert_pct_row(
        "#p { height:20px; } #b1 { position:relative; top:10%; }",
        PTB_B1,
        "#p { height:20px; } #b1 { position:relative; top:2px; }",
        &expected,
    );
}

/// V2 (RED; M2, M4 witness): F's `bottom:25%` of 20 is 5, so `top` is -5. Predecessor at
/// 9443455d: the same refusal with `original: 1` and `property: bottom`.
#[test]
fn ptb_red_v2_float_bottom_percentage_of_an_explicit_parent() {
    let expected = [
        pct_lit("c1.offset.bottom", 5.0),
        pct_lit("c1.offset.top", -5.0),
    ];
    assert_pct_row(
        "#p { height:20px; } #f { position:relative; bottom:25%; }",
        "#p { height:20px; } #f { position:relative; }",
        "#p { height:20px; } #f { position:relative; bottom:5px; }",
        &expected,
    );
}

// FLIP rows: they read the PTB offset facts and the `IndefiniteOffsetBasis` reason.

/// `IndefiniteOffsetBasis` refusals `(fragment, projected, property)`.
fn ptb_assert_indefinite(cases: &[(&str, usize, &str)]) {
    for &(fragment, projected, property) in cases {
        pht_assert_refusals(&[(fragment, projected, property, "IndefiniteOffsetBasis")]);
    }
}

/// OBb (carried PHT plan review F-4 with PTB): under the OB geometry (O's content width
/// NaN), P's `top:10%` under O's `height:200px` is refused with `OuterHeightBasis`,
/// property `top`, before the held `ParentHorizontal` error.
#[test]
fn ptb_obb_outer_height_offset_refusal_precedes_the_held_parent_error() {
    let css = pct_css(5, "#o { height:200px; } #p { position:relative; top:10%; }");
    assert_eq!(
        pht_error_in(&css, (0.0, f32::NAN, 10.0)),
        pht_refusal(1, "top", "OuterHeightBasis"),
        "{css}"
    );
}

/// V3 (precedence, 9.4.3): with both `top:10%` and `bottom:50%`, `top` wins: B2's used
/// `top` 2 and `bottom` -2 (a `bottom`-wins result is -10 and 10).
#[test]
fn ptb_v3_top_wins_over_bottom() {
    let expected = [
        pct_lit("c2.offset.top", 2.0),
        pct_lit("c2.offset.bottom", -2.0),
    ];
    assert_pct_row(
        "#p { height:20px; } #b2 { position:relative; top:10%; bottom:50%; }",
        "#p { height:20px; } #b2 { position:relative; }",
        "#p { height:20px; } #b2 { position:relative; top:2px; bottom:10px; }",
        &expected,
    );
}

/// V3b (review F-1, ruling PB-2; PTB M23 witness): a `bottom` percentage is resolved even
/// when 9.4.3 ignores it (`top` is not `auto`), a conservative choice: under P's
/// `height:<f32::MAX>px`, B1's `top:0px; bottom:200%` is `Percentage { source: 2,
/// property: "bottom", error: Unrepresentable }`. Its former Auto-P case resolves under PTA
/// (A8).
#[test]
fn ptb_v3b_an_ignored_bottom_percentage_is_still_resolved_or_refused() {
    pht_assert_unrepresentable(
        &[("#b1 { position:relative; top:0px; bottom:200%; }", "bottom")],
        &format!("#p {{ height:{PCT_F32_MAX_PX}; }}"),
    );
}

/// V4: the children's basis is P's used content height: P's computed 20 is clamped by
/// `max-height:10px` to 10, so B1's `top:50%` is 5 (the computed 20 gives 10, the width
/// 50).
#[test]
fn ptb_v4_child_basis_is_the_constrained_parent_height() {
    let expected = [
        pct_lit("c0.offset.top", 5.0),
        pct_lit("c0.offset.bottom", -5.0),
    ];
    assert_pct_row(
        "#p { height:20px; max-height:10px; } #b1 { position:relative; top:50%; }",
        "#p { height:20px; max-height:10px; } #b1 { position:relative; }",
        "#p { height:20px; max-height:10px; } #b1 { position:relative; top:5px; }",
        &expected,
    );
}

/// V4b: P's `min-height:20px` raises its `height:10px`, so B1's `top:50%` is 10 (without
/// P's minimum: 5).
#[test]
fn ptb_v4b_child_basis_includes_the_parent_minimum() {
    let expected = [
        pct_lit("c0.offset.top", 10.0),
        pct_lit("c0.offset.bottom", -10.0),
    ];
    assert_pct_row(
        "#p { height:10px; min-height:20px; } #b1 { position:relative; top:50%; }",
        "#p { height:10px; min-height:20px; } #b1 { position:relative; }",
        "#p { height:10px; min-height:20px; } #b1 { position:relative; top:10px; }",
        &expected,
    );
}

/// V8 (PTB M7 witness, kept by its explicit-P case): a child's offsets resolve after its
/// height category: B1's `height:auto` is `HeightKind` before its `top` percentage
/// resolves, under the Auto P (where `top:10%` resolves only after P's bottom, PTA) and
/// under P's `height:<f32::MAX>px` (where `top:200%` would be `Percentage`).
#[test]
fn ptb_v8_offsets_follow_the_height_category() {
    for (parent, top) in [
        (String::new(), "10%"),
        (format!("#p {{ height:{PCT_F32_MAX_PX}; }}"), "200%"),
    ] {
        let css = pct_css(
            5,
            &format!("{parent} #b1 {{ height:auto; position:relative; top:{top}; }}"),
        );
        assert_eq!(
            pht_error_in(&css, PCT_OUTER),
            pht_refusal(2, "height", "HeightKind"),
            "{css}"
        );
    }
}

/// V9 (M11, M20, M22 witness): under an Auto O, P's `top` or `bottom` percentage is refused
/// at P (`top` before `bottom`).
#[test]
fn ptb_v9_parent_offsets_under_an_auto_outer_height_are_refused() {
    ptb_assert_indefinite(&[
        ("#p { position:relative; top:10%; }", 1, "top"),
        ("#p { position:relative; bottom:10%; }", 1, "bottom"),
        ("#p { position:relative; top:10%; bottom:10%; }", 1, "top"),
    ]);
}

/// V10 (M12 witness): under O's `height:200px`, P's `top` or `bottom` percentage is refused
/// with `OuterHeightBasis` (`build_captured_sibling_layout` supplies no O content height;
/// OCH).
#[test]
fn ptb_v10_parent_offsets_under_an_explicit_outer_height_are_refused() {
    pht_assert_refusals(&[
        (
            "#o { height:200px; } #p { position:relative; top:10%; }",
            1,
            "top",
            "OuterHeightBasis",
        ),
        (
            "#o { height:200px; } #p { position:relative; bottom:10%; }",
            1,
            "bottom",
            "OuterHeightBasis",
        ),
    ]);
}

/// V10b (M13 witness): P's chain under an explicit O is `height`, `min-height`,
/// `max-height`, then `top`, `bottom`.
#[test]
fn ptb_v10b_parent_chain_order() {
    pht_assert_refusals(&[
        (
            "#o { height:200px; } #p { height:50%; position:relative; top:10%; }",
            1,
            "height",
            "OuterHeightBasis",
        ),
        (
            "#o { height:200px; } #p { max-height:50%; position:relative; bottom:10%; }",
            1,
            "max-height",
            "OuterHeightBasis",
        ),
    ]);
}

/// V10c: under an Auto O, P's `height:50%` takes the 10.5 rule without an error, and its
/// `top:10%` is refused.
#[test]
fn ptb_v10c_parent_height_used_as_auto_and_a_parent_offset() {
    ptb_assert_indefinite(&[("#p { height:50%; position:relative; top:10%; }", 1, "top")]);
}

/// V11 (M16 witness): P's offset refusal precedes every child error (B1's
/// `overflow:scroll`).
#[test]
fn ptb_v11_parent_offset_refusal_precedes_the_children() {
    ptb_assert_indefinite(&[(
        "#p { position:relative; top:10%; } #b1 { overflow:scroll; }",
        1,
        "top",
    )]);
}

/// V11b (M14 witness): P's `overflow` test precedes its offset refusal.
#[test]
fn ptb_v11b_parent_overflow_precedes_the_offset_refusal() {
    pht_assert_refusals(&[(
        "#p { overflow:visible; position:relative; top:10%; }",
        1,
        "overflow",
        "ParentOverflow",
    )]);
}

/// V12 (PTB M15 witness, kept by its explicit-P case): a child's admission tests precede
/// its offset resolution: B1's `overflow:scroll` is `ChildOverflow` under the Auto P
/// (where `top:10%` resolves only after P's bottom, PTA) and under P's
/// `height:<f32::MAX>px` (where `top:200%` would be `Percentage`).
#[test]
fn ptb_v12_child_admission_precedes_the_offset_refusal() {
    for (parent, top) in [
        (String::new(), "10%"),
        (format!("#p {{ height:{PCT_F32_MAX_PX}; }}"), "200%"),
    ] {
        let css = pct_css(
            5,
            &format!("{parent} #b1 {{ overflow:scroll; position:relative; top:{top}; }}"),
        );
        assert_eq!(
            pht_error_in(&css, PCT_OUTER),
            pht_refusal(2, "overflow", "ChildOverflow"),
            "{css}"
        );
    }
}

/// V13: under P's `height:<f32::MAX>px` a child's `top:200%` or `bottom:200%` is
/// unrepresentable: `Percentage { source: 2, property, error: Unrepresentable }` with
/// `top` first.
#[test]
fn ptb_v13_unrepresentable_offsets_and_their_order() {
    pht_assert_unrepresentable(
        &[
            ("#b1 { position:relative; top:200%; }", "top"),
            ("#b1 { position:relative; top:200%; bottom:200%; }", "top"),
            ("#b1 { position:relative; bottom:200%; }", "bottom"),
        ],
        &format!("#p {{ height:{PCT_F32_MAX_PX}; }}"),
    );
}

/// V13b (M9 witness): a child's offset resolution precedes P's held horizontal error: with
/// O's content width NaN, B1's `top:200%` of P's `<f32::MAX>px` is reported first, while
/// its `top:2px` twin returns the held error (PHT HP).
#[test]
fn ptb_v13b_child_offset_resolution_precedes_the_held_parent_error() {
    let outer = (0.0, f32::NAN, 10.0);
    for (fragment, text) in [
        (
            "#b1 { position:relative; top:200%; }",
            "Percentage { source: 2, property: \"top\", error: Unrepresentable }",
        ),
        (
            "#b1 { position:relative; top:2px; }",
            "ParentHorizontal(Width(InvalidInput(\"containing_width\")))",
        ),
    ] {
        let css = pct_css(5, &format!("#p {{ height:{PCT_F32_MAX_PX}; }} {fragment}"));
        assert_eq!(pht_error_in(&css, outer), text, "{css}");
    }
}

/// V15 (Q4=C): under P `overflow:auto` with `height:20px`, B1's `top:75%` is 15 and its
/// shifted margin box bottom 31 lies on P's content bottom (admitted); `top:80%` (16:
/// margin box bottom 32, border box [27, 28]) and `top:100%` (20: border box [31, 32])
/// leave it: `ParentAutoOverflow { source: 2 }`.
#[test]
fn ptb_v15_auto_parent_containment_of_shifted_children() {
    let expected = [
        pct_lit("c0.offset.top", 15.0),
        pct_lit("c0.offset.bottom", -15.0),
    ];
    assert_pct_row(
        "#p { overflow:auto; height:20px; } #b1 { position:relative; top:75%; }",
        "#p { overflow:auto; height:20px; } #b1 { position:relative; }",
        "#p { overflow:auto; height:20px; } #b1 { position:relative; top:15px; }",
        &expected,
    );
    for top in ["80%", "100%"] {
        let css = pct_css(
            5,
            &format!(
                "#p {{ overflow:auto; height:20px; }} #b1 {{ position:relative; top:{top}; }}"
            ),
        );
        assert_eq!(
            pht_error_in(&css, PCT_OUTER),
            "ParentAutoOverflow { source: 2 }",
            "{css}"
        );
    }
}

/// V15b (Q4=C basis): B1's `top:20%` of P's 20 is 4 and stays contained (P's width basis
/// gives 20: margin box bottom 36, `ParentAutoOverflow`).
#[test]
fn ptb_v15b_auto_parent_uses_the_height_basis() {
    let expected = [
        pct_lit("c0.offset.top", 4.0),
        pct_lit("c0.offset.bottom", -4.0),
    ];
    assert_pct_row(
        "#p { overflow:auto; height:20px; } #b1 { position:relative; top:20%; }",
        "#p { overflow:auto; height:20px; } #b1 { position:relative; }",
        "#p { overflow:auto; height:20px; } #b1 { position:relative; top:4px; }",
        &expected,
    );
}

/// V15c: under P `hidden` a shifted child is admitted: B1's `top:100%` is 20.
#[test]
fn ptb_v15c_hidden_parent_admits_a_shifted_child() {
    let expected = [
        pct_lit("c0.offset.top", 20.0),
        pct_lit("c0.offset.bottom", -20.0),
    ];
    assert_pct_row(
        "#p { height:20px; } #b1 { position:relative; top:100%; }",
        PTB_B1,
        "#p { height:20px; } #b1 { position:relative; top:20px; }",
        &expected,
    );
}

/// V16 (M3 witness): both axes in one row under P's `width:50px; height:20px`: `top:10%` is
/// 2 of the height, `left:10%` 5 of the width (swapped bases give 5 and 2).
#[test]
fn ptb_v16_offsets_use_their_own_axis() {
    let expected = [
        pct_lit("c0.offset.top", 2.0),
        pct_lit("c0.offset.bottom", -2.0),
        pct_lit("c0.offset.left", 5.0),
        pct_lit("c0.offset.right", -5.0),
    ];
    assert_pct_row(
        "#p { width:50px; height:20px; } #b1 { position:relative; top:10%; left:10%; }",
        "#p { width:50px; height:20px; } #b1 { position:relative; }",
        "#p { width:50px; height:20px; } #b1 { position:relative; top:2px; left:5px; }",
        &expected,
    );
}

/// V17: a zero basis: P's `height:0px` makes B1's `top:50%` 0, the same result as the
/// `top:0px` twin (compared by `Debug` text, layout or error).
#[test]
fn ptb_v17_zero_basis() {
    assert_eq!(
        pht_result("#p { height:0px; } #b1 { position:relative; top:50%; }"),
        pht_result("#p { height:0px; } #b1 { position:relative; top:0px; }")
    );
}

/// V18 (M21 witness): a negative percentage is allowed (CSS2.2 9.3.2): B1's `top:-10%` of
/// 20 is -2, `bottom` 2.
#[test]
fn ptb_v18_negative_offset_percentage() {
    let expected = [
        pct_lit("c0.offset.top", -2.0),
        pct_lit("c0.offset.bottom", 2.0),
    ];
    assert_pct_row(
        "#p { height:20px; } #b1 { position:relative; top:-10%; }",
        PTB_B1,
        "#p { height:20px; } #b1 { position:relative; top:-2px; }",
        &expected,
    );
}

/// RW6 (M18 witness): B1's `top:0.049%` of 20 is one nearest-even rounding of the exact
/// 0.0098 (`3c20902e`, `bottom` `bc20902e`), equal to the `top:0.0098px` twin in every
/// number; the f32 pipeline f32(f32(f32(p) * 20) / 100) gives `3c20902d`.
#[test]
fn ptb_rw6_one_rounding_offset() {
    assert_eq!((0.049_f32 * 20.0 / 100.0).to_bits(), 0x3c20902d);
    let expected = [
        pct_lit("c0.offset.top", f32::from_bits(0x3c20902e)),
        pct_lit("c0.offset.bottom", f32::from_bits(0xbc20902e)),
    ];
    assert_pct_row(
        "#p { height:20px; } #b1 { position:relative; top:0.049%; }",
        PTB_B1,
        "#p { height:20px; } #b1 { position:relative; top:0.0098px; }",
        &expected,
    );
}

/// The offset facts of `source` in the Sequence facts of the N5 `fragment`, with every
/// source's original index.
fn ptb_offset(fragment: &str, source: usize) -> (Option<[BoxLengthFact; 4]>, Vec<usize>) {
    let css = pct_css(5, fragment);
    let (rows, _, originals) = pct_facts(5, &css);
    (rows[source].offset, originals)
}

/// FX9: the facts keep `top:2.5%` and `bottom:-10%` exactly, `Specified`, with no counted
/// conversion: `numeric_calls` equals the `#b1 { position:relative; }` twin's.
#[test]
fn ptb_fx9_offset_percentage_facts_are_exact_and_unconverted() {
    let fragment = "#b1 { position:relative; top:2.5%; bottom:-10%; }";
    let (offset, _) = ptb_offset(fragment, 2);
    let specified = BoxFactOrigin::Specified;
    let auto = BoxLengthFact::Auto {
        origin: BoxFactOrigin::Initial,
    };
    assert_eq!(
        offset,
        Some([
            pct_fact("2.5", specified),
            auto,
            pct_fact("-10", specified),
            auto
        ]),
        "{fragment}"
    );
    let (_, receipt, _) = pct_facts(5, &pct_css(5, fragment));
    let (_, twin, _) = pct_facts(5, &pct_css(5, "#b1 { position:relative; }"));
    assert_eq!(receipt.numeric_calls, twin.numeric_calls, "{fragment}");
}

/// FX10: `top:inherit` copies P's computed `top:10%` with origin `Inherited`; the layout is
/// refused at P (V9).
#[test]
fn ptb_fx10_inherit_keeps_the_offset_percentage() {
    let fragment = "#p { position:relative; top:10%; } #b1 { position:relative; top:inherit; }";
    let (offset, originals) = ptb_offset(fragment, 2);
    let inherited = BoxFactOrigin::Inherited {
        original_parent: originals[1],
    };
    assert_eq!(
        offset.map(|offset| offset[0]),
        Some(pct_fact("10", inherited)),
        "{fragment}"
    );
    ptb_assert_indefinite(&[(fragment, 1, "top")]);
}

/// FX11: the bounded representation: `18446744073709551616%` (2^64) is
/// `PercentagePrecision` at B1's `top` and `bottom`; the largest stored coefficient
/// `18446744073709551615%` is kept exactly.
#[test]
fn ptb_fx11_precision_boundary() {
    for name in ["top", "bottom"] {
        let css = pct_css(
            5,
            &format!("#b1 {{ position:relative; {name}:18446744073709551616%; }}"),
        );
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
    let fragment = "#b1 { position:relative; top:18446744073709551615%; }";
    let (offset, _) = ptb_offset(fragment, 2);
    assert_eq!(
        offset.map(|offset| offset[0]),
        Some(pct_fact("18446744073709551615", BoxFactOrigin::Specified)),
        "{fragment}"
    );
}

/// FX12: a relative O keeps its `top:10%` fact (offsets are resolved on every relative
/// Sequence row, O included) and the layout stops at the REL O fence.
#[test]
fn ptb_fx12_relative_outer_offset_percentage_reaches_the_outer_fence() {
    let fragment = "#o { position:relative; top:10%; }";
    let (offset, _) = ptb_offset(fragment, 0);
    assert_eq!(
        offset.map(|offset| offset[0]),
        Some(pct_fact("10", BoxFactOrigin::Specified)),
        "{fragment}"
    );
    pht_assert_refusals(&[(fragment, 0, "display/position/float", "DisplayOrFlow")]);
}

/// The work fixture: PCT's, PPM's and PHT's percentages, P's explicit height and a
/// relative child at every source with `top` and `bottom` percentages that resolve to 0.
const PTB_WORK_FRAGMENT: &str = "#p { height:20px; } \
                                 #b1, #f, #b2, #b3, #b4 { position:relative; top:0%; bottom:0%; }";

/// Facade and core work (M19 witness): exact and one short at N5, N6 and N7 with
/// percentages on every source; the facade charge is `546 + 32N + 526(N-2) +
/// 313(N-3)`; the layout equals the control with the children relative and no offsets.
#[test]
fn ptb_work_thresholds_with_percentages() {
    for (nodes, facade, core) in [(5, 2910, 1249), (6, 3781, 1501), (7, 4652, 1754)] {
        assert_eq!(
            facade,
            546 + 32 * nodes + 526 * (nodes - 2) + 313 * (nodes - 3)
        );
        let earlier = format!("{PCT_WORK_FRAGMENT} {PPM_WORK_FRAGMENT} {PHT_WORK_FRAGMENT}");
        let css = pct_css(nodes, &format!("{earlier} {PTB_WORK_FRAGMENT}"));
        let control = format!(
            "{earlier} #p {{ height:20px; }} #b1, #f, #b2, #b3, #b4 {{ position:relative; }}"
        );
        assert_pct_view(
            &pct_layout(nodes, &css),
            &pct_layout(nodes, &pct_css(nodes, &control)),
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

/// D4 measurement (x86_64; measured at FLIP): the fact row and the refusal reason with its
/// new fieldless variant `IndefiniteOffsetBasis` (both unchanged in size).
#[test]
fn ptb_representation_measurement() {
    assert_ne!(
        CapturedClearanceUnsupported::IndefiniteOffsetBasis,
        CapturedClearanceUnsupported::OuterHeightBasis
    );
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
        println!("PTB_REPRESENTATION {name} {size} {align}");
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    assert_eq!(
        measured.map(|(_, size, align)| (size, align)),
        [(872, 8), (1, 1)]
    );
}

// PTA rows (G4d-3b child part, reading (b)): a child's `top` and `bottom` percentages under
// an Auto P resolve against P's final used content height, after P's bottom and before
// the `auto` containment test (CSS2.2 9.3.2, 9.4.3). Baseline vertical model (MMX): P
// content [11, 22], so the Auto P's used content height is 11; B1 [11, 12] with
// margin-bottom 4; F [16, 18]; B2 [18, 19] with margins 3. The RED part (PB9 and A1)
// compiles against the PTB commit ea24b2b7.

/// PB9 (carried PTB plan review F-1, ruling PB-9; M17 witness, the narrowed-arm mutant
/// MPB9): P's ignored `bottom:10%` (P's `top:1px` is not `auto`) is still refused at P:
/// `IndefiniteOffsetBasis` under the Auto O and `OuterHeightBasis` under O's
/// `height:200px`, property `bottom` (the reviewer's validated row). Passes at RED and
/// after PTA (PTA does not change P's step).
#[test]
fn pta_red_guard_pb9_parent_ignored_bottom_percentage_is_refused() {
    pht_assert_refusals(&[
        (
            "#p { position:relative; top:1px; bottom:10%; }",
            1,
            "bottom",
            "IndefiniteOffsetBasis",
        ),
        (
            "#o { height:200px; } #p { position:relative; top:1px; bottom:10%; }",
            1,
            "bottom",
            "OuterHeightBasis",
        ),
    ]);
}

/// A1 (RED; M1, M2, M3, M5, M6, M7, M11, M14 witness): under the Auto P, B1's `top:50%`
/// of P's used content height 11 is 5.5, so `bottom` is -5.5 (P's width 100 gives 50,
/// P's content bottom 22 gives 11, P's border-box height 13 gives 6.5, a 0 basis gives
/// 0); every normal-flow number is the control's (9.4.3). Predecessor at ea24b2b7:
/// `Captured(Unsupported { projected: 2, property: "top", reason: IndefiniteOffsetBasis
/// })`.
#[test]
fn pta_red_a1_child_top_percentage_of_an_auto_parent() {
    let expected = [
        pct_lit("c0.offset.top", 5.5),
        pct_lit("c0.offset.bottom", -5.5),
    ];
    assert_pct_row(
        "#b1 { position:relative; top:50%; }",
        "#b1 { position:relative; }",
        "#b1 { position:relative; top:5.5px; }",
        &expected,
    );
}

// PTA FLIP rows: they need the vertical pass after P's bottom.

/// A2 (M12 witness): under the Auto P, B2's `bottom:50%` of 11 is 5.5, so `top` is -5.5.
#[test]
fn pta_a2_tail_bottom_percentage_of_an_auto_parent() {
    let expected = [
        pct_lit("c2.offset.bottom", 5.5),
        pct_lit("c2.offset.top", -5.5),
    ];
    assert_pct_row(
        "#b2 { position:relative; bottom:50%; }",
        "#b2 { position:relative; }",
        "#b2 { position:relative; bottom:5.5px; }",
        &expected,
    );
}

/// A3: under the Auto P, F's `top:100%` of 11 is 11, so `bottom` is -11.
#[test]
fn pta_a3_float_top_percentage_of_an_auto_parent() {
    let expected = [
        pct_lit("c1.offset.top", 11.0),
        pct_lit("c1.offset.bottom", -11.0),
    ];
    assert_pct_row(
        "#f { position:relative; top:100%; }",
        "#f { position:relative; }",
        "#f { position:relative; top:11px; }",
        &expected,
    );
}

/// A4 (M4 witness): the basis is P's used content height after the 10.7 rerun: P's
/// `min-height:20px` raises the tentative 11 to 20, so B1's `top:50%` is 10 (the tentative
/// height gives 5.5).
#[test]
fn pta_a4_basis_after_the_parent_minimum() {
    let expected = [
        pct_lit("c0.offset.top", 10.0),
        pct_lit("c0.offset.bottom", -10.0),
    ];
    assert_pct_row(
        "#p { min-height:20px; } #b1 { position:relative; top:50%; }",
        "#p { min-height:20px; } #b1 { position:relative; }",
        "#p { min-height:20px; } #b1 { position:relative; top:10px; }",
        &expected,
    );
}

/// A5: P's `max-height:5px` lowers the tentative 11 to 5, so B1's `top:50%` is 2.5 (the
/// tentative height gives 5.5); P `hidden` admits the children below its content box.
#[test]
fn pta_a5_basis_after_the_parent_maximum() {
    let expected = [
        pct_lit("c0.offset.top", 2.5),
        pct_lit("c0.offset.bottom", -2.5),
    ];
    assert_pct_row(
        "#p { max-height:5px; } #b1 { position:relative; top:50%; }",
        "#p { max-height:5px; } #b1 { position:relative; }",
        "#p { max-height:5px; } #b1 { position:relative; top:2.5px; }",
        &expected,
    );
}

/// A6: P's `height:50%` under the Auto O is used as `auto` (PHT, CSS2.2 10.5), so P takes
/// the Auto route (used content height 11) and B1's `top:50%` is 5.5.
#[test]
fn pta_a6_parent_height_used_as_auto_gives_the_auto_basis() {
    let expected = [
        pct_lit("c0.offset.top", 5.5),
        pct_lit("c0.offset.bottom", -5.5),
    ];
    assert_pct_row(
        "#p { height:50%; } #b1 { position:relative; top:50%; }",
        "#p { height:50%; } #b1 { position:relative; }",
        "#p { height:50%; } #b1 { position:relative; top:5.5px; }",
        &expected,
    );
}

/// A7 (precedence, 9.4.3): with `top:100%` and `bottom:50%` under the Auto P, `top` wins:
/// B1's used `top` 11 and `bottom` -11 (a `bottom`-wins result is -5.5 and 5.5).
#[test]
fn pta_a7_top_wins_over_bottom() {
    let expected = [
        pct_lit("c0.offset.top", 11.0),
        pct_lit("c0.offset.bottom", -11.0),
    ];
    assert_pct_row(
        "#b1 { position:relative; top:100%; bottom:50%; }",
        "#b1 { position:relative; }",
        "#b1 { position:relative; top:11px; bottom:5.5px; }",
        &expected,
    );
}

/// A8 (ruling PB-2; the migrated first case of V3b): under the Auto P, B1's ignored
/// `bottom:10%` (`top:0px` wins) is resolved, not refused: a layout with `top` 0 and
/// `bottom` +0; the twin's `bottom:1.1px` is the same f32 as 10% of 11.
#[test]
fn pta_a8_an_ignored_bottom_percentage_is_resolved() {
    let expected = [
        pct_lit("c0.offset.top", 0.0),
        pct_lit("c0.offset.bottom", 0.0),
    ];
    assert_pct_row(
        "#b1 { position:relative; top:0px; bottom:10%; }",
        "#b1 { position:relative; }",
        "#b1 { position:relative; top:0px; bottom:1.1px; }",
        &expected,
    );
}

/// RS (signed route, MMX P6): under `#b2 { margin-bottom:-3px; }` P's Auto height takes
/// the float-extended signed route with selected bottom 18, so its used content height is
/// 7 and B1's `top:50%` is 3.5 (P's content bottom 18 gives 9).
#[test]
fn pta_rs_signed_route_basis() {
    let expected = [
        pct_lit("c0.offset.top", 3.5),
        pct_lit("c0.offset.bottom", -3.5),
    ];
    assert_pct_row(
        "#b2 { margin-bottom:-3px; } #b1 { position:relative; top:50%; }",
        "#b2 { margin-bottom:-3px; } #b1 { position:relative; }",
        "#b2 { margin-bottom:-3px; } #b1 { position:relative; top:3.5px; }",
        &expected,
    );
}

/// A10 (Q4=C): under P `overflow:auto`, B1's `top:50%` (5.5) moves its border box to
/// [16.5, 17.5] and its margin box bottom to 21.5, inside P's content box [11, 22]
/// (admitted); `top:100%` (11) moves its border box to [22, 23]:
/// `ParentAutoOverflow { source: 2 }`.
#[test]
fn pta_a10_auto_parent_containment_of_shifted_children() {
    let expected = [
        pct_lit("c0.offset.top", 5.5),
        pct_lit("c0.offset.bottom", -5.5),
    ];
    assert_pct_row(
        "#p { overflow:auto; } #b1 { position:relative; top:50%; }",
        "#p { overflow:auto; } #b1 { position:relative; }",
        "#p { overflow:auto; } #b1 { position:relative; top:5.5px; }",
        &expected,
    );
    let css = pct_css(
        5,
        "#p { overflow:auto; } #b1 { position:relative; top:100%; }",
    );
    assert_eq!(
        pht_error_in(&css, PCT_OUTER),
        "ParentAutoOverflow { source: 2 }",
        "{css}"
    );
}

/// A11 (M10 witness; ruling PB-2): under P's `min-height:<f32::MAX>px` the 10.7 rerun
/// gives P's content bottom f32(11 + MAX) = MAX and the used content height MAX, so every
/// `200%` is unrepresentable: `top`; an ignored `bottom` is still resolved: `bottom`;
/// `top` before `bottom`; source 2 before source 4. A finite offset under the same P is a
/// layout.
#[test]
fn pta_a11_unrepresentable_offsets_of_an_auto_parent_and_their_order() {
    let parent = format!("#p {{ min-height:{PCT_F32_MAX_PX}; }}");
    pht_assert_unrepresentable(
        &[
            ("#b1 { position:relative; top:200%; }", "top"),
            ("#b1 { position:relative; top:0px; bottom:200%; }", "bottom"),
            ("#b1 { position:relative; top:200%; bottom:200%; }", "top"),
            (
                "#b1 { position:relative; top:200%; } #b2 { position:relative; bottom:200%; }",
                "top",
            ),
        ],
        &parent,
    );
    pct_layout(
        5,
        &pct_css(
            5,
            &format!("{parent} #b1 {{ position:relative; top:1px; }}"),
        ),
    );
}

/// A12 (M8 witness): the vertical pass follows P's bottom: under P's
/// `min-height:<f32::MAX>px; margin-bottom:<f32::MAX>px`, P's border bottom MAX plus its
/// margin MAX overflows first, before B1's unrepresentable `top:200%`.
#[test]
fn pta_a12_parent_bottom_precedes_the_vertical_pass() {
    let css = pct_css(
        5,
        &format!(
            "#p {{ min-height:{PCT_F32_MAX_PX}; margin-bottom:{PCT_F32_MAX_PX}; }} \
             #b1 {{ position:relative; top:200%; }}"
        ),
    );
    assert_eq!(
        pht_error_in(&css, PCT_OUTER),
        "Bottom(Unrepresentable { stage: MarginBottom })",
        "{css}"
    );
}

/// A13 (M9 witness): the vertical pass precedes the `auto` containment test: under P's
/// `overflow:auto; min-height:<f32::MAX>px`, B1's `top:200%` is `Percentage` at source 2
/// although B2's `left:-20px` leaves P's content box; with B1's `top:1px` the containment
/// test gives `ParentAutoOverflow { source: 4 }`.
#[test]
fn pta_a13_vertical_pass_precedes_the_containment_test() {
    let parent = format!("#p {{ overflow:auto; min-height:{PCT_F32_MAX_PX}; }}");
    pht_assert_unrepresentable(
        &[(
            "#b1 { position:relative; top:200%; } #b2 { position:relative; left:-20px; }",
            "top",
        )],
        &parent,
    );
    let css = pct_css(
        5,
        &format!(
            "{parent} #b1 {{ position:relative; top:1px; }} \
             #b2 {{ position:relative; left:-20px; }}"
        ),
    );
    assert_eq!(
        pht_error_in(&css, PCT_OUTER),
        "ParentAutoOverflow { source: 4 }",
        "{css}"
    );
}

/// A14 (both axes under the Auto P; with V16 it separates the M13 and M18 masks): B1's
/// `left:10%` is 10 of P's width 100 in the placement loop, its `top:50%` 5.5 of P's used
/// content height in the vertical pass.
#[test]
fn pta_a14_both_axes_of_an_auto_parent() {
    let expected = [
        pct_lit("c0.offset.top", 5.5),
        pct_lit("c0.offset.bottom", -5.5),
        pct_lit("c0.offset.left", 10.0),
        pct_lit("c0.offset.right", -10.0),
    ];
    assert_pct_row(
        "#b1 { position:relative; top:50%; left:10%; }",
        "#b1 { position:relative; }",
        "#b1 { position:relative; top:5.5px; left:10px; }",
        &expected,
    );
}

/// RW7 (M15 witness): B1's `top:0.001%` of 11 is one nearest-even rounding of the exact
/// 0.00011 (`38e6afcd`, `bottom` `b8e6afcd`), equal to the `top:0.00011px` twin in every
/// number; the f32 pipeline f32(f32(f32(p) * 11) / 100) gives `38e6afce`.
#[test]
fn pta_rw7_one_rounding_offset_of_an_auto_parent() {
    assert_eq!((0.001_f32 * 11.0 / 100.0).to_bits(), 0x38e6afce);
    let expected = [
        pct_lit("c0.offset.top", f32::from_bits(0x38e6afcd)),
        pct_lit("c0.offset.bottom", f32::from_bits(0xb8e6afcd)),
    ];
    assert_pct_row(
        "#b1 { position:relative; top:0.001%; }",
        "#b1 { position:relative; }",
        "#b1 { position:relative; top:0.00011px; }",
        &expected,
    );
}

/// The PTA work fixture: under the Auto P, a relative child at every source with `top` and
/// `bottom` percentages that resolve to 0 against P's used content height.
const PTA_WORK_FRAGMENT: &str = "#b1, #f, #b2, #b3, #b4 { position:relative; top:0%; bottom:0%; }";

/// Facade and core work under the Auto P (M16 witness): exact and one short at N5, N6 and
/// N7 with PCT's, PPM's and PHT's percentages and the children's offset percentages
/// resolved in the vertical pass; the facade charge is `546 + 32N + 526(N-2) + 313(N-3)`,
/// as on the explicit path (`ptb_work_thresholds_with_percentages`); the layout equals the
/// control with the children relative and no offsets.
#[test]
fn pta_work_thresholds_under_an_auto_parent() {
    for (nodes, facade, core) in [(5, 2910, 1249), (6, 3781, 1501), (7, 4652, 1754)] {
        assert_eq!(
            facade,
            546 + 32 * nodes + 526 * (nodes - 2) + 313 * (nodes - 3)
        );
        let earlier = format!("{PCT_WORK_FRAGMENT} {PPM_WORK_FRAGMENT} {PHT_WORK_FRAGMENT}");
        let css = pct_css(nodes, &format!("{earlier} {PTA_WORK_FRAGMENT}"));
        let control = format!("{earlier} #b1, #f, #b2, #b3, #b4 {{ position:relative; }}");
        assert_pct_view(
            &pct_layout(nodes, &css),
            &pct_layout(nodes, &pct_css(nodes, &control)),
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

// OCH: P's height and offset percentages against a host-supplied O content height reuse
// the `pct`, `ppm`, `pht` and `ptb` helpers as a child module of this one (design
// section 6).
#[path = "outer_content_height.rs"]
mod outer_content_height;
