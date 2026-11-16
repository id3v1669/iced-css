#[path = "convert.rs"]
mod convert;
#[path = "counter_format.rs"]
mod counter_format;
use super::{Budget, NumericError, NumericLimits};
pub use convert::{ConversionError, FiniteF32, NonnegativeF32, RangeEvent};
pub use counter_format::{
    CounterError, CounterLimits, CounterStyle, format_counter, format_counters,
};
#[path = "counter_scope.rs"]
pub(crate) mod counter_scope;
const BASE: u64 = 1_000_000_000;

/// Little-endian base-10^9 limbs, without high zero limbs. Empty is zero.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Natural(Vec<u32>);
impl Natural {
    fn from_digits(digits: &[u8], budget: &mut Budget) -> Result<Self, NumericError> {
        budget.spend(digits.len())?;
        let mut limbs = Vec::new();
        limbs
            .try_reserve_exact(digits.len().div_ceil(9))
            .map_err(|_| NumericError::Allocation)?;
        for chunk in digits.rchunks(9) {
            // Input is already ASCII digits; at most nine digits fit below BASE.
            limbs.push(chunk.iter().fold(0u32, |n, b| n * 10 + u32::from(b - b'0')));
        }
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        Ok(Self(limbs))
    }
    fn is_zero(&self) -> bool {
        self.0.is_empty()
    }
    /// PCT: the limbs of a `u64` (at most three base-10^9 limbs).
    fn from_u64(mut value: u64) -> Result<Self, NumericError> {
        let mut limbs = Vec::new();
        limbs
            .try_reserve_exact(3)
            .map_err(|_| NumericError::Allocation)?;
        while value != 0 {
            limbs.push((value % BASE) as u32);
            value /= BASE;
        }
        Ok(Self(limbs))
    }
    fn multiply_small(&mut self, factor: u32, budget: &mut Budget) -> Result<(), NumericError> {
        budget.spend(self.0.len())?;
        self.0
            .try_reserve(1)
            .map_err(|_| NumericError::Allocation)?;
        let mut carry = 0u64;
        for limb in &mut self.0 {
            // Callers use factors below BASE (CSS ratios and binary conversion).
            // The u64 product fits and the final carry remains one base limb.
            let value = u64::from(*limb) * u64::from(factor) + carry;
            *limb = (value % BASE) as u32;
            carry = value / BASE;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
        Ok(())
    }
    fn remainder(&self, divisor: u32, budget: &mut Budget) -> Result<u32, NumericError> {
        budget.spend(self.0.len())?;
        let mut remainder = 0u64;
        for limb in self.0.iter().rev() {
            remainder = (remainder * BASE + u64::from(*limb)) % u64::from(divisor);
        }
        Ok(remainder as u32)
    }
    fn divide_exact(&mut self, divisor: u32, budget: &mut Budget) -> Result<(), NumericError> {
        budget.spend(self.0.len())?;
        let mut remainder = 0u64;
        for limb in self.0.iter_mut().rev() {
            let value = remainder * BASE + u64::from(*limb);
            *limb = (value / u64::from(divisor)) as u32;
            remainder = value % u64::from(divisor);
        }
        debug_assert_eq!(remainder, 0);
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
        Ok(())
    }
}

/// Canonical decimal coefficient/scale plus the separately retained authored sign.
/// Raw source spans/spelling belong to syntax provenance, not semantic equality.
#[derive(Debug, Clone)]
pub struct Decimal {
    negative: bool,
    coefficient: Natural,
    scale: usize,
}
impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        self.is_negative() == other.is_negative()
            && self.coefficient == other.coefficient
            && self.scale == other.scale
    }
}
impl Eq for Decimal {}
impl Decimal {
    pub fn parse(input: &str, limits: NumericLimits) -> Result<Self, NumericError> {
        Self::parse_with(input, limits, &mut Budget::new(limits))
    }
    fn parse_with(
        input: &str,
        limits: NumericLimits,
        budget: &mut Budget,
    ) -> Result<Self, NumericError> {
        budget.spend(input.len())?;
        let (negative, body) = match input.as_bytes().first() {
            Some(b'-') => (true, &input[1..]),
            Some(b'+') => (false, &input[1..]),
            _ => (false, input),
        };
        let (integer, fraction) = match body.split_once('.') {
            Some((i, f)) if !f.is_empty() => (i, f),
            Some(_) => return Err(NumericError::InvalidNumber),
            None => (body, ""),
        };
        if integer.is_empty() && fraction.is_empty()
            || !integer
                .bytes()
                .chain(fraction.bytes())
                .all(|b| b.is_ascii_digit())
        {
            return Err(NumericError::InvalidNumber);
        }
        let count = integer.len() + fraction.len();
        if count > limits.digits.get() {
            return Err(NumericError::DigitLimit);
        }
        budget.spend(count)?;
        let mut digits = Vec::new();
        digits
            .try_reserve_exact(count)
            .map_err(|_| NumericError::Allocation)?;
        digits.extend_from_slice(integer.as_bytes());
        digits.extend_from_slice(fraction.as_bytes());
        let mut scale = fraction.len();
        while scale > 0 && digits.last() == Some(&b'0') {
            digits.pop();
            scale -= 1;
        }
        let first = digits
            .iter()
            .position(|b| *b != b'0')
            .unwrap_or(digits.len());
        let coefficient = Natural::from_digits(&digits[first..], budget)?;
        if coefficient.is_zero() {
            scale = 0;
        }
        Ok(Self {
            negative,
            coefficient,
            scale,
        })
    }
    pub fn authored_negative(&self) -> bool {
        self.negative
    }
    pub fn is_negative(&self) -> bool {
        self.negative && !self.coefficient.is_zero()
    }
    pub fn is_zero(&self) -> bool {
        self.coefficient.is_zero()
    }
    /// The bounded `Copy` form of this decimal (PCT), or `None` when its canonical
    /// coefficient exceeds `u64::MAX` or its scale exceeds `u32::MAX`.
    pub fn compact(&self) -> Option<CompactDecimal> {
        let mut coefficient = 0u64;
        for &limb in self.coefficient.0.iter().rev() {
            coefficient = coefficient
                .checked_mul(BASE)?
                .checked_add(u64::from(limb))?;
        }
        Some(CompactDecimal {
            negative: self.is_negative(),
            coefficient,
            scale: u32::try_from(self.scale).ok()?,
        })
    }
}

