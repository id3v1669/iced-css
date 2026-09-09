//! Explicit lower/upper mapping selection over immutable original-input contexts.
//! Locale None satisfies no locale condition but still permits context-only rows.
//! This does not infer source language, materialize output or implement CSS words.
use super::{
    casing_contexts::CasingContexts,
    casing_data::{
        CaseMapping, CasingCondition, CasingLocale, ConditionalCaseMapping, base_lower_mapping,
        base_upper_mapping, conditional_mappings,
    },
    newlines::NormalizedText,
};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasingOperation {
    Lower,
    Upper,
}
#[derive(Debug, Clone, Copy)]
pub struct CasingSelectionLimits {
    pub scalars: NonZeroUsize,
    /// Actual admitted vector payload, excluding borrowed/static owners and
    /// allocator overhead or failed-operation physical allocation peaks.
    pub retained_bytes: NonZeroUsize,
    /// Exact prepaid structural reservation 1+n, not CPU instructions.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasingSelectionError {
    Scalars,
    Bytes,
    Work,
    Allocation,
    Ambiguous { index: usize },
}
impl std::fmt::Display for CasingSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "explicit casing selection: {self:?}")
    }
}
impl std::error::Error for CasingSelectionError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasingSelectionOrigin {
    Base,
    Conditional(&'static ConditionalCaseMapping),
}
#[derive(Debug, Clone, Copy)]
enum Selection {
    Base(CaseMapping),
    Conditional(&'static ConditionalCaseMapping),
}
/// Selected static facts remain associated with their exact context/source chain.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{casing_contexts::CasingContexts, casing_selection::*};
/// fn cannot_drop(contexts: CasingContexts<'_, '_>, limits: CasingSelectionLimits) {
///     let result = select_case_mappings(&contexts, CasingOperation::Lower, None, limits).unwrap();
///     drop(contexts);
///     let _ = result.mapping(0);
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css_core::text::{newlines::NormalizedText, casing_contexts::*, casing_selection::*};
/// fn cannot_drop(text: NormalizedText<'_>, cl: CasingContextLimits, sl: CasingSelectionLimits) {
///     let contexts = evaluate_casing_contexts(&text, cl).unwrap();
///     let result = select_case_mappings(&contexts, CasingOperation::Lower, None, sl).unwrap();
///     drop(text);
///     let _ = result.text().source();
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css_core::text::{newlines::*, casing_contexts::*, casing_selection::*};
/// fn cannot_drop(source: String, nl: NewlineLimits, cl: CasingContextLimits, sl: CasingSelectionLimits) {
///     let text = normalize_default_newlines(&source, nl).unwrap();
///     let contexts = evaluate_casing_contexts(&text, cl).unwrap();
///     let result = select_case_mappings(&contexts, CasingOperation::Lower, None, sl).unwrap();
///     drop(source);
///     let _ = result.text().source();
/// }
/// ```
#[must_use]
#[derive(Debug)]
pub struct CasingSelection<'contexts, 'text, 'source> {
    contexts: &'contexts CasingContexts<'text, 'source>,
    operation: CasingOperation,
    locale: Option<CasingLocale>,
    records: Vec<Selection>,
    retained_bytes: usize,
    work: usize,
}
impl<'contexts, 'text, 'source> CasingSelection<'contexts, 'text, 'source> {
    pub fn contexts(&self) -> &'contexts CasingContexts<'text, 'source> {
        self.contexts
    }
    pub fn text(&self) -> &'text NormalizedText<'source> {
        self.contexts.text()
    }
    pub fn operation(&self) -> CasingOperation {
        self.operation
    }
    pub fn locale(&self) -> Option<CasingLocale> {
        self.locale
    }
    pub fn scalar_count(&self) -> usize {
        self.records.len()
    }
    pub fn mapping(&self, index: usize) -> Option<CaseMapping> {
        self.records.get(index).map(|record| match record {
            Selection::Base(mapping) => *mapping,
            Selection::Conditional(row) => CaseMapping::Replacement(match self.operation {
                CasingOperation::Lower => row.lower(),
                CasingOperation::Upper => row.upper(),
            }),
        })
    }
    pub fn origin(&self, index: usize) -> Option<CasingSelectionOrigin> {
        self.records.get(index).map(|record| match record {
            Selection::Base(_) => CasingSelectionOrigin::Base,
            Selection::Conditional(row) => CasingSelectionOrigin::Conditional(row),
        })
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
}
fn work_for(count: usize) -> Result<usize, CasingSelectionError> {
    count.checked_add(1).ok_or(CasingSelectionError::Work)
}
fn bytes_for(capacity: usize, allowance: usize) -> Result<usize, CasingSelectionError> {
    capacity
        .checked_mul(size_of::<Selection>())
        .filter(|n| *n <= allowance && *n <= isize::MAX as usize)
        .ok_or(CasingSelectionError::Bytes)
}
fn matching_row<'rows>(
    rows: &'rows [ConditionalCaseMapping],
    contexts: &CasingContexts<'_, '_>,
    index: usize,
    locale: Option<CasingLocale>,
) -> Result<Option<&'rows ConditionalCaseMapping>, CasingSelectionError> {
    let mut selected = None;
    for row in rows {
        let mut satisfied = true;
        for condition in row.conditions() {
            let value = match *condition {
                CasingCondition::Locale(wanted) => locale == Some(wanted),
                CasingCondition::Context { context, negated } => {
                    contexts
                        .matches(index, context)
                        .expect("complete context owner covers every original scalar")
                        != negated
                }
            };
            satisfied &= value;
        }
        if satisfied {
            if selected.is_some() {
                return Err(CasingSelectionError::Ambiguous { index });
            }
            selected = Some(row);
        }
    }
    Ok(selected)
}
/// Select against original immutable contexts with an explicitly supplied locale.
/// None is not host-language inference or a CSS unknown-language fallback.
/// Conditional empty/identity sequences retain exact static-row provenance.
///
/// # Errors
/// Rejects input/work/requested or actual payload admission and allocation failure.
/// A second matched row reports Ambiguous at the normalized scalar index; no first
/// or last winner precedence is inferred. Failure publishes no partial selection.
pub fn select_case_mappings<'contexts, 'text, 'source>(
    contexts: &'contexts CasingContexts<'text, 'source>,
    operation: CasingOperation,
    locale: Option<CasingLocale>,
    limits: CasingSelectionLimits,
) -> Result<CasingSelection<'contexts, 'text, 'source>, CasingSelectionError> {
    let count = contexts.scalar_count();
    if count > limits.scalars.get() {
        return Err(CasingSelectionError::Scalars);
    }
    let work = work_for(count)?;
    if work > limits.work.get() {
        return Err(CasingSelectionError::Work);
    }
    bytes_for(count, limits.retained_bytes.get())?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(count)
        .map_err(|_| CasingSelectionError::Allocation)?;
    let retained_bytes = bytes_for(records.capacity(), limits.retained_bytes.get())?;
    for (index, scalar) in contexts.text().text().chars().enumerate() {
        let record = match matching_row(conditional_mappings(scalar), contexts, index, locale)? {
            Some(row) => Selection::Conditional(row),
            None => Selection::Base(match operation {
                CasingOperation::Lower => base_lower_mapping(scalar),
                CasingOperation::Upper => base_upper_mapping(scalar),
            }),
        };
        records.push(record);
    }
    Ok(CasingSelection {
        contexts,
        operation,
        locale,
        records,
        retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::super::{
        casing_contexts::{CasingContextLimits, evaluate_casing_contexts},
        newlines::{NewlineLimits, normalize_default_newlines},
    };
    use super::*;
    #[test]
    fn synthetic_duplicate_real_rows_reach_same_matcher_ambiguity() {
        let n = NonZeroUsize::new(1000).unwrap();
        let text = normalize_default_newlines(
            "ΑΣ",
            NewlineLimits {
                input_bytes: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let contexts = evaluate_casing_contexts(
            &text,
            CasingContextLimits {
                scalars: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let row = conditional_mappings('Σ')[0];
        // Explicitly synthetic duplication; no public arbitrary-row owner exists.
        assert!(matches!(
            matching_row(&[row, row], &contexts, 1, None),
            Err(CasingSelectionError::Ambiguous { index: 1 })
        ));
        assert!(
            matching_row(&[row, row], &contexts, 0, None)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn checked_accounting_uses_actual_private_enum_capacity() {
        assert_eq!(work_for(usize::MAX), Err(CasingSelectionError::Work));
        assert_eq!(
            bytes_for(usize::MAX, usize::MAX),
            Err(CasingSelectionError::Bytes)
        );
        let inaccessible = isize::MAX as usize / size_of::<Selection>() + 1;
        assert_eq!(
            bytes_for(inaccessible, usize::MAX),
            Err(CasingSelectionError::Bytes)
        );
        let n = NonZeroUsize::new(1000).unwrap();
        let text = normalize_default_newlines(
            "ß漢",
            NewlineLimits {
                input_bytes: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let contexts = evaluate_casing_contexts(
            &text,
            CasingContextLimits {
                scalars: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let result = select_case_mappings(
            &contexts,
            CasingOperation::Upper,
            None,
            CasingSelectionLimits {
                scalars: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        assert_eq!(
            result.retained_bytes(),
            result.records.capacity() * size_of::<Selection>()
        );
    }
}
