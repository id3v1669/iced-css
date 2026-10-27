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

// PCT: the bounded `Copy` decimal and its resolution (CP1-CP7).
use iced_css_core::values::CompactDecimal;

fn decimal(text: &str) -> Decimal {
    Decimal::parse(text, limits()).unwrap()
}

fn compact(text: &str) -> CompactDecimal {
    decimal(text).compact().unwrap()
}

/// `text`% of `basis` through both resolvers, which must agree bit for bit.
fn percentage_of(text: &str, basis: f32) -> Result<u32, ConversionError> {
    let exact = decimal(text).percentage_finite_f32(basis, limits());
    let bounded = compact(text).percentage_finite_f32(basis, limits());
    let bits = |result: Result<iced_css_core::values::FiniteF32, ConversionError>| {
        result.map(|value| (value.value.to_bits(), value.rounded_to_zero))
    };
    assert_eq!(bits(bounded), bits(exact), "{text}% of {basis}");
    bounded.map(|value| value.value.to_bits())
}

/// CP1: 50% of 100 is 50; a zero basis or a zero percentage is +0; a non-finite basis
/// is `InvalidFiniteContext`, on both paths.
#[test]
fn pct_cp1_shared_path_agrees_on_plain_values() {
    assert_eq!(percentage_of("50", 100.0), Ok(50.0_f32.to_bits()));
    assert_eq!(percentage_of("50", 0.0), Ok(0));
    assert_eq!(percentage_of("-0", 100.0), Ok(0));
    assert!(!compact("-0").is_negative() && compact("-0").is_zero());
    assert_eq!(
        percentage_of("50", f32::NAN),
        Err(ConversionError::InvalidFiniteContext)
    );
    assert_eq!(
        percentage_of("50", f32::INFINITY),
        Err(ConversionError::InvalidFiniteContext)
    );
}

/// CP2 (M23 witness): 0.007% of 100 is one nearest-even rounding of 0.007 (`3be56042`); the scale
/// is kept.
#[test]
fn pct_cp2_scale_is_kept() {
    assert_eq!(percentage_of("0.007", 100.0), Ok(0x3be56042));
    assert_eq!(percentage_of("0.049", 100.0), Ok(0x3d48b439));
}

/// CP3: 1.001% of 7 is one rounding of 0.07007 (`3d8f80dc`).
#[test]
fn pct_cp3_one_rounding_of_the_exact_product() {
    assert_eq!(percentage_of("1.001", 7.0), Ok(0x3d8f80dc));
}

/// CP4 (M22 witness): the sign is the percentage's sign times the basis's: -10% of 100 is -10 and of
/// -100 is 10.
#[test]
fn pct_cp4_sign_of_the_product() {
    assert!(compact("-10").is_negative());
    assert_eq!(percentage_of("-10", 100.0), Ok((-10.0_f32).to_bits()));
    assert_eq!(percentage_of("-10", -100.0), Ok(10.0_f32.to_bits()));
}

/// CP5: 200% of `f32::MAX` is `Unrepresentable` on both paths.
#[test]
fn pct_cp5_unrepresentable_product() {
    assert_eq!(
        percentage_of("200", f32::MAX),
        Err(ConversionError::Unrepresentable)
    );
}

/// CP6: the largest stored coefficient is `u64::MAX`; 18446744073709551615% of 100
/// rounds once to 2^64.
#[test]
fn pct_cp6_largest_coefficient_is_stored() {
    assert!(decimal("18446744073709551615").compact().is_some());
    assert_eq!(
        percentage_of("18446744073709551615", 100.0),
        Ok(2.0_f32.powi(64).to_bits())
    );
}

/// CP7: one more than `u64::MAX` (canonical coefficient) is not stored; trailing zeros
/// of a fraction are not part of the coefficient.
#[test]
fn pct_cp7_coefficient_above_u64_is_refused() {
    assert_eq!(decimal("18446744073709551616").compact(), None);
    assert_eq!(
        compact("1844674407370955161.50"),
        compact("1844674407370955161.5")
    );
}
