//! Source-mapped CSS22 LF conversion in one uniform logical text scope.
use super::{
    lf_whitespace::{
        LfWhitespaceAction, LfWhitespaceError, LfWhitespaceLimits, remove_whitespace_around_lf,
    },
    line_edges::WhiteSpace,
    newlines::NormalizedText,
    unicode::{is_bidi_control, is_lf_cjk_ideograph},
};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfDecision {
    Keep,
    /// Step1 removed an original space or tab surrounding LF.
    RemoveWhitespace,
    /// Render an original LF as U+0020.
    Space,
    /// Remove an original LF between the selected Han ideographic neighbors.
    RemoveLf,
}
#[derive(Debug, Clone, Copy)]
pub struct LfConversionLimits {
    pub scalars: NonZeroUsize,
    /// Admitted successful peak action/decision payload capacities, not allocator
    /// internals or failed-operation physical peaks. Normalized text is borrowed.
    pub retained_bytes: NonZeroUsize,
    /// Exact prepaid reservation 1+6*n, not executed scans or CPU instructions.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfConversionError {
    Scalars,
    Bytes,
    Work,
    Allocation,
    Step1(LfWhitespaceError),
}
impl std::fmt::Display for LfConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LF rendering conversion: {self:?}")
    }
}
impl std::error::Error for LfConversionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Step1(error) => Some(error),
            _ => None,
        }
    }
}
/// Decisions retain the exact normalized owner and its original scalar ranges.
/// No source string or map is mutated or copied.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{lf_conversion::*, newlines::NormalizedText, line_edges::WhiteSpace};
/// fn cannot_drop(text: NormalizedText<'_>, limits: LfConversionLimits) {
///     let result = convert_line_feeds(&text, WhiteSpace::Normal, limits).unwrap();
///     drop(text);
///     let _ = result.decision(0);
/// }
/// ```
/// ```compile_fail,E0505
/// use iced_css_core::text::{lf_conversion::*, newlines::*, line_edges::WhiteSpace};
/// fn cannot_drop_source(source: String, nl: NewlineLimits, limits: LfConversionLimits) {
///     let text = normalize_default_newlines(&source, nl).unwrap();
///     let result = convert_line_feeds(&text, WhiteSpace::Normal, limits).unwrap();
///     drop(source);
///     let _ = result.text().source();
/// }
/// ```
#[must_use]
#[derive(Debug)]
pub struct LfConversion<'text, 'source> {
    text: &'text NormalizedText<'source>,
    decisions: Vec<LfDecision>,
    retained_bytes: usize,
    peak_retained_bytes: usize,
    work: usize,
}
impl<'text, 'source> LfConversion<'text, 'source> {
    pub fn text(&self) -> &'text NormalizedText<'source> {
        self.text
    }
    pub fn decisions(&self) -> &[LfDecision] {
        &self.decisions
    }
    pub fn decision(&self, index: usize) -> Option<&LfDecision> {
        self.decisions.get(index)
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Actual payload capacities admitted together; excludes borrowed text,
    /// allocator overhead and any rejected reservation's transient physical peak.
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
}
fn work(count: usize, allowance: usize) -> Result<usize, LfConversionError> {
    let amount = count
        .checked_mul(6)
        .and_then(|n| n.checked_add(1))
        .ok_or(LfConversionError::Work)?;
    if amount > allowance {
        return Err(LfConversionError::Work);
    }
    Ok(amount)
}
fn bytes<T>(capacity: usize) -> Result<usize, LfConversionError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or(LfConversionError::Bytes)
}
fn peak(scratch: usize, result: usize, allowance: usize) -> Result<usize, LfConversionError> {
    scratch
        .checked_add(result)
        .filter(|n| *n <= allowance && *n <= isize::MAX as usize)
        .ok_or(LfConversionError::Bytes)
}
/// Normalize first; this operation internally establishes trusted step1 removals.
/// Normal/nowrap LF renders as a space unless both nearest original retained
/// scalar neighbors are Unicode17 Script=Han AND Ideographic. Step1 removals and
/// Bidi_Control are transparent; original LF always resets context. Other modes
/// retain LF. No atomics, cross-element assembly or line-breaking policy is supplied.
///
/// The two context scans always run; step1 reserves 3*n even in preserved modes
/// where it skips scans. The total prepaid reservation is exactly 1+6*n.
///
/// # Errors
/// Rejects count/work/payload admission, arithmetic or allocation failures without
/// a partial result. Successful reservation capacity is checked after acquisition;
/// this does not bound a failed operation's physical allocator peak.
pub fn convert_line_feeds<'text, 'source>(
    text: &'text NormalizedText<'source>,
    mode: WhiteSpace,
    limits: LfConversionLimits,
) -> Result<LfConversion<'text, 'source>, LfConversionError> {
    let count = text.scalars().len();
    if count > limits.scalars.get() {
        return Err(LfConversionError::Scalars);
    }
    let work = work(count, limits.work.get())?;
    let requested = bytes::<LfDecision>(count)?;
    peak(
        bytes::<LfWhitespaceAction>(count)?,
        requested,
        limits.retained_bytes.get(),
    )?;
    // Successful 1+6*n admission proves 3*n cannot overflow.
    let step1_work = NonZeroUsize::new((count * 3).max(1)).expect("positive minimum");
    let scratch = remove_whitespace_around_lf(
        text,
        mode,
        LfWhitespaceLimits {
            scalars: limits.scalars,
            retained_bytes: limits
                .retained_bytes
                .min(NonZeroUsize::new(isize::MAX as usize).expect("positive addressability")),
            work: step1_work,
        },
    )
    .map_err(LfConversionError::Step1)?;
    let scratch_bytes = bytes::<LfWhitespaceAction>(scratch.capacity())?;
    peak(scratch_bytes, requested, limits.retained_bytes.get())?;
    let mut decisions = Vec::new();
    decisions
        .try_reserve_exact(count)
        .map_err(|_| LfConversionError::Allocation)?;
    let retained_bytes = bytes::<LfDecision>(decisions.capacity())?;
    let peak_retained_bytes = peak(scratch_bytes, retained_bytes, limits.retained_bytes.get())?;
    for action in &scratch {
        decisions.push(if *action == LfWhitespaceAction::Remove {
            LfDecision::RemoveWhitespace
        } else {
            LfDecision::Keep
        });
    }
    let mut neighbor = false;
    for (scalar, decision) in text.text().chars().zip(&mut decisions) {
        if *decision == LfDecision::RemoveWhitespace || is_bidi_control(scalar) {
            continue;
        }
        if scalar == '\n' {
            if !mode.preserves_newlines() {
                *decision = if neighbor {
                    LfDecision::RemoveLf
                } else {
                    LfDecision::Space
                };
            }
            neighbor = false;
        } else {
            neighbor = is_lf_cjk_ideograph(scalar);
        }
    }
    neighbor = false;
    for (scalar, decision) in text.text().chars().rev().zip(decisions.iter_mut().rev()) {
        if *decision == LfDecision::RemoveWhitespace || is_bidi_control(scalar) {
            continue;
        }
        if scalar == '\n' {
            if *decision == LfDecision::RemoveLf && !neighbor {
                *decision = LfDecision::Space;
            }
            neighbor = false;
        } else {
            neighbor = is_lf_cjk_ideograph(scalar);
        }
    }
    drop(scratch);
    Ok(LfConversion {
        text,
        decisions,
        retained_bytes,
        peak_retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accounts_check_actual_capacity_overflow_and_addressability() {
        assert_eq!(work(usize::MAX, usize::MAX), Err(LfConversionError::Work));
        assert_eq!(
            bytes::<LfDecision>(usize::MAX),
            Err(LfConversionError::Bytes)
        );
        assert_eq!(
            peak(isize::MAX as usize, 1, usize::MAX),
            Err(LfConversionError::Bytes)
        );
        assert_eq!(
            peak(usize::MAX, 1, usize::MAX),
            Err(LfConversionError::Bytes)
        );
        let text = super::super::newlines::normalize_default_newlines(
            "漢\n漢",
            super::super::newlines::NewlineLimits {
                input_bytes: NonZeroUsize::MAX,
                retained_bytes: NonZeroUsize::new(1000).unwrap(),
                work: NonZeroUsize::MAX,
            },
        )
        .unwrap();
        let limits = LfConversionLimits {
            scalars: NonZeroUsize::MAX,
            retained_bytes: NonZeroUsize::MAX,
            work: NonZeroUsize::MAX,
        };
        let got = convert_line_feeds(&text, WhiteSpace::Normal, limits).unwrap();
        let scratch = remove_whitespace_around_lf(
            &text,
            WhiteSpace::Normal,
            LfWhitespaceLimits {
                scalars: NonZeroUsize::MAX,
                retained_bytes: NonZeroUsize::new(1000).unwrap(),
                work: NonZeroUsize::MAX,
            },
        )
        .unwrap();
        assert_eq!(
            got.retained_bytes,
            got.decisions.capacity() * size_of::<LfDecision>()
        );
        assert_eq!(
            got.peak_retained_bytes,
            got.retained_bytes + scratch.capacity() * size_of::<LfWhitespaceAction>()
        );
    }
    #[test]
    fn step1_error_exposes_its_cause() {
        use std::error::Error;
        let error = LfConversionError::Step1(LfWhitespaceError::Allocation);
        assert_eq!(
            error.source().unwrap().to_string(),
            LfWhitespaceError::Allocation.to_string()
        );
        assert!(LfConversionError::Bytes.source().is_none());
    }
}
