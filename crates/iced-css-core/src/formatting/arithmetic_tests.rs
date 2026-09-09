use super::Exact;
use crate::{
    formatting::block::BlockWidthError,
    values::{Decimal, NumericLimits},
};
use std::num::{NonZeroU32, NonZeroUsize};

fn divide(value: Exact, count: u32) -> Result<f32, BlockWidthError> {
    value.divided_to_f32(NonZeroU32::new(count).unwrap())
}

#[test]
fn exact_division_known_bits_and_subnormal_ties() {
    for (value, count, bits) in [
        (1., 3, 0x3eaaaaab),
        (-1., 3, 0xbeaaaaab),
        (1., 16_777_217, 0x337fffff),
        (f32::from_bits(1), 2, 0),
        (f32::from_bits(3), 2, 2),
        (f32::from_bits(2), 3, 1),
        (-f32::from_bits(1), 2, 0),
        (-0., u32::MAX, 0),
        (f32::MAX, 1, 0x7f7fffff),
    ] {
        assert_eq!(
            divide(Exact::from_f32(value), count).unwrap().to_bits(),
            bits
        );
    }
    let residual = Exact::from_f32(1.).sub(Exact::from_f32(2.0f32.powi(-25)));
    assert_eq!(divide(residual, 3).unwrap().to_bits(), 0x3eaaaaaa);
    // This quotient has guard one, no lower whole bits, and nonzero remainder.
    assert_eq!(
        divide(Exact::from_f32(f32::from_bits(2)), 3)
            .unwrap()
            .to_bits(),
        1
    );
}

#[test]
fn exact_division_normal_midpoints_and_overflow_preserve_existing_rounding() {
    for (base, expected) in [(1., 0x3f800000), (f32::from_bits(0x3f800001), 0x3f800002)] {
        let midpoint = Exact::from_f32(base).add(Exact::from_f32(2.0f32.powi(-24)));
        assert_eq!(
            divide(midpoint.add(midpoint).add(midpoint), 3)
                .unwrap()
                .to_bits(),
            expected
        );
        // Add two accumulator quanta to the numerator. Division by three
        // leaves a fractional quantum, requiring sticky at a normal midpoint.
        let above = midpoint
            .add(midpoint)
            .add(midpoint)
            .add(Exact::from_f32(f32::from_bits(1)));
        let upper = if base == 1. { 0x3f800001 } else { 0x3f800002 };
        assert_eq!(divide(above, 3).unwrap().to_bits(), upper);
    }
    let transition =
        Exact::from_f32(f32::from_bits(0x007fffff)).add(Exact::from_f32(f32::MIN_POSITIVE));
    assert_eq!(divide(transition, 2).unwrap().to_bits(), 0x00800000);
    let max = Exact::from_f32(f32::MAX);
    let midpoint = max.add(Exact::from_f32(2.0f32.powi(103)));
    assert_eq!(divide(midpoint, 1), midpoint.to_f32());
    assert_eq!(divide(midpoint, 1), Err(BlockWidthError::Unrepresentable));
    let below = midpoint.sub(Exact::from_f32(f32::from_bits(1)));
    assert_eq!(divide(below, 1), below.to_f32());
    assert_eq!(divide(below, 1).unwrap(), f32::MAX);
    assert_eq!(
        divide(max.add(max), 1),
        Err(BlockWidthError::Unrepresentable)
    );
    assert_eq!(divide(max.add(max), 2).unwrap(), f32::MAX);
}

#[test]
fn exact_division_matches_independent_decimal_residual_oracle() {
    let limits = NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1_000_000).unwrap(),
    };
    let inputs = [
        0.,
        f32::from_bits(1),
        f32::from_bits(3),
        f32::MIN_POSITIVE,
        2.0f32.powi(-25),
        1.,
        f32::from_bits(0x3f800001),
        f32::MAX,
    ];
    for a in inputs {
        for b in inputs {
            let exact = Exact::from_f32(a).sub(Exact::from_f32(b));
            let decimal = Decimal::from_finite_f32(a, limits)
                .unwrap()
                .checked_add(&Decimal::from_finite_f32(-b, limits).unwrap(), limits)
                .unwrap();
            for count in [1, 2, 3, 7, 16_777_217, u32::MAX] {
                let denominator = Decimal::parse(&count.to_string(), limits).unwrap();
                let expected = decimal.ratio_finite_f32(&denominator, limits).unwrap();
                assert_eq!(
                    divide(exact, count).unwrap().to_bits(),
                    expected.value.to_bits(),
                    "a={a:?}, b={b:?}, count={count}"
                );
            }
        }
    }
}
