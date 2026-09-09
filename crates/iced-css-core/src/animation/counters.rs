//! Bounded counter directive list combination, without repeatable-list expansion.
use super::{
    combine::{Combination, Endpoint, discrete_choice},
    integer::combine_integer,
};
use crate::values::{IntegerInterpolationError, NumericLimits, property::CounterDirective};
use std::{borrow::Cow, num::NonZeroUsize};
#[derive(Debug, Clone, Copy)]
pub struct CounterCombinationLimits {
    pub entries: NonZeroUsize,
    /// Output vector plus copied String capacities, excluding integer coefficients.
    pub retained_bytes: NonZeroUsize,
    /// One unit per compared directive pair and per compared name byte.
    pub comparison_work: NonZeroUsize,
    /// Applied independently to each combined integer, including its allocation.
    pub numeric: NumericLimits,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterCombinationError {
    NonfiniteProgress,
    Entries,
    Bytes,
    Work,
    Allocation,
    Integer(IntegerInterpolationError),
}
impl std::fmt::Display for CounterCombinationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "counter list animation: {self:?}")
    }
}
impl std::error::Error for CounterCombinationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Integer(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub struct CounterDirectiveSample<'a> {
    directives: Cow<'a, [CounterDirective]>,
    retained_bytes: usize,
}
impl CounterDirectiveSample<'_> {
    pub fn directives(&self) -> &[CounterDirective] {
        &self.directives
    }
    /// Vector/String capacities owned by this sample; integer heaps use their
    /// separate numeric limits. Borrowed endpoint results return zero.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}
fn spend(work: &mut usize) -> Result<(), CounterCombinationError> {
    *work = work.checked_sub(1).ok_or(CounterCombinationError::Work)?;
    Ok(())
}
/// Endpoints must belong to the same resolved counter property. Names must have
/// already been admitted/decoded; empty slices denote no directives here.
/// Count/name mismatch selects the whole input before any integer arithmetic.
/// Errors drop the candidate and never mutate either endpoint.
///
/// ```compile_fail
/// # use iced_css_core::{animation::{combine::Combination,counters::*},values::property::CounterDirective};
/// fn escape(a:&[CounterDirective],b:&[CounterDirective],l:CounterCombinationLimits)->CounterDirectiveSample<'static> {
///     combine_counter_directives(a,b,Combination::Add,l).unwrap()
/// }
/// ```
pub fn combine_counter_directives<'a>(
    start: &'a [CounterDirective],
    end: &'a [CounterDirective],
    operation: Combination,
    limits: CounterCombinationLimits,
) -> Result<CounterDirectiveSample<'a>, CounterCombinationError> {
    let selected =
        discrete_choice(operation).map_err(|_| CounterCombinationError::NonfiniteProgress)?;
    if start.len() > limits.entries.get() || end.len() > limits.entries.get() {
        return Err(CounterCombinationError::Entries);
    }
    let mut compatible = start.len() == end.len();
    let mut work = limits.comparison_work.get();
    if compatible {
        for (a, b) in start.iter().zip(end) {
            spend(&mut work)?;
            if a.name.len() != b.name.len() {
                compatible = false;
                break;
            }
            for (a, b) in a.name.bytes().zip(b.name.bytes()) {
                spend(&mut work)?;
                if a != b {
                    compatible = false;
                    break;
                }
            }
            if !compatible {
                break;
            }
        }
    }
    if !compatible || start.is_empty() {
        return Ok(CounterDirectiveSample {
            directives: Cow::Borrowed(match selected {
                Endpoint::Start => start,
                Endpoint::End => end,
            }),
            retained_bytes: 0,
        });
    }
    let maximum = limits.retained_bytes.get();
    start
        .len()
        .checked_mul(size_of::<CounterDirective>())
        .filter(|n| *n <= maximum)
        .ok_or(CounterCombinationError::Bytes)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(start.len())
        .map_err(|_| CounterCombinationError::Allocation)?;
    let mut bytes = result
        .capacity()
        .checked_mul(size_of::<CounterDirective>())
        .filter(|n| *n <= maximum)
        .ok_or(CounterCombinationError::Bytes)?;
    for (a, b) in start.iter().zip(end) {
        let available = maximum - bytes;
        if a.name.len() > available {
            return Err(CounterCombinationError::Bytes);
        }
        let mut name = String::new();
        name.try_reserve_exact(a.name.len())
            .map_err(|_| CounterCombinationError::Allocation)?;
        if name.capacity() > available {
            return Err(CounterCombinationError::Bytes);
        }
        bytes += name.capacity();
        name.push_str(&a.name);
        let value = combine_integer(&a.value, &b.value, operation, limits.numeric)
            .map_err(CounterCombinationError::Integer)?;
        result.push(CounterDirective { name, value });
    }
    Ok(CounterDirectiveSample {
        directives: Cow::Owned(result),
        retained_bytes: bytes,
    })
}
