//! Exact decimal pair arithmetic with one budget across all intermediate work.
use super::{BASE, Budget, Decimal, Integer, Natural, NumericError, NumericLimits};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecimalArithmeticError {
    NonfiniteProgress,
    Numeric(NumericError),
}
impl From<NumericError> for DecimalArithmeticError {
    fn from(e: NumericError) -> Self {
        Self::Numeric(e)
    }
}
impl std::fmt::Display for DecimalArithmeticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "decimal combination: {self:?}")
    }
}
impl std::error::Error for DecimalArithmeticError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecimalScalingError {
    NonfiniteValue,
    Numeric(NumericError),
}
impl From<NumericError> for DecimalScalingError {
    fn from(error: NumericError) -> Self {
        Self::Numeric(error)
    }
}
impl std::fmt::Display for DecimalScalingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exact decimal scaling: {self:?}")
    }
}
impl std::error::Error for DecimalScalingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Numeric(e) => Some(e),
            _ => None,
        }
    }
}
impl Decimal {
    /// Bounded exact copying from an integer, without text or floating conversion.
    pub fn from_integer(value: &Integer, limits: NumericLimits) -> Result<Self, NumericError> {
        value.magnitude.check_digits(limits)?;
        let mut budget = Budget::new(limits);
        budget.spend(1)?;
        budget.spend(value.magnitude.0.len())?;
        let mut words = Vec::new();
        words
            .try_reserve_exact(value.magnitude.0.len())
            .map_err(|_| NumericError::Allocation)?;
        words.extend_from_slice(&value.magnitude.0);
        Ok(Self {
            negative: value.is_negative(),
            coefficient: Natural(words),
            scale: 0,
        })
    }

