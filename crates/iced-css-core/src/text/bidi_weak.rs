//! W1–W7 current types over complete qualified isolating run sequences.
//! Original classes, explicit levels and topology remain in the borrowed owner chain.
use super::{
    bidi_data::BidiClass,
    bidi_explicit::BidiExplicitLevel,
    bidi_runs::{BidiRunScalar, BidiRunSequences, BidiSequenceDirection},
};
use std::{iter::FusedIterator, mem::size_of, num::NonZeroUsize};
#[derive(Debug, Clone, Copy)]
pub struct BidiWeakLimits {
    pub scalars: NonZeroUsize,
    pub active_scalars: NonZeroUsize,
    pub runs: NonZeroUsize,
    pub sequences: NonZeroUsize,
    /// Actual working plus scratch capacities, excluding predecessors, fixed locals,
    /// allocator metadata and physical peaks of rejected reservations.
    pub retained_bytes: NonZeroUsize,
    /// Exact structural reservation: 1 + 2r + 9a + 8s.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiWeakError {
    Scalars,
    ActiveScalars,
    Runs,
    Sequences,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for BidiWeakError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "weak bidi resolution: {self:?}")
    }
}
impl std::error::Error for BidiWeakError {}
#[derive(Clone, Copy)]
struct W5Right {
    en: bool,
}
/// Immutable post-W7 types; no public partial-stage arrays or mutable state.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(source);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(initial);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(isolates);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(explicit);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(runs);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let view=result.scalar(0).unwrap();
/// drop(result);
/// assert_eq!(view.index(),0);
/// ```
///
/// ```compile_fail,E0597
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let runs=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_weak(&runs,BidiWeakLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let mut iterator;
/// { let sequence=result.sequence(0).unwrap(); iterator=sequence.scalars(); }
/// assert!(iterator.next().is_some());
/// ```
#[must_use]
#[derive(Debug)]
pub struct BidiWeak<'r, 'e, 'i, 'n, 's> {
    runs: &'r BidiRunSequences<'e, 'i, 'n, 's>,
    types: Vec<BidiClass>,
    retained_bytes: usize,
    peak_retained_bytes: usize,
    work: usize,
}
impl<'r, 'e, 'i, 'n, 's> BidiWeak<'r, 'e, 'i, 'n, 's> {
    pub fn runs(&self) -> &'r BidiRunSequences<'e, 'i, 'n, 's> {
        self.runs
    }
    pub fn scalar_count(&self) -> usize {
        self.runs.scalar_count()
    }
    pub fn active_scalar_count(&self) -> usize {
        self.runs.active_scalar_count()
    }
    pub fn run_count(&self) -> usize {
        self.runs.run_count()
    }
    pub fn sequence_count(&self) -> usize {
        self.runs.sequence_count()
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
    pub fn scalar(&self, index: usize) -> Option<BidiWeakScalar<'_, 'r, 'e, 'i, 'n, 's>> {
        self.runs.scalar(index)?;
        Some(BidiWeakScalar { owner: self, index })
    }
    pub fn sequence(&self, index: usize) -> Option<BidiWeakSequence<'_, 'r, 'e, 'i, 'n, 's>> {
        self.runs.sequence(index)?;
        Some(BidiWeakSequence { owner: self, index })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiWeakScalar<'v, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiWeak<'r, 'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'r, 'e, 'i, 'n, 's> BidiWeakScalar<'v, 'r, 'e, 'i, 'n, 's> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn run_scalar(&self) -> BidiRunScalar<'_, 'e, 'i, 'n, 's> {
        self.owner
            .runs
            .scalar(self.index)
            .expect("weak scalar has predecessor")
    }
    pub fn current_type(&self) -> Option<BidiClass> {
        self.run_scalar()
            .membership()
            .map(|m| self.owner.types[m.active_index()])
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiWeakSequence<'v, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiWeak<'r, 'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'r, 'e, 'i, 'n, 's> BidiWeakSequence<'v, 'r, 'e, 'i, 'n, 's> {
    fn parent(&self) -> super::bidi_runs::BidiIsolatingSequence<'_, 'e, 'i, 'n, 's> {
        self.owner
            .runs
            .sequence(self.index)
            .expect("weak sequence has predecessor")
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn level(&self) -> BidiExplicitLevel {
        self.parent().level()
    }
    pub fn sos(&self) -> BidiSequenceDirection {
        self.parent().sos()
    }
    pub fn eos(&self) -> BidiSequenceDirection {
        self.parent().eos()
    }
    pub fn scalar_count(&self) -> usize {
        self.parent().scalar_count()
    }
    pub fn run_count(&self) -> usize {
        self.parent().run_count()
    }
    pub fn scalars(&self) -> BidiWeakSequenceScalars<'_, 'r, 'e, 'i, 'n, 's> {
        let first = {
            let parent = self.parent();
            parent.runs().next().map(|run| run.index())
        };
        BidiWeakSequenceScalars {
            owner: self.owner,
            next_run: first,
            local_scalar: 0,
            remaining: self.scalar_count(),
        }
    }
}
#[derive(Debug)]
pub struct BidiWeakSequenceScalars<'v, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiWeak<'r, 'e, 'i, 'n, 's>,
    next_run: Option<usize>,
    local_scalar: usize,
    remaining: usize,
}
impl<'v, 'r, 'e, 'i, 'n, 's> Iterator for BidiWeakSequenceScalars<'v, 'r, 'e, 'i, 'n, 's> {
    type Item = BidiWeakScalar<'v, 'r, 'e, 'i, 'n, 's>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let run = self
            .owner
            .runs
            .run(self.next_run.expect("nonempty weak cursor"))
            .expect("cursor run");
        let index = run
            .scalar(self.local_scalar)
            .expect("cursor scalar")
            .index();
        self.remaining -= 1;
        self.local_scalar += 1;
        if self.remaining == 0 {
            self.next_run = None;
        } else if self.local_scalar == run.scalar_count() {
            self.next_run = Some(run.next_run_index().expect("sequence has remaining run"));
            self.local_scalar = 0;
        }
        Some(BidiWeakScalar {
            owner: self.owner,
            index,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for BidiWeakSequenceScalars<'_, '_, '_, '_, '_, '_> {
    fn len(&self) -> usize {
        self.remaining
    }
}
impl FusedIterator for BidiWeakSequenceScalars<'_, '_, '_, '_, '_, '_> {}
fn active_index(scalar: &BidiRunScalar<'_, '_, '_, '_, '_>) -> usize {
    scalar
        .membership()
        .expect("sequence scalar retained")
        .active_index()
}
fn initialize(runs: &BidiRunSequences<'_, '_, '_, '_>, types: &mut Vec<BidiClass>) {
    for ri in 0..runs.run_count() {
        let run = runs.run(ri).expect("admitted run");
        for local in 0..run.scalar_count() {
            let scalar = run.scalar(local).expect("admitted run scalar");
            assert_eq!(
                active_index(&scalar),
                types.len(),
                "global run traversal follows active order"
            );
            match scalar.explicit().state() {
                super::bidi_explicit::BidiExplicitState::Retained { current_type, .. } => {
                    types.push(current_type)
                }
                super::bidi_explicit::BidiExplicitState::RemovedByX9 => {
                    panic!("active scalar retained")
                }
            }
        }
    }
}
#[derive(Clone, Copy)]
enum Rule {
    W1,
    W2,
    W3,
    W4,
    W5Left,
    W6,
    W7,
}
fn apply_rule(runs: &BidiRunSequences<'_, '_, '_, '_>, types: &mut [BidiClass], rule: Rule) {
    use BidiClass::*;
    for si in 0..runs.sequence_count() {
        let seq = runs.sequence(si).expect("admitted sequence");
        let mut previous = seq.sos().bidi_class();
        let mut strong = previous;
        let mut en = false;
        let mut scalars = seq.scalars().peekable();
        while let Some(scalar) = scalars.next() {
            let index = active_index(&scalar);
            let old = types[index];
            let next = match rule {
                Rule::W1 => {
                    if old == Nsm {
                        if matches!(previous, Lri | Rli | Fsi | Pdi) {
                            On
                        } else {
                            previous
                        }
                    } else {
                        old
                    }
                }
                Rule::W2 => {
                    if matches!(old, L | R | Al) {
                        strong = old;
                    }
                    if old == En && strong == Al { An } else { old }
                }
                Rule::W3 => {
                    if old == Al {
                        R
                    } else {
                        old
                    }
                }
                Rule::W4 => {
                    let right = scalars
                        .peek()
                        .map_or_else(|| seq.eos().bidi_class(), |v| types[active_index(v)]);
                    if (old == Es && previous == En && right == En)
                        || (old == Cs && previous == right && matches!(previous, En | An))
                    {
                        previous
                    } else {
                        old
                    }
                }
                Rule::W5Left => {
                    if old == Et && en {
                        En
                    } else {
                        old
                    }
                }
                Rule::W6 => {
                    if matches!(old, Es | Et | Cs) {
                        On
                    } else {
                        old
                    }
                }
                Rule::W7 => {
                    if matches!(old, L | R) {
                        strong = old;
                    }
                    if old == En && strong == L { L } else { old }
                }
            };
            types[index] = next;
            // W4 must keep the unmodified W3 left neighbor. W1 chains use updated types.
            previous = if matches!(rule, Rule::W4) { old } else { next };
            en = next == En;
        }
    }
}
fn apply_w5_right(
    runs: &BidiRunSequences<'_, '_, '_, '_>,
    types: &mut [BidiClass],
    scratch: &mut Vec<W5Right>,
) {
    scratch.resize(runs.sequence_count(), W5Right { en: false });
    for ri in (0..runs.run_count()).rev() {
        let run = runs.run(ri).expect("admitted reverse run");
        let carry = &mut scratch[run.sequence_index()];
        for local in (0..run.scalar_count()).rev() {
            let scalar = run.scalar(local).expect("admitted reverse scalar");
            let index = active_index(&scalar);
            if types[index] == BidiClass::Et && carry.en {
                types[index] = BidiClass::En;
            }
            carry.en = types[index] == BidiClass::En;
        }
    }
}
fn bytes_for<T>(capacity: usize) -> Result<usize, BidiWeakError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&b| b <= isize::MAX as usize)
        .ok_or(BidiWeakError::Bytes)
}
fn combined(a: usize, b: usize, allowance: usize) -> Result<usize, BidiWeakError> {
    a.checked_add(b)
        .filter(|&b| b <= allowance)
        .ok_or(BidiWeakError::Bytes)
}
fn work_for(a: usize, r: usize, s: usize, allowance: usize) -> Result<usize, BidiWeakError> {
    a.checked_mul(9)
        .and_then(|a| r.checked_mul(2).and_then(|r| a.checked_add(r)))
        .and_then(|w| s.checked_mul(8).and_then(|s| w.checked_add(s)))
        .and_then(|w| w.checked_add(1))
        .filter(|&w| w <= allowance)
        .ok_or(BidiWeakError::Work)
}
/// Resolve complete-sequence weak rules in order; publish only post-W7 current types.
///
/// # Errors
/// Scalars, ActiveScalars, Runs and Sequences precede checked Work admission.
/// Requested working/scratch payloads each obey isize addressability and their sum
/// obeys the caller allowance. Actual working plus requested scratch is checked
/// before scratch reservation; both actual capacities are checked before any fill.
/// Bytes includes arithmetic overflow; Allocation denotes fallible reservation failure.
/// Failures preserve all predecessors and publish no partial owner. The local metric
/// excludes borrowed predecessor buffers and physical allocator peaks.
pub fn resolve_bidi_weak<'r, 'e, 'i, 'n, 's>(
    runs: &'r BidiRunSequences<'e, 'i, 'n, 's>,
    limits: BidiWeakLimits,
) -> Result<BidiWeak<'r, 'e, 'i, 'n, 's>, BidiWeakError> {
    let n = runs.scalar_count();
    let a = runs.active_scalar_count();
    let r = runs.run_count();
    let s = runs.sequence_count();
    if n > limits.scalars.get() {
        return Err(BidiWeakError::Scalars);
    }
    if a > limits.active_scalars.get() {
        return Err(BidiWeakError::ActiveScalars);
    }
    if r > limits.runs.get() {
        return Err(BidiWeakError::Runs);
    }
    if s > limits.sequences.get() {
        return Err(BidiWeakError::Sequences);
    }
    let work = work_for(a, r, s, limits.work.get())?;
    let scratch_requested = bytes_for::<W5Right>(s)?;
    combined(
        bytes_for::<BidiClass>(a)?,
        scratch_requested,
        limits.retained_bytes.get(),
    )?;
    let mut types = Vec::new();
    types
        .try_reserve_exact(a)
        .map_err(|_| BidiWeakError::Allocation)?;
    let retained_bytes = bytes_for::<BidiClass>(types.capacity())?;
    combined(
        retained_bytes,
        scratch_requested,
        limits.retained_bytes.get(),
    )?;
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(s)
        .map_err(|_| BidiWeakError::Allocation)?;
    let peak_retained_bytes = combined(
        retained_bytes,
        bytes_for::<W5Right>(scratch.capacity())?,
        limits.retained_bytes.get(),
    )?;
    initialize(runs, &mut types);
    for rule in [Rule::W1, Rule::W2, Rule::W3, Rule::W4, Rule::W5Left] {
        apply_rule(runs, &mut types, rule);
    }
    apply_w5_right(runs, &mut types, &mut scratch);
    drop(scratch);
    for rule in [Rule::W6, Rule::W7] {
        apply_rule(runs, &mut types, rule);
    }
    Ok(BidiWeak {
        runs,
        types,
        retained_bytes,
        peak_retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{bidi_explicit::*, bidi_initial::*, bidi_isolates::*, bidi_runs::*};
    #[test]
    fn every_rule_snapshot_and_actual_payload() {
        use BidiClass::*;
        let n = NonZeroUsize::new(usize::MAX).unwrap();
        let initial = evaluate_bidi_initial(
            "ا\u{301}1,1$",
            BidiParagraphMode::RightToLeft,
            BidiInitialLimits {
                input_bytes: n,
                scalars: n,
                paragraphs: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let iso = prepare_bidi_isolates(
            &initial,
            BidiIsolateLimits {
                scalars: n,
                paragraphs: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let ex = resolve_bidi_explicit(
            &iso,
            BidiExplicitLimits {
                scalars: n,
                paragraphs: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let runs = prepare_bidi_run_sequences(
            &ex,
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
        let expected = [
            [Al, Al, En, Cs, En, Et],
            [Al, Al, An, Cs, An, Et],
            [R, R, An, Cs, An, Et],
            [R, R, An, An, An, Et],
            [R, R, An, An, An, Et],
            [R, R, An, An, An, On],
            [R, R, An, An, An, On],
        ];
        let mut types = Vec::with_capacity(runs.active_scalar_count());
        let mut scratch = Vec::with_capacity(runs.sequence_count());
        initialize(&runs, &mut types);
        for (stage, rule) in [
            Rule::W1,
            Rule::W2,
            Rule::W3,
            Rule::W4,
            Rule::W5Left,
            Rule::W6,
            Rule::W7,
        ]
        .into_iter()
        .enumerate()
        {
            apply_rule(&runs, &mut types, rule);
            if stage == 4 {
                apply_w5_right(&runs, &mut types, &mut scratch);
            }
            assert_eq!(types, expected[stage]);
        }
        let r = resolve_bidi_weak(
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
        assert_eq!(r.types, types);
        assert_eq!(r.types.len(), runs.active_scalar_count());
        assert_eq!(
            r.retained_bytes,
            r.types.capacity() * size_of::<BidiClass>()
        );
        assert!(r.types.capacity() >= r.types.len());
        assert!(
            (r.peak_retained_bytes - r.retained_bytes) / size_of::<W5Right>()
                >= runs.sequence_count()
        );
    }
    #[test]
    fn checked_arithmetic() {
        assert_eq!(
            bytes_for::<BidiClass>(usize::MAX),
            Err(BidiWeakError::Bytes)
        );
        assert_eq!(
            bytes_for::<W5Right>(isize::MAX as usize + 1),
            Err(BidiWeakError::Bytes)
        );
        assert_eq!(
            combined(usize::MAX, 1, usize::MAX),
            Err(BidiWeakError::Bytes)
        );
        assert_eq!(
            combined(isize::MAX as usize, 1, usize::MAX),
            Ok(isize::MAX as usize + 1)
        );
        for (a, r, s) in [(usize::MAX, 0, 0), (0, usize::MAX, 0), (0, 0, usize::MAX)] {
            assert_eq!(work_for(a, r, s, usize::MAX), Err(BidiWeakError::Work));
        }
        assert_eq!(work_for(1, 1, 1, 19), Err(BidiWeakError::Work));
        assert_eq!(work_for(0, 0, 0, 1), Ok(1));
    }
}
