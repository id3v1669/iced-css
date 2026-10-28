//! PBR producer facts: computed `border-*-width` rows of both fact entries.
//!
//! Installed at FLIP: it reads `ClearanceBoxFact::border_width` and `BorderWidthFact`.
//! Rows F1-F22 pass the CSS cell as the complete author stylesheet to
//! `sibling_fixture(5, ..)` with `sibling_fact_limits()` (P original 3, O original 4,
//! B1 original 0; rows are indexed by projected source: O 0, P 1, B1 2, F 3, B2 4).
//! Legacy fence rows use `fixture(..)` (P original 3 at row 0, B1 original 0 at row 1)
//! or the nested Legacy projection of the five-source capture.

use super::*;
use iced_css::evaluation::clearance_box_facts::BorderWidthFact;
use iced_css_core::{
    syntax::{DeclarationItem, DeclarationLimits, Declarations},
    values::{
        RangeEvent,
        border_width::{BorderFontContext, compute_border_width},
        property::{PropertyResult, SpecifiedValue, parse_property},
    },
};

const CSS: &str = "#p, #b1, #f, #b2 { display:block; } #p { padding-top:1px; padding-bottom:1px; } #b1 { height:1px; margin-bottom:4px; } #f { float:left; width:10px; height:2px; } #b2 { height:1px; margin-top:3px; clear:both; }";

/// Every Legacy width fact, and every Sequence width of a suppressed side whose
/// longhand is not declared.
const ZERO_WIDTH: BorderWidthFact = BorderWidthFact {
    pixels: 0.,
    event: None,
    origin: BoxFactOrigin::Initial,
};

/// 10^39 px: above f32::MAX (F3, F12).
const F3_PX: &str = "1000000000000000000000000000000000000000px";
/// 10^-51 px: a positive length below half the smallest subnormal (F4, F13).
const F4_PX: &str = "0.000000000000000000000000000000000000000000000000001px";
/// Exactly f32::MAX (F5).
const F5_PX: &str = "340282346638528859811704183484516925440px";

const MAX_BITS: u32 = 0x7f7f_ffff;

/// One successful Sequence batch.
struct FactRun {
    rows: Vec<ClearanceBoxFact>,
    receipt: SiblingBoxFactsReceipt,
    rules: usize,
}

fn fact_run_with(
    nodes: usize,
    css: &str,
    limits: SiblingBoxFactsLimits,
) -> Result<FactRun, SiblingBoxFactsError> {
    let mut out = None;
    sibling_fixture(nodes, css, |projection, display| {
        out = Some(
            evaluate_sibling_box_facts(projection, display, limits).map(|facts| FactRun {
                rows: (0..nodes)
                    .map(|source| *facts.row(source).unwrap())
                    .collect(),
                receipt: facts.receipt(),
                rules: display.bundle().rule_count(),
            }),
        );
    });
    out.unwrap()
}

fn facts(nodes: usize, css: &str) -> FactRun {
    match fact_run_with(nodes, css, sibling_fact_limits()) {
        Ok(run) => run,
        Err(error) => panic!("{css}: expected facts, got {error:?}"),
    }
}

fn facts_error(nodes: usize, css: &str) -> SiblingBoxFactsError {
    match fact_run_with(nodes, css, sibling_fact_limits()) {
        Ok(run) => panic!("{css}: expected a producer error, got {:?}", run.rows),
        Err(error) => error,
    }
}

/// One Legacy batch: rows, receipt and rule count.
type LegacyRun = (Vec<ClearanceBoxFact>, ClearanceBoxFactsReceipt, usize);

/// The four-source Legacy fixture (P is the root).
fn legacy(css: &str) -> Result<LegacyRun, ClearanceBoxFactsError> {
    let mut out = None;
    fixture(css, |projection, display| {
        out = Some(
            evaluate_clearance_box_facts(projection, display, fact_limits()).map(|batch| {
                (
                    (0..4).map(|row| *batch.row(row).unwrap()).collect(),
                    batch.receipt(),
                    display.bundle().rule_count(),
                )
            }),
        );
    });
    out.unwrap()
}