    /// Canonical exact zero, with no coefficient allocation.
    pub const fn zero() -> Self {
        Self {
            negative: false,
            coefficient: Natural(Vec::new()),
            scale: 0,
        }
    }
    /// Exact percentage product, including division by100 before any rounding.
    /// One work budget covers scaling, temporary scale admission and normalization.
    pub fn checked_percentage_of_f32(
        &self,
        basis: f32,
        limits: NumericLimits,
    ) -> Result<Self, DecimalScalingError> {
        if !basis.is_finite() {
            return Err(DecimalScalingError::NonfiniteValue);
        }
        let mut budget = Budget::new(limits);
        let mut value = scaled(self, basis, limits, &mut budget)?;
        if value.is_zero() {
            return Ok(value);
        }
        value.scale = value.scale.checked_add(2).ok_or(NumericError::DigitLimit)?;
        check(&value, limits)?;
        Ok(normalize(value, &mut budget)?)
    }
    /// Preserve the exact finite binary32 value, not a shortest decimal display.
    /// One explicit budget covers the fallible unit coefficient and scaling.
    pub fn from_finite_f32(value: f32, limits: NumericLimits) -> Result<Self, DecimalScalingError> {
        if !value.is_finite() {
            return Err(DecimalScalingError::NonfiniteValue);
        }
        let mut budget = Budget::new(limits);
        let one = Self {
            negative: false,
            coefficient: Natural::from_digits(b"1", &mut budget)?,
            scale: 0,
        };
        Ok(scaled(&one, value, limits, &mut budget)?)
    }
    /// Multiply without converting the decimal operand to f32 or rounding the
    /// product. Inputs remain unchanged on numeric resource failure.
    pub fn checked_scale_f32(
        &self,
        factor: f32,
        limits: NumericLimits,
    ) -> Result<Self, DecimalScalingError> {
        if !factor.is_finite() {
            return Err(DecimalScalingError::NonfiniteValue);
        }
        Ok(scaled(self, factor, limits, &mut Budget::new(limits))?)
    }
}
fn check(value: &Decimal, limits: NumericLimits) -> Result<(), NumericError> {
    value.coefficient.check_digits(limits)?;
    if value.scale > limits.digits.get() {
        return Err(NumericError::DigitLimit);
    }
    Ok(())
}
fn copy(
    value: &Decimal,
    limits: NumericLimits,
    budget: &mut Budget,
) -> Result<Decimal, NumericError> {
    check(value, limits)?;
    budget.spend(1)?;
    budget.spend(value.coefficient.0.len())?;
    let mut words = Vec::new();
    words
        .try_reserve_exact(value.coefficient.0.len())
        .map_err(|_| NumericError::Allocation)?;
    words.extend_from_slice(&value.coefficient.0);
    Ok(Decimal {
        negative: value.is_negative(),
        coefficient: Natural(words),
        scale: value.scale,
    })
}
fn normalize(mut value: Decimal, budget: &mut Budget) -> Result<Decimal, NumericError> {
    budget.spend(1)?;
    if value.coefficient.is_zero() {
        value.negative = false;
        value.scale = 0;
        return Ok(value);
    }
    while value.scale > 0 && value.coefficient.remainder(10, budget)? == 0 {
        value.coefficient.divide_exact(10, budget)?;
        value.scale -= 1;
    }
    Ok(value)
}
/// All callers use factors below2^24; u64 holds (BASE-1)*factor+carry.
fn multiply(
    value: &mut Decimal,
    factor: u32,
    limits: NumericLimits,
    budget: &mut Budget,
) -> Result<(), NumericError> {
    budget.spend(1)?;
    budget.spend(value.coefficient.0.len())?;
    let mut carry = 0u64;
    for word in &mut value.coefficient.0 {
        let product = u64::from(*word) * u64::from(factor) + carry;
        *word = (product % BASE) as u32;
        carry = product / BASE;
    }
    if carry != 0 {
        if value.coefficient.0.len() == limits.digits.get().div_ceil(9) {
            return Err(NumericError::DigitLimit);
        }
        value
            .coefficient
            .0
            .try_reserve_exact(1)
            .map_err(|_| NumericError::Allocation)?;
        value.coefficient.0.push(carry as u32);
    }
    value.coefficient.check_digits(limits)
}
fn scaled(
    value: &Decimal,
    factor: f32,
    limits: NumericLimits,
    budget: &mut Budget,
) -> Result<Decimal, NumericError> {
    let mut result = copy(value, limits, budget)?;
    let bits = factor.to_bits();
    let exp = (bits >> 23) & 255;
    let mantissa = (bits & 0x7fffff) | if exp == 0 { 0 } else { 1 << 23 };
    if mantissa == 0 || result.coefficient.is_zero() {
        result.coefficient.0.clear();
        result.scale = 0;
        result.negative = false;
        return Ok(result);
    }
    result.negative ^= bits >> 31 != 0;
    // Remove exact powers of two before growing a decimal coefficient/scale.
    // For example, 1.0 is 1*2^0, not a required 8388608*2^-23 intermediate.
    let twos = mantissa.trailing_zeros();
    multiply(&mut result, mantissa >> twos, limits, budget)?;
    let exponent = if exp == 0 { -149 } else { exp as i32 - 150 } + twos as i32;
    if exponent < 0 {
        result.scale = result
            .scale
            .checked_add(exponent.unsigned_abs() as usize)
            .ok_or(NumericError::DigitLimit)?;
        check(&result, limits)?;
        for _ in 0..exponent.unsigned_abs() {
            multiply(&mut result, 5, limits, budget)?;
        }
    } else {
        for _ in 0..exponent {
            multiply(&mut result, 2, limits, budget)?;
        }
    }
    normalize(result, budget)
}
fn sum(
    a: &Decimal,
    b: &Decimal,
    limits: NumericLimits,
    budget: &mut Budget,
) -> Result<Decimal, NumericError> {
    let mut a = copy(a, limits, budget)?;
    let mut b = copy(b, limits, budget)?;
    let scale = a.scale.max(b.scale);
    for value in [&mut a, &mut b] {
        for _ in value.scale..scale {
            multiply(value, 10, limits, budget)?;
        }
        value.scale = scale;
    }
    let a = Integer {
        negative: a.negative,
        magnitude: a.coefficient,
    };
    let b = Integer {
        negative: b.negative,
        magnitude: b.coefficient,
    };
    let result = a.add_with_budget(&b, limits, budget)?;
    normalize(
        Decimal {
            negative: result.negative,
            coefficient: result.magnitude,
            scale,
        },
        budget,
    )
}
impl Decimal {
    /// Exact addition without mutating operands. Coefficients and scales of both
    /// inputs and every intermediate must fit the supplied digit/work limits.
    pub fn checked_add(&self, other: &Self, limits: NumericLimits) -> Result<Self, NumericError> {
        sum(self, other, limits, &mut Budget::new(limits))
    }
    /// Evaluate (1-p)*a+p*b exactly for the supplied finite binary32 progress.
    /// Endpoints remain exact decimals; no f32 endpoint conversion or range clamp
    /// occurs. This is arithmetic, not property eligibility or final range policy.
    pub fn checked_interpolate(
        &self,
        other: &Self,
        progress: f32,
        limits: NumericLimits,
    ) -> Result<Self, DecimalArithmeticError> {
        if !progress.is_finite() {
            return Err(DecimalArithmeticError::NonfiniteProgress);
        }
        Ok(interpolate(
            self,
            other,
            progress,
            limits,
            &mut Budget::new(limits),
        )?)
    }
}

