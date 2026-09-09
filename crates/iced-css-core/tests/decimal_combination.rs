use iced_css_core::values::{Decimal, DecimalArithmeticError, NumericError, NumericLimits};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn d(s: &str) -> Decimal {
    Decimal::parse(s, limits()).unwrap()
}
#[test]
fn decimal_alignment_addition_and_interpolation_are_exact() {
    for (a, b, sum, half) in [
        ("0.1", "0.2", "0.3", "0.15"),
        ("-1000.01", "1000.01", "0", "0"),
        ("1.25", "-0.75", "0.5", "0.25"),
        ("-0", "0", "0", "0"),
    ] {
        let a = d(a);
        let b = d(b);
        assert_eq!(a.checked_add(&b, limits()).unwrap(), d(sum));
        assert_eq!(a.checked_interpolate(&b, 0.5, limits()).unwrap(), d(half));
    }
    let zero = d("-0").checked_add(&d("-0"), limits()).unwrap();
    assert!(!zero.authored_negative());
    assert_eq!(
        d("1").checked_interpolate(&d("2"), -2.0, limits()).unwrap(),
        d("-1")
    );
}
#[test]
fn wide_coefficients_cancel_without_float_conversion() {
    let source = "9".repeat(120);
    let a = d(&source);
    assert_eq!(
        a.checked_add(&d("1"), limits()).unwrap(),
        d(&format!("1{}", "0".repeat(120)))
    );
    for p in [0.0, 1.0, f32::MAX, -f32::MAX, f32::from_bits(1)] {
        assert_eq!(a.checked_interpolate(&a, p, limits()).unwrap(), a);
    }
    assert_eq!(a, d(&source));
}
#[test]
fn decimal_arithmetic_limits_and_nonfinite_progress_leave_inputs_unchanged() {
    let a = d("99999");
    let b = d("1");
    let mut small = limits();
    small.digits = NonZeroUsize::new(5).unwrap();
    assert_eq!(
        a.checked_add(&b, small).unwrap_err(),
        NumericError::DigitLimit
    );
    assert_eq!(
        d("0.000001").checked_add(&b, small).unwrap_err(),
        NumericError::DigitLimit
    );
    let mut small = limits();
    small.work = NonZeroUsize::MIN;
    assert_eq!(
        a.checked_add(&b, small).unwrap_err(),
        NumericError::WorkLimit
    );
    assert_eq!(
        a.checked_interpolate(&b, 0.5, small).unwrap_err(),
        DecimalArithmeticError::Numeric(NumericError::WorkLimit)
    );
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            a.checked_interpolate(&b, p, limits()).unwrap_err(),
            DecimalArithmeticError::NonfiniteProgress
        );
    }
    assert_eq!(a, d("99999"));
    assert_eq!(b, d("1"));
}

#[test]
fn integral_progress_does_not_introduce_a_spurious_fractional_scale() {
    let a = d("0.0001");
    let b = d("0.0002");
    let mut small = limits();
    small.digits = NonZeroUsize::new(5).unwrap();
    assert_eq!(a.checked_interpolate(&b, 1.0, small).unwrap(), b);
    assert_eq!(a.checked_interpolate(&b, 2.0, small).unwrap(), d("0.0003"));
}
