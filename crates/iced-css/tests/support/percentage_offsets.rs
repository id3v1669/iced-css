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

/// V3b (review F-1, ruling PB-2; M23 witness): a `bottom` percentage is resolved or
/// refused even when 9.4.3 ignores it (`top` is not `auto`), a conservative over-refusal:
/// under the Auto P, B1's `top:0px; bottom:10%` is `IndefiniteOffsetBasis` with property
/// `bottom`; under P's `height:<f32::MAX>px`, B1's `top:0px; bottom:200%` is
/// `Percentage { source: 2, property: "bottom", error: Unrepresentable }`.
#[test]
fn ptb_v3b_an_ignored_bottom_percentage_is_still_resolved_or_refused() {
    ptb_assert_indefinite(&[(
        "#b1 { position:relative; top:0px; bottom:10%; }",
        2,
        "bottom",
    )]);
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

/// V5: P's `height:50%` under an Auto O is used as `auto` (PHT), so B1's `top:10%` has no
/// basis: `IndefiniteOffsetBasis` at projected 2.
#[test]
fn ptb_v5_parent_used_as_auto_gives_no_offset_basis() {
    ptb_assert_indefinite(&[(
        "#p { height:50%; } #b1 { position:relative; top:10%; }",
        2,
        "top",
    )]);
}

/// V6 (M5, M8, M17 witness): under the Auto P every child's `top` or `bottom` percentage
/// is refused at its own source (a 0 basis gives an offset 0, P's width 10).
#[test]
fn ptb_v6_auto_parent_gives_no_offset_basis() {
    ptb_assert_indefinite(&[
        ("#b1 { position:relative; top:10%; }", 2, "top"),
        ("#b2 { position:relative; bottom:10%; }", 4, "bottom"),
        ("#f { position:relative; top:10%; }", 3, "top"),
    ]);
}

/// V6b: an Auto P with a length `min-height` still gives no basis (PHT H9e).
#[test]
fn ptb_v6b_auto_parent_with_a_minimum_gives_no_offset_basis() {
    ptb_assert_indefinite(&[(
        "#p { min-height:20px; } #b1 { position:relative; top:10%; }",
        2,
        "top",
    )]);
}

/// V7 (M6 witness): a child's `top` is resolved before its `bottom`.
#[test]
fn ptb_v7_child_offset_order() {
    ptb_assert_indefinite(&[("#b1 { position:relative; top:10%; bottom:10%; }", 2, "top")]);
}

/// V8 (M7 witness): a child's offsets resolve after its height category: B1's `height:auto`
/// is `HeightKind` before its `top:10%` is refused.
#[test]
fn ptb_v8_offsets_follow_the_height_category() {
    pht_assert_refusals(&[(
        "#b1 { height:auto; position:relative; top:10%; }",
        2,
        "height",
        "HeightKind",
    )]);
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
/// with `OuterHeightBasis` (O's used content height is not a facade input).
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

/// V12 (M15 witness): a child's admission tests precede its offset refusal.
#[test]
fn ptb_v12_child_admission_precedes_the_offset_refusal() {
    pht_assert_refusals(&[(
        "#b1 { overflow:scroll; position:relative; top:10%; }",
        2,
        "overflow",
        "ChildOverflow",
    )]);
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
/// percentages on every source; the facade charge is `469 + 32N + 491(N-2) +
/// 313(N-3)`; the layout equals the control with the children relative and no offsets.
#[test]
fn ptb_work_thresholds_with_percentages() {
    for (nodes, facade, core) in [(5, 2728, 1249), (6, 3564, 1501), (7, 4400, 1754)] {
        assert_eq!(
            facade,
            469 + 32 * nodes + 491 * (nodes - 2) + 313 * (nodes - 3)
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
        [(848, 8), (1, 1)]
    );
}
