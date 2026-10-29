//! EML: `em` lengths in the Sequence facts against a host-supplied medium (CSS2.2 6.1.2,
//! 15.7, 4.3.2): `evaluate_sibling_box_facts_with_font_medium` computes every row's
//! `font-size` from the keyword table of `medium` (architecture 27) and resolves `em` in
//! every captured length longhand against it, and `ex` against its 0.5em x-height since
//! EXF (the user's U5=B).
//!
//! A child module of `outer_content_height` (EML design section 6): it reuses the `pct`,
//! `ppm`, `pht`, `ptb` and `och` helpers (`pct_css`, `pct_view`, `assert_pct_view`,
//! `pct_lit`, `pct_facts`, `pct_fact_error`, `pct_legacy_error`, `och_layout`, ...) through
//! `use super::*`. The RED part (the helpers, EM0, EM1 and EM17) compiles against the OCH
//! commit 4d3355d3 with the RED stub S0: it names no EML item and reads only published
//! geometry and producer errors as text. Fixture: `baseline_css()` (O content left 0,
//! width 100, top 10; P `hidden` with padding-top and padding-bottom 1; B1 `height:1px`),
//! N5, medium 16 unless stated, so the keyword table is [f32(0.6) * 16, 12, f32(0.89) *
//! 16, 16, f32(1.2) * 16, 24, 32]. Each admitted layout row compares the listed literals
//! and every other published number with its twin (each `em` replaced by the `px` value
//! it resolves to, through the old facts entry), bit for bit.

use super::*;

/// The medium of every row unless stated (iced's default text size, architecture 27).
const EML_MEDIUM: f32 = 16.0;

/// A producer error as text: `pct_sequence_describe`, with the CSS name of the property
/// of a `Length` error.
fn eml_describe(error: &SiblingBoxFactsError) -> String {
    match error {
        SiblingBoxFactsError::Evaluation(ClearanceBoxFactsError::Length {
            original,
            property,
            error,
        }) => format!(
            "Evaluation(Length {{ original: {original}, property: {}, error: {error:?} }})",
            property.descriptor().name
        ),
        other => pct_sequence_describe(other),
    }
}

