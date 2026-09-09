//! Source-preserving CSS22 whitespace steps 1–4 in one uniform inline scope.
use super::{
    lf_conversion::{
        LfConversion, LfConversionError, LfConversionLimits, LfDecision, convert_line_feeds,
    },
    lf_whitespace::LfWhitespaceAction,
    line_edges::WhiteSpace,
    newlines::NormalizedText,
    preserved_spaces::{
        PreservedSpace, PreservedSpaceError, PreservedSpaceLimits, preserved_space_sequences,
    },
    space_collapse::{
        SpaceAction, SpaceCollapseError, SpaceCollapseLimits, SpaceItem, SpaceUnit,
        collapse_adjacent_spaces,
    },
};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct WhitespacePreprocessingLimits {
    pub scalars: NonZeroUsize,
    /// Admitted successful peak owned payload, excluding the borrowed normalized
    /// owner, allocator internals and failed-operation physical peaks.
    pub retained_bytes: NonZeroUsize,
    /// Exact prepaid structural reservation 2+10*n, not CPU instructions.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespacePreprocessingError {
    Scalars,
    Bytes,
    Work,
    Allocation,
    Lf(LfConversionError),
    Preserved(PreservedSpaceError),
    Collapse(SpaceCollapseError),
}
impl std::fmt::Display for WhitespacePreprocessingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "whitespace preprocessing: {self:?}")
    }
}
impl std::error::Error for WhitespacePreprocessingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lf(error) => Some(error),
            Self::Preserved(error) => Some(error),
            Self::Collapse(error) => Some(error),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceRendering {
    /// Render the original normalized scalar unchanged; consult preserved-space
    /// metadata separately for nonbreaking treatment and sequence opportunities.
    Keep,
    /// Render U+0020 with the original scalar's provenance.
    Space,
    RemoveWhitespace,
    RemoveLf,
    RemoveAdjacentSpace,
}

