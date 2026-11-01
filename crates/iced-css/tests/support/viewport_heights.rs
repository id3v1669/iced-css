//! VPH (user decision U2=A, 2026-10-29): `build_captured_sibling_layout_with_viewport_height`
//! takes the host's viewport (initial containing block) height. Under an O whose computed
//! `height` is `auto`, when P's `top` or `bottom` is a percentage, O's percentage
//! `min-height` and `max-height` resolve against it at P's vertical step (CSS2.2 10.1,
//! 10.7: O is the captured root) and constrain O's used content height, the basis of P's
//! offsets (OAH); `None` keeps `IndefiniteOffsetBasis`, and a value that is not finite and
//! nonnegative is O's `Percentage { source: 0, .., InvalidContext }`. The old entries pass
//! `None`. Carried first: EXF-L6 (rows C1 and C2, witnesses of the EXF probes RA and RG).
//!
//! A child module of `ex_fallback_lengths` (VPH design section 5): it reuses `pct_css`,
//! `pct_view`, `pct_lit`, `pct_layout`, `pct_with`, `assert_pct_view`, `pht_refusal`,
//! `och_error`, `och_layout`, `eml_facts`, `eml_font`, `assert_eml_font`, `exf_absolute`,
//! `sibling_fixture`, `sibling_fact_limits`, `layout_limits`, `nz`, `PCT_OUTER` and
//! `PCT_F32_MAX_PX` through `use super::*`. The RED part (C1, C2, the helpers, VG1, VG2
//! and V1) compiles against the EXF commit ebf911bc with the RED stub S0: it reads only
//! published geometry, facts and errors. Fixture: `baseline_css()` (O content left 0,
//! width 100, top 10, O `height` auto unless a row sets it; P `hidden` with padding-top
//! and padding-bottom 1: border top 10, content top 11, content bottom 22, margin bottom
//! 23, so O's tentative content height is 13), N5, host O content height `None` unless
//! stated. Each admitted row compares the listed literals and every other published number
//! with its control (the same viewport, without the percentage side) and every number with
//! its absolute twin (through the old entry), bit for bit.

use super::*;
use iced_css::evaluation::sibling_layout::build_captured_sibling_layout_with_viewport_height;

/// C1 (EXF-L6, EX-7 (b): the X10 sub-row with a two-bit size; probe RA witness): B1's
/// font size has bits `00000002` (no event), its 0.5em x-height is exact (bits
/// `00000001`), so B1's `margin-top:3ex` has bits `00000003`, not rounded to zero. A
/// `fallback_x_height` that flushes a size below `f32::MIN_POSITIVE` to +0 (RA) gives bits
/// `00000000`; X10 cannot separate it (the correct product there is +0 too).
#[test]
fn vph_exf_l6_x10b_a_two_bit_subnormal_size() {
    let fragment =
        "#b1 { font-size:0.0000000000000000000000000000000000000000000028px; margin-top:3ex; }";
    let rows = eml_facts(fragment).0;
    assert_eml_font(
        rows[2].font_size,
        eml_font(f32::from_bits(2), None, BoxFactOrigin::Specified),
        fragment,
    );
    assert_eq!(
        rows[2].margin[0],
        exf_absolute(f32::from_bits(3)),
        "{fragment}"
    );
}

/// C2 (EXF-L6, EX-7 (b): the X13 border sub-row; probe RG witness): a solid
/// `border-top-width:1ex` is one counted conversion, like its `1em` and `8px` twins:
/// `numeric_calls` is the empty fragment's plus one (B1's default `none` side converts
/// nothing). Skipping the count for an `ex` width (RG) gives the empty fragment's.
#[test]
fn vph_exf_l6_x13b_a_border_ex_conversion_is_counted() {
    let calls = |width: &str| {
        let fragment = format!("#b1 {{ border-top-style:solid; border-top-width:{width}; }}");
        eml_facts(&fragment).1.numeric_calls
    };
    let base = eml_facts("").1.numeric_calls;
    assert_eq!(
        (calls("1ex"), calls("1em"), calls("8px")),
        (base + 1, base + 1, base + 1)
    );
}