/// `och_with` (N5, `PCT_OUTER`) with the facts of the font entry and `medium`.
fn eml_with<T>(
    css: &str,
    medium: f32,
    height: Option<f32>,
    observe: impl FnOnce(&CapturedSiblingLayout<'_>) -> T,
) -> Result<T, SiblingLayoutError> {
    let mut out = None;
    sibling_fixture(5, css, |projection, display| {
        let facts = evaluate_sibling_box_facts_with_font_medium(
            projection,
            display,
            medium,
            sibling_fact_limits(),
        )
        .unwrap_or_else(|error| panic!("{css}: producer error {}", eml_describe(&error)));
        let (content_left, content_width, content_top) = PCT_OUTER;
        out = Some(
            build_captured_sibling_layout_with_outer_height(
                &facts,
                ClearanceOuterContentGeometry {
                    outer: &projection.nodes()[0],
                    content_left,
                    content_width,
                    content_top,
                },
                height,
                layout_limits(),
            )
            .map(|layout| observe(&layout)),
        );
    });
    out.unwrap()
}

/// An admitted N5 row with medium 16 and the host value `height`: `fragment` gives the
/// `expected` literals and every other number of its `twin` through the old facts entry
/// (`och_layout`, the same host value), bit for bit.
fn eml_row_with(fragment: &str, twin: &str, height: Option<f32>, expected: &[(String, f32)]) {
    let css = pct_css(5, fragment);
    let view = eml_with(&css, EML_MEDIUM, height, pct_view)
        .unwrap_or_else(|error| panic!("{css}: expected a layout, got {error:?}"));
    assert_pct_view(&view, &och_layout(twin, height), expected, &css);
}

/// `eml_row_with` without a host value.
fn eml_row(fragment: &str, twin: &str, expected: &[(String, f32)]) {
    eml_row_with(fragment, twin, None, expected);
}

/// EM0 (RED guard; M16 witness): the old entry keeps refusing `em` and `ex` and does not
/// read `font-size`: B1's `height:2em` and `height:1ex` are `RelativeLength` at B1
/// (original 0), and P's `font-size:20px` is admitted. Passes at RED and after EML.
#[test]
fn eml_red_guard_em0_the_old_entry_refuses_em() {
    for fragment in ["#b1 { height:2em; }", "#b1 { height:1ex; }"] {
        let css = pct_css(5, fragment);
        let (error, originals) = pct_fact_error(5, &css);
        assert_eq!(
            pct_sequence_describe(&error),
            "Evaluation(Unsupported { original: 0, property: height, target: Element, reason: \
             RelativeLength })",
            "{css}"
        );
        assert_eq!(originals, vec![4, 3, 0, 1, 2], "{css}: originals");
    }
    let css = pct_css(5, "#p { font-size:20px; }");
    assert_eq!(pct_facts(5, &css).2, vec![4, 3, 0, 1, 2], "{css}");
}

/// EM1 (RED; M6 witness): with medium 16, B1's `height:2em` is 32 (O's and P's computed
/// sizes are the medium here; EM2 and EM3 separate them); every other number is the
/// `height:32px` twin's. Predecessor (the RED stub): the producer refuses B1's `height`
/// with `Evaluation(Unsupported { original: 0, property: height, target: Element,
/// reason: RelativeLength })`.
#[test]
fn eml_red_em1_height_in_em_of_the_medium() {
    eml_row(
        "#b1 { height:2em; }",
        "#b1 { height:32px; }",
        &[pct_lit("c0.used_height", 32.0)],
    );
}

/// EM17 (RED guard): the Legacy facts keep refusing `em`: B1's `height:2em` is
/// `RelativeLength` at B1's Legacy original 0. Passes at RED and after EML.
#[test]
fn eml_red_guard_em17_legacy_refuses_em() {
    let css = pct_css(5, "#b1 { height:2em; }");
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

// EML FLIP rows: they read the font entry's facts (`ClearanceBoxFact::font_size`,
// `FontSizeFact`) and its new errors.

use iced_css_core::values::RangeEvent;

/// The rows and the receipt of a font-entry run, or its producer error text.
type EmlRun = Result<(Vec<ClearanceBoxFact>, ClearanceBoxFactsReceipt), String>;

/// The font entry's run for the `nodes`-source `css` with `medium` and `limits`, every
/// source's original index and the rule count.
fn eml_run(
    nodes: usize,
    css: &str,
    medium: f32,
    limits: SiblingBoxFactsLimits,
) -> (EmlRun, Vec<usize>, usize) {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        let originals = (0..nodes)
            .map(|source| projection.original_index(source).unwrap())
            .collect();
        let run = evaluate_sibling_box_facts_with_font_medium(projection, display, medium, limits)
            .map(|facts| {
                let rows = (0..nodes)
                    .map(|source| *facts.row(source).unwrap())
                    .collect();
                (rows, facts.receipt().evaluation)
            })
            .map_err(|error| eml_describe(&error));
        out = Some((run, originals, display.bundle().rule_count()));
    });
    out.unwrap()
}

/// The N5 rows and receipt of `fragment` with medium 16; the facts must succeed.
fn eml_facts(fragment: &str) -> (Vec<ClearanceBoxFact>, ClearanceBoxFactsReceipt) {
    let css = pct_css(5, fragment);
    let (run, originals, _) = eml_run(5, &css, EML_MEDIUM, sibling_fact_limits());
    assert_eq!(originals, vec![4, 3, 0, 1, 2], "{css}: originals");
    run.unwrap_or_else(|error| panic!("{css}: producer error {error}"))
}

/// The N5 producer error text of `fragment` with `medium` and `limits`; facts are a
/// failure.
fn eml_error_with(fragment: &str, medium: f32, limits: SiblingBoxFactsLimits) -> String {
    let css = pct_css(5, fragment);
    match eml_run(5, &css, medium, limits).0 {
        Ok(_) => panic!("{css}: expected a producer error, got facts"),
        Err(error) => error,
    }
}

/// `eml_error_with` with medium 16 and the default limits.
fn eml_error(fragment: &str) -> String {
    eml_error_with(fragment, EML_MEDIUM, sibling_fact_limits())
}

/// The text of `Evaluation(Unsupported { .. })` with `RelativeLength` at the element
/// longhand `name` of `original`.
fn eml_relative(original: usize, name: &str) -> String {
    format!(
        "Evaluation({})",
        pct_refusal(original, name, BoxFactUnsupported::RelativeLength)
    )
}

/// One expected font-size fact.
fn eml_font(pixels: f32, event: Option<RangeEvent>, origin: BoxFactOrigin) -> FontSizeFact {
    FontSizeFact {
        pixels,
        event,
        origin,
    }
}

/// `actual` equals `expected` with bit-equal pixels.
fn assert_eml_font(actual: Option<FontSizeFact>, expected: FontSizeFact, context: &str) {
    let actual = actual.unwrap_or_else(|| panic!("{context}: no font size"));
    assert_eq!(actual, expected, "{context}");
    assert_eq!(
        actual.pixels.to_bits(),
        expected.pixels.to_bits(),
        "{context}"
    );
}