/// The nested five-source Legacy projection of the sibling capture (O is the root).
fn legacy_nested(css: &str) -> Result<Vec<ClearanceBoxFact>, ClearanceBoxFactsError> {
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
        out = Some(
            evaluate_clearance_box_facts(&legacy_projection, display, fact_limits())
                .map(|batch| (0..5).map(|row| *batch.row(row).unwrap()).collect()),
        );
    });
    out.unwrap()
}

fn assert_unsupported(
    error: &ClearanceBoxFactsError,
    expected: (usize, &str, BoxFactTarget, BoxFactUnsupported),
    context: &str,
) {
    let (original, name, target, reason) = expected;
    assert!(
        matches!(
            error,
            ClearanceBoxFactsError::Unsupported {
                original: actual,
                property,
                target: actual_target,
                reason: actual_reason,
            } if *actual == original
                && property.descriptor().name == name
                && *actual_target == target
                && *actual_reason == reason
        ),
        "{context}: expected Unsupported at original {original} on {name}, got {error:?}"
    );
}

fn assert_width(
    actual: BorderWidthFact,
    expected: (u32, Option<RangeEvent>, BoxFactOrigin),
    context: &str,
) {
    assert_eq!(
        (actual.pixels.to_bits(), actual.event, actual.origin),
        expected,
        "{context}: {actual:?}"
    );
}

/// All four sides with the same fact.
fn assert_sides(
    actual: [BorderWidthFact; 4],
    expected: (u32, Option<RangeEvent>, BoxFactOrigin),
    context: &str,
) {
    for (side, fact) in actual.into_iter().enumerate() {
        assert_width(fact, expected, &format!("{context}: side {side}"));
    }
}

/// Sides with their own pixels and one event and origin.
fn assert_pixels(
    actual: [BorderWidthFact; 4],
    pixels: [f32; 4],
    origin: BoxFactOrigin,
    context: &str,
) {
    for (side, (fact, value)) in actual.into_iter().zip(pixels).enumerate() {
        assert_width(
            fact,
            (value.to_bits(), None, origin),
            &format!("{context}: side {side}"),
        );
    }
}

const SPECIFIED_ZERO: (u32, Option<RangeEvent>, BoxFactOrigin) =
    (0, None, BoxFactOrigin::Specified);

fn inherited(original_parent: usize) -> BoxFactOrigin {
    BoxFactOrigin::Inherited { original_parent }
}

/// A complete specified value through the public declaration and property parsers.
fn specified(declaration: &str) -> SpecifiedValue {
    let item = Declarations::new(
        declaration,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: nz(100_000),
                nesting: nz(20),
                retained_bytes: nz(4096),
            },
            name_bytes: nz(4096),
        },
    )
    .next()
    .unwrap()
    .unwrap();
    let DeclarationItem::Declaration(parsed) = item else {
        panic!("{declaration}: not a declaration")
    };
    let PropertyResult::Parsed(value) = parse_property(&parsed, values()).unwrap() else {
        panic!("{declaration}: not a parsed property")
    };
    value.value
}

// Measurement (the GREEN fragment adds the measured x86_64 pin).

#[test]
fn pbr_fact_representation_measurement() {
    let row = (
        std::mem::size_of::<ClearanceBoxFact>(),
        std::mem::align_of::<ClearanceBoxFact>(),
    );
    let width = (
        std::mem::size_of::<BorderWidthFact>(),
        std::mem::align_of::<BorderWidthFact>(),
    );
    println!("PBR_REPRESENTATION ClearanceBoxFact {} {}", row.0, row.1);
    println!("PBR_REPRESENTATION BorderWidthFact {} {}", width.0, width.1);
    assert!(row.0 >= 4 * width.0, "row {row:?} width {width:?}");
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    {
        assert_eq!(row, (872, 8));
        assert_eq!(width, (24, 8));
    }
}

// Literal rows: producer facts.