/// A bounded `Copy` decimal (PCT): the canonical coefficient of a [`Decimal`] that
/// fits `u64`, its scale and its sign, with `-0` stored as +0. It is built only by
/// [`Decimal::compact`], so equality is exact decimal equality, and
/// [`CompactDecimal::percentage_finite_f32`] resolves it on the exact path of
/// [`Decimal::percentage_finite_f32`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactDecimal {
    negative: bool,
    coefficient: u64,
    scale: u32,
}
impl CompactDecimal {
    /// Whether the decimal is negative; `-0` is stored as +0, so a zero is never negative.
    pub fn is_negative(self) -> bool {
        self.negative
    }
    /// Whether the decimal is zero (either sign).
    pub fn is_zero(self) -> bool {
        self.coefficient == 0
    }
}

/// Reduced rational CSS pixels. All possible denominator factors for CSS2.2
/// decimal absolute lengths are2,5,3,127; keeping exponents avoids huge powers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactLength {
    negative: bool,
    numerator: Natural,
    denominator: [usize; 4],
}
impl ExactLength {
    fn from_decimal(
        value: Decimal,
        multiplier: u32,
        divisor: u32,
        budget: &mut Budget,
    ) -> Result<Self, NumericError> {
        if value.is_zero() {
            return Ok(Self {
                negative: false,
                numerator: value.coefficient,
                denominator: [0; 4],
            });
        }
        let negative = value.is_negative();
        let mut numerator = value.coefficient;
        numerator.multiply_small(multiplier, budget)?;
        // The divisor's factors of 2, 5, 3 and 127; it has no others.
        let mut rest = divisor;
        let mut factors = [0usize; 4];
        for (power, prime) in factors.iter_mut().zip([2, 5, 3, 127]) {
            while rest > 1 && rest % prime == 0 {
                rest /= prime;
                *power += 1;
            }
        }
        debug_assert_eq!(rest, 1, "a divisor of 2, 5, 3 and 127 only");
        let mut denominator = [
            value.scale + factors[0],
            value.scale + factors[1],
            factors[2],
            factors[3],
        ];
        for (power, prime) in denominator.iter_mut().zip([2, 5, 3, 127]) {
            while *power > 0 && numerator.remainder(prime, budget)? == 0 {
                numerator.divide_exact(prime, budget)?;
                *power -= 1;
            }
        }
        Ok(Self {
            negative,
            numerator,
            denominator,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativeUnit {
    Em,
    Ex,
    /// CSS Values 3: the root element's font size.
    Rem,
    /// CSS Values 3: the advance of "0". No font is measured for it: half the font size,
    /// the fallback the specification allows.
    Ch,
}
impl RelativeUnit {
    fn parse(unit: &str) -> Option<Self> {
        [
            ("em", Self::Em),
            ("ex", Self::Ex),
            ("rem", Self::Rem),
            ("ch", Self::Ch),
        ]
        .into_iter()
        .find_map(|(name, value)| unit.eq_ignore_ascii_case(name).then_some(value))
    }
    /// What one of it measures where no font is consulted: `ex` and `ch` as half the font
    /// size.
    pub fn fallback_basis(self, font_size: f32, root_font_size: f32) -> f32 {
        match self {
            Self::Em => font_size,
            Self::Ex | Self::Ch => font_size * 0.5,
            Self::Rem => root_font_size,
        }
    }
}
/// Unbound specified operands. Relative/percentage operands still need their
/// property-specific context identity before they can be used as target/cache keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LengthTarget {
    Absolute(ExactLength),
    Relative(Decimal, RelativeUnit),
    Percentage(Decimal),
    Auto,
    /// A `calc()` sum (CSS Values 3).
    Calc(CalcLength),
}
/// A `calc()` sum reduced to one coefficient per unit kind: pixels, percent, then `em`,
/// `ex`, `rem` and `ch`. Kept as bit patterns, so equality is exact.
///
/// CSS Values 4 `min()`, `max()` and `clamp()` add a lower and an upper bound, each a
/// sum of the same kinds but for percentages, which they never hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalcLength {
    terms: [u32; 6],
    /// The lower bound's terms, when `bounds & 1`; the upper's, when `bounds & 2`.
    lower: [u32; 6],
    upper: [u32; 6],
    bounds: u8,
}
impl CalcLength {
    pub fn new(pixels: f32, percent: f32, relative: [f32; 4]) -> Self {
        let [em, ex, rem, ch] = relative;
        Self {
            terms: [pixels, percent, em, ex, rem, ch].map(f32::to_bits),
            lower: [0; 6],
            upper: [0; 6],
            bounds: 0,
        }
    }
    /// `value` kept within `lower` and `upper`, as `clamp()` keeps it; neither bound may
    /// hold a percentage, nor may the value.
    pub fn bounded(value: [f32; 6], lower: Option<[f32; 6]>, upper: Option<[f32; 6]>) -> Self {
        let bits = |terms: [f32; 6]| terms.map(f32::to_bits);
        Self {
            terms: bits(value),
            lower: lower.map_or([0; 6], bits),
            upper: upper.map_or([0; 6], bits),
            bounds: u8::from(lower.is_some()) | (u8::from(upper.is_some()) << 1),
        }
    }
    /// The unbounded sums of the value, then of each bound there is.
    pub fn parts(self) -> (Self, Option<Self>, Option<Self>) {
        let part = |terms: [u32; 6]| Self {
            terms,
            lower: [0; 6],
            upper: [0; 6],
            bounds: 0,
        };
        (
            part(self.terms),
            (self.bounds & 1 != 0).then(|| part(self.lower)),
            (self.bounds & 2 != 0).then(|| part(self.upper)),
        )
    }
    /// Its value with `evaluate` measuring each unbounded sum: the value kept within its
    /// bounds, the lower one winning where they cross (CSS Values 4 `clamp()`).
    pub fn resolve(self, evaluate: impl Fn(Self) -> f32) -> f32 {
        let (value, lower, upper) = self.parts();
        let mut result = evaluate(value);
        if let Some(upper) = upper {
            result = result.min(evaluate(upper));
        }
        if let Some(lower) = lower {
            result = result.max(evaluate(lower));
        }
        result
    }
    pub fn pixels(self) -> f32 {
        f32::from_bits(self.terms[0])
    }
    pub fn percent(self) -> f32 {
        f32::from_bits(self.terms[1])
    }
    pub fn relative(self, unit: RelativeUnit) -> f32 {
        f32::from_bits(
            self.terms[match unit {
                RelativeUnit::Em => 2,
                RelativeUnit::Ex => 3,
                RelativeUnit::Rem => 4,
                RelativeUnit::Ch => 5,
            }],
        )
    }
    /// Whether any font-relative term is present, in the value or a bound.
    pub fn is_font_relative(self) -> bool {
        let relative =
            |terms: &[u32; 6]| terms[2..].iter().any(|bits| f32::from_bits(*bits) != 0.0);
        relative(&self.terms)
            || (self.bounds & 1 != 0 && relative(&self.lower))
            || (self.bounds & 2 != 0 && relative(&self.upper))
    }
    /// Everything but the percentage, in pixels, with `ex` and `ch` as half the font size,
    /// within its bounds.
    pub fn fallback_pixels(self, font_size: f32, root_font_size: f32) -> f32 {
        self.resolve(|part| {
            [
                RelativeUnit::Em,
                RelativeUnit::Ex,
                RelativeUnit::Rem,
                RelativeUnit::Ch,
            ]
            .into_iter()
            .fold(part.pixels(), |total, unit| {
                let count = part.relative(unit);
                // An absent term adds nothing, whatever the font sizes are.
                if count == 0.0 {
                    total
                } else {
                    total + count * unit.fallback_basis(font_size, root_font_size)
                }
            })
        })
    }
}
/// Borrowed normalized operands for bounded border diagnostic identity.
/// This view performs neither conversion nor allocation. Limbs remain owned by
/// the immutable parsed value; their base is 10^9, least significant first.
#[derive(Debug, Clone, Copy)]
pub enum BorderLengthView<'a> {
    Absolute {
        negative: bool,
        limbs: &'a [u32],
        denominator: [usize; 4],
    },
    Relative {
        negative: bool,
        limbs: &'a [u32],
        scale: usize,
        unit: RelativeUnit,
    },
}

impl LengthTarget {
    pub fn border_diagnostic_view(&self) -> Option<BorderLengthView<'_>> {
        match self {
            Self::Absolute(value) => Some(BorderLengthView::Absolute {
                negative: value.negative,
                limbs: &value.numerator.0,
                denominator: value.denominator,
            }),
            Self::Relative(value, unit) => Some(BorderLengthView::Relative {
                negative: value.is_negative(),
                limbs: &value.coefficient.0,
                scale: value.scale,
                unit: *unit,
            }),
            Self::Percentage(_) | Self::Auto | Self::Calc(_) => None,
        }
    }