/// EM2 (M1, M13 witness): an `em` on another property uses the row's own computed size:
/// B1's `font-size:10px; height:2em` is 20 (the parent's 16 would give 32).
#[test]
fn eml_em2_em_uses_the_own_font_size() {
    eml_row(
        "#b1 { font-size:10px; height:2em; }",
        "#b1 { font-size:10px; height:20px; }",
        &[pct_lit("c0.used_height", 20.0)],
    );
}

/// EM3 (M2, M3 witness): an `em` in `font-size` uses the parent's size: under P's
/// `font-size:20px`, B1's `font-size:0.5em` is 10 and its `height:2em` 20 (the medium as
/// the basis gives 8 and 16); with no `font-size` B1 inherits P's 20, so `height:1em` is
/// 20 (the medium would give 16).
#[test]
fn eml_em3_font_size_em_uses_the_parent_size() {
    for (fragment, twin, font, height) in [
        (
            "#p { font-size:20px; } #b1 { font-size:0.5em; height:2em; }",
            "#p { font-size:20px; } #b1 { font-size:0.5em; height:20px; }",
            eml_font(10.0, None, BoxFactOrigin::Specified),
            20.0,
        ),
        (
            "#p { font-size:20px; } #b1 { height:1em; }",
            "#p { font-size:20px; } #b1 { height:20px; }",
            eml_font(20.0, None, BoxFactOrigin::Inherited { original_parent: 3 }),
            20.0,
        ),
    ] {
        eml_row(fragment, twin, &[pct_lit("c0.used_height", height)]);
        assert_eml_font(eml_facts(fragment).0[2].font_size, font, fragment);
    }
}

/// EM4 (M4, M5 witness): O, the root, takes the medium as the basis of its own `em` and
/// percentage and as its value for `inherit` (CSS22-001338; CSS2.2 6.2.1): O's
/// `font-size:2em`, `50%` and `inherit` give B1's `height:1em` 32, 8 and 16.
#[test]
fn eml_em4_the_root_uses_the_medium() {
    for (fragment, twin, font, height) in [
        (
            "#o { font-size:2em; } #b1 { height:1em; }",
            "#o { font-size:2em; } #b1 { height:32px; }",
            eml_font(32.0, None, BoxFactOrigin::Specified),
            32.0,
        ),
        (
            "#o { font-size:50%; } #b1 { height:1em; }",
            "#o { font-size:50%; } #b1 { height:8px; }",
            eml_font(8.0, None, BoxFactOrigin::Specified),
            8.0,
        ),
        (
            "#o { font-size:inherit; } #b1 { height:1em; }",
            "#o { font-size:inherit; } #b1 { height:16px; }",
            eml_font(16.0, None, BoxFactOrigin::Initial),
            16.0,
        ),
    ] {
        eml_row(fragment, twin, &[pct_lit("c0.used_height", height)]);
        assert_eml_font(eml_facts(fragment).0[0].font_size, font, fragment);
    }
}

/// EM5 (M7 witness): keywords through the table of medium 16 and the relative mapping:
/// P's `large`, `xx-small`, `x-small`, `larger` (16 is the table's middle entry, so the
/// next entry) and `smaller` give B1's `height:1em` the bits `4199999a`, `4119999a`, 12,
/// `4199999a` and `4163d70a`; under P's `font-size:17px`, B1's `larger` is the interior
/// value T4 + f32(f32(17 - 16) / f32(T4 - 16)) * f32(T5 - T4), each step one f32
/// rounding: bits `41a5999a`.
#[test]
fn eml_em5_keywords_through_the_table() {
    for (fragment, twin, bits) in [
        (
            "#p { font-size:large; } #b1 { height:1em; }",
            "#p { font-size:large; } #b1 { height:19.2px; }",
            0x4199_999a,
        ),
        (
            "#p { font-size:xx-small; } #b1 { height:1em; }",
            "#p { font-size:xx-small; } #b1 { height:9.6px; }",
            0x4119_999a,
        ),
        (
            "#p { font-size:x-small; } #b1 { height:1em; }",
            "#p { font-size:x-small; } #b1 { height:12px; }",
            0x4140_0000,
        ),
        (
            "#p { font-size:larger; } #b1 { height:1em; }",
            "#p { font-size:larger; } #b1 { height:19.2px; }",
            0x4199_999a,
        ),
        (
            "#p { font-size:smaller; } #b1 { height:1em; }",
            "#p { font-size:smaller; } #b1 { height:14.24px; }",
            0x4163_d70a,
        ),
        (
            "#p { font-size:17px; } #b1 { font-size:larger; height:1em; }",
            "#p { font-size:17px; } #b1 { font-size:larger; height:20.7px; }",
            0x41a5_999a,
        ),
    ] {
        eml_row(
            fragment,
            twin,
            &[pct_lit("c0.used_height", f32::from_bits(bits))],
        );
    }
}

