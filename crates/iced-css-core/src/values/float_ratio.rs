//! Fixed-size nonnegative binary-rational arithmetic for validated internal callers.
//! Numerators use at most56 bits, denominators at most24; cross-products fit80 bits.
use std::cmp::Ordering;
#[derive(Debug, Clone, Copy)]
pub(crate) struct RatioOverflow;
#[derive(Clone, Copy)]
pub(crate) struct Ratio {
    numerator: u128,
    denominator: u128,
    exponent: i32,
}
fn binary(value: f32) -> (u128, i32) {
    let bits = value.to_bits();
    let exponent = (bits >> 23) & 255;
    let mantissa = bits & 0x7fffff;
    if exponent == 0 {
        (u128::from(mantissa), -149)
    } else {
        (u128::from(mantissa | (1 << 23)), exponent as i32 - 150)
    }
}
/// Compare integer * 2^exponent without shifting across the u128 boundary.
fn scaled_cmp(a: u128, ea: i32, b: u128, eb: i32) -> Ordering {
    if a == 0 || b == 0 {
        return a.cmp(&b);
    }
    let a_bits = 128 - a.leading_zeros() as i32;
    let b_bits = 128 - b.leading_zeros() as i32;
    match (a_bits + ea).cmp(&(b_bits + eb)) {
        Ordering::Equal => {
            // Equal highest-bit positions imply that alignment fits within the
            // wider operand's bit count (at most80 bits for these callers).
            if ea >= eb {
                (a << (ea - eb)).cmp(&b)
            } else {
                a.cmp(&(b << (eb - ea)))
            }
        }
        unequal => unequal,
    }
}
impl Ratio {
    pub(crate) fn scaled_integer(
        value: f32,
        multiplier: u32,
        divisor: std::num::NonZeroU16,
    ) -> Self {
        let (mantissa, exponent) = binary(value);
        Self {
            numerator: mantissa * u128::from(multiplier),
            denominator: u128::from(divisor.get()),
            exponent,
        }
    }

    pub(crate) fn product_divided(a: f32, b: f32, divisor: f32) -> Self {
        let (a, ea) = binary(a);
        let (b, eb) = binary(b);
        let (denominator, ed) = binary(divisor);
        debug_assert_ne!(denominator, 0);
        Self {
            numerator: a * b,
            denominator,
            exponent: ea + eb - ed,
        }
    }
    pub(crate) fn compare(self, other: Self) -> Ordering {
        scaled_cmp(
            self.numerator * other.denominator,
            self.exponent,
            other.numerator * self.denominator,
            other.exponent,
        )
    }
    fn compare_binary(self, mantissa: u128, exponent: i32) -> Ordering {
        scaled_cmp(
            self.numerator,
            self.exponent,
            self.denominator * mantissa,
            exponent,
        )
    }
    pub(crate) fn compare_f32(self, value: f32) -> Ordering {
        let (mantissa, exponent) = binary(value);
        self.compare_binary(mantissa, exponent)
    }
    pub(crate) fn round(self) -> Result<f32, RatioOverflow> {
        const MAX: u32 = 0x7f7fffff;
        if self.compare_f32(f32::MAX) != Ordering::Less {
            // Midpoint between largest finite binary32 and the overflow value.
            if self.compare_binary((1 << 25) - 1, 103) != Ordering::Less {
                return Err(RatioOverflow);
            }
            return Ok(f32::MAX);
        }
        let (mut low, mut high) = (0, MAX);
        while low + 1 < high {
            let mid = low + (high - low) / 2;
            if self.compare_f32(f32::from_bits(mid)) == Ordering::Less {
                high = mid;
            } else {
                low = mid;
            }
        }
        let (a, ea) = binary(f32::from_bits(low));
        let (b, eb) = binary(f32::from_bits(high));
        let exponent = ea.min(eb);
        let midpoint = (a << (ea - exponent)) + (b << (eb - exponent));
        let bits = match self.compare_binary(midpoint, exponent - 1) {
            Ordering::Less => low,
            Ordering::Greater => high,
            Ordering::Equal => {
                if low & 1 == 0 {
                    low
                } else {
                    high
                }
            }
        };
        Ok(f32::from_bits(bits))
    }
}
