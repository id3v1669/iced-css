//! Q284 built-in counter text, without list-marker punctuation or counter scoping.
use super::{Budget, Integer, NumericError, NumericLimits};
use std::num::NonZeroUsize;
#[path = "counter_symbols.rs"]
mod symbols;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterStyle {
    None,
    Disc,
    Circle,
    Square,
    Decimal,
    DecimalLeadingZero,
    LowerRoman,
    UpperRoman,
    LowerLatin,
    UpperLatin,
    LowerGreek,
    Armenian,
    Georgian,
}
impl CounterStyle {
    /// A decoded CSS2 keyword, not raw CSS token syntax. No CSS3 style is admitted.
    pub fn parse(name: &str) -> Option<Self> {
        [
            ("none", Self::None),
            ("disc", Self::Disc),
            ("circle", Self::Circle),
            ("square", Self::Square),
            ("decimal", Self::Decimal),
            ("decimal-leading-zero", Self::DecimalLeadingZero),
            ("lower-roman", Self::LowerRoman),
            ("upper-roman", Self::UpperRoman),
            ("lower-latin", Self::LowerLatin),
            ("lower-alpha", Self::LowerLatin),
            ("upper-latin", Self::UpperLatin),
            ("upper-alpha", Self::UpperLatin),
            ("lower-greek", Self::LowerGreek),
            ("armenian", Self::Armenian),
            ("georgian", Self::Georgian),
        ]
        .into_iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, style)| style)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct CounterLimits {
    pub numeric: NumericLimits,
    pub retained_bytes: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterError {
    Numeric(NumericError),
    Bytes,
    InvalidTable,
}
impl From<NumericError> for CounterError {
    fn from(error: NumericError) -> Self {
        Self::Numeric(error)
    }
}
impl std::fmt::Display for CounterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "counter formatting: {self:?}")
    }
}
impl std::error::Error for CounterError {}
struct Output {
    text: String,
    bytes: usize,
}
impl Output {
    fn new(limits: CounterLimits) -> Self {
        Self {
            text: String::new(),
            bytes: limits.retained_bytes.get(),
        }
    }
    fn reserve(&mut self, additional: usize, budget: &mut Budget) -> Result<(), CounterError> {
        let needed = self
            .text
            .len()
            .checked_add(additional)
            .ok_or(CounterError::Bytes)?;
        if needed > self.bytes {
            return Err(CounterError::Bytes);
        }
        if needed > self.text.capacity() {
            budget.spend(self.text.len())?;
            let target = needed.max(self.text.capacity().saturating_mul(2).min(self.bytes));
            self.text
                .try_reserve_exact(target - self.text.len())
                .map_err(|_| NumericError::Allocation)?;
            if self.text.capacity() > self.bytes {
                return Err(CounterError::Bytes);
            }
        }
        Ok(())
    }
    fn push(&mut self, text: &str, budget: &mut Budget) -> Result<(), CounterError> {
        budget.spend(text.len())?;
        budget.spend(1)?;
        self.reserve(text.len(), budget)?;
        self.text.push_str(text);
        Ok(())
    }
}
fn decimal(
    value: &Integer,
    padded: bool,
    out: &mut Output,
    budget: &mut Budget,
) -> Result<(), CounterError> {
    let limbs = &value.magnitude.0;
    let digits = match limbs.last() {
        None => 1,
        Some(top) => (limbs.len() - 1)
            .checked_mul(9)
            .and_then(|n| n.checked_add(top.ilog10() as usize + 1))
            .ok_or(CounterError::Bytes)?,
    };
    let pad = usize::from(padded && digits == 1);
    let count = digits
        .checked_add(pad)
        .and_then(|n| n.checked_add(usize::from(value.negative)))
        .ok_or(CounterError::Bytes)?;
    budget.spend(limbs.len().checked_mul(9).ok_or(NumericError::WorkLimit)?)?;
    budget.spend(count)?;
    out.reserve(count, budget)?;
    if value.negative {
        out.text.push('-');
    }
    if pad != 0 {
        out.text.push('0');
    }
    if limbs.is_empty() {
        out.text.push('0');
        return Ok(());
    }
    for (index, limb) in limbs.iter().rev().enumerate() {
        let mut number = *limb;
        let mut digits = [b'0'; 9];
        for digit in digits.iter_mut().rev() {
            *digit = b'0' + (number % 10) as u8;
            number /= 10;
        }
        let start = if index == 0 {
            9 - (limb.ilog10() as usize + 1)
        } else {
            0
        };
        for digit in &digits[start..] {
            out.text.push(char::from(*digit));
        }
    }
    Ok(())
}
fn additive(
    mut value: u32,
    table: &[(u32, &str)],
    out: &mut Output,
    budget: &mut Budget,
) -> Result<(), CounterError> {
    for &(weight, symbol) in table {
        budget.spend(1)?;
        if weight == 0 {
            return Err(CounterError::InvalidTable);
        }
        let copies = value / weight;
        value %= weight;
        for _ in 0..copies {
            out.push(symbol, budget)?;
        }
        if value == 0 {
            return Ok(());
        }
    }
    Err(CounterError::InvalidTable)
}
fn emit(
    value: &Integer,
    style: CounterStyle,
    limits: CounterLimits,
    out: &mut Output,
    budget: &mut Budget,
) -> Result<(), CounterError> {
    budget.spend(1)?;
    value.magnitude.check_digits(limits.numeric)?;
    match style {
        CounterStyle::None => return Ok(()),
        CounterStyle::Disc => return out.push("•", budget),
        CounterStyle::Circle => return out.push("◦", budget),
        CounterStyle::Square => return out.push("▪", budget),
        CounterStyle::Decimal => return decimal(value, false, out, budget),
        CounterStyle::DecimalLeadingZero => return decimal(value, true, out, budget),
        _ => {}
    }
    // Every selected nondecimal numeric range is below one base10^9 limb.
    let number = if value.negative || value.magnitude.0.len() != 1 {
        return decimal(value, false, out, budget);
    } else {
        value.magnitude.0[0]
    };
    match style {
        CounterStyle::LowerLatin | CounterStyle::UpperLatin if number <= 26 => {
            let base = if style == CounterStyle::LowerLatin {
                b'a'
            } else {
                b'A'
            };
            let symbol = [base + (number - 1) as u8];
            budget.spend(1)?;
            out.reserve(1, budget)?;
            out.text.push(char::from(symbol[0]));
            Ok(())
        }
        CounterStyle::LowerGreek if number <= 24 => {
            out.push(symbols::LOWER_GREEK[number as usize - 1], budget)
        }
        CounterStyle::LowerRoman if number <= 3999 => {
            additive(number, symbols::LOWER_ROMAN, out, budget)
        }
        CounterStyle::UpperRoman if number <= 3999 => {
            additive(number, symbols::UPPER_ROMAN, out, budget)
        }
        CounterStyle::Armenian if number <= 9999 => {
            additive(number, symbols::ARMENIAN, out, budget)
        }
        CounterStyle::Georgian if number <= 19999 => {
            additive(number, symbols::GEORGIAN, out, budget)
        }
        _ => decimal(value, false, out, budget),
    }
}
/// Counter-function text. List marker punctuation/gap is added by its own owner.
pub fn format_counter(
    value: &Integer,
    style: CounterStyle,
    limits: CounterLimits,
) -> Result<String, CounterError> {
    let mut budget = Budget::new(limits.numeric);
    let mut out = Output::new(limits);
    emit(value, style, limits, &mut out, &mut budget)?;
    Ok(out.text)
}
/// Join explicitly supplied scopes, preserving separators even for `none`.
/// The scope owner supplies implicit zero; an empty slice joins to empty text.
pub fn format_counters(
    values: &[Integer],
    separator: &str,
    style: CounterStyle,
    limits: CounterLimits,
) -> Result<String, CounterError> {
    format_iter(
        values,
        separator,
        style,
        limits,
        &mut Budget::new(limits.numeric),
    )
}
pub(super) fn format_iter<'a>(
    values: impl IntoIterator<Item = &'a Integer>,
    separator: &str,
    style: CounterStyle,
    limits: CounterLimits,
    budget: &mut Budget,
) -> Result<String, CounterError> {
    let mut out = Output::new(limits);
    for (index, value) in values.into_iter().enumerate() {
        if index != 0 {
            out.push(separator, budget)?;
        }
        emit(value, style, limits, &mut out, budget)?;
    }
    Ok(out.text)
}