/// EM6: `inherit` copies the parent's computed (absolute) value: P's `margin-left:1em`
/// at P's `font-size:10px` is 10 (P's width 90), and B1's `margin-left:inherit` is 10
/// under B1's own `font-size:20px` (an inherited `em` would give 20).
#[test]
fn eml_em6_inherit_copies_the_computed_length() {
    eml_row(
        "#p { font-size:10px; margin-left:1em; } #b1 { font-size:20px; margin-left:inherit; }",
        "#p { font-size:10px; margin-left:10px; } #b1 { font-size:20px; margin-left:inherit; }",
        &[
            pct_lit("p.margin_left", 10.0),
            pct_lit("p.width", 90.0),
            pct_lit("c0.margin.left", 10.0),
        ],
    );
}

/// EM7: every other length family of B1 at `font-size:4px`: `width:5em`,
/// `min-height:1em`, `max-width:20em`, `margin-top:-0.5em`, `padding-left:0.25em` and a
/// solid `border-left-width:0.5em` equal their `px` twin (20, 4, 80, -2, 1, 2).
#[test]
fn eml_em7_every_length_family() {
    eml_row(
        "#b1 { font-size:4px; width:5em; min-height:1em; max-width:20em; margin-top:-0.5em; \
         padding-left:0.25em; border-left-style:solid; border-left-width:0.5em; }",
        "#b1 { font-size:4px; width:20px; min-height:4px; max-width:80px; margin-top:-2px; \
         padding-left:1px; border-left-style:solid; border-left-width:2px; }",
        &[
            pct_lit("c0.used_width", 20.0),
            pct_lit("c0.used_height", 4.0),
        ],
    );
}

/// EM8 (OC-15; M11 witness): `em` offsets are absolute in the fact row, so P's step, the
/// copy into P's used row, the child loop and the vertical pass read converted values:
/// P's `top:1em; left:0.5em` give P's offsets 16/-16 and 8/-8; B1's `top:-1em;
/// bottom:2em` give -16/16 (`top` wins).
#[test]
fn eml_em8_em_offsets_are_absolute_in_the_facts() {
    eml_row(
        "#p { position:relative; top:1em; left:0.5em; } \
         #b1 { position:relative; top:-1em; bottom:2em; }",
        "#p { position:relative; top:16px; left:8px; } \
         #b1 { position:relative; top:-16px; bottom:32px; }",
        &[
            pct_lit("p.offset.top", 16.0),
            pct_lit("p.offset.bottom", -16.0),
            pct_lit("p.offset.left", 8.0),
            pct_lit("p.offset.right", -8.0),
            pct_lit("c0.offset.top", -16.0),
            pct_lit("c0.offset.bottom", 16.0),
        ],
    );
}

/// EM8b (OC-15 with OCH): through the OCH entry with the host's 200 under O's
/// `height:200px`, P's `top:10%` is 20 and its `left:1em` 16, P's `height:5em` 80 makes P
/// explicit, so B1's `height:50%` is 40, and B1's `bottom:1em` gives -16/16.
#[test]
fn eml_em8b_em_offsets_with_the_host_height() {
    eml_row_with(
        "#o { height:200px; } #p { position:relative; top:10%; left:1em; height:5em; } \
         #b1 { height:50%; position:relative; bottom:1em; }",
        "#o { height:200px; } #p { position:relative; top:10%; left:16px; height:80px; } \
         #b1 { height:50%; position:relative; bottom:16px; }",
        Some(200.0),
        &[
            pct_lit("p.offset.top", 20.0),
            pct_lit("p.offset.left", 16.0),
            pct_lit("p.used_height", 80.0),
            pct_lit("c0.used_height", 40.0),
            pct_lit("c0.offset.top", -16.0),
            pct_lit("c0.offset.bottom", 16.0),
        ],
    );
}