/// F1 (M1, M4 witness): keywords are 1/3/5 and `0` is one counted conversion.
#[test]
fn pbr_f01_keywords_and_zero() {
    let css = "#p { border-style:solid; border-width:thin medium thick 0; }";
    let run = facts(5, css);
    assert_pixels(
        run.rows[1].border_width,
        [1.0, 3.0, 5.0, 0.0],
        BoxFactOrigin::Specified,
        css,
    );
    assert_eq!(run.rows[1].border_widths(), [1.0, 3.0, 5.0, 0.0], "{css}");
    assert_eq!(run.receipt.evaluation.numeric_calls, 1, "{css}");
}

/// F2 (M18 witness): absolute lengths round to the nearest f32, ties to even.
#[test]
fn pbr_f02_absolute_lengths_round_to_nearest_even() {
    let css = "#p { border-style:solid; border-width:0.5px 1in 0.1px 16777217px; }";
    let run = facts(5, css);
    for (fact, bits) in run.rows[1].border_width.into_iter().zip([
        0x3f00_0000,
        0x42c0_0000,
        0x3dcc_cccd,
        0x4b80_0000,
    ]) {
        assert_width(fact, (bits, None, BoxFactOrigin::Specified), css);
    }
    assert_eq!(run.receipt.evaluation.numeric_calls, 4, "{css}");
}

/// F3 (M17 witness): a length above f32::MAX saturates with `UpperSaturation`.
#[test]
fn pbr_f03_upper_saturation() {
    let css = format!("#p {{ border-style:solid; border-top-width:{F3_PX}; }}");
    let run = facts(5, &css);
    assert_width(
        run.rows[1].border_width[0],
        (
            MAX_BITS,
            Some(RangeEvent::UpperSaturation),
            BoxFactOrigin::Specified,
        ),
        &css,
    );
    for &fact in &run.rows[1].border_width[1..] {
        assert_width(
            fact,
            (3.0_f32.to_bits(), None, BoxFactOrigin::Initial),
            &css,
        );
    }
    assert_eq!(run.receipt.evaluation.numeric_calls, 1, "{css}");
}

/// F4: a positive length that rounds to +0 records `PositiveRoundedToZero`.
#[test]
fn pbr_f04_positive_rounded_to_zero() {
    let css = format!("#p {{ border-style:solid; border-top-width:{F4_PX}; }}");
    let run = facts(5, &css);
    assert_width(
        run.rows[1].border_width[0],
        (
            0,
            Some(RangeEvent::PositiveRoundedToZero),
            BoxFactOrigin::Specified,
        ),
        &css,
    );
    assert_eq!(run.receipt.evaluation.numeric_calls, 1, "{css}");
}

/// F5: exactly f32::MAX has no event.
#[test]
fn pbr_f05_exact_maximum_has_no_event() {
    let css = format!("#p {{ border-style:solid; border-top-width:{F5_PX}; }}");
    let run = facts(5, &css);
    assert_width(
        run.rows[1].border_width[0],
        (MAX_BITS, None, BoxFactOrigin::Specified),
        &css,
    );
    assert_eq!(run.receipt.evaluation.numeric_calls, 1, "{css}");
}

/// F6: a suppressed side is +0 before any `RelativeLength` test or conversion.
#[test]
fn pbr_f06_suppressed_sides_skip_relative_and_conversion() {
    for style in ["none", "hidden"] {
        let css = format!("#p {{ border-style:{style}; border-width:5em 1ex 7px thick; }}");
        let run = facts(5, &css);
        assert_sides(run.rows[1].border_width, SPECIFIED_ZERO, &css);
        assert_eq!(run.receipt.evaluation.numeric_calls, 0, "{css}");
    }
}

/// F7: a visible style with no width declaration is the initial `medium`.
#[test]
fn pbr_f07_initial_medium_on_visible_sides() {
    let css = "#p { border-style:solid; }";
    let run = facts(5, css);
    assert_sides(
        run.rows[1].border_width,
        (3.0_f32.to_bits(), None, BoxFactOrigin::Initial),
        css,
    );
}

