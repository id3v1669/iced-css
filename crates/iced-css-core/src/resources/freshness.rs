//! RFC9111 explicit private-cache time arithmetic, not storage/reuse authorization.
use super::{
    age::{AgeField, DeltaSeconds},
    cache_restrictions::MaxAge,
};
/// Caller-admitted UTC whole-second conversion. This module does not parse dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheDate {
    Absent,
    ValidUtcSeconds(i64),
    Invalid,
}
#[derive(Debug, Clone, Copy)]
pub struct CacheAgeInputs {
    pub request_time: i64,
    pub response_time: i64,
    pub now: i64,
    pub date: CacheDate,
    pub age: AgeField,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheTimeError {
    InvalidDate,
    ClockOrder,
}
impl std::fmt::Display for CacheTimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP cache time: {self:?}")
    }
}
impl std::error::Error for CacheTimeError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalculatedCacheAge {
    pub apparent_age: DeltaSeconds,
    pub corrected_age_value: DeltaSeconds,
    pub corrected_initial_age: DeltaSeconds,
    pub resident_time: DeltaSeconds,
    pub current_age: DeltaSeconds,
}
fn elapsed(later: i64, earlier: i64) -> DeltaSeconds {
    let difference = (i128::from(later) - i128::from(earlier)).max(0);
    DeltaSeconds::from_seconds(u64::try_from(difference).unwrap_or(u64::MAX))
}
fn date_or_received(date: CacheDate, received: i64) -> Result<i64, CacheTimeError> {
    match date {
        CacheDate::ValidUtcSeconds(value) => Ok(value),
        CacheDate::Absent => Ok(received),
        CacheDate::Invalid => Err(CacheTimeError::InvalidDate),
    }
}
/// Conservative RFC9111 §4.2.3 maximum, with explicit ordered local clock samples.
/// Absent/ignored Age contributes zero without rewriting its original evidence.
/// Missing Date uses reception time; a cache owner must still record that Date.
pub fn calculate_cache_age(input: CacheAgeInputs) -> Result<CalculatedCacheAge, CacheTimeError> {
    if input.request_time > input.response_time || input.response_time > input.now {
        return Err(CacheTimeError::ClockOrder);
    }
    let date = date_or_received(input.date, input.response_time)?;
    let apparent_age = elapsed(input.response_time, date);
    let age = match input.age {
        AgeField::Seconds { value, .. } => value,
        AgeField::Absent | AgeField::Ignored { .. } => DeltaSeconds::from_seconds(0),
    };
    let corrected_age_value = age.saturating_add(elapsed(input.response_time, input.request_time));
    let corrected_initial_age = apparent_age.max(corrected_age_value);
    let resident_time = elapsed(input.now, input.response_time);
    Ok(CalculatedCacheAge {
        apparent_age,
        corrected_age_value,
        corrected_initial_age,
        resident_time,
        current_age: corrected_initial_age.saturating_add(resident_time),
    })
}
/// Invalid freshness evidence is treated as already stale; Unspecified is not a
/// fabricated heuristic lifetime. Fresh alone never grants storage or reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplicitFreshness {
    Fresh {
        lifetime: DeltaSeconds,
        remaining: DeltaSeconds,
    },
    Stale {
        lifetime: DeltaSeconds,
        overdue: DeltaSeconds,
    },
    InvalidMaxAge,
    InvalidExpires,
    Unspecified,
}
/// RFC9111 §4.2.1 private-cache precedence and §4.2 strict age comparison.
/// Date is needed only for Expires. No no-cache/no-store, Vary, request directive,
/// status/method or validation decision is performed by this arithmetic helper.
pub fn evaluate_explicit_freshness(
    max_age: MaxAge,
    expires: CacheDate,
    date: CacheDate,
    response_time: i64,
    current_age: DeltaSeconds,
) -> Result<ExplicitFreshness, CacheTimeError> {
    let lifetime = match max_age {
        MaxAge::Seconds(value) => value,
        MaxAge::Invalid { .. } => return Ok(ExplicitFreshness::InvalidMaxAge),
        MaxAge::Absent => match expires {
            CacheDate::Absent => return Ok(ExplicitFreshness::Unspecified),
            CacheDate::Invalid => return Ok(ExplicitFreshness::InvalidExpires),
            CacheDate::ValidUtcSeconds(value) => {
                elapsed(value, date_or_received(date, response_time)?)
            }
        },
    };
    Ok(if lifetime > current_age {
        ExplicitFreshness::Fresh {
            lifetime,
            remaining: lifetime.elapsed_since(current_age),
        }
    } else {
        ExplicitFreshness::Stale {
            lifetime,
            overdue: current_age.elapsed_since(lifetime),
        }
    })
}
