use iced_css_core::{
    animation::{combine::Combination, integer::combine_integer},
    values::{Integer, IntegerInterpolationError, NumericError, NumericLimits},
};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn i(s: &str) -> Integer {
    Integer::parse(s, limits()).unwrap()
}
#[test]
fn exact_integer_halfway_rounding_is_toward_positive_infinity() {
    for (a, b, expected) in [
        ("-2", "-1", "-1"),
        ("-1", "0", "0"),
        ("0", "1", "1"),
        ("1", "2", "2"),
    ] {
        for (a, b) in [(a, b), (b, a)] {
            assert_eq!(
                combine_integer(&i(a), &i(b), Combination::Interpolate(0.5), limits()).unwrap(),
                i(expected)
            );
        }
    }
    let before = f32::from_bits(0.1f32.to_bits() - 1);
    assert_eq!(
        i("0").checked_interpolate(&i("-5"), 0.1, limits()).unwrap(),
        i("-1")
    );
    assert_eq!(
        i("0")
            .checked_interpolate(&i("-5"), before, limits())
            .unwrap(),
        i("0")
    );
}
#[test]
fn wide_integer_endpoints_never_pass_through_float_storage() {
    let a = i(&"9".repeat(120));
    let b = i(&format!("1{}", "0".repeat(120)));
    assert_eq!(a.checked_interpolate(&b, 0.5, limits()).unwrap(), b);
    for p in [f32::MAX, -f32::MAX, f32::from_bits(1)] {
        assert_eq!(a.checked_interpolate(&a, p, limits()).unwrap(), a);
    }
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(combine_integer(&a, &i("1"), op, limits()).unwrap(), b);
    }
}
#[test]
fn integer_animation_rejects_nonfinite_progress_and_resource_exhaustion() {
    let a = i("99999");
    let b = i("1");
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_integer(&a, &b, Combination::Interpolate(p), limits()).unwrap_err(),
            IntegerInterpolationError::NonfiniteProgress
        );
    }
    let mut tiny = limits();
    tiny.work = NonZeroUsize::MIN;
    assert_eq!(
        a.checked_interpolate(&b, 0.5, tiny).unwrap_err(),
        IntegerInterpolationError::Numeric(NumericError::WorkLimit)
    );
    let mut tiny = limits();
    tiny.digits = NonZeroUsize::new(5).unwrap();
    assert_eq!(
        combine_integer(&a, &b, Combination::Add, tiny).unwrap_err(),
        IntegerInterpolationError::Numeric(NumericError::DigitLimit)
    );
    assert_eq!(a, i("99999"));
    assert_eq!(b, i("1"));
}
