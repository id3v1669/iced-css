use iced_css_core::values::{ConversionError, Decimal, LengthTarget, NumericLimits};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn length(css: &str) -> iced_css_core::values::FiniteF32 {
    let LengthTarget::Absolute(value) = LengthTarget::parse(css, limits()).unwrap() else {
        panic!()
    };
    value.to_finite_f32(limits()).unwrap()
}

#[test]
fn signed_absolute_ratios_and_nearest_even_rounding_preserve_geometry() {
    for css in ["-1in", "-2.54cm", "-25.4mm", "-72pt", "-6pc", "-96px"] {
        assert_eq!(length(css).value, -96.0);
    }
    assert_eq!(length("-16777217px").value, -16777216.0);
    assert_eq!(length("-16777219px").value, -16777220.0);
    assert_eq!(length("-0px").value.to_bits(), 0);
    assert!(!length("-0px").rounded_to_zero);
    assert_eq!(
        length("-0.0000000000000000000000000000000000000000000001px")
            .value
            .to_bits(),
        0
    );
    assert!(length("-0.0000000000000000000000000000000000000000000001px").rounded_to_zero);
    assert_eq!(
        length("-0.000000000000000000000000000000000000000000001px")
            .value
            .to_bits(),
        0x80000001
    );
}

#[test]
fn signed_context_products_are_resolved_before_range_checks() {
    let coefficient =
        Decimal::parse("-10000000000000000000000000000000000000000", limits()).unwrap();
    assert!(
        coefficient
            .scaled_finite_f32(0.001, limits())
            .unwrap()
            .value
            .is_finite()
    );
    assert_eq!(
        Decimal::parse("-50", limits())
            .unwrap()
            .percentage_finite_f32(-20.0, limits())
            .unwrap()
            .value,
        10.0
    );
    for (coefficient, context, expected) in [
        ("2", -3.0, -6.0),
        ("-2", 3.0, -6.0),
        ("-2", -3.0, 6.0),
        ("-0", -3.0, 0.0),
        ("2", -0.0, 0.0),
    ] {
        assert_eq!(
            Decimal::parse(coefficient, limits())
                .unwrap()
                .scaled_finite_f32(context, limits())
                .unwrap()
                .value
                .to_bits(),
            f32::to_bits(expected)
        );
    }
}

#[test]
fn out_of_range_and_invalid_contexts_are_errors_without_font_saturation() {
    const MAX: &str = "340282346638528859811704183484516925440";
    for sign in ["", "-"] {
        assert_eq!(length(&format!("{sign}{MAX}px")).value.abs(), f32::MAX);
        let LengthTarget::Absolute(value) = LengthTarget::parse(
            &format!("{sign}340282346638528859811704183484516925441px"),
            limits(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(
            value.to_finite_f32(limits()),
            Err(ConversionError::Unrepresentable)
        );
    }
    let one = Decimal::parse("1", limits()).unwrap();
    for context in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            one.scaled_finite_f32(context, limits()),
            Err(ConversionError::InvalidFiniteContext)
        );
    }
    let negative = Decimal::parse("-1", limits()).unwrap();
    assert_eq!(
        negative.scaled_nonnegative_f32(1.0, limits()),
        Err(ConversionError::InvalidNegative)
    );
}
