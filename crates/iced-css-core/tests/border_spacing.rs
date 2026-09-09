use iced_css_core::{
    animation::{
        combine::Combination,
        number::{Clamp, NumberError},
        spacing::*,
    },
    syntax::*,
    values::{
        NumericLimits,
        border_spacing::*,
        color::ColorLimits,
        geometry::{LengthBasis, LengthContext, LengthEvaluationError},
        property::*,
    },
};
use std::num::NonZeroUsize;
fn numeric() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn parsed(input: &str) -> ParsedProperty {
    let n = |n| NonZeroUsize::new(n).unwrap();
    let DeclarationItem::Declaration(d) = Declarations::new(
        input,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: n(100000),
                nesting: n(20),
                retained_bytes: n(4096),
            },
            name_bytes: n(4096),
        },
    )
    .next()
    .unwrap()
    .unwrap() else {
        panic!()
    };
    let PropertyResult::Parsed(value) = parse_property(
        &d,
        ColorLimits {
            syntax: StringLimits {
                work: n(100000),
                retained_bytes: n(4096),
            },
            numeric: numeric(),
        },
    )
    .unwrap() else {
        panic!()
    };
    value
}

#[test]
fn parsed_spacing_computes_horizontal_then_vertical_absolute_lengths() {
    let context = LengthContext {
        font_size: Some(16.0),
        x_height: Some(7.0),
        percentage_basis: None,
    };
    for (css, expected) in [
        ("border-spacing:2px", [2.0, 2.0]),
        ("border-spacing:1em 2ex", [16.0, 14.0]),
        ("border-spacing:1in 2.54cm", [96.0, 96.0]),
        ("border-spacing:-0", [0.0, 0.0]),
    ] {
        let computed = compute_border_spacing(&parsed(css), context, numeric()).unwrap();
        assert_eq!(
            [computed.value.horizontal(), computed.value.vertical()],
            expected
        );
        assert_eq!(computed.rounded_to_zero, [false, false]);
    }
    let e = compute_border_spacing(
        &parsed("border-spacing:1em 2px"),
        LengthContext::default(),
        numeric(),
    )
    .unwrap_err();
    assert_eq!(
        e,
        BorderSpacingError::Length(LengthEvaluationError::MissingContext(LengthBasis::FontSize))
    );
    for css in ["border-spacing:inherit", "background-position:0 0"] {
        assert_eq!(
            compute_border_spacing(&parsed(css), context, numeric()).unwrap_err(),
            BorderSpacingError::Unresolved
        );
    }
}
#[test]
fn spacing_pairs_require_finite_nonnegative_values_and_preserve_underflow_report() {
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
        assert_eq!(
            BorderSpacing::from_pixels(invalid, 1.0).unwrap_err(),
            BorderSpacingError::InvalidComputed
        );
        assert_eq!(
            BorderSpacing::from_pixels(1.0, invalid).unwrap_err(),
            BorderSpacingError::InvalidComputed
        );
    }
    let zero = BorderSpacing::from_pixels(-0.0, -0.0).unwrap();
    assert_eq!(zero.horizontal().to_bits(), 0);
    assert_eq!(zero.vertical().to_bits(), 0);
    let css = format!("border-spacing:0.{}1px 0px", "0".repeat(60));
    let result =
        compute_border_spacing(&parsed(&css), LengthContext::default(), numeric()).unwrap();
    assert_eq!(result.value, zero);
    assert_eq!(result.rounded_to_zero, [true, false]);
    let mut tiny = numeric();
    tiny.work = NonZeroUsize::MIN;
    assert!(matches!(
        compute_border_spacing(
            &parsed("border-spacing:1.234in 2px"),
            LengthContext::default(),
            tiny
        ),
        Err(BorderSpacingError::Length(_))
    ));
}
#[test]
fn each_spacing_component_combines_before_final_range_handling() {
    let a = compute_border_spacing(
        &parsed("border-spacing:2px 20px"),
        LengthContext::default(),
        numeric(),
    )
    .unwrap()
    .value;
    let b = compute_border_spacing(
        &parsed("border-spacing:10px 4px"),
        LengthContext::default(),
        numeric(),
    )
    .unwrap()
    .value;
    for (op, expected, clamps) in [
        (Combination::Interpolate(0.5), [6.0, 12.0], [None, None]),
        (
            Combination::Interpolate(2.0),
            [18.0, 0.0],
            [None, Some(Clamp::Lower)],
        ),
        (
            Combination::Interpolate(-1.0),
            [0.0, 30.0],
            [Some(Clamp::Lower), Some(Clamp::Upper)],
        ),
        (Combination::Add, [12.0, 24.0], [None, None]),
        (Combination::Accumulate, [12.0, 24.0], [None, None]),
    ] {
        let result = combine_border_spacing(a, b, op)
            .unwrap()
            .resolve(30.0)
            .unwrap();
        assert_eq!(
            [result.value.horizontal(), result.value.vertical()],
            expected
        );
        assert_eq!(result.clamped, clamps);
    }
    for maximum in [-1.0, f32::INFINITY, f32::NAN] {
        assert_eq!(
            combine_border_spacing(a, b, Combination::Add)
                .unwrap()
                .resolve(maximum)
                .unwrap_err(),
            NumberError::InvalidRange
        );
    }
}

#[test]
fn typed_invalid_lengths_cannot_bypass_the_property_grammar() {
    use iced_css_core::values::LengthTarget;
    for raw in [
        "auto",
        "10%",
        "-0.0000000000000000000000000000000000000000000000000000000001px",
    ] {
        for reverse in [false, true] {
            let mut property = parsed("border-spacing:1px 2px");
            let bad = LengthTarget::parse(raw, numeric()).unwrap();
            let good = LengthTarget::parse("1px", numeric()).unwrap();
            property.value = if reverse {
                SpecifiedValue::LengthPair(good, bad)
            } else {
                SpecifiedValue::LengthPair(bad, good)
            };
            assert_eq!(
                compute_border_spacing(&property, LengthContext::default(), numeric()).unwrap_err(),
                BorderSpacingError::InvalidValue
            );
        }
    }
}
