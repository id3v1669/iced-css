//! EXF (user decision U5=B, 2026-10-29): in the facts of the font entry
//! (`evaluate_sibling_box_facts_with_font_medium`) an `ex` length resolves against the
//! 0.5em x-height, the CSS2.2 4.3.2 fallback (this layer reads no font metrics): in
//! `font-size` the parent's x-height (the root's: `medium`'s), in every other captured
//! length longhand the row's own; the x-height is one f32 product, and the `ex` length one
//! exact product with it rounded once, as `em` is with the font size. The old entry and
//! the Legacy facts keep refusing `ex` (`RelativeLength`). Carried first: MZ-6 (MFZ-L6),
//! the discriminating variant Z16b of MFZ Z16.
//!
//! A child module of `min_floor_zero_heights` (EXF design section 5): it reuses
//! `eml_row`, `eml_facts`, `eml_error`, `eml_run`, `eml_font`, `assert_eml_font`,
//! `eml_relative`, `EML_MEDIUM`, `pct_css`, `pct_lit`, `pct_fact_error`,
//! `pct_sequence_describe`, `pct_legacy_error`, `pct_describe`, `zhn_error`, `isp_y`,
//! `ISP_INHERITED_TOP`, `sibling_fact_limits`, `PCT_F32_MAX_PX` and `RangeEvent` through
//! `use super::*`. The RED part (Z16b, the guards XG1-XG3 and X1) compiles against the
//! MFZ commit 18cc9acf: it names no new item and reads only published geometry, facts and
//! errors. Fixture: `baseline_css()` (O content left 0, width 100, top 10; P `hidden` with
//! padding-top and padding-bottom 1; B1 `height:1px`), N5, medium 16 (x-height 8) unless
//! stated: projected O 0, P 1, B1 2, F 3, B2 4, originals [4, 3, 0, 1, 2]. Each admitted
//! layout row compares the listed literals and every other published number with its twin
//! (each `ex` replaced by the `px` value it resolves to, through the old facts entry), bit
//! for bit.

use super::*;

/// Z16b (MZ-6, MFZ-L6; RP4 witness): Z16's fragment plus `#b2 { overflow:scroll; }`. The
/// child-loop PTB pass resolves B1's `top:200%` of P's explicit `height:<f32::MAX>px` and
/// fails at B1 before the loop reaches B2's `ChildOverflow`; skipping that pass for a
/// non-`visible` row (RP4) moves the first error to B2. Passes before and after EXF.
#[test]
fn exf_mz6_z16b_the_child_loop_offset_percentage_precedes_a_later_child() {
    assert_eq!(
        zhn_error(
            5,
            &format!(
                "#p {{ height:{PCT_F32_MAX_PX}; }} \
                 #b1 {{ overflow:hidden; height:auto; position:relative; top:200%; }} \
                 #b2 {{ overflow:scroll; }}"
            )
        ),
        "Percentage { source: 2, property: \"top\", error: Unrepresentable }"
    );
}

/// XG1 (RED guard; probe XP1 witness): the old entry keeps refusing `ex` on a margin and on
/// a visible border side: B1's `margin-left:1ex` and solid `border-top-width:1ex` are
/// `RelativeLength` at B1 (original 0). Passes before and after EXF.
#[test]
fn exf_red_guard_xg1_the_old_entry_keeps_refusing_ex() {
    for (fragment, name) in [
        ("#b1 { margin-left:1ex; }", "margin-left"),
        (
            "#b1 { border-top-style:solid; border-top-width:1ex; }",
            "border-top-width",
        ),
    ] {
        let css = pct_css(5, fragment);
        let (error, originals) = pct_fact_error(5, &css);
        assert_eq!(
            pct_sequence_describe(&error),
            eml_relative(0, name),
            "{css}"
        );
        assert_eq!(originals, vec![4, 3, 0, 1, 2], "{css}: originals");
    }
}

