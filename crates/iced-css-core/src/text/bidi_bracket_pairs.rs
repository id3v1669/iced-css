//! BD14–BD16 bracket identification over post-W7 isolating run sequences.
//! Whole-sequence overflow discards its pairs. This does not execute N0 or normalize
//! original source characters; canonical aliases are limited to bracket comparison.
use super::{
    bidi_data::{BidiClass, BidiPairedBracketType, bidi_paired_bracket, bidi_paired_bracket_type},
    bidi_weak::{BidiWeak, BidiWeakScalar, BidiWeakSequence},
};
use std::{iter::FusedIterator, mem::size_of, num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct BidiBracketPairLimits {
    pub scalars: NonZeroUsize,
    pub active_scalars: NonZeroUsize,
    pub sequences: NonZeroUsize,
    pub pairs: NonZeroUsize,
    /// Tracked actual vector capacities plus the selected fixed stack while alive.
    /// Excludes predecessors, other locals, compiler frame layout and allocator overhead.
    pub retained_bytes: NonZeroUsize,
    /// Reserved schedule: 1 + (a > 0 ? 63 : 0) + s + 65a; not executed operation count.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiBracketPairError {
    Scalars,
    ActiveScalars,
    Sequences,
    Pairs,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for BidiBracketPairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bidi bracket identification: {self:?}")
    }
}
impl std::error::Error for BidiBracketPairError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiBracketSequenceStatus {
    Complete,
    Overflow,
}
#[derive(Debug, Clone, Copy)]
struct PendingPair {
    opening: usize,
    closing: Option<usize>,
}
#[derive(Debug)]
struct SequenceRecord {
    pairs: Range<usize>,
    status: BidiBracketSequenceStatus,
}
#[derive(Clone, Copy)]
struct StackEntry {
    expected_close: char,
    slot: usize,
}
const STACK_CAPACITY: usize = 63;
/// Immutable sequence-local pair order and complete/overflow outcomes.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("()");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let weak=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=identify_bidi_bracket_pairs(&weak,BidiBracketPairLimits{scalars:n,active_scalars:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// drop(source);
/// assert_eq!(result.pair_count(),1);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("()");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let weak=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=identify_bidi_bracket_pairs(&weak,BidiBracketPairLimits{scalars:n,active_scalars:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// drop(weak);
/// assert_eq!(result.pair_count(),1);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("()");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let weak=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=identify_bidi_bracket_pairs(&weak,BidiBracketPairLimits{scalars:n,active_scalars:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let sequence=result.sequence(0).unwrap();
/// let view=sequence.pair(0).unwrap();
/// drop(result);
/// assert_eq!(view.opening_scalar_index(),0);
/// ```
///
/// ```compile_fail,E0597
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("()");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let weak=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=identify_bidi_bracket_pairs(&weak,BidiBracketPairLimits{scalars:n,active_scalars:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let mut iterator;
/// { let sequence=result.sequence(0).unwrap(); iterator=sequence.pairs(); }
/// assert!(iterator.next().is_some());
/// ```
///
/// ```compile_fail,E0597
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("()");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let weak=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=identify_bidi_bracket_pairs(&weak,BidiBracketPairLimits{scalars:n,active_scalars:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let endpoint;
/// let sequence=result.sequence(0).unwrap();
/// { let pair=sequence.pair(0).unwrap(); endpoint=pair.opening(); }
/// assert_eq!(endpoint.index(),0);
/// ```
#[must_use]
#[derive(Debug)]
pub struct BidiBracketPairs<'w, 'r, 'e, 'i, 'n, 's> {
    weak: &'w BidiWeak<'r, 'e, 'i, 'n, 's>,
    slots: Vec<PendingPair>,
    sequences: Vec<SequenceRecord>,
    retained_bytes: usize,
    peak_retained_bytes: usize,
    status_stack_bytes: usize,
    work: usize,
}
impl<'w, 'r, 'e, 'i, 'n, 's> BidiBracketPairs<'w, 'r, 'e, 'i, 'n, 's> {
    pub fn weak(&self) -> &'w BidiWeak<'r, 'e, 'i, 'n, 's> {
        self.weak
    }
    pub fn scalar_count(&self) -> usize {
        self.weak.scalar_count()
    }
    pub fn active_scalar_count(&self) -> usize {
        self.weak.active_scalar_count()
    }
    pub fn sequence_count(&self) -> usize {
        self.sequences.len()
    }
    pub fn pair_count(&self) -> usize {
        self.slots.len()
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
    pub fn status_stack_bytes(&self) -> usize {
        self.status_stack_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
    pub fn sequence(
        &self,
        index: usize,
    ) -> Option<BidiBracketSequence<'_, 'w, 'r, 'e, 'i, 'n, 's>> {
        self.sequences.get(index)?;
        Some(BidiBracketSequence { owner: self, index })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiBracketSequence<'v, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiBracketPairs<'w, 'r, 'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'w, 'r, 'e, 'i, 'n, 's> BidiBracketSequence<'v, 'w, 'r, 'e, 'i, 'n, 's> {
    fn record(&self) -> &SequenceRecord {
        &self.owner.sequences[self.index]
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn status(&self) -> BidiBracketSequenceStatus {
        self.record().status
    }
    pub fn pair_count(&self) -> usize {
        self.record().pairs.len()
    }
    pub fn weak_sequence(&self) -> BidiWeakSequence<'_, 'r, 'e, 'i, 'n, 's> {
        self.owner
            .weak
            .sequence(self.index)
            .expect("bracket sequence has weak predecessor")
    }
    pub fn pair(&self, local_index: usize) -> Option<BidiBracketPair<'_, 'w, 'r, 'e, 'i, 'n, 's>> {
        if local_index >= self.pair_count() {
            return None;
        }
        let slot = self.record().pairs.start.checked_add(local_index)?;
        self.owner.slots.get(slot)?;
        Some(BidiBracketPair {
            owner: self.owner,
            sequence: self.index,
            slot,
        })
    }
    pub fn pairs(&self) -> BidiBracketPairsIter<'_, 'w, 'r, 'e, 'i, 'n, 's> {
        BidiBracketPairsIter {
            owner: self.owner,
            sequence: self.index,
            next: self.record().pairs.start,
            end: self.record().pairs.end,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiBracketPair<'v, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiBracketPairs<'w, 'r, 'e, 'i, 'n, 's>,
    sequence: usize,
    slot: usize,
}
impl<'v, 'w, 'r, 'e, 'i, 'n, 's> BidiBracketPair<'v, 'w, 'r, 'e, 'i, 'n, 's> {
    pub fn sequence_index(&self) -> usize {
        self.sequence
    }
    pub fn pair_index(&self) -> usize {
        self.slot - self.owner.sequences[self.sequence].pairs.start
    }
    pub fn opening_scalar_index(&self) -> usize {
        self.owner.slots[self.slot].opening
    }
    pub fn closing_scalar_index(&self) -> usize {
        self.owner.slots[self.slot]
            .closing
            .expect("published pair has closing endpoint")
    }
    pub fn opening(&self) -> BidiWeakScalar<'_, 'r, 'e, 'i, 'n, 's> {
        self.owner
            .weak
            .scalar(self.opening_scalar_index())
            .expect("opening has original weak scalar")
    }
    pub fn closing(&self) -> BidiWeakScalar<'_, 'r, 'e, 'i, 'n, 's> {
        self.owner
            .weak
            .scalar(self.closing_scalar_index())
            .expect("closing has original weak scalar")
    }
}
#[derive(Debug)]
pub struct BidiBracketPairsIter<'v, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiBracketPairs<'w, 'r, 'e, 'i, 'n, 's>,
    sequence: usize,
    next: usize,
    end: usize,
}
impl<'v, 'w, 'r, 'e, 'i, 'n, 's> Iterator for BidiBracketPairsIter<'v, 'w, 'r, 'e, 'i, 'n, 's> {
    type Item = BidiBracketPair<'v, 'w, 'r, 'e, 'i, 'n, 's>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.end {
            return None;
        }
        let slot = self.next;
        self.next += 1;
        Some(BidiBracketPair {
            owner: self.owner,
            sequence: self.sequence,
            slot,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end - self.next;
        (remaining, Some(remaining))
    }
}
impl ExactSizeIterator for BidiBracketPairsIter<'_, '_, '_, '_, '_, '_, '_> {
    fn len(&self) -> usize {
        self.end - self.next
    }
}
impl FusedIterator for BidiBracketPairsIter<'_, '_, '_, '_, '_, '_, '_> {}
// Qualified canonical equivalence on eligible closing brackets only. Compatibility
// decompositions are deliberately absent; raw property APIs stay unchanged.
fn closing_key(c: char) -> char {
    if c == '\u{232a}' { '\u{3009}' } else { c }
}
fn bytes_for<T>(capacity: usize) -> Result<usize, BidiBracketPairError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&b| b <= isize::MAX as usize)
        .ok_or(BidiBracketPairError::Bytes)
}
fn payload(parts: [usize; 3], allowance: usize) -> Result<usize, BidiBracketPairError> {
    parts.into_iter().try_fold(0usize, |a, b| {
        a.checked_add(b)
            .filter(|&x| x <= allowance)
            .ok_or(BidiBracketPairError::Bytes)
    })
}
fn work_for(a: usize, s: usize, allowance: usize) -> Result<usize, BidiBracketPairError> {
    a.checked_mul(65)
        .and_then(|w| w.checked_add(s))
        .and_then(|w| w.checked_add(if a == 0 { 0 } else { STACK_CAPACITY }))
        .and_then(|w| w.checked_add(1))
        .filter(|&w| w <= allowance)
        .ok_or(BidiBracketPairError::Work)
}
fn admit_pair(count: usize, allowance: usize) -> Result<usize, BidiBracketPairError> {
    count
        .checked_add(1)
        .filter(|&n| n <= allowance)
        .ok_or(BidiBracketPairError::Pairs)
}
fn identify_nonempty(
    weak: &BidiWeak<'_, '_, '_, '_, '_>,
    slots: &mut Vec<PendingPair>,
    sequences: &mut Vec<SequenceRecord>,
    pair_limit: usize,
) -> Result<(), BidiBracketPairError> {
    let mut stack = [StackEntry {
        expected_close: '\0',
        slot: 0,
    }; STACK_CAPACITY];
    for si in 0..weak.sequence_count() {
        let start = slots.len();
        let mut used = 0;
        let mut overflow = false;
        let sequence = weak.sequence(si).expect("admitted weak sequence");
        for scalar in sequence.scalars() {
            if scalar.current_type() != Some(BidiClass::On) {
                continue;
            }
            let run_scalar = scalar.run_scalar();
            let explicit = run_scalar.explicit();
            let original = explicit.original();
            let c = original
                .text()
                .chars()
                .next()
                .expect("original scalar text is nonempty");
            match bidi_paired_bracket_type(c) {
                BidiPairedBracketType::Open => {
                    if used == STACK_CAPACITY {
                        overflow = true;
                        break;
                    }
                    let slot = slots.len();
                    slots.push(PendingPair {
                        opening: scalar.index(),
                        closing: None,
                    });
                    stack[used] = StackEntry {
                        expected_close: closing_key(
                            bidi_paired_bracket(c).expect("qualified opener has raw mate"),
                        ),
                        slot,
                    };
                    used += 1;
                }
                BidiPairedBracketType::Close => {
                    let expected = closing_key(c);
                    if let Some(position) = (0..used)
                        .rev()
                        .find(|&j| stack[j].expected_close == expected)
                    {
                        slots[stack[position].slot].closing = Some(scalar.index());
                        used = position;
                    }
                }
                BidiPairedBracketType::None => {}
            }
        }
        if overflow {
            slots.truncate(start);
            sequences.push(SequenceRecord {
                pairs: start..start,
                status: BidiBracketSequenceStatus::Overflow,
            });
            continue;
        }
        // Stable compaction admits only final survivors, never provisional pairs that
        // might be discarded by a later overflow in this complete sequence.
        let mut write = start;
        for read in start..slots.len() {
            let pair = slots[read];
            if pair.closing.is_some() {
                let next = admit_pair(write, pair_limit)?;
                slots[write] = pair;
                write = next;
            }
        }
        slots.truncate(write);
        sequences.push(SequenceRecord {
            pairs: start..write,
            status: BidiBracketSequenceStatus::Complete,
        });
    }
    Ok(())
}
/// Identify eligible original brackets within complete post-W7 sequences.
/// Overflow is a successful, empty whole-sequence outcome, not a budget error.
///
/// # Errors
/// Scalars, ActiveScalars and Sequences precede checked reserved Work admission.
/// Requested pending/header allocations each obey isize addressability; their sum
/// plus selected fixed stack bytes must fit retained_bytes. Actual pending plus
/// requested header and then both actual capacities are admitted before filling or
/// fixed-stack initialization. Bytes includes arithmetic overflow; Allocation denotes
/// fallible reservation failure. Pairs is checked only during normal final compaction.
/// Failure publishes no owner and preserves predecessors. Metrics exclude predecessor
/// buffers, other locals, compiler frame layout and rejected-allocation physical peaks.
pub fn identify_bidi_bracket_pairs<'w, 'r, 'e, 'i, 'n, 's>(
    weak: &'w BidiWeak<'r, 'e, 'i, 'n, 's>,
    limits: BidiBracketPairLimits,
) -> Result<BidiBracketPairs<'w, 'r, 'e, 'i, 'n, 's>, BidiBracketPairError> {
    let n = weak.scalar_count();
    let a = weak.active_scalar_count();
    let s = weak.sequence_count();
    if n > limits.scalars.get() {
        return Err(BidiBracketPairError::Scalars);
    }
    if a > limits.active_scalars.get() {
        return Err(BidiBracketPairError::ActiveScalars);
    }
    if s > limits.sequences.get() {
        return Err(BidiBracketPairError::Sequences);
    }
    let work = work_for(a, s, limits.work.get())?;
    let status_stack_bytes = if a == 0 {
        0
    } else {
        size_of::<[StackEntry; STACK_CAPACITY]>()
    };
    let mut bytes = [
        bytes_for::<PendingPair>(a)?,
        bytes_for::<SequenceRecord>(s)?,
        status_stack_bytes,
    ];
    payload(bytes, limits.retained_bytes.get())?;
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(a)
        .map_err(|_| BidiBracketPairError::Allocation)?;
    bytes[0] = bytes_for::<PendingPair>(slots.capacity())?;
    payload(bytes, limits.retained_bytes.get())?;
    let mut sequences = Vec::new();
    sequences
        .try_reserve_exact(s)
        .map_err(|_| BidiBracketPairError::Allocation)?;
    bytes[1] = bytes_for::<SequenceRecord>(sequences.capacity())?;
    let peak_retained_bytes = payload(bytes, limits.retained_bytes.get())?;
    let retained_bytes = payload([bytes[0], bytes[1], 0], limits.retained_bytes.get())?;
    if a > 0 {
        identify_nonempty(weak, &mut slots, &mut sequences, limits.pairs.get())?;
    }
    Ok(BidiBracketPairs {
        weak,
        slots,
        sequences,
        retained_bytes,
        peak_retained_bytes,
        status_stack_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{
        bidi_explicit::*, bidi_initial::*, bidi_isolates::*, bidi_runs::*, bidi_weak::*,
    };
    #[test]
    fn actual_capacity_survives_compaction_and_rollback() {
        let n = NonZeroUsize::new(usize::MAX).unwrap();
        for source in [
            String::new(),
            "\u{ad}".into(),
            "abc".into(),
            "(())(".into(),
            format!("()(){}", "(".repeat(64)),
        ] {
            let initial = evaluate_bidi_initial(
                &source,
                BidiParagraphMode::AutoLeftToRight,
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
            let r = identify_bidi_bracket_pairs(
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
            assert!(r.slots.capacity() >= weak.active_scalar_count());
            assert!(r.sequences.capacity() >= weak.sequence_count());
            assert_eq!(r.sequences.len(), weak.sequence_count());
            assert_eq!(
                r.retained_bytes,
                r.slots.capacity() * size_of::<PendingPair>()
                    + r.sequences.capacity() * size_of::<SequenceRecord>()
            );
            assert_eq!(
                r.status_stack_bytes,
                if weak.active_scalar_count() == 0 {
                    0
                } else {
                    size_of::<[StackEntry; 63]>()
                }
            );
            assert_eq!(
                r.peak_retained_bytes,
                r.retained_bytes + r.status_stack_bytes
            );
            assert!(2 * r.slots.len() <= weak.active_scalar_count());
            assert!(r.slots.iter().all(|p| p.closing.is_some()));
            if source.len() > 64 {
                assert!(r.slots.is_empty());
                assert!(r.slots.capacity() >= 68);
                assert_eq!(r.sequences[0].status, BidiBracketSequenceStatus::Overflow);
            }
        }
    }
    #[test]
    fn checked_arithmetic_and_key_domain() {
        assert_eq!(
            bytes_for::<PendingPair>(usize::MAX),
            Err(BidiBracketPairError::Bytes)
        );
        assert_eq!(
            bytes_for::<SequenceRecord>(isize::MAX as usize / size_of::<SequenceRecord>() + 1),
            Err(BidiBracketPairError::Bytes)
        );
        assert_eq!(
            payload([usize::MAX, 1, 0], usize::MAX),
            Err(BidiBracketPairError::Bytes)
        );
        assert_eq!(
            payload([isize::MAX as usize, 1, 0], usize::MAX),
            Ok(isize::MAX as usize + 1)
        );
        assert_eq!(
            work_for(usize::MAX, 0, usize::MAX),
            Err(BidiBracketPairError::Work)
        );
        assert_eq!(
            work_for(0, usize::MAX, usize::MAX),
            Err(BidiBracketPairError::Work)
        );
        assert_eq!(work_for(0, 0, 1), Ok(1));
        assert_eq!(
            admit_pair(usize::MAX, usize::MAX),
            Err(BidiBracketPairError::Pairs)
        );
        assert_eq!(admit_pair(1, 1), Err(BidiBracketPairError::Pairs));
        assert_eq!(closing_key('\u{232a}'), '\u{3009}');
        assert_eq!(closing_key('\u{3009}'), '\u{3009}');
        assert_eq!(closing_key('\u{ff09}'), '\u{ff09}');
    }
}