/// `pct_with` through the new entry with O's content geometry `outer` and `heights`, the
/// host O content height and the viewport height (the only `ClearanceOuterContentGeometry`
/// literal of this module).
fn vph_with<T>(
    nodes: usize,
    css: &str,
    outer: (f32, f32, f32),
    heights: (Option<f32>, Option<f32>),
    limits: SiblingLayoutLimits,
    observe: impl FnOnce(&CapturedSiblingLayout<'_>) -> T,
) -> Result<T, SiblingLayoutError> {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let facts = evaluate_sibling_box_facts(projection, display, sibling_fact_limits())
            .unwrap_or_else(|error| {
                panic!("{css}: producer error {}", pct_sequence_describe(&error))
            });
        let (content_left, content_width, content_top) = outer;
        let (height, viewport) = heights;
        out = Some(
            build_captured_sibling_layout_with_viewport_height(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left,
                    content_width,
                    content_top,
                },
                height,
                viewport,
                limits,
            )
            .map(|layout| observe(&layout)),
        );
    });
    out.unwrap()
}

/// The N5 view of `fragment` through the new entry with the host O content height
/// `height` and the viewport height `viewport`; a layout is required.
fn vph_layout(fragment: &str, height: Option<f32>, viewport: Option<f32>) -> Vec<(String, f32)> {
    let css = pct_css(5, fragment);
    vph_with(
        5,
        &css,
        PCT_OUTER,
        (height, viewport),
        layout_limits(),
        pct_view,
    )
    .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"))
}

/// The `Debug` text of the N5 layout error of `fragment` through the new entry with O's
/// content geometry `outer`; a layout is a failure.
fn vph_error_in(fragment: &str, outer: (f32, f32, f32), viewport: Option<f32>) -> String {
    let css = pct_css(5, fragment);
    match vph_with(5, &css, outer, (None, viewport), layout_limits(), |_| ()) {
        Ok(()) => panic!("{css} (viewport {viewport:?}): expected a layout error, got a layout"),
        Err(error) => format!("{error:?}"),
    }
}

/// `vph_error_in` with `PCT_OUTER`.
fn vph_error(fragment: &str, viewport: Option<f32>) -> String {
    vph_error_in(fragment, PCT_OUTER, viewport)
}

/// An admitted N5 row: `fragment` through the new entry with the viewport height
/// `viewport` gives the `expected` literals and every other number of `control` (the new
/// entry, the same viewport), and every number of its absolute `twin` through the old
/// entry.
fn vph_row(
    fragment: &str,
    viewport: Option<f32>,
    control: &str,
    twin: &str,
    expected: &[(String, f32)],
) {
    let css = pct_css(5, fragment);
    let view = vph_layout(fragment, None, viewport);
    assert_pct_view(&view, &vph_layout(control, None, viewport), expected, &css);
    assert_pct_view(&view, &pct_layout(5, &pct_css(5, twin)), &[], &css);
}

/// P's offset literals: `top` and `bottom`.
fn vph_offsets(top: f32, bottom: f32) -> [(String, f32); 2] {
    [
        pct_lit("p.offset.top", top),
        pct_lit("p.offset.bottom", bottom),
    ]
}

/// VG1 (RED guard; VM13, VM15 witness): without a viewport height the new entry keeps
/// `IndefiniteOffsetBasis` at P's step for O's `min-height:10%` with P's `top:50%` and
/// for O's `max-height:10%` with P's `bottom:50%`; the OCH entry with a host O content
/// height `Some(40.0)` supplies no viewport height and refuses too (review C-1; VP-2).
/// Passes before and after VPH.
#[test]
fn vph_red_guard_vg1_no_viewport_keeps_the_refusal() {
    for (fragment, property) in [
        (
            "#o { min-height:10%; } #p { position:relative; top:50%; }",
            "top",
        ),
        (
            "#o { max-height:10%; } #p { position:relative; bottom:50%; }",
            "bottom",
        ),
    ] {
        assert_eq!(
            vph_error(fragment, None),
            pht_refusal(1, property, "IndefiniteOffsetBasis"),
            "{fragment}"
        );
    }
    assert_eq!(
        och_error(
            "#o { min-height:10%; } #p { position:relative; top:50%; }",
            Some(40.0)
        ),
        pht_refusal(1, "top", "IndefiniteOffsetBasis")
    );
}

/// VG2 (RED guard; probe VP7 witness): the viewport height is not O's used content height:
/// under O's `height:50%` P's `height:50%` keeps `OuterHeightBasis` without a host O
/// content height, with the viewport height 40. Passes before and after VPH.
#[test]
fn vph_red_guard_vg2_the_viewport_is_not_the_outer_content_height() {
    assert_eq!(
        vph_error("#o { height:50%; } #p { height:50%; }", Some(40.0)),
        pht_refusal(1, "height", "OuterHeightBasis")
    );
}