/// XG2 (RED guard; probe XP1 witness): the Legacy facts keep refusing `ex`: B1's
/// `height:1ex` is `RelativeLength` at B1's Legacy original 0. Passes before and after EXF.
#[test]
fn exf_red_guard_xg2_legacy_keeps_refusing_ex() {
    let css = pct_css(5, "#b1 { height:1ex; }");
    let (error, originals) = pct_legacy_error(&css);
    let error = error.unwrap_or_else(|| panic!("{css}: Legacy facts Ok"));
    assert_eq!(
        pct_describe(&error),
        "Unsupported { original: 0, property: height, target: Element, reason: \
         RelativeLength }",
        "{css}"
    );
    assert_eq!(originals, vec![4, 3, 0, 1, 2], "{css}: Legacy originals");
}

/// XG3 (RED guard; design section 7): a static P's `top:3ex` under P's
/// `font-size:<f32::MAX>px` is unrepresentable for B1's `top:inherit` (its x-height MAX/2
/// is exact, three times it is not), so B1 keeps `InheritedOffset` (the ISP `.ok()`).
/// Passes before and after EXF.
#[test]
fn exf_red_guard_xg3_an_unrepresentable_ex_parent_offset_stays_refused() {
    assert_eq!(
        eml_error(&format!(
            "#p {{ font-size:{PCT_F32_MAX_PX}; top:3ex; }} \
             #b1 {{ position:relative; top:inherit; }}"
        )),
        ISP_INHERITED_TOP
    );
}

/// X1 (RED; XM1, XM6, XM7, XM11 witness): with medium 16 B1's x-height is 8 (the 0.5em
/// fallback), so B1's `height:2ex` is 16; every other number is the `height:16px` twin's.
/// Predecessor (MFZ commit 18cc9acf): the producer refuses B1's `height` with
/// `Evaluation(Unsupported { original: 0, property: height, target: Element, reason:
/// RelativeLength })`.
#[test]
fn exf_red_x1_height_in_ex_of_the_medium() {
    eml_row(
        "#b1 { height:2ex; }",
        "#b1 { height:16px; }",
        &[pct_lit("c0.used_height", 16.0)],
    );
}

// EXF FLIP rows: they need the product change (P1-P7): `ex` against the 0.5em x-height.

/// A `Specified` absolute length fact of `pixels`, not rounded to zero.
fn exf_absolute(pixels: f32) -> BoxLengthFact {
    BoxLengthFact::Absolute(AbsoluteBoxFact {
        pixels,
        rounded_to_zero: false,
        origin: BoxFactOrigin::Specified,
    })
}

/// X2: an `ex` on another property uses the row's own x-height: B1's `font-size:10px;
/// height:3ex` is 15 (the parent's 20 would give 30).
#[test]
fn exf_x2_ex_uses_the_own_font_size() {
    eml_row(
        "#p { font-size:20px; } #b1 { font-size:10px; height:3ex; }",
        "#p { font-size:20px; } #b1 { font-size:10px; height:15px; }",
        &[pct_lit("c0.used_height", 15.0)],
    );
}

/// X3 (XM2, XM3, XM4, XM5 witness): an `ex` in `font-size` uses the parent's x-height:
/// under P's `font-size:20px`, B1's `font-size:1ex` is 10 and its `height:2ex` 10 (the
/// medium's x-height would give 8 and 8).
#[test]
fn exf_x3_font_size_ex_uses_the_parent_x_height() {
    let fragment = "#p { font-size:20px; } #b1 { font-size:1ex; height:2ex; }";
    eml_row(
        fragment,
        "#p { font-size:20px; } #b1 { font-size:1ex; height:10px; }",
        &[pct_lit("c0.used_height", 10.0)],
    );
    assert_eml_font(
        eml_facts(fragment).0[2].font_size,
        eml_font(10.0, None, BoxFactOrigin::Specified),
        fragment,
    );
}

/// X4 (XM2, XM4 witness; probe XP2 witness): O, the root, takes `medium`'s x-height 8 as
/// the basis of its own `ex` (CSS22-001338, as EML's root `em`): O's `font-size:3ex` is 24
/// and B1's `height:1em` 24 (an unhalved basis gives 48).
#[test]
fn exf_x4_the_root_uses_the_medium_x_height() {
    let fragment = "#o { font-size:3ex; } #b1 { height:1em; }";
    eml_row(
        fragment,
        "#o { font-size:3ex; } #b1 { height:24px; }",
        &[pct_lit("c0.used_height", 24.0)],
    );
    assert_eml_font(
        eml_facts(fragment).0[0].font_size,
        eml_font(24.0, None, BoxFactOrigin::Specified),
        fragment,
    );
}

