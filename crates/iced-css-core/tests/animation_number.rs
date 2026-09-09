use iced_css_core::animation::{combine::Combination, number::*};
fn resolve(a: f32, b: f32, op: Combination, lo: f32, hi: f32) -> NumericSample {
    combine_number(a, b, op).unwrap().resolve(lo, hi).unwrap()
}
#[test]
fn weighted_combination_keeps_extreme_cancellation_before_final_range() {
    for p in [-f32::MAX, -2.0, 0.0, 0.5, 1.0, 2.0, f32::MAX] {
        let s = resolve(
            f32::MAX,
            f32::MAX,
            Combination::Interpolate(p),
            -f32::MAX,
            f32::MAX,
        );
        assert_eq!(s.value, f32::MAX);
        assert_eq!(s.clamped, None);
    }
    assert_eq!(
        resolve(
            -f32::MAX,
            f32::MAX,
            Combination::Interpolate(0.5),
            -f32::MAX,
            f32::MAX
        )
        .value,
        0.0
    );
    let s = resolve(
        -f32::MAX,
        f32::MAX,
        Combination::Interpolate(2.0),
        -f32::MAX,
        f32::MAX,
    );
    assert_eq!(s.value, f32::MAX);
    assert_eq!(s.clamped, Some(Clamp::Upper));
    let s = resolve(10.0, 20.0, Combination::Interpolate(-2.0), 0.0, 100.0);
    assert_eq!(s.value, 0.0);
    assert_eq!(s.clamped, Some(Clamp::Lower));
}
#[test]
fn subnormal_rounding_is_ties_even_and_zero_is_canonical() {
    for (bits, expected) in [(1, 0), (2, 1), (3, 2), (5, 2), (7, 4)] {
        for sign in [1.0, -1.0] {
            let s = resolve(
                0.0,
                sign * f32::from_bits(bits),
                Combination::Interpolate(0.5),
                -1.0,
                1.0,
            );
            let expected = if expected == 0 {
                0
            } else {
                expected | if sign < 0.0 { 1 << 31 } else { 0 }
            };
            assert_eq!(s.value.to_bits(), expected);
            assert_eq!(s.clamped, None);
        }
    }
}
#[test]
fn addition_accumulation_and_final_range_are_explicit() {
    for op in [Combination::Add, Combination::Accumulate] {
        let intermediate = combine_number(2.0, 3.0, op).unwrap();
        assert_eq!(
            intermediate.resolve(0.0, 10.0).unwrap(),
            NumericSample {
                value: 5.0,
                clamped: None
            }
        );
        assert_eq!(
            intermediate.resolve(0.0, 4.0).unwrap(),
            NumericSample {
                value: 4.0,
                clamped: Some(Clamp::Upper)
            }
        );
        assert_eq!(
            intermediate.resolve(6.0, 10.0).unwrap(),
            NumericSample {
                value: 6.0,
                clamped: Some(Clamp::Lower)
            }
        );
        assert_eq!(
            intermediate.resolve(5.0, 5.0).unwrap(),
            NumericSample {
                value: 5.0,
                clamped: None
            }
        );
    }
}
#[test]
fn nonfinite_inputs_and_invalid_final_ranges_are_rejected() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_number(value, 1.0, Combination::Add).unwrap_err(),
            NumberError::NonfiniteEndpoint
        );
        assert_eq!(
            combine_number(1.0, value, Combination::Add).unwrap_err(),
            NumberError::NonfiniteEndpoint
        );
        assert_eq!(
            combine_number(0.0, 1.0, Combination::Interpolate(value)).unwrap_err(),
            NumberError::NonfiniteProgress
        );
        assert_eq!(
            combine_number(0.0, 1.0, Combination::Add)
                .unwrap()
                .resolve(value, 1.0)
                .unwrap_err(),
            NumberError::InvalidRange
        );
    }
    assert_eq!(
        combine_number(0.0, 1.0, Combination::Add)
            .unwrap()
            .resolve(2.0, 1.0)
            .unwrap_err(),
        NumberError::InvalidRange
    );
}