/// V1 (RED; VM1, VM9, VM10, VM11 witness): with the viewport height 40, O's `min-height:50%`
/// is 20 > 13, so O's used content height is 20 and P's `top:50%` is 10, `bottom` -10 (O's
/// content width as the basis gives top 25; the indefinite rule, 0, gives 6.5). Predecessor
/// (EXF commit ebf911bc with S0): `Captured(Unsupported { projected: 1, property: "top",
/// reason: IndefiniteOffsetBasis })`.
#[test]
fn vph_red_v1_outer_min_height_percentage_of_the_viewport() {
    vph_row(
        "#o { min-height:50%; } #p { position:relative; top:50%; }",
        Some(40.0),
        "#o { min-height:50%; } #p { position:relative; }",
        "#o { min-height:20px; } #p { position:relative; top:10px; }",
        &vph_offsets(10.0, -10.0),
    );
}

// VPH FLIP rows: they need the product change (F1-F6, W): O's percentage limits against
// the viewport height and the 45-action charge.

/// The `Debug` text of O's `Percentage` failure for `property` with `error`.
fn vph_percentage(property: &str, error: &str) -> String {
    format!("Percentage {{ source: 0, property: \"{property}\", error: {error} }}")
}

/// V2 (VM11 witness): O's `max-height:25%` of 40 is 10 < 13, so P's `top:50%` is 5 (the
/// indefinite `none` gives 6.5).
#[test]
fn vph_v2_outer_max_height_percentage() {
    vph_row(
        "#o { max-height:25%; } #p { position:relative; top:50%; }",
        Some(40.0),
        "#o { max-height:25%; } #p { position:relative; }",
        "#o { max-height:10px; } #p { position:relative; top:5px; }",
        &vph_offsets(5.0, -5.0),
    );
}

/// V3 (VM12 witness; probe VP3 witness): O's `min-height:50%` (20) and `max-height:25%`
/// (10): 13 > 10 gives 10, then 10 < 20 gives 20, so P's `top:50%` is 10 (the limits
/// swapped give 6.5; min before max gives 5).
#[test]
fn vph_v3_the_min_height_wins() {
    vph_row(
        "#o { min-height:50%; max-height:25%; } #p { position:relative; top:50%; }",
        Some(40.0),
        "#o { min-height:50%; max-height:25%; } #p { position:relative; }",
        "#o { min-height:20px; max-height:10px; } #p { position:relative; top:10px; }",
        &vph_offsets(10.0, -10.0),
    );
}

/// V4: one rounding: O's `min-height:33.3%` of the viewport height `100.1_f32` (bits
/// `42c83333`) is the exact product rounded once, bits `4205554c` (the f32 pipeline
/// f32(f32(0.333) * V) gives `4205554d`); P's `top:50%` is its exact half, bits
/// `4185554c`.
#[test]
fn vph_v4_one_rounding() {
    let viewport = f32::from_bits(0x42c8_3333);
    let limit = f32::from_bits(0x4205_554c);
    assert_eq!(
        (viewport, limit / 2.0),
        (100.1, f32::from_bits(0x4185_554c))
    );
    vph_row(
        "#o { min-height:33.3%; } #p { position:relative; top:50%; }",
        Some(viewport),
        "#o { min-height:33.3%; } #p { position:relative; }",
        "#o { min-height:33.33329772949219px; } #p { position:relative; \
         top:16.666648864746094px; }",
        &vph_offsets(limit / 2.0, -limit / 2.0),
    );
}

/// V5: P's `bottom:50%` of O's used content height 20 (`min-height:50%` of 40) is 10, so
/// `top` is -10.
#[test]
fn vph_v5_a_bottom_percentage() {
    vph_row(
        "#o { min-height:50%; } #p { position:relative; bottom:50%; }",
        Some(40.0),
        "#o { min-height:50%; } #p { position:relative; }",
        "#o { min-height:20px; } #p { position:relative; bottom:10px; }",
        &vph_offsets(-10.0, 10.0),
    );
}

/// V6 (probe VP1 witness): without a percentage limit of O the viewport height is not
/// read or validated: NaN, +inf and -1 each give the `None` layout (O's `min-height:4px`,
/// P's `top:50%` of 13 is 6.5).
#[test]
fn vph_v6_not_read_without_a_percentage_limit() {
    for viewport in [f32::NAN, f32::INFINITY, -1.0] {
        vph_row(
            "#o { min-height:4px; } #p { position:relative; top:50%; }",
            Some(viewport),
            "#o { min-height:4px; } #p { position:relative; }",
            "#o { min-height:4px; } #p { position:relative; top:6.5px; }",
            &vph_offsets(6.5, -6.5),
        );
    }
}