fn interpolate(
    a: &Decimal,
    b: &Decimal,
    progress: f32,
    limits: NumericLimits,
    budget: &mut Budget,
) -> Result<Decimal, NumericError> {
    check(a, limits)?;
    check(b, limits)?;
    let mut ap = scaled(a, progress, limits, budget)?;
    if !ap.is_zero() {
        ap.negative = !ap.negative;
    }
    let bp = scaled(b, progress, limits, budget)?;
    let partial = sum(a, &ap, limits, budget)?;
    sum(&partial, &bp, limits, budget)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerInterpolationError {
    NonfiniteProgress,
    Numeric(NumericError),
}
impl From<NumericError> for IntegerInterpolationError {
    fn from(e: NumericError) -> Self {
        Self::Numeric(e)
    }
}
impl std::fmt::Display for IntegerInterpolationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "integer interpolation: {self:?}")
    }
}
impl std::error::Error for IntegerInterpolationError {}
fn from_integer(
    value: &Integer,
    limits: NumericLimits,
    budget: &mut Budget,
) -> Result<Decimal, NumericError> {
    value.magnitude.check_digits(limits)?;
    budget.spend(1)?;
    budget.spend(value.magnitude.0.len())?;
    let mut words = Vec::new();
    words
        .try_reserve_exact(value.magnitude.0.len())
        .map_err(|_| NumericError::Allocation)?;
    words.extend_from_slice(&value.magnitude.0);
    Ok(Decimal {
        negative: value.negative,
        coefficient: Natural(words),
        scale: 0,
    })
}
fn round_integer(
    value: Decimal,
    limits: NumericLimits,
    budget: &mut Budget,
) -> Result<Integer, NumericError> {
    check(&value, limits)?;
    budget.spend(1)?;
    let mut magnitude = value.coefficient;
    let mut last = 0u32;
    let mut sticky = false;
    for _ in 0..value.scale {
        budget.spend(1)?;
        if magnitude.is_zero() {
            last = 0;
            break;
        }
        budget.spend(magnitude.0.len())?;
        let mut remainder = 0u64;
        for word in magnitude.0.iter_mut().rev() {
            let number = remainder * BASE + u64::from(*word);
            *word = (number / 10) as u32;
            remainder = number % 10;
        }
        while magnitude.0.last() == Some(&0) {
            budget.spend(1)?;
            magnitude.0.pop();
        }
        sticky |= last != 0;
        last = remainder as u32;
    }
    let increment = last > 5 || (last == 5 && (!value.negative || sticky));
    if increment {
        budget.spend(
            magnitude
                .0
                .len()
                .checked_add(1)
                .ok_or(NumericError::DigitLimit)?,
        )?;
        let mut carry = true;
        for word in &mut magnitude.0 {
            if *word < BASE as u32 - 1 {
                *word += 1;
                carry = false;
                break;
            }
            *word = 0;
        }
        if carry {
            if magnitude.0.len() == limits.digits.get().div_ceil(9) {
                return Err(NumericError::DigitLimit);
            }
            magnitude
                .0
                .try_reserve_exact(1)
                .map_err(|_| NumericError::Allocation)?;
            magnitude.0.push(1);
        }
    }
    magnitude.check_digits(limits)?;
    Ok(Integer {
        negative: value.negative && !magnitude.is_zero(),
        magnitude,
    })
}
impl Integer {
    /// Exact real-number interpolation followed by nearest-integer rounding,
    /// with halfway values rounded toward positive infinity. One work budget
    /// covers endpoint copies, intermediate arithmetic and final rounding.
    pub fn checked_interpolate(
        &self,
        other: &Self,
        progress: f32,
        limits: NumericLimits,
    ) -> Result<Self, IntegerInterpolationError> {
        if !progress.is_finite() {
            return Err(IntegerInterpolationError::NonfiniteProgress);
        }
        let mut budget = Budget::new(limits);
        let a = from_integer(self, limits, &mut budget)?;
        let b = from_integer(other, limits, &mut budget)?;
        let value = interpolate(&a, &b, progress, limits, &mut budget)?;
        Ok(round_integer(value, limits, &mut budget)?)
    }
}
