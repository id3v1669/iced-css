//! Exact bounded sums of finite f32 box-equation inputs, in units of 2^-150.
//! Flat box/line callers use at most thirty-two original finite-input terms.
//! Inline hierarchies admit at most u32::MAX nodes: each local raise is <= 2M,
//! where M = f32::MAX. Local origins are <= 2nM, group edges <= (2n+1)M,
//! heights <= (4n+2)M, and final coordinates <= (8n+4)M < 2^35 M.
//! Thus at most 314 magnitude bits are needed in this quantum (320 available).
//! Original contributions are halved at most once; hierarchy assembly never halves.
//! Fixed tables sum at most u32::MAX finite widths, below 2^32 M and 310 bits.
//! Positive integer division retains the fractional quantum as a rounding sticky bit.
use super::block::BlockWidthError;
use std::{cmp::Ordering, num::NonZeroU32};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Exact {
    words: [u64; 5],
    negative: bool,
}
impl Exact {
    pub const ZERO: Self = Self {
        words: [0; 5],
        negative: false,
    };
    pub fn from_f32(value: f32) -> Self {
        debug_assert!(value.is_finite());
        let bits = value.to_bits();
        let exponent = (bits >> 23) & 255;
        let mantissa = (bits & 0x7fffff) | if exponent == 0 { 0 } else { 1 << 23 };
        let shift = if exponent == 0 { 1 } else { exponent } as usize;
        let mut result = Self::ZERO;
        let word = shift / 64;
        let offset = shift % 64;
        result.words[word] = u64::from(mantissa) << offset;
        if offset > 40 {
            result.words[word + 1] = u64::from(mantissa) >> (64 - offset);
        }
        result.negative = bits >> 31 != 0 && mantissa != 0;
        result
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
            // Thirty-two finite f32 terms fit within 284 of the available 320 bits.
            assert_eq!(carry, 0, "box equation exceeded its fixed term bound");
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
            result.negative = larger.negative && result.words != [0; 5];
            result
        }
    }
    pub fn sub(self, mut other: Self) -> Self {
        if other.words != [0; 5] {
            other.negative = !other.negative;
        }
        self.add(other)
    }
    pub fn half(self) -> Self {
        // Every input f32 is an even multiple of the accumulator quantum.
        debug_assert_eq!(self.words[0] & 1, 0);
        let mut result = self;
        for index in 0..5 {
            result.words[index] = (self.words[index] >> 1)
                | if index < 4 {
                    self.words[index + 1] << 63
                } else {
                    0
                };
        }
        result.negative &= result.words != [0; 5];
        result
    }
    fn bit(self, index: usize) -> bool {
        self.words[index / 64] & (1 << (index % 64)) != 0
    }
    fn below(self, bit: usize) -> bool {
        let word = bit / 64;
        self.words[..word].iter().any(|w| *w != 0)
            || (!bit.is_multiple_of(64) && self.words[word] & ((1u64 << (bit % 64)) - 1) != 0)
    }
    fn rounded_mantissa(self, shift: usize, sticky: bool) -> u32 {
        let word = shift / 64;
        let offset = shift % 64;
        let mut upper = self.words[word] >> offset;
        if offset != 0 && word < 4 {
            upper |= self.words[word + 1] << (64 - offset);
        }
        let mut result = upper as u32;
        if self.bit(shift - 1) && (self.below(shift - 1) || sticky || result & 1 != 0) {
            result += 1;
        }
        result
    }
    pub fn to_f32(self) -> Result<f32, BlockWidthError> {
        self.round_with_sticky(false)
    }
    /// Divide before the sole binary32 rounding; no heap storage or float divisor.
    pub fn divided_to_f32(self, divisor: NonZeroU32) -> Result<f32, BlockWidthError> {
        let divisor = u128::from(divisor.get());
        let mut quotient = self;
        let mut remainder = 0u128;
        for word in quotient.words.iter_mut().rev() {
            // remainder < divisor <= u32::MAX: at most 96 concatenated bits.
            // The quotient of this step fits one u64 because remainder < divisor.
            let wide = (remainder << 64) | u128::from(*word);
            *word = (wide / divisor) as u64;
            remainder = wide % divisor;
        }
        quotient.round_with_sticky(remainder != 0)
    }
    fn round_with_sticky(self, sticky: bool) -> Result<f32, BlockWidthError> {
        // A zero quotient plus its fractional quantum is below 2^-150, hence
        // below the smallest positive binary32 midpoint, even when sticky is set.
        let Some(word) = self.words.iter().rposition(|word| *word != 0) else {
            return Ok(0.0);
        };
        let highest = word * 64 + 63 - self.words[word].leading_zeros() as usize;
        let payload = if highest < 24 {
            self.rounded_mantissa(1, sticky)
        } else {
            let mut exponent = highest - 23;
            let mut mantissa = self.rounded_mantissa(exponent, sticky);
            if mantissa == 1 << 24 {
                mantissa >>= 1;
                exponent += 1;
            }
            if exponent >= 255 {
                return Err(BlockWidthError::Unrepresentable);
            }
            ((exponent as u32) << 23) | (mantissa & 0x7fffff)
        };
        if payload == 0 {
            return Ok(0.0);
        }
        Ok(f32::from_bits(
            payload | if self.negative { 1 << 31 } else { 0 },
        ))
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

#[cfg(test)]
#[path = "arithmetic_tests.rs"]
mod tests;