/// EM9 (migrated by EXF, the user's U5=B; XM8 witness; M12 witness by the tail): in the
/// font entry `ex` resolves against the 0.5em x-height: B1's `height:1ex` is 8, P's
/// `font-size:1ex` 8 (the medium's 16 halved) and B1's solid `border-top-width:1ex` 8
/// (before EXF the three were `RelativeLength`); a `border-top-width` of `1ex` or `1em`
/// on a side whose style is `none` is +0 with no event and no counted conversion (style
/// first).
#[test]
fn eml_em9_ex_resolves_and_a_suppressed_side_stays_zero() {
    let specified = BoxFactOrigin::Specified;
    let fragment = "#b1 { height:1ex; }";
    assert_eq!(
        eml_facts(fragment).0[2].height,
        BoxLengthFact::Absolute(AbsoluteBoxFact {
            pixels: 8.0,
            rounded_to_zero: false,
            origin: specified,
        }),
        "{fragment}"
    );
    let fragment = "#p { font-size:1ex; }";
    assert_eml_font(
        eml_facts(fragment).0[1].font_size,
        eml_font(8.0, None, specified),
        fragment,
    );
    let fragment = "#b1 { border-top-style:solid; border-top-width:1ex; }";
    assert_eq!(
        eml_facts(fragment).0[2].border_width[0],
        BorderWidthFact {
            pixels: 8.0,
            event: None,
            origin: specified,
        },
        "{fragment}"
    );
    let calls = eml_facts("").1.numeric_calls;
    for fragment in [
        "#b1 { border-top-width:1ex; }",
        "#b1 { border-top-width:1em; }",
    ] {
        let (rows, receipt) = eml_facts(fragment);
        let width = rows[2].border_width[0];
        assert_eq!(width.pixels.to_bits(), 0, "{fragment}");
        assert_eq!(width.event, None, "{fragment}");
        assert_eq!(width.origin, BoxFactOrigin::Specified, "{fragment}");
        assert_eq!(receipt.numeric_calls, calls, "{fragment}");
    }
}

/// EM10 (M17 witness): provenance with B1's `font-size:10px`: O `Initial` 16, P inherited
/// from O, B1 `Specified` 10, F and B2 inherited from P; the old entry's rows carry no
/// font size.
#[test]
fn eml_em10_font_size_provenance() {
    let (rows, _) = eml_facts("#b1 { font-size:10px; }");
    let inherited = |original_parent| BoxFactOrigin::Inherited { original_parent };
    for (index, expected) in [
        eml_font(16.0, None, BoxFactOrigin::Initial),
        eml_font(16.0, None, inherited(4)),
        eml_font(10.0, None, BoxFactOrigin::Specified),
        eml_font(16.0, None, inherited(3)),
        eml_font(16.0, None, inherited(3)),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eml_font(rows[index].font_size, expected, &format!("row {index}"));
    }
    let css = pct_css(5, "#b1 { font-size:10px; }");
    for (index, row) in pct_facts(5, &css).0.iter().enumerate() {
        assert_eq!(row.font_size, None, "old entry row {index}");
    }
}

/// EM11 (M8 witness): one rounding: B1's `height:0.3em` at P's `font-size:3px` is the
/// exact 0.9 rounded once, bits `3f666666` (the f32 pipeline f32(f32(0.3) * 3) gives
/// `3f666667`), equal to the `height:0.9px` twin.
#[test]
fn eml_em11_one_rounding() {
    eml_row(
        "#p { font-size:3px; } #b1 { height:0.3em; }",
        "#p { font-size:3px; } #b1 { height:0.9px; }",
        &[pct_lit("c0.used_height", f32::from_bits(0x3f66_6666))],
    );
}

/// EM12: a zero font size gives +0: B1's `font-size:0px; margin-top:-1em` has the
/// margin-top bits `00000000` and the font size +0.
#[test]
fn eml_em12_zero_font_size() {
    eml_row(
        "#b1 { font-size:0px; margin-top:-1em; }",
        "#b1 { font-size:0px; margin-top:0px; }",
        &[pct_lit("c0.margin.top", 0.0)],
    );
    let fragment = "#b1 { font-size:0px; margin-top:-1em; }";
    assert_eml_font(
        eml_facts(fragment).0[2].font_size,
        eml_font(0.0, None, BoxFactOrigin::Specified),
        fragment,
    );
}