    /// Parse one decoded length-like value, not a declaration or full CSS token stream.
    /// Property-specific keyword/range validity is a later, separate check.
    pub fn parse(input: &str, limits: NumericLimits) -> Result<Self, NumericError> {
        let mut budget = Budget::new(limits);
        if input.eq_ignore_ascii_case("auto") {
            budget.spend(input.len())?;
            return Ok(Self::Auto);
        }
        if let Some(number) = input.strip_suffix('%') {
            return Ok(Self::Percentage(Decimal::parse_with(
                number,
                limits,
                &mut budget,
            )?));
        }
        if let Some(index) = input.len().checked_sub(2)
            && let (Some(number), Some(unit)) = (input.get(..index), input.get(index..))
        {
            let ratio = if unit.eq_ignore_ascii_case("px") {
                Some((1, 1))
            } else if unit.eq_ignore_ascii_case("in") {
                Some((96, 1))
            } else if unit.eq_ignore_ascii_case("cm") {
                Some((4800, 127))
            } else if unit.eq_ignore_ascii_case("mm") {
                Some((480, 127))
            } else if unit.eq_ignore_ascii_case("pt") {
                Some((4, 3))
            } else if unit.eq_ignore_ascii_case("pc") {
                Some((16, 1))
            } else {
                None
            };
            if let Some((n, d)) = ratio {
                return Ok(Self::Absolute(ExactLength::from_decimal(
                    Decimal::parse_with(number, limits, &mut budget)?,
                    n,
                    d,
                    &mut budget,
                )?));
            }
            if let Some(unit) = RelativeUnit::parse(unit) {
                return Ok(Self::Relative(
                    Decimal::parse_with(number, limits, &mut budget)?,
                    unit,
                ));
            }
        }
        // CSS Values 3: `vw`, `vh`, `vmin` and `vmax` are pixels of the viewport they are
        // parsed for; with none, they are no length.
        for unit in ["vmin", "vmax", "vw", "vh"] {
            if let Some(number) = input
                .len()
                .checked_sub(unit.len())
                .and_then(|at| input.get(at..).filter(|end| end.eq_ignore_ascii_case(unit)))
                .and_then(|_| input.get(..input.len() - unit.len()))
            {
                let Some(pixels) = crate::syntax::viewport_unit(unit) else {
                    return Err(NumericError::UnsupportedUnit);
                };
                // Hundredths of a pixel per unit, exactly.
                let multiplier = (pixels * 10_000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32;
                return Ok(Self::Absolute(ExactLength::from_decimal(
                    Decimal::parse_with(number, limits, &mut budget)?,
                    multiplier,
                    10_000,
                    &mut budget,
                )?));
            }
        }
        let zero = Decimal::parse_with(input, limits, &mut budget)?;
        if !zero.is_zero() {
            return Err(NumericError::UnsupportedUnit);
        }
        Ok(Self::Absolute(ExactLength::from_decimal(
            zero,
            1,
            1,
            &mut budget,
        )?))
    }
}

/// Exact signed CSS integer. Source spelling belongs to syntax provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Integer {
    negative: bool,
    magnitude: Natural,
}
impl Integer {
    /// Parse CSS2.2 integer syntax, with no decimal point or exponent notation.
    pub fn parse(input: &str, limits: NumericLimits) -> Result<Self, NumericError> {
        let mut budget = Budget::new(limits);
        budget.spend(input.len())?;
        if input.contains('.') {
            return Err(NumericError::InvalidNumber);
        }
        let decimal = Decimal::parse_with(input, limits, &mut budget)?;
        Ok(Self {
            negative: decimal.is_negative(),
            magnitude: decimal.coefficient,
        })
    }
    pub fn is_negative(&self) -> bool {
        self.negative
    }
    pub fn is_zero(&self) -> bool {
        self.magnitude.is_zero()
    }
    /// The value saturated to `i32`, for orderings such as stacking.
    pub fn saturating_i32(&self) -> i32 {
        let magnitude = self.magnitude.0.iter().rev().fold(0_i64, |total, limb| {
            total
                .saturating_mul(BASE as i64)
                .saturating_add(i64::from(*limb))
        });
        let signed = if self.negative { -magnitude } else { magnitude };
        i32::try_from(signed).unwrap_or(if self.negative { i32::MIN } else { i32::MAX })
    }
    /// The value clamped to 0..=255 (CSS2.2 4.3.6 rgb() integer clipping).
    pub fn clamped_byte(&self) -> u8 {
        if self.negative {
            return 0;
        }
        let limbs = &self.magnitude.0;
        if limbs.iter().skip(1).any(|limb| *limb != 0) {
            return u8::MAX;
        }
        limbs.first().map_or(0, |limb| (*limb).min(255) as u8)
    }
    /// Add without modifying either operand. Both input magnitudes and the result
    /// must fit the supplied digit limit; work/allocation exhaustion returns an error.
    /// This is integer arithmetic, not counter scoping or animation rounding.
    pub fn checked_add(&self, other: &Self, limits: NumericLimits) -> Result<Self, NumericError> {
        self.add_with_budget(other, limits, &mut Budget::new(limits))
    }
    fn add_with_budget(
        &self,
        other: &Self,
        limits: NumericLimits,
        budget: &mut Budget,
    ) -> Result<Self, NumericError> {
        use std::cmp::Ordering;
        self.magnitude.check_digits(limits)?;
        other.magnitude.check_digits(limits)?;
        budget.spend(self.magnitude.0.len())?;
        budget.spend(other.magnitude.0.len())?;
        let max_len = self.magnitude.0.len().max(other.magnitude.0.len());
        budget.spend(max_len)?;
        let capacity = max_len
            .checked_add(1)
            .ok_or(NumericError::DigitLimit)?
            .min(limits.digits.get().div_ceil(9));
        let mut limbs = Vec::new();
        limbs
            .try_reserve_exact(capacity)
            .map_err(|_| NumericError::Allocation)?;
        let negative;
        if self.negative == other.negative {
            negative = self.negative;
            let mut carry = 0u64;
            for i in 0..max_len {
                // Each limb is below BASE; the sum is at most 2*BASE-1.
                let sum = u64::from(*self.magnitude.0.get(i).unwrap_or(&0))
                    + u64::from(*other.magnitude.0.get(i).unwrap_or(&0))
                    + carry;
                limbs.push((sum % BASE) as u32);
                carry = sum / BASE;
            }
            if carry != 0 {
                if limbs.len() == capacity {
                    return Err(NumericError::DigitLimit);
                }
                limbs.push(carry as u32);
            }
        } else {
            let order = self
                .magnitude
                .0
                .len()
                .cmp(&other.magnitude.0.len())
                .then_with(|| {
                    self.magnitude
                        .0
                        .iter()
                        .rev()
                        .cmp(other.magnitude.0.iter().rev())
                });
            let (larger, smaller) = if order == Ordering::Less {
                (other, self)
            } else {
                (self, other)
            };
            negative = larger.negative;
            let mut borrow = 0i64;
            for (i, limb) in larger.magnitude.0.iter().enumerate() {
                // The difference including borrow lies in [-BASE, BASE-1].
                let difference = i64::from(*limb)
                    - i64::from(*smaller.magnitude.0.get(i).unwrap_or(&0))
                    - borrow;
                borrow = i64::from(difference < 0);
                limbs.push((difference + borrow * BASE as i64) as u32);
            }
            debug_assert_eq!(borrow, 0);
        }
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        let magnitude = Natural(limbs);
        magnitude.check_digits(limits)?;
        Ok(Self {
            negative: negative && !magnitude.is_zero(),
            magnitude,
        })
    }
}
impl Natural {
    fn check_digits(&self, limits: NumericLimits) -> Result<(), NumericError> {
        let digits = self.decimal_digits()?;
        if digits > limits.digits.get() {
            Err(NumericError::DigitLimit)
        } else {
            Ok(())
        }
    }
}