/// V6b (VM8 witness; probe VP4 witness): without P's offset percentage the viewport height
/// is not read: O's `min-height:10%` with NaN and O's `min-height:200%` with `f32::MAX`
/// (which would be unrepresentable) each lay out equal to the `None` run.
#[test]
fn vph_v6b_not_read_without_a_parent_offset_percentage() {
    for (fragment, viewport) in [
        ("#o { min-height:10%; } #p { position:relative; }", f32::NAN),
        (
            "#o { min-height:200%; } #p { position:relative; }",
            f32::MAX,
        ),
    ] {
        assert_pct_view(
            &vph_layout(fragment, None, Some(viewport)),
            &vph_layout(fragment, None, None),
            &[],
            fragment,
        );
    }
}

/// V7 (VM8 witness; probes VP1, VP10 witness): under an explicit O the viewport height is
/// not read: O's `height:200px; min-height:10%` with the host O content height 150 gives
/// P's `height:50%` 75 with the viewport NaN, 40 or `None` (OCH OC1).
#[test]
fn vph_v7_not_read_under_an_explicit_outer() {
    let fragment = "#o { height:200px; min-height:10%; } #p { height:50%; }";
    for viewport in [Some(f32::NAN), Some(40.0), None] {
        assert_pct_view(
            &vph_layout(fragment, Some(150.0), viewport),
            &och_layout(fragment, Some(150.0)),
            &[pct_lit("p.used_height", 75.0)],
            fragment,
        );
    }
}

/// V8 (VM2, VM4, VM5, VM6 witness): a viewport height that is not finite and nonnegative
/// is O's `InvalidContext` at P's step, named by O's percentage limit: NaN, +inf, -inf and
/// -1 under O's `min-height:10%` (P's `top:50%`) and under O's `max-height:10%` (P's
/// `bottom:50%`).
#[test]
fn vph_v8_an_invalid_viewport_height() {
    for viewport in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
        for (fragment, property) in [
            (
                "#o { min-height:10%; } #p { position:relative; top:50%; }",
                "min-height",
            ),
            (
                "#o { max-height:10%; } #p { position:relative; bottom:50%; }",
                "max-height",
            ),
        ] {
            assert_eq!(
                vph_error(fragment, Some(viewport)),
                vph_percentage(property, "InvalidContext"),
                "{fragment} (viewport {viewport})"
            );
        }
    }
}

/// V9 (VM3 witness; probe VP5 witness): zero is a valid viewport height and -0.0 resolves
/// like it: O's `min-height:10%` of -0.0 is +0 (P's `top:50%` of 13 is 6.5); O's
/// `max-height:50%` of +0 and of -0.0 is +0, so P's `top:50%` and `bottom` are +0 (bits
/// `00000000`).
#[test]
fn vph_v9_zero_and_negative_zero() {
    vph_row(
        "#o { min-height:10%; } #p { position:relative; top:50%; }",
        Some(-0.0),
        "#o { min-height:10%; } #p { position:relative; }",
        "#o { min-height:0px; } #p { position:relative; top:6.5px; }",
        &vph_offsets(6.5, -6.5),
    );
    for viewport in [0.0, -0.0] {
        vph_row(
            "#o { max-height:50%; } #p { position:relative; top:50%; }",
            Some(viewport),
            "#o { max-height:50%; } #p { position:relative; }",
            "#o { max-height:0px; } #p { position:relative; top:0px; }",
            &vph_offsets(0.0, 0.0),
        );
    }
}

/// V10 (probe VP2 witness): range: with the viewport height `f32::MAX`, O's
/// `min-height:100%` is `f32::MAX` exactly and P's `top:50%` its exact half
/// 170141173319264429905852091742258462720; O's `min-height:200%` and `max-height:200%`
/// exceed the finite range (`Unrepresentable` at O).
#[test]
fn vph_v10_range() {
    vph_row(
        "#o { min-height:100%; } #p { position:relative; top:50%; }",
        Some(f32::MAX),
        "#o { min-height:100%; } #p { position:relative; }",
        &format!(
            "#o {{ min-height:{PCT_F32_MAX_PX}; }} #p {{ position:relative; \
             top:170141173319264429905852091742258462720px; }}"
        ),
        &vph_offsets(f32::MAX / 2.0, -f32::MAX / 2.0),
    );
    for (fragment, property) in [
        (
            "#o { min-height:200%; } #p { position:relative; top:50%; }",
            "min-height",
        ),
        (
            "#o { max-height:200%; } #p { position:relative; bottom:50%; }",
            "max-height",
        ),
    ] {
        assert_eq!(
            vph_error(fragment, Some(f32::MAX)),
            vph_percentage(property, "Unrepresentable"),
            "{fragment}"
        );
    }
}

