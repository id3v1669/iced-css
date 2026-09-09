//! Unicode17 Table3-17 context predicates over one immutable normalized input.
//! The normalized string is this engine's original input, never a mapped result.
//! This does not assemble CSS context domains or select locale/mapping behavior.
use super::{
    casing_data::{
        CasingContext, canonical_combining_class, is_case_ignorable, is_cased, is_soft_dotted,
    },
    newlines::NormalizedText,
};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct CasingContextLimits {
    pub scalars: NonZeroUsize,
    /// Actual flags-vector capacity; borrowed text/maps and static tables excluded.
    /// Also the admitted successful owned payload peak, not allocator physical
    /// peak on rejected reservations. An oversized allowance itself is valid.
    pub retained_bytes: NonZeroUsize,
    /// Prepaid structural work: admission + initialization + two scans = 1+3*n.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasingContextError {
    Scalars,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for CasingContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "original-input casing contexts: {self:?}")
    }
}
impl std::error::Error for CasingContextError {}
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default)]
struct Flags(u8);
fn mask(context: CasingContext) -> u8 {
    match context {
        CasingContext::FinalSigma => 1,
        CasingContext::AfterSoftDotted => 2,
        CasingContext::MoreAbove => 4,
        CasingContext::BeforeDot => 8,
        CasingContext::AfterI => 16,
    }
}
impl Flags {
    fn set(&mut self, context: CasingContext, value: bool) {
        if value {
            self.0 |= mask(context);
        } else {
            self.0 &= !mask(context);
        }
    }
}
/// Private flags remain tied to the exact normalized owner and its source.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{casing_contexts::*, casing_data::CasingContext, newlines::*};
/// use std::num::NonZeroUsize;
/// let n = NonZeroUsize::new(1000).unwrap();
/// let text = normalize_default_newlines("I", NewlineLimits { input_bytes:n, retained_bytes:n, work:n }).unwrap();
/// let contexts = evaluate_casing_contexts(&text, CasingContextLimits { scalars:n, retained_bytes:n, work:n }).unwrap();
/// drop(text);
/// assert_eq!(contexts.matches(0, CasingContext::AfterI), Some(false));
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{casing_contexts::*, newlines::*};
/// use std::num::NonZeroUsize;
/// let n = NonZeroUsize::new(1000).unwrap();
/// let source = String::from("I");
/// let text = normalize_default_newlines(&source, NewlineLimits { input_bytes:n, retained_bytes:n, work:n }).unwrap();
/// let contexts = evaluate_casing_contexts(&text, CasingContextLimits { scalars:n, retained_bytes:n, work:n }).unwrap();
/// drop(source);
/// assert_eq!(contexts.text().source(), "I");
/// ```
#[must_use]
#[derive(Debug)]
pub struct CasingContexts<'text, 'source> {
    text: &'text NormalizedText<'source>,
    flags: Vec<Flags>,
    retained_bytes: usize,
    work: usize,
}
impl<'text, 'source> CasingContexts<'text, 'source> {
    pub fn text(&self) -> &'text NormalizedText<'source> {
        self.text
    }
    pub fn scalar_count(&self) -> usize {
        self.flags.len()
    }
    /// None denotes an invalid scalar index; valid false predicates return Some(false).
    pub fn matches(&self, index: usize, context: CasingContext) -> Option<bool> {
        self.flags
            .get(index)
            .map(|flags| flags.0 & mask(context) != 0)
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
}
fn work_for(count: usize) -> Result<usize, CasingContextError> {
    count
        .checked_mul(3)
        .and_then(|n| n.checked_add(1))
        .ok_or(CasingContextError::Work)
}
fn bytes_for(capacity: usize, allowance: usize) -> Result<usize, CasingContextError> {
    capacity
        .checked_mul(size_of::<Flags>())
        .filter(|&n| n <= allowance && n <= isize::MAX as usize)
        .ok_or(CasingContextError::Bytes)
}
/// Evaluate all five Unicode17 contexts on the unchanged normalized string.
///
/// The caller establishes the intended complete context domain; this function
/// neither assembles it across sources nor treats a widget or element as a
/// boundary automatically. It does not select CSS ordering, locale conditions,
/// mappings, words, output text or caret behavior. Source ranges remain borrowed
/// through the exact normalized owner.
///
/// # Errors
/// Returns `Scalars`, `Work` or `Bytes` when the count, prepaid structural work,
/// or requested/actual flags capacity exceeds its limit (or representable size).
/// `Allocation` denotes a failed fallible reservation. Failure publishes no flags
/// and leaves the borrowed owner unchanged.
pub fn evaluate_casing_contexts<'text, 'source>(
    text: &'text NormalizedText<'source>,
    limits: CasingContextLimits,
) -> Result<CasingContexts<'text, 'source>, CasingContextError> {
    use CasingContext::*;
    let count = text.scalars().len();
    if count > limits.scalars.get() {
        return Err(CasingContextError::Scalars);
    }
    let work = work_for(count)?;
    if work > limits.work.get() {
        return Err(CasingContextError::Work);
    }
    bytes_for(count, limits.retained_bytes.get())?;
    let mut flags = Vec::new();
    flags
        .try_reserve_exact(count)
        .map_err(|_| CasingContextError::Allocation)?;
    let retained_bytes = bytes_for(flags.capacity(), limits.retained_bytes.get())?;
    flags.resize(count, Flags::default());
    let (mut preceding_cased, mut after_soft, mut after_i) = (false, false, false);
    for (scalar, flag) in text.text().chars().zip(&mut flags) {
        flag.set(FinalSigma, preceding_cased);
        flag.set(AfterSoftDotted, after_soft);
        flag.set(AfterI, after_i);
        // Possessive skipping: even Cased + Case_Ignorable cannot be a witness.
        if !is_case_ignorable(scalar) {
            preceding_cased = is_cased(scalar);
        }
        let ccc = canonical_combining_class(scalar);
        if ccc == 0 || ccc == 230 {
            // Unicode17 Soft_Dotted and I have CCC0 (qualified in tests).
            after_soft = is_soft_dotted(scalar);
            after_i = scalar == 'I';
        }
    }
    let (mut following_cased, mut more_above, mut before_dot) = (false, false, false);
    for (scalar, flag) in text.text().chars().rev().zip(flags.iter_mut().rev()) {
        flag.set(
            FinalSigma,
            flag.0 & mask(FinalSigma) != 0 && !following_cased,
        );
        flag.set(MoreAbove, more_above);
        flag.set(BeforeDot, before_dot);
        if !is_case_ignorable(scalar) {
            following_cased = is_cased(scalar);
        }
        let ccc = canonical_combining_class(scalar);
        if ccc == 0 || ccc == 230 {
            more_above = ccc == 230;
            before_dot = scalar == '\u{307}';
        }
    }
    Ok(CasingContexts {
        text,
        flags,
        retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_arithmetic_and_actual_capacity() {
        assert_eq!(work_for(0), Ok(1));
        assert_eq!(work_for(usize::MAX), Err(CasingContextError::Work));
        assert_eq!(work_for(usize::MAX / 3), Err(CasingContextError::Work));
        assert_eq!(
            bytes_for(isize::MAX as usize + 1, usize::MAX),
            Err(CasingContextError::Bytes)
        );
        assert_eq!(bytes_for(2, 1), Err(CasingContextError::Bytes));
        assert_eq!(bytes_for(2, usize::MAX), Ok(2));
        let n = NonZeroUsize::new(1000).unwrap();
        let text = super::super::newlines::normalize_default_newlines(
            "aΣ",
            super::super::newlines::NewlineLimits {
                input_bytes: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let got = evaluate_casing_contexts(
            &text,
            CasingContextLimits {
                scalars: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        assert_eq!(
            got.retained_bytes(),
            got.flags.capacity() * size_of::<Flags>()
        );
        assert!(got.flags.iter().all(|flag| flag.0 & !31 == 0));
    }
}