impl LengthTarget {
    /// Construct one dimension from its raw decimal spelling and decoded unit.
    /// This does not admit exponents or apply any property's sign restrictions.
    pub fn from_dimension(
        number: &str,
        unit: &str,
        limits: NumericLimits,
    ) -> Result<Self, NumericError> {
        let mut budget = Budget::new(limits);
        budget.spend(unit.len())?;
        let value = Decimal::parse_with(number, limits, &mut budget)?;
        if let Some(unit) = RelativeUnit::parse(unit) {
            return Ok(Self::Relative(value, unit));
        }
        let (n, d) = if unit.eq_ignore_ascii_case("px") {
            (1, 1)
        } else if unit.eq_ignore_ascii_case("in") {
            (96, 1)
        } else if unit.eq_ignore_ascii_case("cm") {
            (4800, 127)
        } else if unit.eq_ignore_ascii_case("mm") {
            (480, 127)
        } else if unit.eq_ignore_ascii_case("pt") {
            (4, 3)
        } else if unit.eq_ignore_ascii_case("pc") {
            (16, 1)
        } else if let Some(pixels) = crate::syntax::viewport_unit(unit) {
            // CSS Values 3: hundredths of a pixel per viewport unit, exactly.
            (
                (pixels * 10_000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32,
                10_000,
            )
        } else {
            return Err(NumericError::UnsupportedUnit);
        };
        Ok(Self::Absolute(ExactLength::from_decimal(
            value,
            n,
            d,
            &mut budget,
        )?))
    }
    pub fn is_negative(&self) -> bool {
        match self {
            Self::Absolute(value) => value.negative,
            Self::Relative(value, _) | Self::Percentage(value) => value.is_negative(),
            // A sum has no sign until it is used; a negative result is clipped there.
            Self::Auto | Self::Calc(_) => false,
        }
    }
}

#[path = "font_weight.rs"]
pub mod font_weight;

#[path = "decimal_arithmetic.rs"]
mod decimal_arithmetic;
pub use decimal_arithmetic::{
    DecimalArithmeticError, DecimalScalingError, IntegerInterpolationError,
};

#[path = "comparison.rs"]
mod comparison;

#[cfg(test)]
mod cf34_border_view_tests {
    use super::*;
    use std::num::NonZeroUsize;

