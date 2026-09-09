//! Unicode17 default UAX29 word boundaries on an unchanged, complete raw input.
//! The caller supplies the domain. This selects no CSS words, casing, tailoring,
//! normalization, grapheme/line breaks or caret policy.
use super::word_data::{WordBreak, is_extended_pictographic, word_break};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct WordBoundaryLimits {
    pub input_bytes: NonZeroUsize,
    pub scalars: NonZeroUsize,
    /// Admitted successful cache plus output capacity, excluding borrowed/static
    /// storage and allocator overhead. Not failed-allocation physical peak.
    pub retained_bytes: NonZeroUsize,
    /// Exact prepaid structural reservation: 1 + 4 * scalar count.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordBoundaryError {
    InputBytes,
    Scalars,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for WordBoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "raw word boundaries: {self:?}")
    }
}
impl std::error::Error for WordBoundaryError {}
/// Immutable offsets borrow the exact original source, including CR/CRLF.
/// Empty input owns the sole endpoint `[0]`, without claiming a linguistic word.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::word_boundaries::*;
/// use std::num::NonZeroUsize;
/// let n = NonZeroUsize::new(1000).unwrap();
/// let source = String::from("words");
/// let boundaries = evaluate_word_boundaries(&source, WordBoundaryLimits {
///     input_bytes:n, scalars:n, retained_bytes:n, work:n }).unwrap();
/// drop(source);
/// assert_eq!(boundaries.source(), "words");
/// ```
#[must_use]
#[derive(Debug)]
pub struct WordBoundaries<'source> {
    source: &'source str,
    offsets: Vec<usize>,
    scalar_count: usize,
    retained_bytes: usize,
    peak_retained_bytes: usize,
    work: usize,
}
impl<'source> WordBoundaries<'source> {
    pub fn source(&self) -> &'source str {
        self.source
    }
    pub fn offsets(&self) -> &[usize] {
        &self.offsets
    }
    pub fn scalar_count(&self) -> usize {
        self.scalar_count
    }
    /// None means beyond-end or interior UTF-8 byte; both endpoints are valid.
    /// This query gives boundaries, not byte provenance or insertion affinity.
    pub fn is_boundary(&self, offset: usize) -> Option<bool> {
        self.source
            .is_char_boundary(offset)
            .then(|| self.offsets.binary_search(&offset).is_ok())
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
}
struct Classified {
    end: usize,
    class: WordBreak,
    extended_pictographic: bool,
    next_significant: Option<WordBreak>,
}
fn ignored(c: WordBreak) -> bool {
    matches!(c, WordBreak::Extend | WordBreak::Format | WordBreak::Zwj)
}
fn newline(c: WordBreak) -> bool {
    matches!(c, WordBreak::Cr | WordBreak::Lf | WordBreak::Newline)
}
fn ah(c: Option<WordBreak>) -> bool {
    matches!(c, Some(WordBreak::ALetter | WordBreak::HebrewLetter))
}
fn ml(c: Option<WordBreak>) -> bool {
    matches!(
        c,
        Some(WordBreak::MidLetter | WordBreak::MidNumLet | WordBreak::SingleQuote)
    )
}
fn mn(c: Option<WordBreak>) -> bool {
    matches!(
        c,
        Some(WordBreak::MidNum | WordBreak::MidNumLet | WordBreak::SingleQuote)
    )
}
fn word_part(c: Option<WordBreak>) -> bool {
    ah(c) || matches!(c, Some(WordBreak::Numeric | WordBreak::Katakana))
}
fn boundary(
    current: &Classified,
    right: &Classified,
    last: Option<WordBreak>,
    previous: Option<WordBreak>,
    odd_ri: bool,
) -> bool {
    use WordBreak::*;
    let r = Some(right.class);
    // Physical WB3..3d precede ignore rewriting (WB4).
    if current.class == Cr && right.class == Lf {
        return false;
    }
    if newline(current.class) || newline(right.class) {
        return true;
    }
    if current.class == Zwj && right.extended_pictographic {
        return false;
    }
    if current.class == WSegSpace && right.class == WSegSpace {
        return false;
    }
    if ignored(right.class) {
        return false;
    }
    // WB5..13b: history and lookahead contain only significant classes.
    let joins = (ah(last) && ah(r))
        || (ah(last) && ml(r) && ah(right.next_significant))
        || (ah(previous) && ml(last) && ah(r))
        || (last==Some(HebrewLetter) && r==Some(SingleQuote))
        || (last==Some(HebrewLetter) && r==Some(DoubleQuote) && right.next_significant==Some(HebrewLetter))
        || (previous==Some(HebrewLetter) && last==Some(DoubleQuote) && r==Some(HebrewLetter))
        || (last==Some(Numeric) && r==Some(Numeric))
        || (ah(last) && r==Some(Numeric))
        || (last==Some(Numeric) && ah(r))
        || (previous==Some(Numeric) && mn(last) && r==Some(Numeric))
        || (last==Some(Numeric) && mn(r) && right.next_significant==Some(Numeric))
        || (last==Some(Katakana) && r==Some(Katakana))
        || ((word_part(last) || last==Some(ExtendNumLet)) && r==Some(ExtendNumLet))
        || (last==Some(ExtendNumLet) && word_part(r))
        // WB15/16: ignored input never resets the effective RI suffix.
        || (last==Some(RegionalIndicator) && r==Some(RegionalIndicator) && odd_ri);
    !joins // WB999
}
fn allocation_bytes<T>(capacity: usize) -> Result<usize, WordBoundaryError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or(WordBoundaryError::Bytes)
}
fn combined_bytes(a: usize, b: usize, allowance: usize) -> Result<usize, WordBoundaryError> {
    a.checked_add(b)
        .filter(|&bytes| bytes <= allowance)
        .ok_or(WordBoundaryError::Bytes)
}
fn offset_capacity(count: usize) -> Result<usize, WordBoundaryError> {
    count.checked_add(1).ok_or(WordBoundaryError::Bytes)
}
fn charge(work: usize, amount: usize, allowance: usize) -> Result<usize, WordBoundaryError> {
    work.checked_add(amount)
        .filter(|&w| w <= allowance)
        .ok_or(WordBoundaryError::Work)
}
/// Evaluate default UAX29 revision47 boundaries without transforming the source.
/// Work pays before counting each scalar, then prepays classification, reverse
/// lookahead and forward decisions. Static property lookups have fixed bounds.
/// Empty input retains an endpoint allocation. Offsets reserve n+1 entries even
/// when fewer are selected; peak includes the temporary classified cache.
///
/// # Errors
/// Returns the corresponding limit error for input bytes, scalars, structural
/// work or requested/actual payload admission, including arithmetic overflow.
/// Allocation denotes fallible reservation failure. Errors publish no result.
pub fn evaluate_word_boundaries(
    source: &str,
    limits: WordBoundaryLimits,
) -> Result<WordBoundaries<'_>, WordBoundaryError> {
    let mut work = charge(0, 1, limits.work.get())?;
    if source.len() > limits.input_bytes.get() {
        return Err(WordBoundaryError::InputBytes);
    }
    let mut chars = source.chars();
    let mut count = 0usize;
    while !chars.as_str().is_empty() {
        let next = count
            .checked_add(1)
            .filter(|&n| n <= limits.scalars.get())
            .ok_or(WordBoundaryError::Scalars)?;
        work = charge(work, 1, limits.work.get())?;
        chars.next();
        count = next;
    }
    work = charge(
        work,
        count.checked_mul(3).ok_or(WordBoundaryError::Work)?,
        limits.work.get(),
    )?;
    let capacity = offset_capacity(count)?;
    let requested_cache = allocation_bytes::<Classified>(count)?;
    let requested_offsets = allocation_bytes::<usize>(capacity)?;
    combined_bytes(
        requested_cache,
        requested_offsets,
        limits.retained_bytes.get(),
    )?;
    let mut cache = Vec::<Classified>::new();
    cache
        .try_reserve_exact(count)
        .map_err(|_| WordBoundaryError::Allocation)?;
    let cache_bytes = allocation_bytes::<Classified>(cache.capacity())?;
    combined_bytes(cache_bytes, requested_offsets, limits.retained_bytes.get())?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(capacity)
        .map_err(|_| WordBoundaryError::Allocation)?;
    let retained_bytes = allocation_bytes::<usize>(offsets.capacity())?;
    let peak_retained_bytes =
        combined_bytes(cache_bytes, retained_bytes, limits.retained_bytes.get())?;
    offsets.push(0);
    for (start, c) in source.char_indices() {
        cache.push(Classified {
            end: start + c.len_utf8(),
            class: word_break(c),
            extended_pictographic: is_extended_pictographic(c),
            next_significant: None,
        });
    }
    let mut following = None;
    for entry in cache.iter_mut().rev() {
        entry.next_significant = following;
        if !ignored(entry.class) {
            following = Some(entry.class);
        }
    }
    let (mut last, mut previous, mut odd_ri) = (None, None, false);
    for (i, current) in cache.iter().enumerate() {
        if !ignored(current.class) {
            previous = last;
            last = Some(current.class);
            odd_ri = current.class == WordBreak::RegionalIndicator && !odd_ri;
        }
        if cache
            .get(i + 1)
            .is_none_or(|right| boundary(current, right, last, previous, odd_ri))
        {
            offsets.push(current.end);
        }
    }
    drop(cache);
    Ok(WordBoundaries {
        source,
        offsets,
        scalar_count: count,
        retained_bytes,
        peak_retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_admission_arithmetic() {
        assert_eq!(offset_capacity(usize::MAX), Err(WordBoundaryError::Bytes));
        assert_eq!(
            allocation_bytes::<usize>(usize::MAX),
            Err(WordBoundaryError::Bytes)
        );
        assert_eq!(
            allocation_bytes::<u8>(isize::MAX as usize + 1),
            Err(WordBoundaryError::Bytes)
        );
        assert_eq!(
            combined_bytes(usize::MAX, 1, usize::MAX),
            Err(WordBoundaryError::Bytes)
        );
        assert_eq!(
            combined_bytes(isize::MAX as usize, 1, usize::MAX),
            Ok(isize::MAX as usize + 1)
        );
        assert_eq!(
            charge(usize::MAX, 1, usize::MAX),
            Err(WordBoundaryError::Work)
        );
        assert_eq!(charge(1, 4, 4), Err(WordBoundaryError::Work));
        assert_eq!(allocation_bytes::<Classified>(0), Ok(0));
        let max = NonZeroUsize::new(usize::MAX).unwrap();
        let limits = WordBoundaryLimits {
            input_bytes: max,
            scalars: max,
            retained_bytes: max,
            work: max,
        };
        for source in ["", "abc", "𐐀é"] {
            let result = evaluate_word_boundaries(source, limits).unwrap();
            assert_eq!(
                result.retained_bytes,
                result.offsets.capacity() * size_of::<usize>()
            );
            assert!(result.offsets.capacity() > result.scalar_count);
            let cache_bytes = result.peak_retained_bytes - result.retained_bytes;
            assert_eq!(cache_bytes % size_of::<Classified>(), 0);
            assert!(cache_bytes >= result.scalar_count * size_of::<Classified>());
            if source.is_empty() {
                assert_eq!(cache_bytes, 0);
            }
        }
    }
}
