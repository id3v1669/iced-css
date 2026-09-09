//! Exact finite-f32 numeric pair combination followed by explicit final bounds.
//! Products use units of 2^-298. For M=f32::MAX<2^128, the absolute sum
//! |a|+|p*a|+|p*b| <= M+2M^2 <2^258 needs at most556 magnitude bits.
//! Nine u64 words supply576 bits; arbitrary repeated addition is not exposed.
use super::combine::Combination;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberError {
    NonfiniteEndpoint,
    NonfiniteProgress,
    InvalidRange,
}
impl std::fmt::Display for NumberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "numeric animation: {self:?}")
    }
}
impl std::error::Error for NumberError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clamp {
    Lower,
    Upper,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumericSample {
    pub value: f32,
    pub clamped: Option<Clamp>,
}
/// Exact pair result. Apply the target property's final computed/used range only
/// at its prescribed stage; this type does not select property eligibility.
#[derive(Debug, Clone, Copy)]
pub struct CombinedNumber(Exact);
impl CombinedNumber {
    /// A signed precision-underflow flag for property owners that need it.
    /// Range clamps to zero are deliberately not classified as underflow.
    pub(super) fn resolve_with_underflow(
        self,
        minimum: f32,
        maximum: f32,
    ) -> Result<(NumericSample, bool), NumberError> {
        let sample = self.resolve(minimum, maximum)?;
        let rounded_to_zero =
            sample.value == 0.0 && sample.clamped.is_none() && self.0 != Exact::from_f32(0.0);
        Ok((sample, rounded_to_zero))
    }
    /// Font-size's approved final domain, with exact positive-to-zero detection.
    pub(super) fn resolve_font_size(self) -> (NumericSample, bool) {
        let sample = self
            .resolve(0.0, f32::MAX)
            .expect("finite ordered font-size bounds");
        let positive_rounded_to_zero =
            sample.value == 0.0 && sample.clamped.is_none() && self.0 > Exact::from_f32(0.0);
        (sample, positive_rounded_to_zero)
    }
    /// Final legacy weight quantization, without a prior binary32 rounding.
    pub(super) fn legacy_weight(self) -> crate::values::font_weight::FontWeight {
        let mut weight = 100u16;
        while weight < 900 && self.0 >= Exact::from_f32(f32::from(weight + 50)) {
            weight += 100;
        }
        crate::values::font_weight::FontWeight::new(weight)
            .expect("eight hundred-step thresholds yield a valid CSS2.2 weight")
    }
    /// Clamp the exact result, then round once to binary32, nearest/ties-even.
    /// Ordinary precision rounding is not reported as a range clamp.
    pub fn resolve(self, minimum: f32, maximum: f32) -> Result<NumericSample, NumberError> {
        if !minimum.is_finite() || !maximum.is_finite() || minimum > maximum {
            return Err(NumberError::InvalidRange);
        }
        let low = Exact::from_f32(minimum);
        let high = Exact::from_f32(maximum);
        let (exact, clamped) = if self.0 < low {
            (low, Some(Clamp::Lower))
        } else if self.0 > high {
            (high, Some(Clamp::Upper))
        } else {
            (self.0, None)
        };
        Ok(NumericSample {
            value: exact.to_f32(),
            clamped,
        })
    }
}
/// Combine finite numeric endpoints. Progress may extrapolate outside zero to one.
/// Addition and default accumulation use the same sum. No early range clamp or
/// intermediate f32 rounding occurs, including in (1-p) for interpolation.
pub fn combine_number(
    start: f32,
    end: f32,
    operation: Combination,
) -> Result<CombinedNumber, NumberError> {
    if !start.is_finite() || !end.is_finite() {
        return Err(NumberError::NonfiniteEndpoint);
    }
    let result = match operation {
        Combination::Interpolate(p) => {
            if !p.is_finite() {
                return Err(NumberError::NonfiniteProgress);
            }
            Exact::from_f32(start)
                .sub(Exact::product(start, p))
                .add(Exact::product(end, p))
        }
        Combination::Add | Combination::Accumulate => {
            Exact::from_f32(start).add(Exact::from_f32(end))
        }
    };
    Ok(CombinedNumber(result))
}
fn binary(value: f32) -> (u64, i32, bool) {
    let bits = value.to_bits();
    let exponent = (bits >> 23) & 255;
    let mantissa = (bits & 0x7fffff) | if exponent == 0 { 0 } else { 1 << 23 };
    (
        u64::from(mantissa),
        if exponent == 0 {
            -149
        } else {
            exponent as i32 - 150
        },
        bits >> 31 != 0,
    )
}
use std::cmp::Ordering;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Exact {
    words: [u64; 9],
    negative: bool,
}
impl Exact {
    pub const ZERO: Self = Self {
        words: [0; 9],
        negative: false,
    };
    fn term(mantissa: u64, exponent: i32, negative: bool) -> Self {
        let shift = (exponent + 298) as usize;
        let mut result = Self::ZERO;
        let word = shift / 64;
        let offset = shift % 64;
        result.words[word] = mantissa << offset;
        if offset != 0 && word < 8 {
            result.words[word + 1] = mantissa >> (64 - offset);
        }
        result.negative = negative && mantissa != 0;
        result
    }
    fn from_f32(value: f32) -> Self {
        let (mantissa, exponent, negative) = binary(value);
        Self::term(mantissa, exponent, negative)
    }
    fn product(a: f32, b: f32) -> Self {
        let (ma, ea, na) = binary(a);
        let (mb, eb, nb) = binary(b);
        Self::term(ma * mb, ea + eb, na != nb)
    }
    fn magnitude_cmp(&self, other: &Self) -> Ordering {
        self.words.iter().rev().cmp(other.words.iter().rev())
    }
    pub fn add(self, other: Self) -> Self {
        if self.negative == other.negative {
            let mut result = Self::ZERO;
            let mut carry = 0u128;
            for (index, word) in result.words.iter_mut().enumerate() {
                let sum = u128::from(self.words[index]) + u128::from(other.words[index]) + carry;
                *word = sum as u64;
                carry = sum >> 64;
            }
            // The public pair operations require at most 556 magnitude bits.
            assert_eq!(carry, 0, "numeric animation exceeded its fixed pair bound");
            result.negative = self.negative;
            result
        } else {
            let (larger, smaller) = if self.magnitude_cmp(&other) == Ordering::Less {
                (other, self)
            } else {
                (self, other)
            };
            let mut result = Self::ZERO;
            let mut borrow = false;
            for (index, word) in result.words.iter_mut().enumerate() {
                let (a, first) = larger.words[index].overflowing_sub(smaller.words[index]);
                let (b, second) = a.overflowing_sub(u64::from(borrow));
                *word = b;
                borrow = first || second;
            }
            debug_assert!(!borrow);
            result.negative = larger.negative && result.words != [0; 9];
            result
        }
    }
    pub fn sub(self, mut other: Self) -> Self {
        if other.words != [0; 9] {
            other.negative = !other.negative;
        }
        self.add(other)
    }
    fn bit(self, index: usize) -> bool {
        self.words[index / 64] & (1 << (index % 64)) != 0
    }
    fn below(self, bit: usize) -> bool {
        let word = bit / 64;
        self.words[..word].iter().any(|w| *w != 0)
            || (!bit.is_multiple_of(64) && self.words[word] & ((1u64 << (bit % 64)) - 1) != 0)
    }
    fn rounded_mantissa(self, shift: usize) -> u32 {
        let word = shift / 64;
        let offset = shift % 64;
        let mut upper = self.words[word] >> offset;
        if offset != 0 && word < 8 {
            upper |= self.words[word + 1] << (64 - offset);
        }
        let mut result = upper as u32;
        if self.bit(shift - 1) && (self.below(shift - 1) || result & 1 != 0) {
            result += 1;
        }
        result
    }
    fn to_f32(self) -> f32 {
        let Some(word) = self.words.iter().rposition(|word| *word != 0) else {
            return 0.0;
        };
        let highest = word * 64 + 63 - self.words[word].leading_zeros() as usize;
        let payload = if highest < 172 {
            self.rounded_mantissa(149)
        } else {
            let mut exponent = highest - 171;
            let mut mantissa = self.rounded_mantissa(highest - 23);
            if mantissa == 1 << 24 {
                mantissa >>= 1;
                exponent += 1;
            }
            // Only called after exact clamping to finite f32 bounds.
            assert!(exponent < 255);
            ((exponent as u32) << 23) | (mantissa & 0x7fffff)
        };
        if payload == 0 {
            return 0.0;
        }
        f32::from_bits(payload | if self.negative { 1 << 31 } else { 0 })
    }
}
impl Ord for Exact {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.negative, other.negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => self.magnitude_cmp(other),
            (true, true) => self.magnitude_cmp(other).reverse(),
        }
    }
}
impl PartialOrd for Exact {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