/// F8: a relative width on a visible side is `RelativeLength`.
#[test]
fn pbr_f08_relative_width_on_a_visible_side() {
    let css = "#p { border-style:solid; border-left-width:1em; }";
    let error = match facts_error(5, css) {
        SiblingBoxFactsError::Evaluation(error) => error,
        other => panic!("{css}: {other:?}"),
    };
    assert_unsupported(
        &error,
        (
            3,
            "border-left-width",
            BoxFactTarget::Element,
            BoxFactUnsupported::RelativeLength,
        ),
        css,
    );
}

/// F9 (M10 witness): `inherit` on a visible side copies the parent's computed fact.
#[test]
fn pbr_f09_inherit_copies_the_parent_width() {
    let css = "#p { border-style:solid; border-width:4px; } \
               #b1 { border-style:solid; border-width:inherit; }";
    let run = facts(5, css);
    assert_sides(
        run.rows[2].border_width,
        (4.0_f32.to_bits(), None, inherited(3)),
        css,
    );
}

/// F10 (M9 witness): the own `hidden` style forces +0 before `inherit`.
#[test]
fn pbr_f10_own_suppression_precedes_inherit() {
    let css = "#p { border-style:solid; border-width:4px; } \
               #b1 { border-style:hidden; border-width:inherit; }";
    let run = facts(5, css);
    assert_sides(run.rows[2].border_width, (0, None, inherited(3)), css);
}

/// F11: inheriting from a style-forced zero gives the parent's computed zero.
#[test]
fn pbr_f11_inherit_from_a_suppressed_parent() {
    let css = "#o { border-width:9px; border-style:none; } \
               #p { border-style:solid; border-width:inherit; }";
    let run = facts(5, css);
    assert_sides(run.rows[0].border_width, SPECIFIED_ZERO, css);
    assert_sides(run.rows[1].border_width, (0, None, inherited(4)), css);
}

/// F12 (DR-2): an inherited saturated width keeps the parent's event.
#[test]
fn pbr_f12_inherited_saturation_keeps_its_event() {
    let css = format!(
        "#p {{ border-top-style:solid; border-top-width:{F3_PX}; }} \
         #b1 {{ border-top-style:solid; border-top-width:inherit; }}"
    );
    let run = facts(5, &css);
    assert_width(
        run.rows[2].border_width[0],
        (MAX_BITS, Some(RangeEvent::UpperSaturation), inherited(3)),
        &css,
    );
}

/// F13 (DR-2): an inherited positive-to-zero width keeps the parent's event.
#[test]
fn pbr_f13_inherited_rounding_to_zero_keeps_its_event() {
    let css = format!(
        "#p {{ border-top-style:solid; border-top-width:{F4_PX}; }} \
         #b1 {{ border-top-style:solid; border-top-width:inherit; }}"
    );
    let run = facts(5, &css);
    assert_width(
        run.rows[2].border_width[0],
        (0, Some(RangeEvent::PositiveRoundedToZero), inherited(3)),
        &css,
    );
}

/// F14 (DR-4): `inherit` on the root O defaults to the initial `medium`.
#[test]
fn pbr_f14_root_inherit_is_initial_medium() {
    let css = "#o { border-style:solid; border-width:inherit; }";
    let run = facts(5, css);
    assert_sides(
        run.rows[0].border_width,
        (3.0_f32.to_bits(), None, BoxFactOrigin::Initial),
        css,
    );
}

/// F15 (DR-4): `-0px` on a visible side is one conversion giving +0.
#[test]
fn pbr_f15_negative_zero_is_positive_zero() {
    let css = "#p { border-style:solid; border-top-width:-0px; }";
    let run = facts(5, css);
    assert_width(
        run.rows[1].border_width[0],
        (0, None, BoxFactOrigin::Specified),
        css,
    );
    assert_eq!(run.receipt.evaluation.numeric_calls, 1, "{css}");
}

/// F16 (M16 witness): a hidden child with a relative width is admitted.
#[test]
fn pbr_f16_hidden_relative_child_width_is_admitted() {
    let css = "#b1 { border-style:hidden; border-width:5ex; }";
    let run = facts(5, css);
    assert_sides(run.rows[2].border_width, SPECIFIED_ZERO, css);
    assert_eq!(run.receipt.evaluation.numeric_calls, 0, "{css}");
}

