//! Allocation-free exact comparisons with explicit per-call work limits.
use super::{Budget, Decimal, Integer, Natural, NumericError, NumericLimits};
use std::cmp::Ordering;
impl Natural {
    pub(super) fn decimal_digits(&self) -> Result<usize, NumericError> {
        match self.0.last() {
            None => Ok(1),
            Some(top) => (self.0.len() - 1)
                .checked_mul(9)
                .and_then(|n| n.checked_add(top.ilog10() as usize + 1))
                .ok_or(NumericError::DigitLimit),
        }
    }
    /// Index from the most significant decimal digit; absent suffix digits are0.
    fn decimal_digit(&self, index: usize, digits: usize) -> u32 {
        if index >= digits || self.is_zero() {
            return 0;
        }
        let place = digits - 1 - index;
        (self.0[place / 9] / 10_u32.pow((place % 9) as u32)) % 10
    }
}
impl Integer {
    /// Exact signed ordering without subtraction or machine-integer narrowing.
    /// Both operands must fit the caller's digit limit. One unit admits the
    /// comparison, then each compared magnitude limb costs one work unit.
    pub fn checked_cmp(
        &self,
        other: &Self,
        limits: NumericLimits,
    ) -> Result<Ordering, NumericError> {
        self.magnitude.check_digits(limits)?;
        other.magnitude.check_digits(limits)?;
        let mut budget = Budget::new(limits);
        budget.spend(1)?;
        if self.is_negative() != other.is_negative() {
            return Ok(if self.is_negative() {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        let mut order = self.magnitude.0.len().cmp(&other.magnitude.0.len());
        if order == Ordering::Equal {
            for (a, b) in self
                .magnitude
                .0
                .iter()
                .rev()
                .zip(other.magnitude.0.iter().rev())
            {
                budget.spend(1)?;
                order = a.cmp(b);
                if order != Ordering::Equal {
                    break;
                }
            }
        }
        Ok(if self.is_negative() {
            order.reverse()
        } else {
            order
        })
    }
}
fn leading_position(da: usize, sa: usize, db: usize, sb: usize) -> Ordering {
    match (da.checked_sub(sa), db.checked_sub(sb)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => (sb - db).cmp(&(sa - da)),
    }
}
impl Decimal {
    /// Compare signs, leading decimal positions and significant digits, treating
    /// missing coefficient suffixes as zeros. No scale-alignment allocation or
    /// float conversion occurs. Work is one admission plus each compared digit.
    pub fn checked_cmp(
        &self,
        other: &Self,
        limits: NumericLimits,
    ) -> Result<Ordering, NumericError> {
        self.coefficient.check_digits(limits)?;
        other.coefficient.check_digits(limits)?;
        if self.scale > limits.digits.get() || other.scale > limits.digits.get() {
            return Err(NumericError::DigitLimit);
        }
        let mut budget = Budget::new(limits);
        budget.spend(1)?;
        if self.is_zero() && other.is_zero() {
            return Ok(Ordering::Equal);
        }
        if self.is_negative() != other.is_negative() {
            return Ok(if self.is_negative() {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        if self.is_zero() {
            return Ok(Ordering::Less);
        }
        if other.is_zero() {
            return Ok(Ordering::Greater);
        }
        let da = self.coefficient.decimal_digits()?;
        let db = other.coefficient.decimal_digits()?;
        let mut order = leading_position(da, self.scale, db, other.scale);
        if order == Ordering::Equal {
            for i in 0..da.max(db) {
                budget.spend(1)?;
                order = self
                    .coefficient
                    .decimal_digit(i, da)
                    .cmp(&other.coefficient.decimal_digit(i, db));
                if order != Ordering::Equal {
                    break;
                }
            }
        }
        Ok(if self.is_negative() {
            order.reverse()
        } else {
            order
        })
    }
}
