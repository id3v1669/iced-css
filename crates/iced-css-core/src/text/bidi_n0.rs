//! N0 bracket directions and deferred propagation through raw original NSMs.
//! Post-N0 types are not resolved neutrals or final implicit levels.
use super::{
    bidi_bracket_pairs::{BidiBracketPairs, BidiBracketSequence},
    bidi_data::BidiClass,
    bidi_explicit::BidiExplicitLevel,
    bidi_runs::{BidiRunSequences, BidiSequenceDirection},
    bidi_weak::BidiWeakScalar,
};
use std::{iter::FusedIterator, mem::size_of, num::NonZeroUsize};
#[derive(Debug, Clone, Copy)]
pub struct BidiN0Limits {
    pub scalars: NonZeroUsize,
    pub active_scalars: NonZeroUsize,
    pub runs: NonZeroUsize,
    pub sequences: NonZeroUsize,
    pub pairs: NonZeroUsize,
    /// Actual output capacity payload; excludes predecessors, fixed cursor locals,
    /// compiler frame layout, allocator overhead and rejected-allocation physical peaks.
    pub retained_bytes: NonZeroUsize,
    /// Reserved schedule: 1 + r + 66a + 2s + 3k; not executed operation count.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiN0Error {
    Scalars,
    ActiveScalars,
    Runs,
    Sequences,
    Pairs,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for BidiN0Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bidi N0 resolution: {self:?}")
    }
}
impl std::error::Error for BidiN0Error {}
/// Immutable N0 output with the exact qualified pair owner as predecessor.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("(a)");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let weak=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let pairs=identify_bidi_bracket_pairs(&weak,BidiBracketPairLimits{scalars:n,active_scalars:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_n0(&pairs,BidiN0Limits{scalars:n,active_scalars:n,runs:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// drop(source);
/// assert_eq!(result.scalar_count(),3);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("(a)");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let weak=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let pairs=identify_bidi_bracket_pairs(&weak,BidiBracketPairLimits{scalars:n,active_scalars:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_n0(&pairs,BidiN0Limits{scalars:n,active_scalars:n,runs:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// drop(weak);
/// assert_eq!(result.scalar_count(),3);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("(a)");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let weak=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let pairs=identify_bidi_bracket_pairs(&weak,BidiBracketPairLimits{scalars:n,active_scalars:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_n0(&pairs,BidiN0Limits{scalars:n,active_scalars:n,runs:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// drop(pairs);
/// assert_eq!(result.scalar_count(),3);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("(a)");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let weak=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let pairs=identify_bidi_bracket_pairs(&weak,BidiBracketPairLimits{scalars:n,active_scalars:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_n0(&pairs,BidiN0Limits{scalars:n,active_scalars:n,runs:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let view=result.scalar(0).unwrap();
/// drop(result);
/// assert_eq!(view.index(),0);
/// ```
///
/// ```compile_fail,E0597
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("(a)");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let weak=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let pairs=identify_bidi_bracket_pairs(&weak,BidiBracketPairLimits{scalars:n,active_scalars:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_n0(&pairs,BidiN0Limits{scalars:n,active_scalars:n,runs:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let mut iterator;
/// { let sequence=result.sequence(0).unwrap(); iterator=sequence.scalars(); }
/// assert!(iterator.next().is_some());
/// ```
#[must_use]
#[derive(Debug)]
pub struct BidiN0<'p, 'w, 'r, 'e, 'i, 'n, 's> {
    pairs: &'p BidiBracketPairs<'w, 'r, 'e, 'i, 'n, 's>,
    types: Vec<BidiClass>,
    retained_bytes: usize,
    work: usize,
}
impl<'p, 'w, 'r, 'e, 'i, 'n, 's> BidiN0<'p, 'w, 'r, 'e, 'i, 'n, 's> {
    pub fn pairs(&self) -> &'p BidiBracketPairs<'w, 'r, 'e, 'i, 'n, 's> {
        self.pairs
    }
    pub fn scalar_count(&self) -> usize {
        self.pairs.scalar_count()
    }
    pub fn active_scalar_count(&self) -> usize {
        self.pairs.active_scalar_count()
    }
    pub fn run_count(&self) -> usize {
        self.pairs.weak().run_count()
    }
    pub fn sequence_count(&self) -> usize {
        self.pairs.sequence_count()
    }
    pub fn pair_count(&self) -> usize {
        self.pairs.pair_count()
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
    pub fn scalar(&self, index: usize) -> Option<BidiN0Scalar<'_, 'p, 'w, 'r, 'e, 'i, 'n, 's>> {
        self.pairs.weak().scalar(index)?;
        Some(BidiN0Scalar { owner: self, index })
    }
    pub fn sequence(&self, index: usize) -> Option<BidiN0Sequence<'_, 'p, 'w, 'r, 'e, 'i, 'n, 's>> {
        self.pairs.sequence(index)?;
        Some(BidiN0Sequence { owner: self, index })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiN0Scalar<'v, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiN0<'p, 'w, 'r, 'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'p, 'w, 'r, 'e, 'i, 'n, 's> BidiN0Scalar<'v, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn weak_scalar(&self) -> BidiWeakScalar<'_, 'r, 'e, 'i, 'n, 's> {
        self.owner
            .pairs
            .weak()
            .scalar(self.index)
            .expect("N0 scalar has weak predecessor")
    }
    pub fn current_type(&self) -> Option<BidiClass> {
        self.weak_scalar()
            .run_scalar()
            .membership()
            .map(|m| self.owner.types[m.active_index()])
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiN0Sequence<'v, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiN0<'p, 'w, 'r, 'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'p, 'w, 'r, 'e, 'i, 'n, 's> BidiN0Sequence<'v, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn bracket_sequence(&self) -> BidiBracketSequence<'_, 'w, 'r, 'e, 'i, 'n, 's> {
        self.owner
            .pairs
            .sequence(self.index)
            .expect("N0 sequence has bracket predecessor")
    }
    pub fn scalar_count(&self) -> usize {
        self.bracket_sequence().weak_sequence().scalar_count()
    }
    pub fn run_count(&self) -> usize {
        self.bracket_sequence().weak_sequence().run_count()
    }
    pub fn level(&self) -> BidiExplicitLevel {
        self.bracket_sequence().weak_sequence().level()
    }
    pub fn sos(&self) -> BidiSequenceDirection {
        self.bracket_sequence().weak_sequence().sos()
    }
    pub fn eos(&self) -> BidiSequenceDirection {
        self.bracket_sequence().weak_sequence().eos()
    }
    pub fn scalars(&self) -> BidiN0Scalars<'_, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
        let runs = self.owner.pairs.weak().runs();
        let first = {
            let seq = runs
                .sequence(self.index)
                .expect("N0 sequence has run predecessor");
            seq.runs().next().map(|r| r.index())
        };
        BidiN0Scalars {
            owner: self.owner,
            next_run: first,
            local_index: 0,
            remaining: self.scalar_count(),
        }
    }
}
#[derive(Debug)]
pub struct BidiN0Scalars<'v, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiN0<'p, 'w, 'r, 'e, 'i, 'n, 's>,
    next_run: Option<usize>,
    local_index: usize,
    remaining: usize,
}
impl<'v, 'p, 'w, 'r, 'e, 'i, 'n, 's> Iterator for BidiN0Scalars<'v, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    type Item = BidiN0Scalar<'v, 'p, 'w, 'r, 'e, 'i, 'n, 's>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let run = self
            .owner
            .pairs
            .weak()
            .runs()
            .run(self.next_run.expect("nonempty N0 cursor"))
            .expect("cursor run");
        let index = run.scalar(self.local_index).expect("cursor scalar").index();
        self.remaining -= 1;
        self.local_index += 1;
        if self.remaining == 0 {
            self.next_run = None;
        } else if self.local_index == run.scalar_count() {
            self.next_run = Some(run.next_run_index().expect("sequence has remaining run"));
            self.local_index = 0;
        }
        Some(BidiN0Scalar {
            owner: self.owner,
            index,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for BidiN0Scalars<'_, '_, '_, '_, '_, '_, '_, '_> {
    fn len(&self) -> usize {
        self.remaining
    }
}
impl FusedIterator for BidiN0Scalars<'_, '_, '_, '_, '_, '_, '_, '_> {}
#[derive(Debug)]
struct Cursor {
    run: Option<usize>,
    local: usize,
}
impl Cursor {
    fn at(runs: &BidiRunSequences<'_, '_, '_, '_>, index: usize, sequence: usize) -> Self {
        let scalar = runs.scalar(index).expect("validated pair endpoint");
        let m = scalar.membership().expect("endpoint retained");
        assert_eq!(m.sequence_index(), sequence);
        let run = runs.run(m.run_index()).expect("endpoint run");
        let first = run
            .scalar(0)
            .expect("run nonempty")
            .membership()
            .expect("run scalar retained")
            .active_index();
        let local = m
            .active_index()
            .checked_sub(first)
            .expect("endpoint follows run start");
        assert_eq!(
            run.scalar(local).expect("endpoint local index").index(),
            index
        );
        Self {
            run: Some(m.run_index()),
            local,
        }
    }
    fn index(&self, runs: &BidiRunSequences<'_, '_, '_, '_>) -> Option<usize> {
        self.run.map(|index| {
            runs.run(index)
                .expect("cursor run")
                .scalar(self.local)
                .expect("cursor local scalar")
                .index()
        })
    }
    fn advance(&mut self, runs: &BidiRunSequences<'_, '_, '_, '_>) {
        if let Some(index) = self.run {
            let run = runs.run(index).expect("cursor run");
            self.local += 1;
            if self.local == run.scalar_count() {
                self.run = run.next_run_index();
                self.local = 0;
            }
        }
    }
}
fn active(runs: &BidiRunSequences<'_, '_, '_, '_>, index: usize) -> usize {
    runs.scalar(index)
        .expect("admitted original scalar")
        .membership()
        .expect("active scalar")
        .active_index()
}
fn strong(c: BidiClass) -> Option<BidiClass> {
    match c {
        BidiClass::L => Some(BidiClass::L),
        BidiClass::R | BidiClass::En | BidiClass::An => Some(BidiClass::R),
        _ => None,
    }
}
fn process(pairs: &BidiBracketPairs<'_, '_, '_, '_, '_, '_>, types: &mut Vec<BidiClass>) {
    let weak = pairs.weak();
    let runs = weak.runs();
    // Copy in global active order without a scan over removed original records.
    for ri in 0..runs.run_count() {
        let run = runs.run(ri).expect("admitted run");
        for local in 0..run.scalar_count() {
            let scalar = run.scalar(local).expect("admitted local scalar");
            assert_eq!(
                scalar.membership().expect("active member").active_index(),
                types.len()
            );
            types.push(
                weak.scalar(scalar.index())
                    .expect("weak member")
                    .current_type()
                    .expect("active weak type"),
            );
        }
    }
    for si in 0..pairs.sequence_count() {
        let pair_sequence = pairs.sequence(si).expect("admitted pair sequence");
        let sequence = runs.sequence(si).expect("admitted sequence");
        let first = { sequence.runs().next().expect("sequence nonempty").index() };
        let mut preceding = Cursor {
            run: Some(first),
            local: 0,
        };
        let mut latest = sequence.sos().bidi_class();
        let own = if sequence.level().number().is_multiple_of(2) {
            BidiClass::L
        } else {
            BidiClass::R
        };
        for pair in pair_sequence.pairs() {
            let opening = pair.opening_scalar_index();
            let closing = pair.closing_scalar_index();
            assert_eq!(
                runs.scalar(closing)
                    .expect("validated closing endpoint")
                    .membership()
                    .expect("closing retained")
                    .sequence_index(),
                si
            );
            while let Some(index) = preceding.index(runs) {
                if index >= opening {
                    break;
                }
                if let Some(direction) = strong(types[active(runs, index)]) {
                    latest = direction;
                }
                preceding.advance(runs);
            }
            assert_eq!(
                preceding.index(runs),
                Some(opening),
                "prefix reaches opening in this sequence"
            );
            let mut interior = Cursor::at(runs, opening, si);
            interior.advance(runs);
            let mut has_own = false;
            let mut has_opposite = false;
            loop {
                let index = interior
                    .index(runs)
                    .expect("closing endpoint remains in sequence");
                if index == closing {
                    break;
                }
                assert!(index < closing, "interior cursor does not cross closing");
                if let Some(direction) = strong(types[active(runs, index)]) {
                    if direction == own {
                        has_own = true;
                    } else {
                        has_opposite = true;
                    }
                }
                interior.advance(runs);
            }
            let selected = if has_own {
                Some(own)
            } else if has_opposite {
                Some(latest)
            } else {
                None
            };
            if let Some(direction) = selected {
                types[active(runs, opening)] = direction;
                types[active(runs, closing)] = direction;
            }
        }
    }
    // Deferred original-NSM propagation begins only after all pair decisions.
    for si in 0..pairs.sequence_count() {
        let sequence = weak.sequence(si).expect("admitted weak sequence");
        let mut carry = None;
        for scalar in sequence.scalars() {
            let run_scalar = scalar.run_scalar();
            let index = run_scalar
                .membership()
                .expect("active weak member")
                .active_index();
            let original = run_scalar.explicit().original().class();
            if original == BidiClass::Nsm {
                if let Some(direction) = carry {
                    types[index] = direction;
                }
            } else {
                carry = if scalar.current_type() == Some(BidiClass::On)
                    && matches!(types[index], BidiClass::L | BidiClass::R)
                {
                    Some(types[index])
                } else {
                    None
                };
            }
        }
    }
}
fn bytes_for(capacity: usize, allowance: usize) -> Result<usize, BidiN0Error> {
    capacity
        .checked_mul(size_of::<BidiClass>())
        .filter(|&b| b <= isize::MAX as usize && b <= allowance)
        .ok_or(BidiN0Error::Bytes)
}
fn work_for(
    a: usize,
    r: usize,
    s: usize,
    k: usize,
    allowance: usize,
) -> Result<usize, BidiN0Error> {
    a.checked_mul(66)
        .and_then(|w| w.checked_add(r))
        .and_then(|w| s.checked_mul(2).and_then(|s| w.checked_add(s)))
        .and_then(|w| k.checked_mul(3).and_then(|k| w.checked_add(k)))
        .and_then(|w| w.checked_add(1))
        .filter(|&w| w <= allowance)
        .ok_or(BidiN0Error::Work)
}
/// Resolve pair directions in opening order, then propagate changed directions
/// through original-NSM chains. Interior cursors stay within their qualified sequence.
///
/// # Errors
/// Scalars, ActiveScalars, Runs, Sequences and Pairs precede checked Work admission.
/// Requested output payload is checked against isize and caller limits before fallible
/// reservation; actual capacity receives the same checks before initialization.
/// Bytes includes arithmetic overflow; Allocation denotes reservation failure.
/// Failure publishes no owner and preserves predecessors. Metrics count only the
/// output capacity, not borrowed predecessors, fixed cursors or physical allocator peaks.
pub fn resolve_bidi_n0<'p, 'w, 'r, 'e, 'i, 'n, 's>(
    pairs: &'p BidiBracketPairs<'w, 'r, 'e, 'i, 'n, 's>,
    limits: BidiN0Limits,
) -> Result<BidiN0<'p, 'w, 'r, 'e, 'i, 'n, 's>, BidiN0Error> {
    let n = pairs.scalar_count();
    let a = pairs.active_scalar_count();
    let r = pairs.weak().run_count();
    let s = pairs.sequence_count();
    let k = pairs.pair_count();
    if n > limits.scalars.get() {
        return Err(BidiN0Error::Scalars);
    }
    if a > limits.active_scalars.get() {
        return Err(BidiN0Error::ActiveScalars);
    }
    if r > limits.runs.get() {
        return Err(BidiN0Error::Runs);
    }
    if s > limits.sequences.get() {
        return Err(BidiN0Error::Sequences);
    }
    if k > limits.pairs.get() {
        return Err(BidiN0Error::Pairs);
    }
    let work = work_for(a, r, s, k, limits.work.get())?;
    bytes_for(a, limits.retained_bytes.get())?;
    let mut types = Vec::new();
    types
        .try_reserve_exact(a)
        .map_err(|_| BidiN0Error::Allocation)?;
    let retained_bytes = bytes_for(types.capacity(), limits.retained_bytes.get())?;
    process(pairs, &mut types);
    Ok(BidiN0 {
        pairs,
        types,
        retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{
        bidi_bracket_pairs::*, bidi_explicit::*, bidi_initial::*, bidi_isolates::*, bidi_runs::*,
        bidi_weak::*,
    };
    #[test]
    fn cursor_membership_termination_and_actual_capacity() {
        let n = NonZeroUsize::new(usize::MAX).unwrap();
        for source in ["", "\u{ad}", "(\u{2067}א\u{2069}a)", "((a))"] {
            let initial = evaluate_bidi_initial(
                source,
                BidiParagraphMode::LeftToRight,
                BidiInitialLimits {
                    input_bytes: n,
                    scalars: n,
                    paragraphs: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            let isolates = prepare_bidi_isolates(
                &initial,
                BidiIsolateLimits {
                    scalars: n,
                    paragraphs: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            let explicit = resolve_bidi_explicit(
                &isolates,
                BidiExplicitLimits {
                    scalars: n,
                    paragraphs: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            let runs = prepare_bidi_run_sequences(
                &explicit,
                BidiRunLimits {
                    scalars: n,
                    paragraphs: n,
                    active_scalars: n,
                    runs: n,
                    sequences: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            let weak = resolve_bidi_weak(
                &runs,
                BidiWeakLimits {
                    scalars: n,
                    active_scalars: n,
                    runs: n,
                    sequences: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            let pairs = identify_bidi_bracket_pairs(
                &weak,
                BidiBracketPairLimits {
                    scalars: n,
                    active_scalars: n,
                    sequences: n,
                    pairs: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            let r = resolve_bidi_n0(
                &pairs,
                BidiN0Limits {
                    scalars: n,
                    active_scalars: n,
                    runs: n,
                    sequences: n,
                    pairs: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            assert_eq!(r.types.len(), runs.active_scalar_count());
            assert!(r.types.capacity() >= r.types.len());
            assert_eq!(
                r.retained_bytes,
                r.types.capacity() * size_of::<BidiClass>()
            );
            assert_eq!(r.retained_bytes, r.peak_retained_bytes());
            for si in 0..runs.sequence_count() {
                let seq = runs.sequence(si).unwrap();
                let expected: Vec<_> = seq.scalars().map(|v| v.index()).collect();
                for (start, &index) in expected.iter().enumerate() {
                    let mut cursor = Cursor::at(&runs, index, si);
                    for &want in &expected[start..] {
                        assert_eq!(cursor.index(&runs), Some(want));
                        assert_eq!(
                            runs.scalar(want)
                                .unwrap()
                                .membership()
                                .unwrap()
                                .sequence_index(),
                            si
                        );
                        cursor.advance(&runs);
                    }
                    assert_eq!(cursor.index(&runs), None);
                    cursor.advance(&runs);
                    assert_eq!(cursor.index(&runs), None);
                }
            }
        }
    }
    #[test]
    fn checked_arithmetic_and_numeric_search_types() {
        assert_eq!(bytes_for(usize::MAX, usize::MAX), Err(BidiN0Error::Bytes));
        assert_eq!(
            bytes_for(isize::MAX as usize + 1, usize::MAX),
            Err(BidiN0Error::Bytes)
        );
        assert_eq!(bytes_for(2, 1), Err(BidiN0Error::Bytes));
        for (a, r, s, k) in [
            (usize::MAX, 0, 0, 0),
            (0, usize::MAX, 0, 0),
            (0, 0, usize::MAX, 0),
            (0, 0, 0, usize::MAX),
        ] {
            assert_eq!(work_for(a, r, s, k, usize::MAX), Err(BidiN0Error::Work));
        }
        assert_eq!(work_for(0, 0, 0, 0, 1), Ok(1));
        assert_eq!(strong(BidiClass::En), Some(BidiClass::R));
        assert_eq!(strong(BidiClass::An), Some(BidiClass::R));
        assert_eq!(strong(BidiClass::On), None);
    }
}