/// V11 (VM7 witness): O's unrepresentable `min-height:200%` of `f32::MAX` fails at P's
/// step, before B1's `overflow:scroll` (a resolution after P's bottom gives
/// `ChildOverflow` at 2).
#[test]
fn vph_v11_the_limit_error_precedes_the_children() {
    assert_eq!(
        vph_error(
            "#o { min-height:200%; } #p { position:relative; top:50%; } #b1 { overflow:scroll; }",
            Some(f32::MAX)
        ),
        vph_percentage("min-height", "Unrepresentable")
    );
}

/// V12 (VM7 witness): under O's content width NaN, the invalid viewport height NaN fails
/// at P's step, before P's held horizontal error
/// (`ParentHorizontal(Width(InvalidInput("containing_width")))`).
#[test]
fn vph_v12_the_validity_error_precedes_the_held_parent_error() {
    assert_eq!(
        vph_error_in(
            "#o { min-height:10%; } #p { position:relative; top:50%; }",
            (0.0, f32::NAN, 10.0),
            Some(f32::NAN)
        ),
        vph_percentage("min-height", "InvalidContext")
    );
}

/// V13 (probe VP12 witness): P's `overflow` test precedes the viewport height's validity:
/// P's `visible` is `ParentOverflow` with the viewport NaN.
#[test]
fn vph_v13_the_parent_overflow_test_precedes() {
    assert_eq!(
        vph_error(
            "#o { min-height:10%; } #p { overflow:visible; position:relative; top:50%; }",
            Some(f32::NAN)
        ),
        pht_refusal(1, "overflow", "ParentOverflow")
    );
}

/// V14 (VM14 witness): `min-height` before `max-height`: both `200%` of `f32::MAX` fail
/// at `min-height`; both `10%` with the viewport NaN name `min-height`.
#[test]
fn vph_v14_min_height_before_max_height() {
    for (fragment, viewport, error) in [
        (
            "#o { min-height:200%; max-height:200%; } #p { position:relative; top:50%; }",
            f32::MAX,
            "Unrepresentable",
        ),
        (
            "#o { min-height:10%; max-height:10%; } #p { position:relative; top:50%; }",
            f32::NAN,
            "InvalidContext",
        ),
    ] {
        assert_eq!(
            vph_error(fragment, Some(viewport)),
            vph_percentage("min-height", error),
            "{fragment}"
        );
    }
}

/// V15 (U3 floor): with P's `margin-top:-20px` O's tentative content height is -7: O's
/// `min-height:10%` of 40 is 4, so O's used content height is 4 and P's `top:50%` 2 (MFZ
/// F1 with a resolved minimum); O's `max-height:10%` (4) leaves -7 to O's default
/// `min-height` 0, so P's `top` and `bottom` are +0.
#[test]
fn vph_v15_the_user_floor_takes_a_resolved_min_height() {
    vph_row(
        "#o { min-height:10%; } #p { position:relative; top:50%; margin-top:-20px; }",
        Some(40.0),
        "#o { min-height:10%; } #p { position:relative; margin-top:-20px; }",
        "#o { min-height:4px; } #p { position:relative; top:2px; margin-top:-20px; }",
        &vph_offsets(2.0, -2.0),
    );
    vph_row(
        "#o { max-height:10%; } #p { position:relative; top:50%; margin-top:-20px; }",
        Some(40.0),
        "#o { max-height:10%; } #p { position:relative; margin-top:-20px; }",
        "#o { max-height:4px; } #p { position:relative; top:0px; margin-top:-20px; }",
        &vph_offsets(0.0, 0.0),
    );
}