/// F17: the Sequence twin of the Legacy zero-border proof.
#[test]
fn pbr_f17_sequence_twin_of_the_legacy_zero_border_proof() {
    let css = "#p {border-width:3em; border-style:hidden;} \
               #b1 {border-style:inherit; border-width:5ex;} #f {border:none 2em;} \
               #b2 {border-style:inherit;}";
    let run = facts(5, css);
    assert_eq!(run.receipt.evaluation.numeric_calls, 0, "{css}");
    for row in &run.rows[1..4] {
        assert_sides(row.border_width, SPECIFIED_ZERO, css);
    }
    assert_sides(
        run.rows[4].border_width,
        (0, None, BoxFactOrigin::Initial),
        css,
    );
    for row in [2, 4] {
        for style in run.rows[row].border_style {
            assert_eq!(style.value, BorderStyle::Hidden, "{css}: row {row}");
        }
    }
}

/// F18: O's widths are captured.
#[test]
fn pbr_f18_outer_widths_are_captured() {
    let css = "#o { border:7px solid; }";
    let run = facts(5, css);
    assert_sides(
        run.rows[0].border_width,
        (7.0_f32.to_bits(), None, BoxFactOrigin::Specified),
        css,
    );
}

/// F19: the producer's computed rows equal the core `compute_border_width` oracle.
#[test]
fn pbr_f19_compute_rows_match_the_core_oracle() {
    let solid = specified("border-top-style:solid");
    let context = BorderFontContext {
        size: 16.0,
        x_height: 8.0,
    };
    let inputs = [
        "thin",
        "medium",
        "thick",
        "0",
        "0.5px",
        "1in",
        "16777217px",
        F3_PX,
        F4_PX,
        F5_PX,
    ];
    assert_eq!(inputs.len(), 10);
    for value in inputs {
        let declaration = format!("border-top-width:{value}");
        let oracle =
            compute_border_width(&specified(&declaration), &solid, context, values().numeric)
                .unwrap();
        let css = format!("#p {{ border-top-style:solid; {declaration}; }}");
        let run = facts(5, &css);
        assert_width(
            run.rows[1].border_width[0],
            (
                oracle.value.to_bits(),
                oracle.event,
                BoxFactOrigin::Specified,
            ),
            &css,
        );
    }
}

/// F20: four conversions per bordered row.
#[test]
fn pbr_f20_four_conversions_per_bordered_row() {
    let css = "#p { border-style:solid; border-width:2px; } \
               #b1 { border-style:solid; border-width:3px; }";
    let run = facts(5, css);
    assert_eq!(run.receipt.evaluation.numeric_calls, 8, "{css}");
    assert_sides(
        run.rows[1].border_width,
        (2.0_f32.to_bits(), None, BoxFactOrigin::Specified),
        css,
    );
    assert_sides(
        run.rows[2].border_width,
        (3.0_f32.to_bits(), None, BoxFactOrigin::Specified),
        css,
    );
}

/// F21: the Legacy entry still rejects a visible P style.
#[test]
fn pbr_f21_legacy_entry_keeps_border_style() {
    let css = "#p{border:2px solid}";
    let error = legacy(css).err().unwrap();
    assert_unsupported(
        &error,
        (
            3,
            "border-top-style",
            BoxFactTarget::Element,
            BoxFactUnsupported::BorderStyle,
        ),
        css,
    );
}

/// F22 (DR-1, Sequence counterpart of LF2): own `hidden` under `inherit` is +0 with
/// true provenance.
#[test]
fn pbr_f22_sequence_inherit_under_hidden_is_inherited_zero() {
    let css = "#o { border-width:2px; border-style:solid; } \
               #p { border-width:inherit; border-style:hidden; }";
    let run = facts(5, css);
    assert_sides(
        run.rows[0].border_width,
        (2.0_f32.to_bits(), None, BoxFactOrigin::Specified),
        css,
    );
    assert_sides(run.rows[1].border_width, (0, None, inherited(4)), css);
}

// Legacy fence rows.

