//! RFC9111 §4.3.4 selection over an already eligible cache candidate set.
//! Does not qualify dates/validator strength, merge headers or authorize reuse.
use super::entity_tag::{EntityTag, EntityTagError};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatorStrength {
    Strong,
    Weak,
}
/// Caller-qualified RFC9110 §8.8.2.2 strength and UTC conversion. This component
/// cannot establish a common clock or infer strength from a time difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastModifiedValidator {
    pub utc_seconds: i64,
    pub strength: ValidatorStrength,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct ResponseValidators<'a> {
    pub entity_tag: Option<EntityTag<'a>>,
    pub last_modified: Option<LastModifiedValidator>,
}
impl ResponseValidators<'_> {
    fn has_strong(self) -> bool {
        self.entity_tag.is_some_and(|tag| !tag.is_weak())
            || self
                .last_modified
                .is_some_and(|date| date.strength == ValidatorStrength::Strong)
    }
    fn is_empty(self) -> bool {
        self.entity_tag.is_none() && self.last_modified.is_none()
    }
}
#[derive(Debug, Clone, Copy)]
pub struct ValidationCandidate<'a> {
    pub validators: ResponseValidators<'a>,
    /// Admitted response-Date ordering, not receipt order or a local generation.
    pub date: Option<i64>,
}
#[derive(Debug, Clone, Copy)]
pub struct ValidationSelectionLimits {
    pub entries: NonZeroUsize,
    pub index_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSelectionError {
    Entries,
    Bytes,
    Work,
    Allocation,
    MissingDate { candidate: usize },
    EntityTag(EntityTagError),
}
impl std::fmt::Display for ValidationSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "304 selection: {self:?}")
    }
}
impl std::error::Error for ValidationSelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EntityTag(error) => Some(error),
            _ => None,
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum ValidationSelection {
    None,
    Strong(Vec<usize>),
    Weak(usize),
    WithoutValidator(usize),
}
fn spend(work: &mut usize, count: usize) -> Result<(), ValidationSelectionError> {
    *work = work
        .checked_sub(count)
        .ok_or(ValidationSelectionError::Work)?;
    Ok(())
}
fn matches(
    received: ResponseValidators<'_>,
    stored: ResponseValidators<'_>,
    strong: bool,
    work: &mut usize,
) -> Result<bool, ValidationSelectionError> {
    spend(work, 3)?;
    let tag_match = match (received.entity_tag, stored.entity_tag) {
        (Some(new), Some(old)) if !strong || !new.is_weak() => {
            let cost = new
                .opaque()
                .len()
                .min(old.opaque().len())
                .checked_add(1)
                .ok_or(ValidationSelectionError::Work)?;
            spend(work, cost)?;
            let budget = NonZeroUsize::new(cost).ok_or(ValidationSelectionError::Work)?;
            if strong {
                new.strong_eq(old, budget)
            } else {
                new.weak_eq(old, budget)
            }
            .map_err(ValidationSelectionError::EntityTag)?
        }
        _ => false,
    };
    let date_match = match (received.last_modified, stored.last_modified) {
        (Some(new), Some(old)) => {
            new.utc_seconds == old.utc_seconds
                && (!strong
                    || (new.strength == ValidatorStrength::Strong
                        && old.strength == ValidatorStrength::Strong))
        }
        _ => false,
    };
    Ok(tag_match || date_match)
}
fn push_index(
    selected: &mut Vec<usize>,
    index: usize,
    limits: ValidationSelectionLimits,
    work: &mut usize,
) -> Result<(), ValidationSelectionError> {
    let maximum = limits
        .entries
        .get()
        .min(limits.index_bytes.get() / size_of::<usize>());
    if selected.len() == selected.capacity() {
        let old = selected.capacity();
        if old >= maximum {
            return Err(ValidationSelectionError::Bytes);
        }
        let next = old.saturating_mul(2).max(1).min(maximum);
        spend(
            work,
            selected
                .len()
                .checked_mul(size_of::<usize>())
                .ok_or(ValidationSelectionError::Work)?,
        )?;
        selected
            .try_reserve_exact(next - selected.len())
            .map_err(|_| ValidationSelectionError::Allocation)?;
        if selected.capacity() > maximum {
            return Err(ValidationSelectionError::Bytes);
        }
    }
    spend(work, 1)?;
    selected.push(index);
    Ok(())
}
/// The caller has already applied request URI/method/Vary and other section 4
/// eligibility conditions. Select only by the first applicable validator branch.
/// No caller state is mutated; an error never exposes a partial selection.
/// Strong results own bounded indexes; the other outcomes allocate nothing.
pub fn select_304_updates(
    received: ResponseValidators<'_>,
    candidates: &[ValidationCandidate<'_>],
    limits: ValidationSelectionLimits,
) -> Result<ValidationSelection, ValidationSelectionError> {
    if candidates.len() > limits.entries.get() {
        return Err(ValidationSelectionError::Entries);
    }
    if limits.index_bytes.get() > isize::MAX as usize {
        return Err(ValidationSelectionError::Bytes);
    }
    let mut work = limits.work.get();
    spend(&mut work, 3)?;
    if received.has_strong() {
        let mut selected = Vec::new();
        for (index, candidate) in candidates.iter().enumerate() {
            if matches(received, candidate.validators, true, &mut work)? {
                push_index(&mut selected, index, limits, &mut work)?;
            }
        }
        return Ok(if selected.is_empty() {
            ValidationSelection::None
        } else {
            ValidationSelection::Strong(selected)
        });
    }
    if received.is_empty() {
        return Ok(
            if candidates.len() == 1 && candidates[0].validators.is_empty() {
                ValidationSelection::WithoutValidator(0)
            } else {
                ValidationSelection::None
            },
        );
    }
    let mut best: Option<(usize, Option<i64>)> = None;
    for (index, candidate) in candidates.iter().enumerate() {
        if !matches(received, candidate.validators, false, &mut work)? {
            continue;
        }
        if let Some((previous, date)) = best {
            let previous_date = date.ok_or(ValidationSelectionError::MissingDate {
                candidate: previous,
            })?;
            let current_date = candidate
                .date
                .ok_or(ValidationSelectionError::MissingDate { candidate: index })?;
            if current_date <= previous_date {
                continue;
            }
        }
        best = Some((index, candidate.date));
    }
    Ok(best.map_or(ValidationSelection::None, |(index, _)| {
        ValidationSelection::Weak(index)
    }))
}