/// X5 (XM8, XM9 witness): every other length family of B1 at `font-size:8px` (x-height
/// 4): `width:5ex`, `min-height:1ex`, `max-width:20ex`, `margin-top:-0.5ex`,
/// `padding-left:0.25ex` and a solid `border-left-width:0.5ex` equal their `px` twin (20,
/// 4, 80, -2, 1, 2).
#[test]
fn exf_x5_every_length_family() {
    eml_row(
        "#b1 { font-size:8px; width:5ex; min-height:1ex; max-width:20ex; margin-top:-0.5ex; \
         padding-left:0.25ex; border-left-style:solid; border-left-width:0.5ex; }",
        "#b1 { font-size:8px; width:20px; min-height:4px; max-width:80px; margin-top:-2px; \
         padding-left:1px; border-left-style:solid; border-left-width:2px; }",
        &[
            pct_lit("c0.used_width", 20.0),
            pct_lit("c0.used_height", 4.0),
        ],
    );
}

/// X5b (XM9 witness): `max-height:0.125ex`, `padding-bottom:0.5ex` and a solid
/// `border-bottom-width:0.25ex` at `font-size:8px` equal their `px` twin (0.5, 2, 1): B1's
/// used height is 0.5.
#[test]
fn exf_x5b_max_height_vertical_padding_and_bottom_border() {
    eml_row(
        "#b1 { font-size:8px; max-height:0.125ex; padding-bottom:0.5ex; \
         border-bottom-style:solid; border-bottom-width:0.25ex; }",
        "#b1 { font-size:8px; max-height:0.5px; padding-bottom:2px; \
         border-bottom-style:solid; border-bottom-width:1px; }",
        &[pct_lit("c0.used_height", 0.5)],
    );
}

/// X5c (plan addition; probe XP7 witness): the remaining families at `font-size:8px`:
/// `min-width:4ex` over `width:1px`, `margin-bottom:0.5ex`, `padding-top:0.5ex`,
/// `padding-right:0.25ex`, solid `border-top-width:0.25ex` and `border-right-width:0.5ex`
/// and a relative `right:1ex` equal their `px` twin (16, 2, 2, 1, 1, 2, 4): B1's used
/// width is 16.
#[test]
fn exf_x5c_min_width_and_the_remaining_sides() {
    eml_row(
        "#b1 { font-size:8px; width:1px; min-width:4ex; margin-bottom:0.5ex; padding-top:0.5ex; \
         padding-right:0.25ex; border-top-style:solid; border-top-width:0.25ex; \
         border-right-style:solid; border-right-width:0.5ex; position:relative; right:1ex; }",
        "#b1 { font-size:8px; width:1px; min-width:16px; margin-bottom:2px; padding-top:2px; \
         padding-right:1px; border-top-style:solid; border-top-width:1px; \
         border-right-style:solid; border-right-width:2px; position:relative; right:4px; }",
        &[
            pct_lit("c0.used_width", 16.0),
            pct_lit("c0.offset.left", -4.0),
            pct_lit("c0.offset.right", 4.0),
        ],
    );
}

/// X6: `ex` offsets are absolute in the fact row, so P's step, the copy into P's used row,
/// the child loop and the vertical pass read converted values: P's `top:1ex; left:0.5ex`
/// give P's offsets 8/-8 and 4/-4; B1's `top:-1ex; bottom:2ex` give -8/8 (`top` wins).
#[test]
fn exf_x6_ex_offsets_are_absolute_in_the_facts() {
    eml_row(
        "#p { position:relative; top:1ex; left:0.5ex; } \
         #b1 { position:relative; top:-1ex; bottom:2ex; }",
        "#p { position:relative; top:8px; left:4px; } \
         #b1 { position:relative; top:-8px; bottom:16px; }",
        &[
            pct_lit("p.offset.top", 8.0),
            pct_lit("p.offset.bottom", -8.0),
            pct_lit("p.offset.left", 4.0),
            pct_lit("p.offset.right", -4.0),
            pct_lit("c0.offset.top", -8.0),
            pct_lit("c0.offset.bottom", 8.0),
        ],
    );
}