    fn limits() -> NumericLimits {
        NumericLimits {
            digits: NonZeroUsize::new(256).unwrap(),
            work: NonZeroUsize::new(100_000).unwrap(),
        }
    }

    #[test]
    fn border_view_preserves_canonical_length_equality() {
        for input in ["1in", "96px", "96.000px"] {
            let value = LengthTarget::parse(input, limits()).unwrap();
            let Some(BorderLengthView::Absolute {
                negative,
                limbs,
                denominator,
            }) = value.border_diagnostic_view()
            else {
                panic!("absolute border operand");
            };
            assert!(!negative);
            assert_eq!(limbs, &[96]);
            assert_eq!(denominator, [0; 4]);
        }
        for input in ["10em", "10.000em"] {
            let value = LengthTarget::parse(input, limits()).unwrap();
            let Some(BorderLengthView::Relative {
                negative,
                limbs,
                scale,
                unit,
            }) = value.border_diagnostic_view()
            else {
                panic!("relative border operand");
            };
            assert!(!negative);
            assert_eq!(limbs, &[10]);
            assert_eq!(scale, 0);
            assert_eq!(unit, RelativeUnit::Em);
        }
        let zero = LengthTarget::parse("-0em", limits()).unwrap();
        assert!(matches!(
            zero.border_diagnostic_view(),
            Some(BorderLengthView::Relative {
                negative: false,
                limbs: [],
                scale: 0,
                unit: RelativeUnit::Em,
            })
        ));
    }

    #[test]
    fn border_view_does_not_convert_or_allocate() {
        let value =
            LengthTarget::parse("1000000000000000000000000000000000000000px", limits()).unwrap();
        let LengthTarget::Absolute(original) = &value else {
            panic!("absolute");
        };
        let Some(BorderLengthView::Absolute {
            limbs, denominator, ..
        }) = value.border_diagnostic_view()
        else {
            panic!("borrowed absolute view");
        };
        assert!(std::ptr::eq(limbs, original.numerator.0.as_slice()));
        assert_eq!(limbs, &[0, 0, 0, 0, 1000]);
        assert_eq!(denominator, [0; 4]);
        assert!(
            LengthTarget::parse("50%", limits())
                .unwrap()
                .border_diagnostic_view()
                .is_none()
        );
        assert!(LengthTarget::Auto.border_diagnostic_view().is_none());
    }
}