/// LF1: a Legacy width under `none` is never computed (and never converted).
#[test]
fn pbr_lf1_legacy_width_is_never_computed() {
    let css = "#b1{border-width:1em}";
    let (rows, receipt, _) = legacy(css).unwrap();
    assert_eq!(rows[1].border_width, [ZERO_WIDTH; 4], "{css}");
    assert_sides(rows[1].border_width, (0, None, BoxFactOrigin::Initial), css);
    assert_eq!(receipt.numeric_calls, 0, "{css}");
    let (with, with_receipt, _) = legacy("#b1{height:1px; border-width:1em}").unwrap();
    let (without, without_receipt, _) = legacy("#b1{height:1px}").unwrap();
    assert_eq!(with_receipt.numeric_calls, without_receipt.numeric_calls);
    assert_eq!(with_receipt.numeric_calls, 1);
    assert_eq!(with, without);
}

/// LF2 (M19 witness in the nested form): Legacy width facts are (+0, `None`,
/// `Initial`) even under `inherit`; that origin is not provenance.
#[test]
fn pbr_lf2_legacy_inherit_under_hidden_is_initial() {
    let css = "#p{border-width:inherit; border-style:hidden}";
    let (rows, _, _) = legacy(css).unwrap();
    assert_eq!(rows[0].border_width, [ZERO_WIDTH; 4], "{css}");
    assert_eq!(rows[0].border_style[0].value, BorderStyle::Hidden, "{css}");
    let nested = "#o { border-width:2px; } #p { border-width:inherit; border-style:hidden; }";
    let rows = legacy_nested(nested).unwrap();
    for row in [1, 0] {
        assert_eq!(
            rows[row].border_width, [ZERO_WIDTH; 4],
            "{nested}: row {row}"
        );
        assert_sides(
            rows[row].border_width,
            (0, None, BoxFactOrigin::Initial),
            nested,
        );
    }
}

/// LF3: the Legacy entry rejects `0 solid` (as `tests/clearance_box_facts.rs`).
#[test]
fn pbr_lf3_legacy_zero_solid_is_border_style() {
    let css = "#b1{border:0 solid}";
    let error = legacy(css).err().unwrap();
    assert_unsupported(
        &error,
        (
            0,
            "border-top-style",
            BoxFactTarget::Element,
            BoxFactUnsupported::BorderStyle,
        ),
        css,
    );
}

/// LF4: the Legacy default CSS keeps zero widths and the Legacy receipt.
#[test]
fn pbr_lf4_legacy_default_css_keeps_widths_and_receipt() {
    let (rows, receipt, rules) = legacy(CSS).unwrap();
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.border_width, [ZERO_WIDTH; 4], "row {index}");
        assert_sides(row.border_width, (0, None, BoxFactOrigin::Initial), CSS);
        assert_eq!(row.border_widths(), [0.0; 4]);
    }
    assert_eq!(receipt.work, 110 + rules);
    assert_eq!(receipt.numeric_work, 56 * 100_000);
}

// New pin N3, fact part.

/// N3: a zero-width styled child's row equals the `border-style:none` control except
/// its styles and its width origin (`Specified` against `Initial`).
#[test]
fn pbr_n3_zero_width_styled_child_facts() {
    let mut compared = 0;
    for (nodes, source, child) in [(5, 2, "b1"), (5, 3, "f"), (5, 4, "b2"), (6, 5, "b3")] {
        let control_css = format!("{} #{child} {{ border-style:none; }}", baseline_css());
        let control = facts(nodes, &control_css);
        assert_eq!(control.rows[source].border_width, [ZERO_WIDTH; 4]);
        for (declarations, style) in [
            ("border:0 solid;", BorderStyle::Solid),
            ("border:3px hidden;", BorderStyle::Hidden),
            ("border-style:dotted; border-width:0;", BorderStyle::Dotted),
        ] {
            let css = format!("{} #{child} {{ {declarations} }}", baseline_css());
            let run = facts(nodes, &css);
            for (index, (row, expected)) in run.rows.iter().zip(&control.rows).enumerate() {
                if index == source {
                    assert_sides(row.border_width, SPECIFIED_ZERO, &css);
                    for fact in row.border_style {
                        assert_eq!(
                            fact,
                            KeywordBoxFact {
                                value: style,
                                origin: BoxFactOrigin::Specified,
                            },
                            "{css}"
                        );
                    }
                    assert_eq!(
                        ClearanceBoxFact {
                            border_style: expected.border_style,
                            border_width: expected.border_width,
                            ..*row
                        },
                        *expected,
                        "{css}"
                    );
                } else {
                    assert_eq!(row, expected, "{css}: source {index}");
                }
            }
            compared += 1;
        }
    }
    assert_eq!(compared, 12);
}