/// X7: `inherit` copies the parent's computed (absolute) value: P's `margin-left:2ex` at
/// P's `font-size:10px` is 10 (P's width 90), and B1's `margin-left:inherit` is 10 under
/// B1's own `font-size:20px` (an inherited `ex` would give 20).
#[test]
fn exf_x7_inherit_copies_the_computed_length() {
    eml_row(
        "#p { font-size:10px; margin-left:2ex; } #b1 { font-size:20px; margin-left:inherit; }",
        "#p { font-size:10px; margin-left:10px; } #b1 { font-size:20px; margin-left:inherit; }",
        &[
            pct_lit("p.margin_left", 10.0),
            pct_lit("p.width", 90.0),
            pct_lit("c0.margin.left", 10.0),
        ],
    );
}

/// X8 (ISP; probe XP6 witness): a static P's `top:1ex` computes against P's own x-height
/// 5 (P's `font-size:10px`), not B1's 10: B1's `top:inherit` is 5.
#[test]
fn exf_x8_isp_an_ex_parent_offset_uses_the_parent_x_height() {
    eml_row(
        "#p { font-size:10px; top:1ex; } #b1 { font-size:20px; position:relative; \
         top:inherit; }",
        "#b1 { position:relative; top:5px; }",
        &isp_y("c0", 5.0),
    );
}

/// X8b (ISP): through a static P from O, O's `top:3ex` computes against O's own x-height
/// 5 (O's `font-size:10px`), not P's 10: B1's `top:inherit` is 15 (P's size would give 30).
#[test]
fn exf_x8b_isp_an_ex_chain_from_o_uses_the_outer_x_height() {
    eml_row(
        "#o { font-size:10px; top:3ex; } #p { font-size:20px; top:inherit; } \
         #b1 { position:relative; top:inherit; }",
        "#b1 { position:relative; top:15px; }",
        &isp_y("c0", 15.0),
    );
}

/// X9: one rounding: B1's `height:0.3ex` at P's `font-size:3px` (x-height 1.5, exact) is
/// the exact 0.45 rounded once, bits `3ee66666` (the f32 pipeline f32(f32(0.3) * 1.5)
/// gives `3ee66667`), equal to the `height:0.45px` twin.
#[test]
fn exf_x9_one_rounding() {
    eml_row(
        "#p { font-size:3px; } #b1 { height:0.3ex; }",
        "#p { font-size:3px; } #b1 { height:0.45px; }",
        &[pct_lit("c0.used_height", f32::from_bits(0x3ee6_6666))],
    );
}

/// X10 (XM12 witness): the x-height of the smallest subnormal font size (bits `00000001`)
/// is f32(2^-150), a tie rounded to even, +0, so B1's `margin-top:3ex` is +0 without the
/// rounded-to-zero flag (the exact half of the size inside the product, or the size times
/// 3 halved afterwards, gives bits `00000002`).
#[test]
fn exf_x10_the_x_height_of_the_smallest_subnormal_is_zero() {
    let fragment =
        "#b1 { font-size:0.0000000000000000000000000000000000000000000014px; margin-top:3ex; }";
    let rows = eml_facts(fragment).0;
    assert_eml_font(
        rows[2].font_size,
        eml_font(f32::from_bits(1), None, BoxFactOrigin::Specified),
        fragment,
    );
    assert_eq!(rows[2].margin[0], exf_absolute(0.0), "{fragment}");
    let BoxLengthFact::Absolute(margin) = rows[2].margin[0] else {
        panic!("{fragment}: margin-top is not absolute");
    };
    assert_eq!(margin.pixels.to_bits(), 0, "{fragment}");
}