/// EM13 (M19 witness): range. Under P's `font-size:<f32::MAX>px` (exact, no event), B1's
/// `height:2em` exceeds the finite range (`Length` with `Unrepresentable`), while B1's
/// `font-size:2em` and its solid `border-top-width:2em` saturate at `f32::MAX` with
/// `UpperSaturation`.
#[test]
fn eml_em13_range() {
    let parent = format!("#p {{ font-size:{PCT_F32_MAX_PX}; }}");
    assert_eq!(
        eml_error(&format!("{parent} #b1 {{ height:2em; }}")),
        "Evaluation(Length { original: 0, property: height, error: \
         Conversion(Unrepresentable) })"
    );
    let saturated = eml_font(
        f32::MAX,
        Some(RangeEvent::UpperSaturation),
        BoxFactOrigin::Specified,
    );
    let fragment = format!("{parent} #b1 {{ font-size:2em; }}");
    let (rows, _) = eml_facts(&fragment);
    assert_eml_font(
        rows[1].font_size,
        eml_font(f32::MAX, None, BoxFactOrigin::Specified),
        &fragment,
    );
    assert_eml_font(rows[2].font_size, saturated, &fragment);
    let fragment = format!("{parent} #b1 {{ border-top-style:solid; border-top-width:2em; }}");
    let width = eml_facts(&fragment).0[2].border_width[0];
    assert_eq!(width.pixels, f32::MAX, "{fragment}");
    assert_eq!(width.event, Some(RangeEvent::UpperSaturation), "{fragment}");
}

/// EM14 (M18 witness): the medium is validated first: 0, -1, NaN and +infinity are
/// `InvalidMedium`; `f32::MAX` (1.2 * MAX overflows) and the smallest subnormal (0.6 and
/// 0.75 of it round to the same value) are `InvalidTable`; NaN with a `sources` limit of
/// 4 is still `InvalidMedium`, where medium 16 gives `Sources`.
#[test]
fn eml_em14_the_medium_is_validated_first() {
    for (medium, expected) in [
        (0.0, "FontMedium(InvalidMedium)"),
        (-1.0, "FontMedium(InvalidMedium)"),
        (f32::NAN, "FontMedium(InvalidMedium)"),
        (f32::INFINITY, "FontMedium(InvalidMedium)"),
        (f32::MAX, "FontMedium(InvalidTable)"),
        (f32::from_bits(1), "FontMedium(InvalidTable)"),
    ] {
        assert_eq!(
            eml_error_with("", medium, sibling_fact_limits()),
            expected,
            "{medium}"
        );
    }
    let mut limits = sibling_fact_limits();
    limits.sources = nz(4);
    assert_eq!(
        eml_error_with("", f32::NAN, limits),
        "FontMedium(InvalidMedium)"
    );
    assert_eq!(eml_error_with("", EML_MEDIUM, limits), "Sources");
}

/// EM15 (M13 witness; migrated by EXF, the user's U5=B; XM5 witness): `font-size` is
/// computed before the row's other longhands: B1's `font-size:1ex` with `height:1ex`, in
/// either declaration order, gives B1's font size 8 (the parent's 16 halved) and its
/// `height` 4 (its own 8 halved; `height` before `font-size` would give 8). Before EXF
/// both orders were refused at `font-size`.
#[test]
fn eml_em15_font_size_precedes_the_other_longhands() {
    for fragment in [
        "#b1 { font-size:1ex; height:1ex; }",
        "#b1 { height:1ex; font-size:1ex; }",
    ] {
        let rows = eml_facts(fragment).0;
        assert_eml_font(
            rows[2].font_size,
            eml_font(8.0, None, BoxFactOrigin::Specified),
            fragment,
        );
        assert_eq!(
            rows[2].height,
            BoxLengthFact::Absolute(AbsoluteBoxFact {
                pixels: 4.0,
                rounded_to_zero: false,
                origin: BoxFactOrigin::Specified,
            }),
            "{fragment}"
        );
    }
}

/// The work fragment of EM16: an `em` and a percentage `font-size`, a keyword, and `em`
/// lengths.
const EML_WORK_FRAGMENT: &str = "#o { font-size:20px; } #p { font-size:larger; } \
                                 #b1 { font-size:2em; height:1em; } #b2 { font-size:150%; }";

/// `EML_WORK_FRAGMENT` with B1's `height:1px`, admitted by the old entry (which reads no
/// `font-size`); the same rules.
const EML_WORK_TWIN: &str = "#o { font-size:20px; } #p { font-size:larger; } \
                             #b1 { font-size:2em; height:1px; } #b2 { font-size:150%; }";

