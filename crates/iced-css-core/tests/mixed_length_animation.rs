use iced_css_core::{
    animation::{combine::Combination, length_percentage::*, number::Clamp},
    values::{Decimal, DecimalScalingError, NumericError, NumericLimits},
};
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
fn pixels(v: f32) -> LengthPercentage {
    LengthPercentage::from_pixels(v, limits()).unwrap()
}
fn percent(s: &str) -> LengthPercentage {
    LengthPercentage::from_percentage(d(s))
}
#[test]
fn mixed_components_bind_only_when_the_caller_supplies_the_current_basis() {
    let a = pixels(10.0);
    let b = percent("100");
    let sample =
        combine_length_percentage(&a, &b, Combination::Interpolate(0.5), limits()).unwrap();
    assert_eq!(sample.components(), (&d("5"), &d("50")));
    for (basis, want) in [(20.0, 15.0), (100.0, 55.0), (-20.0, -5.0)] {
        let actual = sample.resolve(basis, -100.0, 100.0, limits()).unwrap();
        assert_eq!(actual.value, want);
        assert_eq!(actual.clamped, None);
        assert!(!actual.rounded_to_zero);
    }
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(
            combine_length_percentage(&a, &b, op, limits())
                .unwrap()
                .resolve(20.0, -100.0, 100.0, limits())
                .unwrap()
                .value,
            30.0
        );
    }
    let sample =
        combine_length_percentage(&a, &b, Combination::Interpolate(2.0), limits()).unwrap();
    assert_eq!(sample.components(), (&d("-10"), &d("200")));
    assert_eq!(
        sample.resolve(20.0, -100.0, 100.0, limits()).unwrap().value,
        30.0
    );
    assert_eq!(
        sample.resolve(20.0, 0.0, 25.0, limits()).unwrap().clamped,
        Some(Clamp::Upper)
    );
}
#[test]
fn exact_components_cancel_before_any_finite_range_enforcement() {
    let a = pixels(f32::MAX);
    let b = percent("-100");
    let first = combine_length_percentage(&a, &b, Combination::Add, limits()).unwrap();
    let doubled = combine_length_percentage(&first, &first, Combination::Add, limits()).unwrap();
    assert!(
        doubled
            .components()
            .0
            .checked_cmp(
                &Decimal::from_finite_f32(f32::MAX, limits()).unwrap(),
                limits()
            )
            .unwrap()
            .is_gt()
    );
    let sample = doubled.resolve(f32::MAX, 0.0, 1.0, limits()).unwrap();
    assert_eq!(sample.value, 0.0);
    assert_eq!(sample.clamped, None);
    assert!(!sample.rounded_to_zero);
    let huge = percent(&format!("1{}", "0".repeat(40)));
    assert!(
        huge.resolve(f32::from_bits(1), 0.0, 1.0, limits())
            .unwrap()
            .value
            > 0.00000001
    );
}
#[test]
fn final_subnormal_rounding_and_clamping_are_distinct() {
    let tiny = f32::from_bits(1);
    for p in ["50", "-50"] {
        let sample = percent(p)
            .resolve(tiny, -f32::MAX, f32::MAX, limits())
            .unwrap();
        assert_eq!(sample.value.to_bits(), 0);
        assert!(sample.rounded_to_zero);
        assert_eq!(sample.clamped, None);
    }
    let sample = percent("-50").resolve(tiny, 0.0, 1.0, limits()).unwrap();
    assert_eq!(sample.value, 0.0);
    assert!(!sample.rounded_to_zero);
    assert_eq!(sample.clamped, Some(Clamp::Lower));
    let sample = percent("100").resolve(tiny, 0.0, 1.0, limits()).unwrap();
    assert_eq!(sample.value.to_bits(), 1);
    assert!(!sample.rounded_to_zero);
    let sample = percent("0").resolve(100.0, -0.0, 1.0, limits()).unwrap();
    assert_eq!(sample.value.to_bits(), 0);
    assert!(!sample.rounded_to_zero);
    assert!(Decimal::zero().is_zero());
    assert!(!Decimal::zero().authored_negative());
    assert_eq!(
        d("300").checked_percentage_of_f32(0.5, limits()).unwrap(),
        d("1.5")
    );
}
#[test]
fn invalid_context_and_limits_do_not_produce_partial_or_fabricated_values() {
    let a = percent("1");
    let b = pixels(2.0);
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(matches!(
            LengthPercentage::from_pixels(value, limits()),
            Err(MixedValueError::InvalidPixels)
        ));
        assert_eq!(
            a.resolve(value, 0.0, 1.0, limits()).unwrap_err(),
            MixedValueError::InvalidBasis
        );
        assert!(matches!(
            combine_length_percentage(&a, &b, Combination::Interpolate(value), limits()),
            Err(MixedValueError::NonfiniteProgress)
        ));
        assert_eq!(
            d("1")
                .checked_percentage_of_f32(value, limits())
                .unwrap_err(),
            DecimalScalingError::NonfiniteValue
        );
    }
    for (low, high) in [(1.0, 0.0), (f32::NAN, 1.0), (0.0, f32::INFINITY)] {
        assert_eq!(
            a.resolve(1.0, low, high, limits()).unwrap_err(),
            MixedValueError::InvalidRange
        );
    }
    let mut tiny = limits();
    tiny.work = NonZeroUsize::MIN;
    assert!(matches!(
        a.resolve(1.0, 0.0, 1.0, tiny),
        Err(MixedValueError::Numeric(NumericError::WorkLimit))
    ));
    assert!(matches!(
        combine_length_percentage(&a, &b, Combination::Add, tiny),
        Err(MixedValueError::Numeric(NumericError::WorkLimit))
    ));
    assert_eq!(a.components(), (&d("0"), &d("1")));
    assert_eq!(b.components(), (&d("2"), &d("0")));
}
