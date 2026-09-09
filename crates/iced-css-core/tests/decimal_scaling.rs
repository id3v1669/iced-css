use iced_css_core::values::{Decimal, DecimalScalingError, NumericError, NumericLimits};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn decimal(s: &str) -> Decimal {
    Decimal::parse(s, limits()).unwrap()
}
#[test]
fn binary32_conversion_preserves_exact_identity_and_canonical_zero() {
    for (value, want) in [
        (0.0, "0"),
        (-0.0, "0"),
        (1.5, "1.5"),
        (-2.75, "-2.75"),
        (0.1, "0.100000001490116119384765625"),
        (f32::MAX, "340282346638528859811704183484516925440"),
    ] {
        assert_eq!(
            Decimal::from_finite_f32(value, limits()).unwrap(),
            decimal(want)
        );
    }
    for bits in [
        1, 2, 3, 0x007fffff, 0x00800000, 0x3f800001, 0x7f7fffff, 0x80000001, 0xff7fffff,
    ] {
        let value = f32::from_bits(bits);
        assert_eq!(
            Decimal::from_finite_f32(value, limits())
                .unwrap()
                .scaled_finite_f32(1.0, limits())
                .unwrap()
                .value
                .to_bits(),
            bits
        );
    }
}
#[test]
fn products_do_not_round_the_decimal_operand_or_the_binary_factor() {
    for (a, factor, want) in [
        ("10", 0.1, "1.00000001490116119384765625"),
        ("0.125", 8.0, "1"),
        ("-1.25", -2.0, "2.5"),
        ("123.456", -0.0, "0"),
    ] {
        let a = decimal(a);
        let before = a.clone();
        assert_eq!(
            a.checked_scale_f32(factor, limits()).unwrap(),
            decimal(want)
        );
        assert_eq!(a, before);
    }
    let huge = decimal(&format!("1{}", "0".repeat(40)));
    assert!(
        huge.checked_scale_f32(f32::from_bits(1), limits())
            .unwrap()
            .scaled_finite_f32(1.0, limits())
            .unwrap()
            .value
            > 0.000001
    );
    let mut tight = limits();
    tight.digits = NonZeroUsize::new(5).unwrap();
    assert_eq!(
        decimal("0.0001").checked_scale_f32(2.0, tight).unwrap(),
        decimal("0.0002")
    );
    tight.digits = NonZeroUsize::MIN;
    assert_eq!(Decimal::from_finite_f32(1.0, tight).unwrap(), decimal("1"));
}
#[test]
fn nonfinite_and_resource_errors_are_explicit_and_leave_inputs_intact() {
    let value = decimal("123.45");
    let before = value.clone();
    for factor in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            Decimal::from_finite_f32(factor, limits()).unwrap_err(),
            DecimalScalingError::NonfiniteValue
        );
        assert_eq!(
            value.checked_scale_f32(factor, limits()).unwrap_err(),
            DecimalScalingError::NonfiniteValue
        );
    }
    let mut tiny = limits();
    tiny.work = NonZeroUsize::MIN;
    assert_eq!(
        Decimal::from_finite_f32(1.0, tiny).unwrap_err(),
        DecimalScalingError::Numeric(NumericError::WorkLimit)
    );
    assert_eq!(
        value.checked_scale_f32(1.0, tiny).unwrap_err(),
        DecimalScalingError::Numeric(NumericError::WorkLimit)
    );
    tiny = limits();
    tiny.digits = NonZeroUsize::MIN;
    assert_eq!(
        Decimal::from_finite_f32(f32::from_bits(1), tiny).unwrap_err(),
        DecimalScalingError::Numeric(NumericError::DigitLimit)
    );
    assert_eq!(
        value.checked_scale_f32(0.0, tiny).unwrap_err(),
        DecimalScalingError::Numeric(NumericError::DigitLimit)
    );
    assert_eq!(value, before);
}