/// Immutable preprocessing components bound to one exact normalized owner/mode.
/// Indices are normalized scalar indices, not original bytes or source-node IDs.
/// Original text, ranges and component causes remain available for later mapping.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{newlines::NormalizedText, line_edges::WhiteSpace,
///     whitespace_preprocessing::*};
/// fn cannot_drop(text: NormalizedText<'_>, limits: WhitespacePreprocessingLimits) {
///     let result = preprocess_whitespace(&text, WhiteSpace::Normal, limits).unwrap();
///     drop(text);
///     let _ = result.preserved_space(0);
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css_core::text::{newlines::*, line_edges::WhiteSpace,
///     whitespace_preprocessing::*};
/// fn cannot_drop(source: String, nl: NewlineLimits, limits: WhitespacePreprocessingLimits) {
///     let text = normalize_default_newlines(&source, nl).unwrap();
///     let result = preprocess_whitespace(&text, WhiteSpace::Normal, limits).unwrap();
///     drop(source);
///     let _ = result.text().source();
/// }
/// ```
#[must_use]
#[derive(Debug)]
pub struct WhitespacePreprocessing<'text, 'source> {
    lf: LfConversion<'text, 'source>,
    mode: WhiteSpace,
    preserved: Vec<PreservedSpace>,
    collapse: Vec<SpaceAction>,
    retained_bytes: usize,
    peak_retained_bytes: usize,
    work: usize,
}
impl<'text, 'source> WhitespacePreprocessing<'text, 'source> {
    pub fn text(&self) -> &'text NormalizedText<'source> {
        self.lf.text()
    }
    pub fn mode(&self) -> WhiteSpace {
        self.mode
    }
    pub fn scalar_count(&self) -> usize {
        self.lf.decisions().len()
    }
    pub fn lf_conversion(&self) -> &LfConversion<'text, 'source> {
        &self.lf
    }
    pub fn lf_decision(&self, index: usize) -> Option<&LfDecision> {
        self.lf.decision(index)
    }
    pub fn preserved_space(&self, index: usize) -> Option<&PreservedSpace> {
        self.preserved.get(index)
    }
    pub fn collapse_action(&self, index: usize) -> Option<&SpaceAction> {
        self.collapse.get(index)
    }
    /// Constant-time effective rendering. Component queries retain all causes,
    /// including a converted LF subsequently removed by adjacent collapse.
    pub fn rendering(&self, index: usize) -> Option<WhitespaceRendering> {
        let lf = self.lf_decision(index)?;
        let collapse = self.collapse_action(index)?;
        Some(match (lf, collapse) {
            (LfDecision::RemoveWhitespace, _) => WhitespaceRendering::RemoveWhitespace,
            (LfDecision::RemoveLf, _) => WhitespaceRendering::RemoveLf,
            (_, SpaceAction::RemovedAdjacentSpace) => WhitespaceRendering::RemoveAdjacentSpace,
            (_, SpaceAction::TabToSpace) | (LfDecision::Space, _) => WhitespaceRendering::Space,
            (LfDecision::Keep, SpaceAction::Unchanged) => WhitespaceRendering::Keep,
        })
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Actual payload capacities admitted together, including temporary items
    /// and the nested LF scratch peak. Borrowed owners are accounted separately.
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
}

fn work(count: usize, allowance: usize) -> Result<usize, WhitespacePreprocessingError> {
    count
        .checked_mul(10)
        .and_then(|n| n.checked_add(2))
        .filter(|n| *n <= allowance)
        .ok_or(WhitespacePreprocessingError::Work)
}
fn bytes<T>(capacity: usize) -> Result<usize, WhitespacePreprocessingError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or(WhitespacePreprocessingError::Bytes)
}
// Each allocation has its own addressability check. A sum of different vectors
// may exceed isize::MAX if it still fits usize and the aggregate allowance.
fn sum<const N: usize>(
    parts: [usize; N],
    allowance: usize,
) -> Result<usize, WhitespacePreprocessingError> {
    parts
        .into_iter()
        .try_fold(0_usize, |total, part| total.checked_add(part))
        .filter(|n| *n <= allowance)
        .ok_or(WhitespacePreprocessingError::Bytes)
}
fn remaining(allowance: usize, occupied: usize) -> Result<usize, WhitespacePreprocessingError> {
    allowance
        .checked_sub(occupied)
        .ok_or(WhitespacePreprocessingError::Bytes)
}
fn nested_bytes(
    remaining: usize,
    needed: usize,
) -> Result<NonZeroUsize, WhitespacePreprocessingError> {
    if needed > remaining || needed > isize::MAX as usize {
        return Err(WhitespacePreprocessingError::Bytes);
    }
    // Zero is representable as one only for zero required payload. The actual
    // returned capacity is still checked against the genuine remaining budget.
    Ok(NonZeroUsize::new(remaining.min(isize::MAX as usize).max(1)).expect("positive minimum"))
}
fn subwork(needed: usize) -> NonZeroUsize {
    NonZeroUsize::new(needed.max(1)).expect("positive minimum")
}

/// Preprocess one caller-established uniform inline scope with no internal
/// element boundary for preserved-space step2. LF conversion supplies step1;
/// a single temporary item vector joins its decisions to adjacent collapse.
///
/// The normalized owner and original text/ranges stay borrowed and unchanged.
/// This does not choose scopes, normalize, remove selected line edges, determine
/// line breaks, transform case or assemble a complete source formatting stream.
///
/// # Errors
/// Rejects count/work/payload arithmetic and allocation failures, preserving
/// nested helper errors. No partial result escapes. Capacities are checked after
/// successful acquisition; failed-operation physical allocator peaks are not bounded.
/// A large allowance alone does not reject an admissible small input.
pub fn preprocess_whitespace<'text, 'source>(
    text: &'text NormalizedText<'source>,
    mode: WhiteSpace,
    limits: WhitespacePreprocessingLimits,
) -> Result<WhitespacePreprocessing<'text, 'source>, WhitespacePreprocessingError> {
    let count = text.scalars().len();
    if count > limits.scalars.get() {
        return Err(WhitespacePreprocessingError::Scalars);
    }
    let work = work(count, limits.work.get())?;
    let allowance = limits.retained_bytes.get();
    let requested_lf = bytes::<LfDecision>(count)?;
    let requested_preserved = bytes::<PreservedSpace>(count)?;
    let requested_items = bytes::<SpaceItem>(count)?;
    let requested_collapse = bytes::<SpaceAction>(count)?;
    let lf_minimum = sum(
        [bytes::<LfWhitespaceAction>(count)?, requested_lf],
        allowance.min(isize::MAX as usize),
    )?;
    sum(
        [
            requested_lf,
            requested_preserved,
            requested_items,
            requested_collapse,
        ],
        allowance,
    )?;

    // Successful 2+10*n admission proves these smaller work expressions fit.
    let lf = convert_line_feeds(
        text,
        mode,
        LfConversionLimits {
            scalars: limits.scalars,
            retained_bytes: nested_bytes(allowance, lf_minimum)?,
            work: subwork(1 + 6 * count),
        },
    )
    .map_err(WhitespacePreprocessingError::Lf)?;
    let lf_bytes = lf.retained_bytes();
    let lf_peak = sum(
        [lf.peak_retained_bytes()],
        allowance.min(isize::MAX as usize),
    )?;
    sum(
        [
            lf_bytes,
            requested_preserved,
            requested_items,
            requested_collapse,
        ],
        allowance,
    )?;

    let preserved = preserved_space_sequences(
        text,
        mode,
        PreservedSpaceLimits {
            scalars: limits.scalars,
            retained_bytes: nested_bytes(remaining(allowance, lf_bytes)?, requested_preserved)?,
            work: subwork(2 * count),
        },
    )
    .map_err(WhitespacePreprocessingError::Preserved)?;
    let preserved_bytes = bytes::<PreservedSpace>(preserved.capacity())?;
    sum(
        [
            lf_bytes,
            preserved_bytes,
            requested_items,
            requested_collapse,
        ],
        allowance,
    )?;

    let mut items = Vec::new();
    items
        .try_reserve_exact(count)
        .map_err(|_| WhitespacePreprocessingError::Allocation)?;
    let items_bytes = bytes::<SpaceItem>(items.capacity())?;
    sum(
        [lf_bytes, preserved_bytes, items_bytes, requested_collapse],
        allowance,
    )?;
    for (scalar, decision) in text.text().chars().zip(lf.decisions()) {
        items.push(SpaceItem {
            unit: match decision {
                LfDecision::Keep => SpaceUnit::Scalar(scalar),
                LfDecision::Space => SpaceUnit::Scalar(' '),
                LfDecision::RemoveWhitespace | LfDecision::RemoveLf => SpaceUnit::Removed,
            },
            white_space: mode,
        });
    }
    let occupied = sum([lf_bytes, preserved_bytes, items_bytes], allowance)?;
    let collapse = collapse_adjacent_spaces(
        &items,
        SpaceCollapseLimits {
            items: limits.scalars,
            retained_bytes: nested_bytes(remaining(allowance, occupied)?, requested_collapse)?,
            work: subwork(count),
        },
    )
    .map_err(WhitespacePreprocessingError::Collapse)?;
    let collapse_bytes = bytes::<SpaceAction>(collapse.capacity())?;
    let peak_retained_bytes = lf_peak.max(sum([occupied, collapse_bytes], allowance)?);
    let retained_bytes = sum([lf_bytes, preserved_bytes, collapse_bytes], allowance)?;
    drop(items);
    Ok(WhitespacePreprocessing {
        lf,
        mode,
        preserved,
        collapse,
        retained_bytes,
        peak_retained_bytes,
        work,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn accounting_rejects_overflow_but_allows_separate_addressable_payloads() {
        let max = usize::MAX;
        let addressable = isize::MAX as usize;
        assert_eq!(work(max, max), Err(WhitespacePreprocessingError::Work));
        assert_eq!(work(0, 2), Ok(2));
        assert_eq!(work(0, 1), Err(WhitespacePreprocessingError::Work));
        assert_eq!(
            bytes::<SpaceItem>(max),
            Err(WhitespacePreprocessingError::Bytes)
        );
        assert_eq!(
            bytes::<u8>(addressable + 1),
            Err(WhitespacePreprocessingError::Bytes)
        );
        assert_eq!(bytes::<u8>(addressable), Ok(addressable));
        assert_eq!(sum([addressable, 1], max), Ok(addressable + 1));
        assert_eq!(
            sum([addressable, 1], addressable),
            Err(WhitespacePreprocessingError::Bytes)
        );
        assert_eq!(sum([max, 1], max), Err(WhitespacePreprocessingError::Bytes));
        assert_eq!(sum([7, 3], 9), Err(WhitespacePreprocessingError::Bytes));
        assert_eq!(sum([7, 3], 10), Ok(10));
    }

    #[test]
    fn remaining_allowances_never_grant_nonempty_payload_from_zero() {
        assert_eq!(remaining(10, 11), Err(WhitespacePreprocessingError::Bytes));
        assert_eq!(remaining(10, 10), Ok(0));
        assert_eq!(nested_bytes(0, 0).unwrap().get(), 1);
        assert_eq!(nested_bytes(0, 1), Err(WhitespacePreprocessingError::Bytes));
        assert_eq!(nested_bytes(7, 8), Err(WhitespacePreprocessingError::Bytes));
        assert_eq!(nested_bytes(7, 7).unwrap().get(), 7);
        assert_eq!(
            nested_bytes(usize::MAX, 7).unwrap().get(),
            isize::MAX as usize
        );
        assert_eq!(
            nested_bytes(usize::MAX, isize::MAX as usize + 1),
            Err(WhitespacePreprocessingError::Bytes)
        );
        assert_eq!(subwork(0).get(), 1);
        // The nonzero compatibility placeholder is still checked against the
        // genuine remaining zero after a hypothetical acquired capacity.
        assert_eq!(
            sum([bytes::<SpaceAction>(1).unwrap()], 0),
            Err(WhitespacePreprocessingError::Bytes)
        );
    }

    #[test]
    fn retained_accounts_use_owned_capacities_and_peak_includes_temporary_items() {
        for source in ["", " ", "a\t \n\n漢漢"] {
            let text = super::super::newlines::normalize_default_newlines(
                source,
                super::super::newlines::NewlineLimits {
                    input_bytes: NonZeroUsize::MAX,
                    retained_bytes: NonZeroUsize::new(10000).unwrap(),
                    work: NonZeroUsize::MAX,
                },
            )
            .unwrap();
            let result = preprocess_whitespace(
                &text,
                WhiteSpace::Normal,
                WhitespacePreprocessingLimits {
                    scalars: NonZeroUsize::MAX,
                    retained_bytes: NonZeroUsize::MAX,
                    work: NonZeroUsize::MAX,
                },
            )
            .unwrap();
            let retained = result.lf.retained_bytes()
                + result.preserved.capacity() * size_of::<PreservedSpace>()
                + result.collapse.capacity() * size_of::<SpaceAction>();
            assert_eq!(result.retained_bytes(), retained);
            assert_eq!(result.preserved.len(), result.scalar_count());
            assert_eq!(result.collapse.len(), result.scalar_count());
            assert!(
                result.peak_retained_bytes()
                    >= retained + result.scalar_count() * size_of::<SpaceItem>()
            );
            assert!(result.peak_retained_bytes() >= result.lf.peak_retained_bytes());
            if source.is_empty() {
                assert_eq!(result.peak_retained_bytes(), 0);
            }
        }
    }

    #[test]
    fn nested_error_sources_preserve_stage_and_step1_causes() {
        use super::super::lf_whitespace::LfWhitespaceError;
        let lf = LfConversionError::Step1(LfWhitespaceError::Allocation);
        let error = WhitespacePreprocessingError::Lf(lf);
        assert_eq!(error.source().unwrap().to_string(), lf.to_string());
        assert_eq!(
            error.source().unwrap().source().unwrap().to_string(),
            LfWhitespaceError::Allocation.to_string()
        );
        let preserved = PreservedSpaceError::Bytes;
        assert_eq!(
            WhitespacePreprocessingError::Preserved(preserved)
                .source()
                .unwrap()
                .to_string(),
            preserved.to_string()
        );
        let collapse = SpaceCollapseError::StageOrder { item: 7 };
        assert_eq!(
            WhitespacePreprocessingError::Collapse(collapse)
                .source()
                .unwrap()
                .to_string(),
            collapse.to_string()
        );
        assert!(WhitespacePreprocessingError::Bytes.source().is_none());
    }
}