/// EM16 (M14, M15 witness): the font entry's work is `36 * n + 2 + r` and its numeric
/// allowance `23 * n * numeric.work`, exact and one short, at N5 and N7; the old entry
/// keeps `35 * n + 2 + r` and `22 * n * numeric.work` on the same sources.
#[test]
fn eml_em16_work_and_numeric_allowance() {
    for nodes in [5, 7] {
        let css = pct_css(nodes, EML_WORK_FRAGMENT);
        let (run, _, rules) = eml_run(nodes, &css, EML_MEDIUM, sibling_fact_limits());
        let receipt = run.unwrap_or_else(|error| panic!("N{nodes}: {error}")).1;
        let work = 36 * nodes + 2 + rules;
        let allowance = 23 * nodes * 100_000;
        assert_eq!(
            (receipt.work, receipt.numeric_work),
            (work, allowance),
            "N{nodes}"
        );
        let mut exact = sibling_fact_limits();
        exact.evaluation.work = nz(work);
        exact.evaluation.numeric_work = nz(allowance);
        assert!(
            eml_run(nodes, &css, EML_MEDIUM, exact).0.is_ok(),
            "N{nodes} exact"
        );
        let mut short = exact;
        short.evaluation.work = nz(work - 1);
        assert_eq!(
            eml_run(nodes, &css, EML_MEDIUM, short).0.err(),
            Some("Evaluation(Work)".to_owned()),
            "N{nodes} work one short"
        );
        let mut short = exact;
        short.evaluation.numeric_work = nz(allowance - 1);
        assert_eq!(
            eml_run(nodes, &css, EML_MEDIUM, short).0.err(),
            Some("Evaluation(NumericWork)".to_owned()),
            "N{nodes} numeric allowance one short"
        );
        let (_, receipt, _) = pct_facts(nodes, &pct_css(nodes, EML_WORK_TWIN));
        assert_eq!(
            (receipt.work, receipt.numeric_work),
            (35 * nodes + 2 + rules, 22 * nodes * 100_000),
            "N{nodes} old entry"
        );
    }
}

/// EM18 (review C-5 (a); E1 witness): `initial` is not a CSS 2.2 value, so B1's
/// `font-size:initial` is dropped by the parser and B1 inherits P's `font-size:20px`
/// (origin `Inherited`), and its `height:1em` is 20 (the medium would give 16). The
/// `Initial` arm of a row with a parent is therefore not reachable from author CSS: the
/// `font` shorthand yields `SpecifiedValue::Initial` only for an omitted longhand, and
/// `font-size` cannot be omitted there.
#[test]
fn eml_em18_initial_is_not_a_css22_font_size() {
    let fragment = "#p { font-size:20px; } #b1 { font-size:initial; height:1em; }";
    eml_row(
        fragment,
        "#p { font-size:20px; } #b1 { font-size:initial; height:20px; }",
        &[pct_lit("c0.used_height", 20.0)],
    );
    assert_eml_font(
        eml_facts(fragment).0[2].font_size,
        eml_font(20.0, None, BoxFactOrigin::Inherited { original_parent: 3 }),
        fragment,
    );
}

/// EM19 (review C-5 (b); M21, M22 witness): `numeric_calls` counts one conversion per
/// computed `font-size` length, `em` or percentage, none for a keyword, the initial value
/// or inheritance, and an `em` length like a `px` one: O's `large` (a keyword), P's `150%`,
/// B1's `12px` and B2's `0.5em` (three conversions; F inherits) with B1's `height:2em`
/// count three more than the old entry's run of the `height:24px` twin.
#[test]
fn eml_em19_numeric_calls() {
    let fragment = "#o { font-size:large; } #p { font-size:150%; } \
                    #b1 { font-size:12px; height:2em; } #b2 { font-size:0.5em; }";
    let twin = "#o { font-size:large; } #p { font-size:150%; } \
                #b1 { font-size:12px; height:24px; } #b2 { font-size:0.5em; }";
    let calls = eml_facts(fragment).1.numeric_calls;
    let old = pct_facts(5, &pct_css(5, twin)).1.numeric_calls;
    assert_eq!((calls, old), (13, 10), "{fragment}");
}

/// EM20 (review C-5 (c); M23 witness): an inherited fact keeps the parent's range event:
/// O's `font-size:<f32::MAX>px` and P's `font-size:2em` saturate P at `f32::MAX` with
/// `UpperSaturation`; B1, F and B2 inherit pixels and event from P.
#[test]
fn eml_em20_an_inherited_fact_keeps_the_event() {
    let fragment = format!("#o {{ font-size:{PCT_F32_MAX_PX}; }} #p {{ font-size:2em; }}");
    let (rows, _) = eml_facts(&fragment);
    let event = Some(RangeEvent::UpperSaturation);
    assert_eml_font(
        rows[1].font_size,
        eml_font(f32::MAX, event, BoxFactOrigin::Specified),
        &fragment,
    );
    for (index, row) in rows.iter().enumerate().skip(2) {
        assert_eml_font(
            row.font_size,
            eml_font(
                f32::MAX,
                event,
                BoxFactOrigin::Inherited { original_parent: 3 },
            ),
            &format!("{fragment}: row {index}"),
        );
    }
}