/// X11 (XM1, XM3, XM12 witness; probes XP4, XP5 witness, each sub-row needed): range.
/// Under P's `font-size:<f32::MAX>px` (exact, no event) the x-height MAX/2 is exact: B1's
/// `height:2ex` is `f32::MAX` exactly, `height:3ex` exceeds the finite range (`Length` with
/// `Unrepresentable`); B1's `font-size:2ex` is `f32::MAX` with no event and `3ex`
/// saturates with `UpperSaturation`; a solid `border-top-width` likewise.
#[test]
fn exf_x11_range() {
    let parent = format!("#p {{ font-size:{PCT_F32_MAX_PX}; }}");
    let fragment = format!("{parent} #b1 {{ height:2ex; }}");
    assert_eq!(
        eml_facts(&fragment).0[2].height,
        exf_absolute(f32::MAX),
        "{fragment}"
    );
    assert_eq!(
        eml_error(&format!("{parent} #b1 {{ height:3ex; }}")),
        "Evaluation(Length { original: 0, property: height, error: \
         Conversion(Unrepresentable) })"
    );
    for (number, event) in [("2", None), ("3", Some(RangeEvent::UpperSaturation))] {
        let fragment = format!("{parent} #b1 {{ font-size:{number}ex; }}");
        assert_eml_font(
            eml_facts(&fragment).0[2].font_size,
            eml_font(f32::MAX, event, BoxFactOrigin::Specified),
            &fragment,
        );
        let fragment =
            format!("{parent} #b1 {{ border-top-style:solid; border-top-width:{number}ex; }}");
        let width = eml_facts(&fragment).0[2].border_width[0];
        assert_eq!(width.pixels, f32::MAX, "{fragment}");
        assert_eq!(width.event, event, "{fragment}");
    }
}

/// X12: a zero font size gives +0: B1's `font-size:0px; margin-top:-1ex` has the
/// margin-top bits `00000000`.
#[test]
fn exf_x12_zero_font_size() {
    eml_row(
        "#b1 { font-size:0px; margin-top:-1ex; }",
        "#b1 { font-size:0px; margin-top:0px; }",
        &[pct_lit("c0.margin.top", 0.0)],
    );
}

/// The fragment of X13 and X14: an `ex` `font-size` and two `ex` lengths.
const EXF_COUNT_FRAGMENT: &str = "#p { font-size:2ex; } #b1 { height:2ex; margin-left:1ex; }";

/// `EXF_COUNT_FRAGMENT` with each `ex` replaced by `em`.
const EXF_COUNT_TWIN: &str = "#p { font-size:2em; } #b1 { height:2em; margin-left:1em; }";

/// X13 (XM10 witness; probe XP3 witness): `numeric_calls` counts an `ex` like an `em`, the
/// `font-size` `ex` included: two more than the baseline run (P's `font-size` and B1's
/// `margin-left`; B1's `height` replaces the baseline's counted `height:1px`).
#[test]
fn exf_x13_numeric_calls_count_ex_like_em() {
    let calls = |fragment| eml_facts(fragment).1.numeric_calls;
    let base = calls("");
    assert_eq!(
        (calls(EXF_COUNT_FRAGMENT), calls(EXF_COUNT_TWIN)),
        (base + 2, base + 2)
    );
}

/// X14: the font entry's work stays `36 * n + 2 + r` and its numeric allowance
/// `23 * n * numeric.work`, at N5 and N7, equal to the `em` twin's.
#[test]
fn exf_x14_work_is_unchanged() {
    for nodes in [5, 7] {
        let receipt = |fragment| {
            let css = pct_css(nodes, fragment);
            let (run, _, rules) = eml_run(nodes, &css, EML_MEDIUM, sibling_fact_limits());
            let receipt = run.unwrap_or_else(|error| panic!("N{nodes}: {error}")).1;
            (receipt.work, receipt.numeric_work, rules)
        };
        let (work, numeric_work, rules) = receipt(EXF_COUNT_FRAGMENT);
        assert_eq!(
            (work, numeric_work),
            (36 * nodes + 2 + rules, 23 * nodes * 100_000),
            "N{nodes}"
        );
        assert_eq!(
            receipt(EXF_COUNT_TWIN),
            (work, numeric_work, rules),
            "N{nodes} em twin"
        );
    }
}

/// X15 (error order): with B1's `height:1ex` admitted, B2's later
/// `padding-top:18446744073709551616%` (2^64, outside the bounded representation) is the
/// producer's first error (predecessor: `RelativeLength` at B1, original 0).
#[test]
fn exf_x15_a_later_row_error_now_surfaces() {
    assert_eq!(
        eml_error("#b1 { height:1ex; } #b2 { padding-top:18446744073709551616%; }"),
        "Evaluation(Unsupported { original: 2, property: padding-top, target: Element, reason: \
         PercentagePrecision })"
    );
}
