use super::{CompactDecimal, Decimal, ExactLength, NumericError, NumericLimits};

/// Range events only; ordinary precision rounding does not produce an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeEvent {
    UpperSaturation,
    PositiveRoundedToZero,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonnegativeF32 {
    pub value: f32,
    pub event: Option<RangeEvent>,
}
/// Finite signed geometry result; unlike font/border conversion, no saturation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FiniteF32 {
    pub value: f32,
    pub rounded_to_zero: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionError {
    ZeroDenominator,
    InvalidNegative,
    InvalidContext,
    InvalidFiniteContext,
    Unrepresentable,
    Numeric(NumericError),
}
impl From<NumericError> for ConversionError {
    fn from(error: NumericError) -> Self {
        Self::Numeric(error)
    }
}
impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroDenominator => f.write_str("exact decimal ratio has a zero denominator"),
            Self::InvalidNegative => {
                f.write_str("negative input outside nonnegative conversion domain")
            }
            Self::InvalidContext => f.write_str("context must be finite and nonnegative"),
            Self::InvalidFiniteContext => f.write_str("context must be finite"),
            Self::Unrepresentable => {
                f.write_str("resolved value exceeds finite f32 geometry range")
            }
            Self::Numeric(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ConversionError {}
use super::{Budget, Natural};
use std::cmp::Ordering;

struct Rational {
    numerator: Natural,
    denominator: Natural,
}
impl Rational {
    fn new(
        numerator: &Natural,
        powers: [usize; 4],
        limits: NumericLimits,
        budget: &mut Budget,
    ) -> Result<Self, NumericError> {
        numerator.check_digits(limits)?;
        let numerator = copy(numerator, budget)?;
        let mut denominator = Natural::from_digits(b"1", budget)?;
        for (power, prime) in powers.into_iter().zip([2, 5, 3, 127]) {
            for _ in 0..power {
                multiply(&mut denominator, prime, limits, budget)?;
            }
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }
    fn scale(
        &mut self,
        value: f32,
        percentage: bool,
        limits: NumericLimits,
        budget: &mut Budget,
    ) -> Result<(), NumericError> {
        let (mantissa, exponent) = binary(value.to_bits());
        multiply(&mut self.numerator, mantissa, limits, budget)?;
        if exponent >= 0 {
            power_two(&mut self.numerator, exponent as u32, limits, budget)?;
        } else {
            power_two(
                &mut self.denominator,
                exponent.unsigned_abs(),
                limits,
                budget,
            )?;
        }
        if percentage {
            multiply(&mut self.denominator, 100, limits, budget)?;
        }
        Ok(())
    }
    /// Compare exact n/d with mantissa * 2^exponent by cross multiplication.
    fn compare(
        &self,
        mantissa: u32,
        exponent: i32,
        limits: NumericLimits,
        budget: &mut Budget,
    ) -> Result<Ordering, NumericError> {
        if mantissa == 0 {
            return Ok(if self.numerator.is_zero() {
                Ordering::Equal
            } else {
                Ordering::Greater
            });
        }
        let mut left = copy(&self.numerator, budget)?;
        let mut right = copy(&self.denominator, budget)?;
        multiply(&mut right, mantissa, limits, budget)?;
        if exponent >= 0 {
            power_two(&mut right, exponent as u32, limits, budget)?;
        } else {
            power_two(&mut left, exponent.unsigned_abs(), limits, budget)?;
        }
        budget.spend(left.0.len().max(right.0.len()))?;
        Ok(left
            .0
            .len()
            .cmp(&right.0.len())
            .then_with(|| left.0.iter().rev().cmp(right.0.iter().rev())))
    }
    fn round(
        &self,
        limits: NumericLimits,
        budget: &mut Budget,
    ) -> Result<NonnegativeF32, NumericError> {
        const MAX: u32 = 0x7f7fffff;
        if self.numerator.is_zero() {
            return Ok(result(0, None));
        }
        let (mantissa, exponent) = binary(MAX);
        match self.compare(mantissa, exponent, limits, budget)? {
            Ordering::Greater => return Ok(result(MAX, Some(RangeEvent::UpperSaturation))),
            Ordering::Equal => return Ok(result(MAX, None)),
            Ordering::Less => {}
        }
        // Positive finite binary32 bit patterns have the same order as their values.
        let (mut low, mut high) = (0, MAX - 1);
        while low < high {
            let mid = low + (high - low).div_ceil(2);
            let (mantissa, exponent) = binary(mid);
            if self.compare(mantissa, exponent, limits, budget)? == Ordering::Less {
                high = mid - 1;
            } else {
                low = mid;
            }
        }
        let (a, ea) = binary(low);
        let (b, eb) = binary(low + 1);
        let exponent = ea.min(eb);
        // Adjacent floats differ in exponent by at most one; this fits 25 bits.
        let midpoint = (a << (ea - exponent)) + (b << (eb - exponent));
        let bits = match self.compare(midpoint, exponent - 1, limits, budget)? {
            Ordering::Less => low,
            Ordering::Greater => low + 1,
            Ordering::Equal => {
                if low & 1 == 0 {
                    low
                } else {
                    low + 1
                }
            }
        };
        Ok(result(
            bits,
            if bits == 0 {
                Some(RangeEvent::PositiveRoundedToZero)
            } else {
                None
            },
        ))
    }
}
fn result(bits: u32, event: Option<RangeEvent>) -> NonnegativeF32 {
    NonnegativeF32 {
        value: f32::from_bits(bits),
        event,
    }
}
fn binary(bits: u32) -> (u32, i32) {
    let fraction = bits & 0x7fffff;
    let exponent = (bits >> 23) & 0xff;
    if exponent == 0 {
        (fraction, -149)
    } else {
        (fraction | (1 << 23), exponent as i32 - 150)
    }
}
fn copy(value: &Natural, budget: &mut Budget) -> Result<Natural, NumericError> {
    budget.spend(value.0.len())?;
    let mut limbs = Vec::new();
    limbs
        .try_reserve_exact(value.0.len())
        .map_err(|_| NumericError::Allocation)?;
    limbs.extend_from_slice(&value.0);
    Ok(Natural(limbs))
}
fn multiply(
    value: &mut Natural,
    factor: u32,
    limits: NumericLimits,
    budget: &mut Budget,
) -> Result<(), NumericError> {
    value.multiply_small(factor, budget)?;
    value.check_digits(limits)
}
fn power_two(
    value: &mut Natural,
    mut exponent: u32,
    limits: NumericLimits,
    budget: &mut Budget,
) -> Result<(), NumericError> {
    while exponent > 0 {
        // 2^29 remains below the decimal limb base, preserving one-limb carry.
        let step = exponent.min(29);
        multiply(value, 1 << step, limits, budget)?;
        exponent -= step;
    }
    Ok(())
}
impl ExactLength {
    /// Round exact signed CSS pixels, rejecting resolved magnitudes above MAX.
    /// Property-specific sign validation must precede this geometry boundary.
    pub fn to_finite_f32(&self, limits: NumericLimits) -> Result<FiniteF32, ConversionError> {
        let mut budget = Budget::new(limits);
        finite_result(
            Rational::new(&self.numerator, self.denominator, limits, &mut budget)?
                .round(limits, &mut budget)?,
            self.negative,
        )
    }
    /// Selected font-size/border-width boundary, not a universal geometry clamp.
    pub fn to_nonnegative_f32(
        &self,
        limits: NumericLimits,
    ) -> Result<NonnegativeF32, ConversionError> {
        if self.negative {
            return Err(ConversionError::InvalidNegative);
        }
        let mut budget = Budget::new(limits);
        Ok(
            Rational::new(&self.numerator, self.denominator, limits, &mut budget)?
                .round(limits, &mut budget)?,
        )
    }
}
impl Decimal {
    /// Divide exact decimals before one signed nearest/ties-even conversion.
    /// Common decimal scales cancel before expansion. A single work budget covers
    /// all copies, scale expansion and rounding comparisons. Exact magnitudes above
    /// f32::MAX are rejected; this function does not select a property clamp.
    pub fn ratio_finite_f32(
        &self,
        denominator: &Self,
        limits: NumericLimits,
    ) -> Result<FiniteF32, ConversionError> {
        if denominator.is_zero() {
            return Err(ConversionError::ZeroDenominator);
        }
        self.coefficient.check_digits(limits)?;
        denominator.coefficient.check_digits(limits)?;
        if self.scale > limits.digits.get() || denominator.scale > limits.digits.get() {
            return Err(NumericError::DigitLimit.into());
        }
        if self.is_zero() {
            return Ok(FiniteF32 {
                value: 0.0,
                rounded_to_zero: false,
            });
        }
        let mut budget = Budget::new(limits);
        let mut rational = Rational {
            numerator: copy(&self.coefficient, &mut budget)?,
            denominator: copy(&denominator.coefficient, &mut budget)?,
        };
        let (target, power) = if denominator.scale >= self.scale {
            (&mut rational.numerator, denominator.scale - self.scale)
        } else {
            (&mut rational.denominator, self.scale - denominator.scale)
        };
        for _ in 0..power {
            multiply(target, 10, limits, &mut budget)?;
        }
        finite_result(
            rational.round(limits, &mut budget)?,
            self.is_negative() ^ denominator.is_negative(),
        )
    }
    /// Multiply an exact signed coefficient by an explicitly supplied finite
    /// signed context before rounding. The caller selects/validates the basis.
    pub fn scaled_finite_f32(
        &self,
        context: f32,
        limits: NumericLimits,
    ) -> Result<FiniteF32, ConversionError> {
        self.resolve_finite(context, false, limits)
    }
    /// Exact signed percentage product, including division by 100 before rounding.
    pub fn percentage_finite_f32(
        &self,
        context: f32,
        limits: NumericLimits,
    ) -> Result<FiniteF32, ConversionError> {
        self.resolve_finite(context, true, limits)
    }
    fn resolve_finite(
        &self,
        context: f32,
        percentage: bool,
        limits: NumericLimits,
    ) -> Result<FiniteF32, ConversionError> {
        if !context.is_finite() {
            return Err(ConversionError::InvalidFiniteContext);
        }
        resolve_finite_parts(
            &self.coefficient,
            self.scale,
            self.is_negative(),
            context,
            percentage,
            limits,
        )
    }
    /// Resolve an exact coefficient against a supplied f32 context before rounding.
    pub fn scaled_nonnegative_f32(
        &self,
        context: f32,
        limits: NumericLimits,
    ) -> Result<NonnegativeF32, ConversionError> {
        self.resolve_nonnegative(context, false, limits)
    }
    /// Resolve a percentage against a supplied f32 basis, with exact division by 100.
    pub fn percentage_nonnegative_f32(
        &self,
        context: f32,
        limits: NumericLimits,
    ) -> Result<NonnegativeF32, ConversionError> {
        self.resolve_nonnegative(context, true, limits)
    }
    fn resolve_nonnegative(
        &self,
        context: f32,
        percentage: bool,
        limits: NumericLimits,
    ) -> Result<NonnegativeF32, ConversionError> {
        if self.is_negative() {
            return Err(ConversionError::InvalidNegative);
        }
        if !context.is_finite() || context < 0.0 {
            return Err(ConversionError::InvalidContext);
        }
        self.coefficient.check_digits(limits)?;
        if context == 0.0 || self.is_zero() {
            return Ok(result(0, None));
        }
        let mut budget = Budget::new(limits);
        let mut rational = Rational::new(
            &self.coefficient,
            [self.scale, self.scale, 0, 0],
            limits,
            &mut budget,
        )?;
        rational.scale(context, percentage, limits, &mut budget)?;
        Ok(rational.round(limits, &mut budget)?)
    }
}

impl CompactDecimal {
    /// Exact signed percentage product (PCT): the shared exact path of
    /// [`Decimal::percentage_finite_f32`] on the equal decimal, with division by 100
    /// before one nearest/ties-even rounding; zero is +0 and a magnitude above
    /// `f32::MAX` is `Unrepresentable`.
    pub fn percentage_finite_f32(
        self,
        context: f32,
        limits: NumericLimits,
    ) -> Result<FiniteF32, ConversionError> {
        if !context.is_finite() {
            return Err(ConversionError::InvalidFiniteContext);
        }
        resolve_finite_parts(
            &Natural::from_u64(self.coefficient)?,
            self.scale as usize,
            self.negative,
            context,
            true,
            limits,
        )
    }
}

/// The one exact signed resolution path shared by `Decimal` and `CompactDecimal`
/// (PCT): `|coefficient| / 10^scale * |context|` (and `/ 100` for a percentage),
/// rounded once; `context` is finite.
fn resolve_finite_parts(
    coefficient: &Natural,
    scale: usize,
    negative: bool,
    context: f32,
    percentage: bool,
    limits: NumericLimits,
) -> Result<FiniteF32, ConversionError> {
    coefficient.check_digits(limits)?;
    if context == 0.0 || coefficient.is_zero() {
        return Ok(FiniteF32 {
            value: 0.0,
            rounded_to_zero: false,
        });
    }
    let mut budget = Budget::new(limits);
    let mut rational = Rational::new(coefficient, [scale, scale, 0, 0], limits, &mut budget)?;
    rational.scale(context.abs(), percentage, limits, &mut budget)?;
    finite_result(
        rational.round(limits, &mut budget)?,
        negative ^ context.is_sign_negative(),
    )
}

fn finite_result(magnitude: NonnegativeF32, negative: bool) -> Result<FiniteF32, ConversionError> {
    if magnitude.event == Some(RangeEvent::UpperSaturation) {
        return Err(ConversionError::Unrepresentable);
    }
    Ok(FiniteF32 {
        value: if magnitude.value == 0.0 {
            0.0
        } else if negative {
            -magnitude.value
        } else {
            magnitude.value
        },
        rounded_to_zero: magnitude.event == Some(RangeEvent::PositiveRoundedToZero),
    })
}
