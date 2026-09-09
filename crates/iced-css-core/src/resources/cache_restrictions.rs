//! Conservative private-cache restrictions. These never grant storage or reuse:
//! status/method/auth eligibility, dates, Vary and validation remain separate.
use super::{
    age::{AgeLimits, DeltaSeconds, parse_delta_seconds},
    cache_control::CacheControl,
};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct CacheEvaluationLimits {
    pub directives: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheEvaluationError {
    Directives,
    Work,
}
impl std::fmt::Display for CacheEvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "private cache restrictions: {self:?}")
    }
}
impl std::error::Error for CacheEvaluationError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxAge {
    Absent,
    Seconds(DeltaSeconds),
    /// Select the RFC9111 4.2.1 stale policy for duplicates/malformed values.
    Invalid {
        first: usize,
        second: Option<usize>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivateCacheRestrictions {
    pub no_store: bool,
    pub validate_before_reuse: bool,
    pub validate_when_stale: bool,
    pub max_age: MaxAge,
    /// First no-store/must-revalidate with an argument; still honored conservatively.
    pub malformed_restriction: Option<usize>,
}
fn spend(work: &mut usize, n: usize) -> Result<(), CacheEvaluationError> {
    *work = work.checked_sub(n).ok_or(CacheEvaluationError::Work)?;
    Ok(())
}
/// The no-cache field-removal optimization and must-understand override of
/// no-store are deliberately not implemented; the approved architecture applies
/// these restrictions to the whole response. Neither flag authorizes web refresh.
pub fn private_cache_restrictions(
    control: &CacheControl<'_>,
    limits: CacheEvaluationLimits,
) -> Result<PrivateCacheRestrictions, CacheEvaluationError> {
    if control.directives().len() > limits.directives.get() {
        return Err(CacheEvaluationError::Directives);
    }
    let mut work = limits.work.get();
    let mut first_max_age = None;
    let mut result = PrivateCacheRestrictions {
        no_store: false,
        validate_before_reuse: false,
        validate_when_stale: false,
        max_age: MaxAge::Absent,
        malformed_restriction: None,
    };
    for (index, directive) in control.directives().iter().enumerate() {
        let name = directive.name();
        spend(
            &mut work,
            name.len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(1))
                .ok_or(CacheEvaluationError::Work)?,
        )?;
        if name.eq_ignore_ascii_case(b"no-store") {
            result.no_store = true;
            if directive.argument().is_some() {
                result.malformed_restriction.get_or_insert(index);
            }
        } else if name.eq_ignore_ascii_case(b"no-cache") {
            result.validate_before_reuse = true;
        } else if name.eq_ignore_ascii_case(b"must-revalidate") {
            result.validate_when_stale = true;
            if directive.argument().is_some() {
                result.malformed_restriction.get_or_insert(index);
            }
        } else if name.eq_ignore_ascii_case(b"max-age") {
            if let Some(first) = first_max_age {
                if !matches!(
                    result.max_age,
                    MaxAge::Invalid {
                        second: Some(_),
                        ..
                    }
                ) {
                    result.max_age = MaxAge::Invalid {
                        first,
                        second: Some(index),
                    };
                }
                continue;
            }
            first_max_age = Some(index);
            result.max_age = match directive.argument() {
                None => MaxAge::Invalid {
                    first: index,
                    second: None,
                },
                Some(argument) => {
                    let cost = argument
                        .len()
                        .checked_add(1)
                        .ok_or(CacheEvaluationError::Work)?;
                    spend(&mut work, cost)?;
                    // Nonzero by construction; avoids a second independent work allowance.
                    let allowance = NonZeroUsize::new(cost).ok_or(CacheEvaluationError::Work)?;
                    match parse_delta_seconds(
                        argument,
                        AgeLimits {
                            bytes: allowance,
                            work: allowance,
                        },
                    ) {
                        Ok(value) => MaxAge::Seconds(value),
                        Err(super::age::AgeError::Invalid { .. }) => MaxAge::Invalid {
                            first: index,
                            second: None,
                        },
                        Err(_) => return Err(CacheEvaluationError::Work),
                    }
                }
            };
        }
    }
    Ok(result)
}