/// EM21 (review C-5 (d); M24 witness): a percentage `font-size` on a child uses the
/// parent's size: under P's `font-size:20px`, B1's `font-size:50%` is 10 and its
/// `height:2em` 20 (the medium as the basis gives 8 and 16).
#[test]
fn eml_em21_a_child_percentage_uses_the_parent_size() {
    let fragment = "#p { font-size:20px; } #b1 { font-size:50%; height:2em; }";
    eml_row(
        fragment,
        "#p { font-size:20px; } #b1 { font-size:50%; height:20px; }",
        &[pct_lit("c0.used_height", 20.0)],
    );
    assert_eml_font(
        eml_facts(fragment).0[2].font_size,
        eml_font(10.0, None, BoxFactOrigin::Specified),
        fragment,
    );
}

/// D4 measurement (x86_64; measured at FLIP): the fact row with its new `font_size` field
/// and the new `FontSizeFact`.
#[test]
fn eml_representation_measurement() {
    let measured = [
        (
            "ClearanceBoxFact",
            std::mem::size_of::<ClearanceBoxFact>(),
            std::mem::align_of::<ClearanceBoxFact>(),
        ),
        (
            "FontSizeFact",
            std::mem::size_of::<FontSizeFact>(),
            std::mem::align_of::<FontSizeFact>(),
        ),
    ];
    for (name, size, align) in measured {
        println!("EML_REPRESENTATION {name} {size} {align}");
    }
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    assert_eq!(
        measured.map(|(_, size, align)| (size, align)),
        [(872, 8), (24, 8)]
    );
}

// ZHN: the carried EML rows (EM-10: EMX1, EMX2; F-4: EMX3) first; then the zero-height
// normal rows as a child module of this one (ZHN design section 6).

/// The Legacy facts rows of the five-source capture `css`, built as `pct_legacy_error`
/// builds the Legacy projection; a Legacy producer error is a failure.
fn eml_legacy_rows(css: &str) -> Vec<ClearanceBoxFact> {
    let mut out = None;
    sibling_fixture(5, css, |projection, display| {
        let legacy_projection = prepare_clearance_source_projection(
            projection.host(),
            projection.snapshot(),
            ClearanceProjectionLimits {
                work: nz(36),
                retained_bytes: bound(),
            },
        )
        .unwrap();
        let facts = evaluate_clearance_box_facts(&legacy_projection, display, fact_limits())
            .unwrap_or_else(|error| panic!("{css}: Legacy producer error {error:?}"));
        out = Some(
            (0..5)
                .map(|projected| *facts.row(projected).unwrap())
                .collect(),
        );
    });
    out.unwrap()
}

/// EMX1 (EM-10, EML plan review F-2; MX1 witness): B1's `font-size` of 1e-47px, below half
/// the smallest f32 subnormal, is +0 with `PositiveRoundedToZero` (origin `Specified`).
/// Passes before and after ZHN.
#[test]
fn zhn_red_guard_emx1_a_font_size_below_half_the_smallest_subnormal_is_zero() {
    let fragment = "#b1 { font-size:0.00000000000000000000000000000000000000000000001px; }";
    assert_eml_font(
        eml_facts(fragment).0[2].font_size,
        eml_font(
            0.0,
            Some(RangeEvent::PositiveRoundedToZero),
            BoxFactOrigin::Specified,
        ),
        fragment,
    );
}

/// EMX2 (EM-10, EML plan review F-3; MX2 witness): the system font of `font: caption`
/// reaches `compute_font_size` as an unresolved value and is
/// `Evaluation(FontSize { original: 0, error: UnresolvedValue })` at B1. Passes before and
/// after ZHN.
#[test]
fn zhn_red_guard_emx2_a_system_font_is_a_font_size_error() {
    assert_eq!(
        eml_error("#b1 { font: caption; }"),
        "Evaluation(FontSize { original: 0, error: UnresolvedValue })"
    );
}

/// EMX3 (EML plan review F-4): the Legacy facts of `#b1 { font-size:20px; }` are Ok and
/// every Legacy row has `font_size: None`. Passes before and after ZHN.
#[test]
fn zhn_red_guard_emx3_legacy_rows_carry_no_font_size() {
    let css = pct_css(5, "#b1 { font-size:20px; }");
    let rows = eml_legacy_rows(&css);
    assert_eq!(rows.len(), 5, "{css}");
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.font_size, None, "{css}: Legacy row {index}");
    }
}

// ZHN: zero-height Sequence normals whose own margins are not adjoining reuse the `pct`,
// `pht`, `och` and `eml` helpers as a child module of this one (ZHN design section 6).
#[path = "zero_height_normals.rs"]
mod zero_height_normals;