/// V16 (VM10 witness): under an `auto` O the host O content height is not read: with the
/// host value 200 and the viewport height 40, P's `top:50%` is 10 (200 as the basis gives
/// 50, the indefinite rule 6.5).
#[test]
fn vph_v16_the_host_content_height_is_not_read() {
    let fragment = "#o { min-height:50%; } #p { position:relative; top:50%; }";
    let view = vph_layout(fragment, Some(200.0), Some(40.0));
    assert_pct_view(
        &view,
        &vph_layout(
            "#o { min-height:50%; } #p { position:relative; }",
            Some(200.0),
            Some(40.0),
        ),
        &vph_offsets(10.0, -10.0),
        fragment,
    );
    assert_pct_view(
        &view,
        &pct_layout(
            5,
            &pct_css(
                5,
                "#o { min-height:20px; } #p { position:relative; top:10px; }",
            ),
        ),
        &[],
        fragment,
    );
}

/// V17: the children keep their basis: B1's `top:10%` is 10% of P's used content height
/// 11, bits `3f8ccccd`, while P's `top:50%` is 10.
#[test]
fn vph_v17_the_children_keep_their_basis() {
    let b1 = f32::from_bits(0x3f8c_cccd);
    let mut expected = vph_offsets(10.0, -10.0).to_vec();
    expected.push(pct_lit("c0.offset.top", b1));
    expected.push(pct_lit("c0.offset.bottom", -b1));
    vph_row(
        "#o { min-height:50%; } #p { position:relative; top:50%; } \
         #b1 { position:relative; top:10%; }",
        Some(40.0),
        "#o { min-height:50%; } #p { position:relative; } #b1 { position:relative; }",
        "#o { min-height:20px; } #p { position:relative; top:10px; } \
         #b1 { position:relative; top:1.1px; }",
        &expected,
    );
}

/// V18 (plan addition; probe VP8 witness): P's own `height:50%` under the `auto` O keeps
/// the indefinite rule (used as `auto`) with the viewport height 40: P's used height is
/// the twin's 11, while P's `top:50%` is 10 (the viewport height as P's basis gives P a
/// used height of 20 and a `top` of 11).
#[test]
fn vph_v18_the_parent_height_percentage_keeps_the_indefinite_rule() {
    vph_row(
        "#o { min-height:50%; } #p { height:50%; position:relative; top:50%; }",
        Some(40.0),
        "#o { min-height:50%; } #p { height:50%; position:relative; }",
        "#o { min-height:20px; } #p { position:relative; top:10px; }",
        &vph_offsets(10.0, -10.0),
    );
}

/// The facade and core work of one layout.
fn vph_receipt(layout: &CapturedSiblingLayout<'_>) -> (usize, usize) {
    (layout.receipt().work, layout.receipt().core.work)
}

/// VW (probe VP6 witness): the facade work is `752 + 32N + 534(N-2) + 373(N-3)` (VPH: 45
/// fixed on every path): 3260, 4199 and 5138 at N5, N6 and N7 lay out at the exact limit
/// and one short is `Work`, both through the new entry with the viewport height 40 on V1's
/// fragment (the read path) and through the old entry on the baseline; the core work is
/// 2057, 2671 and 3286.
#[test]
fn vph_vw_facade_work_charges_45_fixed() {
    for (nodes, facade, core) in [(5, 3260, 2057), (6, 4199, 2671), (7, 5138, 3286)] {
        assert_eq!(
            facade,
            752 + 32 * nodes + 534 * (nodes - 2) + 373 * (nodes - 3),
            "N{nodes}"
        );
        let limits = |work: usize| {
            let mut limits = layout_limits();
            limits.work = nz(work);
            limits.core.work = nz(core);
            limits
        };
        let read = pct_css(
            nodes,
            "#o { min-height:50%; } #p { position:relative; top:50%; }",
        );
        let old = pct_css(nodes, "");
        let heights = (None, Some(40.0));
        for work in [facade, facade - 1] {
            let results = [
                vph_with(nodes, &read, PCT_OUTER, heights, limits(work), vph_receipt),
                pct_with(nodes, &old, PCT_OUTER, limits(work), vph_receipt),
            ];
            for result in results {
                let result = result.map_err(|error| format!("{error:?}"));
                if work == facade {
                    assert_eq!(result, Ok((facade, core)), "N{nodes}");
                } else {
                    assert_eq!(result, Err("Work".to_owned()), "N{nodes} one short");
                }
            }
        }
    }
}

// CTB: the user's U1=A (its first slice, a collapse-through B1) reuses the `vph`, `zhn`,
// `daz` and `pct` helpers as a child module of this one (CTB design section 6); VPH-L6's
// rows L1-L3 come first.
#[path = "collapse_through_b1.rs"]
mod collapse_through_b1;
