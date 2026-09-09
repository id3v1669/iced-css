//! BD7 level runs, BD13 isolating run sequences and X10 boundary directions.
//! Consumes explicit levels, not final I/L levels; no W/N/I/L or source transformation.
use super::{
    bidi_data::BidiClass,
    bidi_explicit::{BidiExplicit, BidiExplicitLevel, BidiExplicitScalar, BidiExplicitState},
};
use std::{iter::FusedIterator, mem::size_of, num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct BidiRunLimits {
    pub scalars: NonZeroUsize,
    pub paragraphs: NonZeroUsize,
    pub active_scalars: NonZeroUsize,
    pub runs: NonZeroUsize,
    pub sequences: NonZeroUsize,
    /// Actual successful four-vector payload, excluding borrowed predecessors,
    /// fixed locals, allocator overhead and physical rejected-reservation peaks.
    pub retained_bytes: NonZeroUsize,
    /// Exact structural reservation: 1 + 2p + 2n + 3r.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiRunError {
    Scalars,
    Paragraphs,
    ActiveScalars,
    Runs,
    Sequences,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for BidiRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bidi run preparation: {self:?}")
    }
}
impl std::error::Error for BidiRunError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiSequenceDirection {
    LeftToRight,
    RightToLeft,
}
impl BidiSequenceDirection {
    pub fn bidi_class(self) -> BidiClass {
        match self {
            Self::LeftToRight => BidiClass::L,
            Self::RightToLeft => BidiClass::R,
        }
    }
}
#[derive(Debug, Clone, Copy)]
struct Membership {
    active_index: usize,
    run_index: usize,
}
#[derive(Debug)]
struct RunRecord {
    active_range: Range<usize>,
    paragraph_index: usize,
    level: BidiExplicitLevel,
    next_run: Option<usize>,
    incoming: bool,
    sequence: Option<usize>,
}
#[derive(Debug)]
struct SequenceRecord {
    first_run: usize,
    last_run: usize,
    paragraph_index: usize,
    run_count: usize,
    scalar_count: usize,
    level: BidiExplicitLevel,
    sos: BidiSequenceDirection,
    eos: BidiSequenceDirection,
}
/// One immutable owner for grouping, topology and original-source membership.
/// Runs and sequences can span source gaps: they are not contiguous text slices.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(source);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(initial);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(isolates);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(explicit);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let view=result.scalar(0).unwrap();
/// drop(result);
/// assert_eq!(view.index(),0);
/// ```
///
/// ```compile_fail,E0597
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let mut iterator;
/// { let sequence=result.sequence(0).unwrap(); iterator=sequence.scalars(); }
/// assert!(iterator.next().is_some());
/// ```
///
/// ```compile_fail,E0597
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let explicit=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=prepare_bidi_run_sequences(&explicit,BidiRunLimits{scalars:n,paragraphs:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let mut iterator;
/// { let sequence=result.sequence(0).unwrap(); iterator=sequence.runs(); }
/// assert!(iterator.next().is_some());
/// ```
#[must_use]
#[derive(Debug)]
pub struct BidiRunSequences<'explicit, 'isolates, 'initial, 'source> {
    explicit: &'explicit BidiExplicit<'isolates, 'initial, 'source>,
    membership: Vec<Option<Membership>>,
    active: Vec<usize>,
    runs: Vec<RunRecord>,
    sequences: Vec<SequenceRecord>,
    retained_bytes: usize,
    work: usize,
}
impl<'e, 'i, 'n, 's> BidiRunSequences<'e, 'i, 'n, 's> {
    pub fn explicit(&self) -> &'e BidiExplicit<'i, 'n, 's> {
        self.explicit
    }
    pub fn scalar_count(&self) -> usize {
        self.explicit.scalar_count()
    }
    pub fn paragraph_count(&self) -> usize {
        self.explicit.paragraph_count()
    }
    pub fn active_scalar_count(&self) -> usize {
        self.active.len()
    }
    pub fn run_count(&self) -> usize {
        self.runs.len()
    }
    pub fn sequence_count(&self) -> usize {
        self.sequences.len()
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// All four vectors survive; there is no owned scratch.
    pub fn peak_retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
    pub fn scalar(&self, index: usize) -> Option<BidiRunScalar<'_, 'e, 'i, 'n, 's>> {
        self.membership.get(index)?;
        Some(BidiRunScalar { owner: self, index })
    }
    pub fn run(&self, index: usize) -> Option<BidiLevelRun<'_, 'e, 'i, 'n, 's>> {
        self.runs.get(index)?;
        Some(BidiLevelRun { owner: self, index })
    }
    pub fn sequence(&self, index: usize) -> Option<BidiIsolatingSequence<'_, 'e, 'i, 'n, 's>> {
        self.sequences.get(index)?;
        Some(BidiIsolatingSequence { owner: self, index })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BidiRunMembership {
    active_index: usize,
    run_index: usize,
    sequence_index: usize,
}
impl BidiRunMembership {
    pub fn active_index(&self) -> usize {
        self.active_index
    }
    pub fn run_index(&self) -> usize {
        self.run_index
    }
    pub fn sequence_index(&self) -> usize {
        self.sequence_index
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiRunScalar<'v, 'e, 'i, 'n, 's> {
    owner: &'v BidiRunSequences<'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'e, 'i, 'n, 's> BidiRunScalar<'v, 'e, 'i, 'n, 's> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn explicit(&self) -> BidiExplicitScalar<'_, 'i, 'n, 's> {
        self.owner
            .explicit
            .scalar(self.index)
            .expect("run scalar has explicit record")
    }
    pub fn membership(&self) -> Option<BidiRunMembership> {
        self.owner.membership[self.index].map(|m| BidiRunMembership {
            active_index: m.active_index,
            run_index: m.run_index,
            sequence_index: self.owner.runs[m.run_index]
                .sequence
                .expect("published run has sequence"),
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiLevelRun<'v, 'e, 'i, 'n, 's> {
    owner: &'v BidiRunSequences<'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'e, 'i, 'n, 's> BidiLevelRun<'v, 'e, 'i, 'n, 's> {
    fn record(&self) -> &RunRecord {
        &self.owner.runs[self.index]
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn paragraph_index(&self) -> usize {
        self.record().paragraph_index
    }
    pub fn level(&self) -> BidiExplicitLevel {
        self.record().level
    }
    pub fn sequence_index(&self) -> usize {
        self.record().sequence.expect("published run has sequence")
    }
    pub fn next_run_index(&self) -> Option<usize> {
        self.record().next_run
    }
    pub fn scalar_count(&self) -> usize {
        self.record().active_range.len()
    }
    pub fn scalar(&self, local_index: usize) -> Option<BidiRunScalar<'_, 'e, 'i, 'n, 's>> {
        if local_index >= self.scalar_count() {
            return None;
        }
        let position = self.record().active_range.start.checked_add(local_index)?;
        self.owner.scalar(*self.owner.active.get(position)?)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiIsolatingSequence<'v, 'e, 'i, 'n, 's> {
    owner: &'v BidiRunSequences<'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'e, 'i, 'n, 's> BidiIsolatingSequence<'v, 'e, 'i, 'n, 's> {
    fn record(&self) -> &SequenceRecord {
        &self.owner.sequences[self.index]
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn paragraph_index(&self) -> usize {
        self.record().paragraph_index
    }
    pub fn level(&self) -> BidiExplicitLevel {
        self.record().level
    }
    pub fn sos(&self) -> BidiSequenceDirection {
        self.record().sos
    }
    pub fn eos(&self) -> BidiSequenceDirection {
        self.record().eos
    }
    pub fn scalar_count(&self) -> usize {
        self.record().scalar_count
    }
    pub fn run_count(&self) -> usize {
        self.record().run_count
    }
    pub fn runs(&self) -> BidiSequenceRuns<'_, 'e, 'i, 'n, 's> {
        debug_assert!(self.owner.runs[self.record().last_run].next_run.is_none());
        BidiSequenceRuns {
            owner: self.owner,
            next_run: Some(self.record().first_run),
            remaining: self.record().run_count,
        }
    }
    pub fn scalars(&self) -> BidiSequenceScalars<'_, 'e, 'i, 'n, 's> {
        let run = self.record().first_run;
        BidiSequenceScalars {
            owner: self.owner,
            next_run: Some(run),
            active_cursor: self.owner.runs[run].active_range.start,
            remaining: self.record().scalar_count,
        }
    }
}
#[derive(Debug)]
pub struct BidiSequenceRuns<'v, 'e, 'i, 'n, 's> {
    owner: &'v BidiRunSequences<'e, 'i, 'n, 's>,
    next_run: Option<usize>,
    remaining: usize,
}
impl<'v, 'e, 'i, 'n, 's> Iterator for BidiSequenceRuns<'v, 'e, 'i, 'n, 's> {
    type Item = BidiLevelRun<'v, 'e, 'i, 'n, 's>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let index = self.next_run.expect("nonempty sequence cursor");
        self.remaining -= 1;
        self.next_run = if self.remaining == 0 {
            None
        } else {
            Some(
                self.owner.runs[index]
                    .next_run
                    .expect("sequence has remaining run"),
            )
        };
        Some(BidiLevelRun {
            owner: self.owner,
            index,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for BidiSequenceRuns<'_, '_, '_, '_, '_> {
    fn len(&self) -> usize {
        self.remaining
    }
}
impl FusedIterator for BidiSequenceRuns<'_, '_, '_, '_, '_> {}
#[derive(Debug)]
pub struct BidiSequenceScalars<'v, 'e, 'i, 'n, 's> {
    owner: &'v BidiRunSequences<'e, 'i, 'n, 's>,
    next_run: Option<usize>,
    active_cursor: usize,
    remaining: usize,
}
impl<'v, 'e, 'i, 'n, 's> Iterator for BidiSequenceScalars<'v, 'e, 'i, 'n, 's> {
    type Item = BidiRunScalar<'v, 'e, 'i, 'n, 's>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let run = self.next_run.expect("nonempty scalar cursor");
        let index = self.owner.active[self.active_cursor];
        self.remaining -= 1;
        self.active_cursor += 1;
        if self.remaining == 0 {
            self.next_run = None;
        } else if self.active_cursor == self.owner.runs[run].active_range.end {
            let next = self.owner.runs[run]
                .next_run
                .expect("sequence has remaining scalars");
            self.next_run = Some(next);
            self.active_cursor = self.owner.runs[next].active_range.start;
        }
        Some(BidiRunScalar {
            owner: self.owner,
            index,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for BidiSequenceScalars<'_, '_, '_, '_, '_> {
    fn len(&self) -> usize {
        self.remaining
    }
}
impl FusedIterator for BidiSequenceScalars<'_, '_, '_, '_, '_> {}
fn bytes_for<T>(capacity: usize) -> Result<usize, BidiRunError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&b| b <= isize::MAX as usize)
        .ok_or(BidiRunError::Bytes)
}
fn sum_bytes(bytes: [usize; 4], allowance: usize) -> Result<usize, BidiRunError> {
    bytes.into_iter().try_fold(0usize, |a, b| {
        a.checked_add(b)
            .filter(|&x| x <= allowance)
            .ok_or(BidiRunError::Bytes)
    })
}
fn initial_work(n: usize, p: usize, allowance: usize) -> Result<usize, BidiRunError> {
    n.checked_mul(2)
        .and_then(|n| p.checked_mul(2).and_then(|p| n.checked_add(p)))
        .and_then(|w| w.checked_add(1))
        .filter(|&w| w <= allowance)
        .ok_or(BidiRunError::Work)
}
fn final_work(work: usize, r: usize, allowance: usize) -> Result<usize, BidiRunError> {
    r.checked_mul(3)
        .and_then(|r| work.checked_add(r))
        .filter(|&w| w <= allowance)
        .ok_or(BidiRunError::Work)
}
fn increment(count: usize, allowance: usize, error: BidiRunError) -> Result<usize, BidiRunError> {
    count
        .checked_add(1)
        .filter(|&n| n <= allowance)
        .ok_or(error)
}
fn level_at(explicit: &BidiExplicit<'_, '_, '_>, index: usize) -> BidiExplicitLevel {
    match explicit
        .scalar(index)
        .expect("active original index")
        .state()
    {
        BidiExplicitState::Retained { level, .. } => level,
        BidiExplicitState::RemovedByX9 => panic!("active scalar is retained"),
    }
}
fn initiator(class: BidiClass) -> bool {
    matches!(class, BidiClass::Lri | BidiClass::Rli | BidiClass::Fsi)
}
fn direction(level: u8) -> BidiSequenceDirection {
    if level.is_multiple_of(2) {
        BidiSequenceDirection::LeftToRight
    } else {
        BidiSequenceDirection::RightToLeft
    }
}
/// Construct grouping, topology and X10 sos/eos using five prepaid passes.
///
/// # Errors
/// Scalars and Paragraphs precede initial Work admission. Count visits check
/// ActiveScalars then Runs; final Work is admitted before allocation. Three vectors
/// receive requested and staged actual capacity checks before filling. Linking then
/// checks Sequences; only its exact count is reserved for the fourth vector, with
/// actual combined admission before chain traversal. Bytes includes checked overflow
/// and separate allocation isize ceilings; Allocation denotes reservation failure.
/// No partial owner escapes. The metric counts four local vector payloads only;
/// predecessor buffers require separate composition accounting.
pub fn prepare_bidi_run_sequences<'e, 'i, 'n, 's>(
    explicit: &'e BidiExplicit<'i, 'n, 's>,
    limits: BidiRunLimits,
) -> Result<BidiRunSequences<'e, 'i, 'n, 's>, BidiRunError> {
    let n = explicit.scalar_count();
    let p = explicit.paragraph_count();
    if n > limits.scalars.get() {
        return Err(BidiRunError::Scalars);
    }
    if p > limits.paragraphs.get() {
        return Err(BidiRunError::Paragraphs);
    }
    let work = initial_work(n, p, limits.work.get())?;
    let initial = explicit.isolates().initial();
    let mut a = 0;
    let mut r = 0;
    // Pass 1: exact active/run counts; X9 gaps do not reset the prior active level.
    for pi in 0..p {
        let paragraph = initial.paragraph(pi).expect("admitted paragraph");
        let mut previous = None;
        for index in paragraph.scalar_range() {
            if let BidiExplicitState::Retained { level, .. } =
                explicit.scalar(index).expect("admitted scalar").state()
            {
                a = increment(a, limits.active_scalars.get(), BidiRunError::ActiveScalars)?;
                if previous != Some(level) {
                    r = increment(r, limits.runs.get(), BidiRunError::Runs)?;
                }
                previous = Some(level);
            }
        }
    }
    let work = final_work(work, r, limits.work.get())?;
    let mut payload = [
        bytes_for::<Option<Membership>>(n)?,
        bytes_for::<usize>(a)?,
        bytes_for::<RunRecord>(r)?,
        0,
    ];
    sum_bytes(payload, limits.retained_bytes.get())?;
    let mut membership = Vec::new();
    membership
        .try_reserve_exact(n)
        .map_err(|_| BidiRunError::Allocation)?;
    payload[0] = bytes_for::<Option<Membership>>(membership.capacity())?;
    sum_bytes(payload, limits.retained_bytes.get())?;
    let mut active = Vec::new();
    active
        .try_reserve_exact(a)
        .map_err(|_| BidiRunError::Allocation)?;
    payload[1] = bytes_for::<usize>(active.capacity())?;
    sum_bytes(payload, limits.retained_bytes.get())?;
    let mut runs = Vec::<RunRecord>::new();
    runs.try_reserve_exact(r)
        .map_err(|_| BidiRunError::Allocation)?;
    payload[2] = bytes_for::<RunRecord>(runs.capacity())?;
    sum_bytes(payload, limits.retained_bytes.get())?;
    // Pass 2: one membership per original scalar; exact counted sparse capacities.
    for pi in 0..p {
        let paragraph = initial.paragraph(pi).expect("admitted paragraph");
        let mut previous = None;
        for index in paragraph.scalar_range() {
            match explicit.scalar(index).expect("admitted scalar").state() {
                BidiExplicitState::RemovedByX9 => membership.push(None),
                BidiExplicitState::Retained { level, .. } => {
                    let position = active.len();
                    if previous != Some(level) {
                        runs.push(RunRecord {
                            active_range: position..position + 1,
                            paragraph_index: pi,
                            level,
                            next_run: None,
                            incoming: false,
                            sequence: None,
                        });
                    } else {
                        runs.last_mut().expect("prior active run").active_range.end = position + 1;
                    }
                    membership.push(Some(Membership {
                        active_index: position,
                        run_index: runs.len() - 1,
                    }));
                    active.push(index);
                    previous = Some(level);
                }
            }
        }
    }
    assert_eq!(membership.len(), n);
    assert_eq!(active.len(), a);
    assert_eq!(runs.len(), r);
    // Pass 3: increasing edges mean incoming is final when each root is counted.
    let mut s_count = 0;
    for ri in 0..r {
        if !runs[ri].incoming {
            s_count = increment(s_count, limits.sequences.get(), BidiRunError::Sequences)?;
        }
        let last = active[runs[ri].active_range.end - 1];
        let scalar = explicit.scalar(last).expect("run endpoint");
        if initiator(scalar.original().class())
            && let Some(mate) = scalar.isolate().matching_scalar_index()
        {
            let target = membership[mate].expect("matched PDI retained").run_index;
            assert!(target > ri, "successor run is later");
            assert_eq!(active[runs[target].active_range.start], mate);
            assert_eq!(runs[ri].level, runs[target].level);
            assert_eq!(runs[ri].paragraph_index, runs[target].paragraph_index);
            assert!(!runs[target].incoming, "unique incoming edge");
            runs[ri].next_run = Some(target);
            runs[target].incoming = true;
        }
    }
    payload[3] = bytes_for::<SequenceRecord>(s_count)?;
    sum_bytes(payload, limits.retained_bytes.get())?;
    let mut sequences = Vec::new();
    sequences
        .try_reserve_exact(s_count)
        .map_err(|_| BidiRunError::Allocation)?;
    payload[3] = bytes_for::<SequenceRecord>(sequences.capacity())?;
    let retained_bytes = sum_bytes(payload, limits.retained_bytes.get())?;
    // Pass 4 scans roots; Pass 5 visits each run once across all chains.
    let mut assigned = 0;
    for first in 0..r {
        if runs[first].incoming {
            continue;
        }
        let sequence_index = sequences.len();
        let mut current = first;
        let mut run_count = 0;
        let mut scalar_count = 0;
        let last = loop {
            assert!(runs[current].sequence.is_none(), "run assigned once");
            assert_eq!(runs[current].level, runs[first].level);
            runs[current].sequence = Some(sequence_index);
            assigned += 1;
            run_count += 1;
            scalar_count += runs[current].active_range.len();
            if let Some(next) = runs[current].next_run {
                assert!(next > current);
                current = next;
            } else {
                break current;
            }
        };
        let paragraph_index = runs[first].paragraph_index;
        let paragraph = initial
            .paragraph(paragraph_index)
            .expect("sequence paragraph");
        let range = paragraph.scalar_range();
        let base = paragraph.base().level();
        let level = runs[first].level;
        let first_position = runs[first].active_range.start;
        let last_position = runs[last].active_range.end - 1;
        let before = first_position
            .checked_sub(1)
            .and_then(|i| active.get(i))
            .filter(|&&index| range.contains(&index))
            .map_or(base, |&index| level_at(explicit, index).number());
        let last_scalar = explicit
            .scalar(active[last_position])
            .expect("sequence endpoint");
        let unmatched = initiator(last_scalar.original().class())
            && last_scalar.isolate().matching_scalar_index().is_none();
        let after = if unmatched {
            base
        } else {
            active
                .get(last_position + 1)
                .filter(|&&index| range.contains(&index))
                .map_or(base, |&index| level_at(explicit, index).number())
        };
        sequences.push(SequenceRecord {
            first_run: first,
            last_run: last,
            paragraph_index,
            run_count,
            scalar_count,
            level,
            sos: direction(level.number().max(before)),
            eos: direction(level.number().max(after)),
        });
    }
    // These counters were accumulated in prepaid visits, not a final all-runs scan.
    assert_eq!(assigned, r);
    assert_eq!(sequences.len(), s_count);
    Ok(BidiRunSequences {
        explicit,
        membership,
        active,
        runs,
        sequences,
        retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{bidi_explicit::*, bidi_initial::*, bidi_isolates::*};
    #[test]
    fn checked_arithmetic_and_four_actual_capacities() {
        assert_eq!(
            bytes_for::<Option<Membership>>(usize::MAX),
            Err(BidiRunError::Bytes)
        );
        assert_eq!(
            bytes_for::<RunRecord>(isize::MAX as usize / size_of::<RunRecord>() + 1),
            Err(BidiRunError::Bytes)
        );
        assert_eq!(
            sum_bytes([usize::MAX, 1, 0, 0], usize::MAX),
            Err(BidiRunError::Bytes)
        );
        assert_eq!(
            sum_bytes([isize::MAX as usize, 1, 0, 0], usize::MAX),
            Ok(isize::MAX as usize + 1)
        );
        assert_eq!(
            initial_work(usize::MAX, 0, usize::MAX),
            Err(BidiRunError::Work)
        );
        assert_eq!(
            initial_work(0, usize::MAX, usize::MAX),
            Err(BidiRunError::Work)
        );
        assert_eq!(
            final_work(1, usize::MAX, usize::MAX),
            Err(BidiRunError::Work)
        );
        assert_eq!(final_work(5, 1, 7), Err(BidiRunError::Work));
        for error in [
            BidiRunError::ActiveScalars,
            BidiRunError::Runs,
            BidiRunError::Sequences,
        ] {
            assert_eq!(increment(usize::MAX, usize::MAX, error), Err(error));
            assert_eq!(increment(1, 1, error), Err(error));
        }
        let n = NonZeroUsize::new(usize::MAX).unwrap();
        for s in [
            "",
            "\u{ad}\u{202c}",
            "a\u{ad}b",
            "a\u{2067}א\u{2069}",
            "\r\n\n",
        ] {
            let i = evaluate_bidi_initial(
                s,
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
            let iso = prepare_bidi_isolates(
                &i,
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
            let r = prepare_bidi_run_sequences(
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
            assert_eq!(
                r.retained_bytes,
                r.membership.capacity() * size_of::<Option<Membership>>()
                    + r.active.capacity() * size_of::<usize>()
                    + r.runs.capacity() * size_of::<RunRecord>()
                    + r.sequences.capacity() * size_of::<SequenceRecord>()
            );
            assert_eq!(r.membership.len(), i.scalar_count());
            assert!(r.membership.capacity() >= r.membership.len());
            assert!(r.active.capacity() >= r.active.len());
            assert!(r.runs.capacity() >= r.runs.len());
            assert!(r.sequences.capacity() >= r.sequences.len());
            for seq in &r.sequences {
                assert!(r.runs[seq.last_run].next_run.is_none());
                assert!(seq.first_run <= seq.last_run);
            }
            if !s.is_empty() && r.active.is_empty() {
                assert!(r.retained_bytes > 0);
                assert_eq!(r.active.capacity(), 0);
                assert_eq!(r.runs.capacity(), 0);
                assert_eq!(r.sequences.capacity(), 0);
            }
        }
    }
}
