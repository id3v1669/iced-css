//! Unicode17 default extended grapheme boundaries over one unchanged raw input.
//! The caller supplies its complete domain. No CSS first-character policy,
//! tailoring, normalization, shaping-cluster or caret-affinity choice is made.
use super::{
    grapheme_data::{GraphemeBreak, IndicConjunctBreak, grapheme_break, indic_conjunct_break},
    word_data::is_extended_pictographic,
};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy)]
pub struct GraphemeBoundaryLimits {
    pub input_bytes: NonZeroUsize,
    pub scalars: NonZeroUsize,
    /// Admitted successful actual offset capacity, excluding borrowed/static
    /// storage and allocator overhead; not failed-allocation physical peak.
    pub retained_bytes: NonZeroUsize,
    /// Exact prepaid structural reservation: 1 + 2 * scalar count.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphemeBoundaryError {
    InputBytes,
    Scalars,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for GraphemeBoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "raw extended grapheme boundaries: {self:?}")
    }
}
impl std::error::Error for GraphemeBoundaryError {}
/// Immutable offsets borrow the exact raw source, including CR/CRLF.
/// Empty input owns `[0]` as an endpoint convention, without claiming a grapheme.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::grapheme_boundaries::*;
/// use std::num::NonZeroUsize;
/// let n = NonZeroUsize::new(1000).unwrap();
/// let source = String::from("text");
/// let boundaries = evaluate_grapheme_boundaries(&source, GraphemeBoundaryLimits {
///     input_bytes:n, scalars:n, retained_bytes:n, work:n }).unwrap();
/// drop(source);
/// assert_eq!(boundaries.source(), "text");
/// ```
#[must_use]
#[derive(Debug)]
pub struct GraphemeBoundaries<'source> {
    source: &'source str,
    offsets: Vec<usize>,
    scalar_count: usize,
    retained_bytes: usize,
    work: usize,
}
impl<'source> GraphemeBoundaries<'source> {
    pub fn source(&self) -> &'source str {
        self.source
    }
    pub fn offsets(&self) -> &[usize] {
        &self.offsets
    }
    pub fn scalar_count(&self) -> usize {
        self.scalar_count
    }
    /// None denotes beyond-end/interior UTF-8 bytes. Both endpoints are valid.
    /// This is a boundary query, not byte provenance or insertion affinity.
    pub fn is_boundary(&self, offset: usize) -> Option<bool> {
        self.source
            .is_char_boundary(offset)
            .then(|| self.offsets.binary_search(&offset).is_ok())
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// There is no owned scratch; the sole offset Vec is both final and peak payload.
    pub fn peak_retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
}
#[derive(Default)]
enum IndicSuffix {
    #[default]
    Absent,
    Consonant,
    Linked,
}
#[derive(Default)]
struct Prefix {
    previous: Option<GraphemeBreak>,
    odd_ri: bool,
    ep_extend_tail: bool,
    zwj_after_ep: bool,
    indic: IndicSuffix,
}
fn control(g: GraphemeBreak) -> bool {
    matches!(
        g,
        GraphemeBreak::Control | GraphemeBreak::Cr | GraphemeBreak::Lf
    )
}
impl Prefix {
    fn boundary(&self, right: GraphemeBreak, indic: IndicConjunctBreak, ep: bool) -> bool {
        use GraphemeBreak::*;
        let Some(left) = self.previous else {
            return true;
        }; // GB1
        if left == Cr && right == Lf {
            return false;
        } // GB3
        if control(left) || control(right) {
            return true;
        } // GB4/5
        let joins = (left==L && matches!(right,L|V|Lv|Lvt)) // GB6
            || (matches!(left,Lv|V) && matches!(right,V|T)) // GB7
            || (matches!(left,Lvt|T) && right==T) // GB8
            || matches!(right,Extend|Zwj) // GB9
            || right==SpacingMark // GB9a
            || left==Prepend // GB9b
            || (indic==IndicConjunctBreak::Consonant && matches!(self.indic,IndicSuffix::Linked)) // GB9c
            || (ep && self.zwj_after_ep) // GB11
            || (left==RegionalIndicator && right==RegionalIndicator && self.odd_ri); // GB12/13
        !joins // GB999
    }
    fn push(&mut self, g: GraphemeBreak, indic: IndicConjunctBreak, ep: bool) {
        // The qualified ZWJ consumes the OLD EP Extend* suffix.
        self.zwj_after_ep = g == GraphemeBreak::Zwj && self.ep_extend_tail;
        self.ep_extend_tail = ep || (g == GraphemeBreak::Extend && self.ep_extend_tail);
        match indic {
            IndicConjunctBreak::Consonant => self.indic = IndicSuffix::Consonant,
            IndicConjunctBreak::Extend => {}
            IndicConjunctBreak::Linker => {
                if !matches!(self.indic, IndicSuffix::Absent) {
                    self.indic = IndicSuffix::Linked;
                }
            }
            IndicConjunctBreak::None => self.indic = IndicSuffix::Absent,
        }
        // No word-rule ignore rewrite: every non-RI physically resets parity.
        self.odd_ri = g == GraphemeBreak::RegionalIndicator && !self.odd_ri;
        self.previous = Some(g);
    }
}
fn offset_capacity(count: usize) -> Result<usize, GraphemeBoundaryError> {
    count.checked_add(1).ok_or(GraphemeBoundaryError::Bytes)
}
fn bytes_for(capacity: usize, allowance: usize) -> Result<usize, GraphemeBoundaryError> {
    capacity
        .checked_mul(size_of::<usize>())
        .filter(|&b| b <= isize::MAX as usize && b <= allowance)
        .ok_or(GraphemeBoundaryError::Bytes)
}
fn charge(work: usize, amount: usize, allowance: usize) -> Result<usize, GraphemeBoundaryError> {
    work.checked_add(amount)
        .filter(|&w| w <= allowance)
        .ok_or(GraphemeBoundaryError::Work)
}
/// Evaluate UAX29 revision47 default extended rules without transforming input.
/// Counting pays before every decode; a second prepaid scan classifies, evaluates
/// original suffixes and emits offsets. Static property lookups have fixed bounds.
/// The caller establishes the intended complete domain; no widget/element boundary
/// or normalized-source wrapper is inferred.
///
/// # Errors
/// Limit variants include exceeded input/count/work/requested or actual payload
/// allowances and corresponding arithmetic overflow. Allocation denotes fallible
/// reservation failure. Failure publishes no result and preserves the borrowed source.
pub fn evaluate_grapheme_boundaries(
    source: &str,
    limits: GraphemeBoundaryLimits,
) -> Result<GraphemeBoundaries<'_>, GraphemeBoundaryError> {
    let mut work = charge(0, 1, limits.work.get())?;
    if source.len() > limits.input_bytes.get() {
        return Err(GraphemeBoundaryError::InputBytes);
    }
    let mut chars = source.chars();
    let mut count = 0usize;
    while !chars.as_str().is_empty() {
        let next = count
            .checked_add(1)
            .filter(|&n| n <= limits.scalars.get())
            .ok_or(GraphemeBoundaryError::Scalars)?;
        work = charge(work, 1, limits.work.get())?;
        chars.next();
        count = next;
    }
    work = charge(work, count, limits.work.get())?;
    let capacity = offset_capacity(count)?;
    bytes_for(capacity, limits.retained_bytes.get())?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(capacity)
        .map_err(|_| GraphemeBoundaryError::Allocation)?;
    let retained_bytes = bytes_for(offsets.capacity(), limits.retained_bytes.get())?;
    offsets.push(0);
    let mut prefix = Prefix::default();
    for (index, (start, scalar)) in source.char_indices().enumerate() {
        let g = grapheme_break(scalar);
        let indic = indic_conjunct_break(scalar);
        let ep = is_extended_pictographic(scalar);
        if index > 0 && prefix.boundary(g, indic, ep) {
            offsets.push(start);
        }
        prefix.push(g, indic, ep);
        if index + 1 == count {
            offsets.push(source.len());
        } // GB2, within prepaid scalar visit.
    }
    Ok(GraphemeBoundaries {
        source,
        offsets,
        scalar_count: count,
        retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_arithmetic_and_actual_capacity() {
        assert_eq!(
            offset_capacity(usize::MAX),
            Err(GraphemeBoundaryError::Bytes)
        );
        assert_eq!(
            bytes_for(usize::MAX, usize::MAX),
            Err(GraphemeBoundaryError::Bytes)
        );
        assert_eq!(
            bytes_for(isize::MAX as usize / size_of::<usize>() + 1, usize::MAX),
            Err(GraphemeBoundaryError::Bytes)
        );
        assert_eq!(
            charge(usize::MAX, 1, usize::MAX),
            Err(GraphemeBoundaryError::Work)
        );
        assert_eq!(charge(1, 2, 2), Err(GraphemeBoundaryError::Work));
        let max = NonZeroUsize::new(usize::MAX).unwrap();
        for source in ["", "abc", "a\u{308}", "😀"] {
            let r = evaluate_grapheme_boundaries(
                source,
                GraphemeBoundaryLimits {
                    input_bytes: max,
                    scalars: max,
                    retained_bytes: max,
                    work: max,
                },
            )
            .unwrap();
            assert_eq!(r.retained_bytes, r.offsets.capacity() * size_of::<usize>());
            assert_eq!(r.retained_bytes, r.peak_retained_bytes());
            assert!(r.offsets.capacity() > r.scalar_count);
        }
    }
}