// Work rows (DR-7).

/// The facade fixture CSS for the work rows.
fn work_css(nodes: usize) -> String {
    let leading = if nodes == 7 {
        leading_clear_source_rule(7)
    } else {
        ""
    };
    format!("{} {leading} #p {{ border-style:solid; }}", baseline_css())
}

/// W1 (M15a witness): Sequence work is `35 * n + 2 + r`, exact and one short.
#[test]
fn pbr_w1_sequence_work_is_exact() {
    for nodes in [5, 6, 7] {
        let css = work_css(nodes);
        let run = facts(nodes, &css);
        let work = 35 * nodes + 2 + run.rules;
        assert_eq!(run.receipt.evaluation.work, work, "N{nodes}");
        let mut exact = sibling_fact_limits();
        exact.evaluation.work = nz(work);
        assert!(fact_run_with(nodes, &css, exact).is_ok(), "N{nodes} exact");
        let mut short = exact;
        short.evaluation.work = nz(work - 1);
        assert!(
            matches!(
                fact_run_with(nodes, &css, short),
                Err(SiblingBoxFactsError::Evaluation(
                    ClearanceBoxFactsError::Work
                ))
            ),
            "N{nodes} one short"
        );
    }
}

/// W2 (M15b witness, DR-7): the numeric allowance is `22 * n * numeric.work`, exact
/// and one short.
#[test]
fn pbr_w2_sequence_numeric_allowance_is_exact() {
    for nodes in [5, 6, 7] {
        let css = work_css(nodes);
        let run = facts(nodes, &css);
        let allowance = 22 * nodes * 100_000;
        assert_eq!(run.receipt.evaluation.numeric_work, allowance, "N{nodes}");
        let mut exact = sibling_fact_limits();
        exact.evaluation.numeric_work = nz(allowance);
        assert!(fact_run_with(nodes, &css, exact).is_ok(), "N{nodes} exact");
        let mut short = exact;
        short.evaluation.numeric_work = nz(allowance - 1);
        assert!(
            matches!(
                fact_run_with(nodes, &css, short),
                Err(SiblingBoxFactsError::Evaluation(
                    ClearanceBoxFactsError::NumericWork
                ))
            ),
            "N{nodes} one short"
        );
    }
}

/// W3: at N7 the exact-limit facts build at the exact facade and core work (OCH
/// values 4652 and 2002).
#[test]
fn pbr_w3_exact_facts_build_at_the_exact_facade_and_core_work() {
    let css = work_css(7);
    sibling_fixture(7, &css, |projection, display| {
        let rules = display.bundle().rule_count();
        let mut exact = sibling_fact_limits();
        exact.evaluation.work = nz(35 * 7 + 2 + rules);
        exact.evaluation.numeric_work = nz(22 * 7 * 100_000);
        let facts = evaluate_sibling_box_facts(projection, display, exact).unwrap();
        assert_eq!(facts.row(1).unwrap().border_widths(), [3.0; 4]);
        let mut limits = layout_limits();
        limits.work = nz(4652);
        limits.core.work = nz(2002);
        let layout =
            default_outer_layout(&facts, &projection.nodes()[0], 0.0, 100.0, 10.0, limits).unwrap();
        assert_eq!(layout.receipt().work, 4652);
        assert_eq!(layout.receipt().core.work, 2002);
        assert_eq!(layout.parent().origin().content_top, 14.0);
    });
}
