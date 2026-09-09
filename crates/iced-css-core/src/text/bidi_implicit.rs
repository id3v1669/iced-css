//! N1/N2 neutral resolution and I1/I2 implicit levels on exact N0 owners.
//! Original identity and explicit levels remain available through immutable predecessors.
//! This stage does not apply L1 resets or produce visual order.
use super::{
    bidi_data::BidiClass,
    bidi_explicit::{BidiExplicitLevel, BidiExplicitState},
    bidi_n0::{BidiN0, BidiN0Scalar, BidiN0Sequence},
    bidi_runs::{BidiRunSequences, BidiSequenceDirection},
};
use std::{iter::FusedIterator, mem::size_of, num::NonZeroUsize};
/// A resolved implicit level in 0..=126; distinct from an explicit embedding level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BidiResolvedLevel(u8);
impl BidiResolvedLevel {
    pub fn number(self) -> u8 {
        self.0
    }
}
/// Copied resolved state; source identity belongs to its checked scalar view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BidiImplicitState {
    current_type: BidiClass,
    level: BidiResolvedLevel,
}
impl BidiImplicitState {
    pub fn current_type(self) -> BidiClass {
        self.current_type
    }
    pub fn level(self) -> BidiResolvedLevel {
        self.level
    }
}
#[derive(Debug, Clone, Copy)]
struct ImplicitRecord {
    current_type: BidiClass,
    level: BidiResolvedLevel,
}
#[derive(Debug, Clone, Copy)]
pub struct BidiImplicitLimits {
    pub scalars: NonZeroUsize,
    pub active_scalars: NonZeroUsize,
    pub runs: NonZeroUsize,
    pub sequences: NonZeroUsize,
    /// Actual output capacity payload; excludes borrowed owners, fixed locals and allocator metadata.
    pub retained_bytes: NonZeroUsize,
    /// Reserved upper-bound schedule 1 + 2r + s + 4a; not executed operation count.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiImplicitError {
    Scalars,
    ActiveScalars,
    Runs,
    Sequences,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for BidiImplicitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bidi neutral and implicit resolution: {self:?}")
    }
}
impl std::error::Error for BidiImplicitError {}
/// Immutable neutral types and implicit levels borrowing the exact N0 owner.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*,bidi_implicit::*};
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
/// let n0=resolve_bidi_n0(&pairs,BidiN0Limits{scalars:n,active_scalars:n,runs:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_implicit(&n0,BidiImplicitLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(source);
/// assert_eq!(result.scalar_count(),3);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*,bidi_implicit::*};
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
/// let n0=resolve_bidi_n0(&pairs,BidiN0Limits{scalars:n,active_scalars:n,runs:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_implicit(&n0,BidiImplicitLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(n0);
/// assert_eq!(result.scalar_count(),3);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*,bidi_implicit::*};
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
/// let n0=resolve_bidi_n0(&pairs,BidiN0Limits{scalars:n,active_scalars:n,runs:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_implicit(&n0,BidiImplicitLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// drop(pairs);
/// assert_eq!(result.scalar_count(),3);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*,bidi_implicit::*};
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
/// let n0=resolve_bidi_n0(&pairs,BidiN0Limits{scalars:n,active_scalars:n,runs:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_implicit(&n0,BidiImplicitLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let view=result.scalar(0).unwrap();
/// drop(result);
/// assert_eq!(view.index(),0);
/// ```
///
/// ```compile_fail,E0597
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*,bidi_implicit::*};
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
/// let n0=resolve_bidi_n0(&pairs,BidiN0Limits{scalars:n,active_scalars:n,runs:n,sequences:n,pairs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_implicit(&n0,BidiImplicitLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let mut iterator;
/// { let sequence=result.sequence(0).unwrap(); iterator=sequence.scalars(); }
/// assert!(iterator.next().is_some());
/// ```
#[must_use]
#[derive(Debug)]
pub struct BidiImplicit<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    n0: &'z BidiN0<'p, 'w, 'r, 'e, 'i, 'n, 's>,
    records: Vec<ImplicitRecord>,
    retained_bytes: usize,
    work: usize,
}
impl<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> BidiImplicit<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    pub fn n0(&self) -> &'z BidiN0<'p, 'w, 'r, 'e, 'i, 'n, 's> {
        self.n0
    }
    pub fn scalar_count(&self) -> usize {
        self.n0.scalar_count()
    }
    pub fn active_scalar_count(&self) -> usize {
        self.n0.active_scalar_count()
    }
    pub fn run_count(&self) -> usize {
        self.n0.run_count()
    }
    pub fn sequence_count(&self) -> usize {
        self.n0.sequence_count()
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
    pub fn scalar(
        &self,
        index: usize,
    ) -> Option<BidiImplicitScalar<'_, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>> {
        self.n0.scalar(index)?;
        Some(BidiImplicitScalar { owner: self, index })
    }
    pub fn sequence(
        &self,
        index: usize,
    ) -> Option<BidiImplicitSequence<'_, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>> {
        self.n0.sequence(index)?;
        Some(BidiImplicitSequence { owner: self, index })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiImplicitScalar<'v, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiImplicit<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> BidiImplicitScalar<'v, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn n0_scalar(&self) -> BidiN0Scalar<'_, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
        self.owner
            .n0
            .scalar(self.index)
            .expect("implicit scalar has N0 predecessor")
    }
    pub fn state(&self) -> Option<BidiImplicitState> {
        self.n0_scalar()
            .weak_scalar()
            .run_scalar()
            .membership()
            .map(|m| {
                let record = self.owner.records[m.active_index()];
                BidiImplicitState {
                    current_type: record.current_type,
                    level: record.level,
                }
            })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiImplicitSequence<'v, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiImplicit<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> BidiImplicitSequence<'v, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn n0_sequence(&self) -> BidiN0Sequence<'_, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
        self.owner
            .n0
            .sequence(self.index)
            .expect("implicit sequence has N0 predecessor")
    }
    pub fn scalar_count(&self) -> usize {
        self.n0_sequence().scalar_count()
    }
    pub fn run_count(&self) -> usize {
        self.n0_sequence().run_count()
    }
    pub fn explicit_level(&self) -> BidiExplicitLevel {
        self.n0_sequence().level()
    }
    pub fn sos(&self) -> BidiSequenceDirection {
        self.n0_sequence().sos()
    }
    pub fn eos(&self) -> BidiSequenceDirection {
        self.n0_sequence().eos()
    }
    pub fn scalars(&self) -> BidiImplicitScalars<'_, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
        let runs = self.owner.n0.pairs().weak().runs();
        let first = {
            let seq = runs
                .sequence(self.index)
                .expect("implicit sequence has run predecessor");
            seq.runs().next().map(|r| r.index())
        };
        BidiImplicitScalars {
            owner: self.owner,
            next_run: first,
            local_index: 0,
            remaining: self.scalar_count(),
        }
    }
}
#[derive(Debug)]
pub struct BidiImplicitScalars<'v, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiImplicit<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>,
    next_run: Option<usize>,
    local_index: usize,
    remaining: usize,
}
impl<'v, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> Iterator
    for BidiImplicitScalars<'v, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>
{
    type Item = BidiImplicitScalar<'v, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let run = self
            .owner
            .n0
            .pairs()
            .weak()
            .runs()
            .run(self.next_run.expect("nonempty implicit cursor"))
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
        Some(BidiImplicitScalar {
            owner: self.owner,
            index,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for BidiImplicitScalars<'_, '_, '_, '_, '_, '_, '_, '_, '_> {
    fn len(&self) -> usize {
        self.remaining
    }
}
impl FusedIterator for BidiImplicitScalars<'_, '_, '_, '_, '_, '_, '_, '_, '_> {}
#[derive(Debug, Clone, Copy)]
struct Cursor {
    run: Option<usize>,
    local: usize,
}
impl Cursor {
    fn index(self, runs: &BidiRunSequences<'_, '_, '_, '_>) -> Option<usize> {
        self.run.map(|ri| {
            runs.run(ri)
                .expect("cursor run")
                .scalar(self.local)
                .expect("cursor member")
                .index()
        })
    }
    fn advance(&mut self, runs: &BidiRunSequences<'_, '_, '_, '_>) {
        if let Some(ri) = self.run {
            let run = runs.run(ri).expect("cursor run");
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
        .expect("original member")
        .membership()
        .expect("retained member")
        .active_index()
}
fn is_ni(c: BidiClass) -> bool {
    matches!(
        c,
        BidiClass::B
            | BidiClass::S
            | BidiClass::Ws
            | BidiClass::On
            | BidiClass::Fsi
            | BidiClass::Lri
            | BidiClass::Rli
            | BidiClass::Pdi
    )
}
fn direction(c: BidiClass) -> BidiClass {
    match c {
        BidiClass::L => BidiClass::L,
        BidiClass::R | BidiClass::En | BidiClass::An => BidiClass::R,
        _ => unreachable!("active non-NI post-N0 type is strong or numeric"),
    }
}
fn resolve_level(level: BidiExplicitLevel, c: BidiClass) -> BidiResolvedLevel {
    let number = level.number();
    // Branch on the original explicit parity, never the result of an earlier increment.
    let delta = if number.is_multiple_of(2) {
        match c {
            BidiClass::R => 1,
            BidiClass::En | BidiClass::An => 2,
            BidiClass::L => 0,
            _ => unreachable!("resolved type"),
        }
    } else {
        match c {
            BidiClass::L | BidiClass::En | BidiClass::An => 1,
            BidiClass::R => 0,
            _ => unreachable!("resolved type"),
        }
    };
    // The validated explicit domain is 0..125: even124+2 and odd125+1 are at most126.
    BidiResolvedLevel(number + delta)
}
fn flush(
    runs: &BidiRunSequences<'_, '_, '_, '_>,
    records: &mut [ImplicitRecord],
    pending: &mut Option<(Cursor, usize)>,
    previous: BidiClass,
    right: BidiClass,
    embedding: BidiClass,
) {
    if let Some((mut cursor, count)) = pending.take() {
        let selected = if previous == right {
            previous
        } else {
            embedding
        };
        for _ in 0..count {
            let index = cursor.index(runs).expect("pending span member");
            records[active(runs, index)].current_type = selected;
            cursor.advance(runs);
        }
    }
}
fn initialize(n0: &BidiN0<'_, '_, '_, '_, '_, '_, '_>, records: &mut Vec<ImplicitRecord>) {
    let runs = n0.pairs().weak().runs();
    for ri in 0..runs.run_count() {
        let run = runs.run(ri).expect("admitted run");
        for local in 0..run.scalar_count() {
            let scalar = run.scalar(local).expect("admitted member");
            assert_eq!(active(runs, scalar.index()), records.len());
            records.push(ImplicitRecord {
                current_type: n0
                    .scalar(scalar.index())
                    .expect("N0 member")
                    .current_type()
                    .expect("active N0 type"),
                level: BidiResolvedLevel(run.level().number()),
            });
        }
    }
}
fn neutrals(n0: &BidiN0<'_, '_, '_, '_, '_, '_, '_>, records: &mut [ImplicitRecord]) {
    let runs = n0.pairs().weak().runs();
    for si in 0..n0.sequence_count() {
        let sequence = n0.sequence(si).expect("admitted sequence");
        let run_sequence = runs.sequence(si).expect("run sequence");
        let first = {
            run_sequence
                .runs()
                .next()
                .expect("sequence nonempty")
                .index()
        };
        let mut cursor = Cursor {
            run: Some(first),
            local: 0,
        };
        let mut pending: Option<(Cursor, usize)> = None;
        let mut previous = sequence.sos().bidi_class();
        let embedding = if sequence.level().number().is_multiple_of(2) {
            BidiClass::L
        } else {
            BidiClass::R
        };
        while let Some(index) = cursor.index(runs) {
            let c = n0
                .scalar(index)
                .expect("N0 member")
                .current_type()
                .expect("active member");
            if is_ni(c) {
                if let Some((_, count)) = &mut pending {
                    *count += 1;
                } else {
                    pending = Some((cursor, 1));
                }
            } else {
                let right = direction(c);
                flush(runs, records, &mut pending, previous, right, embedding);
                previous = right;
            }
            cursor.advance(runs);
        }
        flush(
            runs,
            records,
            &mut pending,
            previous,
            sequence.eos().bidi_class(),
            embedding,
        );
    }
}
fn implicit_levels(runs: &BidiRunSequences<'_, '_, '_, '_>, records: &mut [ImplicitRecord]) {
    for ri in 0..runs.run_count() {
        let run = runs.run(ri).expect("admitted run");
        for local in 0..run.scalar_count() {
            let scalar = run.scalar(local).expect("admitted member");
            let BidiExplicitState::Retained { level, .. } = scalar.explicit().state() else {
                unreachable!("run member retained")
            };
            let record = &mut records[active(runs, scalar.index())];
            record.level = resolve_level(level, record.current_type);
        }
    }
}
fn bytes_for(capacity: usize, allowance: usize) -> Result<usize, BidiImplicitError> {
    capacity
        .checked_mul(size_of::<ImplicitRecord>())
        .filter(|&b| b <= isize::MAX as usize && b <= allowance)
        .ok_or(BidiImplicitError::Bytes)
}
fn work_for(a: usize, r: usize, s: usize, allowance: usize) -> Result<usize, BidiImplicitError> {
    a.checked_mul(4)
        .and_then(|w| r.checked_mul(2).and_then(|r| w.checked_add(r)))
        .and_then(|w| w.checked_add(s))
        .and_then(|w| w.checked_add(1))
        .filter(|&w| w <= allowance)
        .ok_or(BidiImplicitError::Work)
}
/// Resolve neutral spans against immutable post-N0 context, then assign implicit levels.
///
/// # Errors
/// Original scalars, active scalars, runs and sequences are admitted in that order,
/// followed by checked Work and requested Bytes. Fallible reservation can return
/// Allocation; actual capacity is checked again before any initialization. No partial
/// owner escapes. Bytes covers arithmetic overflow and per-allocation isize limits.
/// Peak payload equals retained output capacity and excludes borrowed predecessors,
/// fixed locals, allocator metadata and physical peaks of failed allocator calls.
pub fn resolve_bidi_implicit<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>(
    n0: &'z BidiN0<'p, 'w, 'r, 'e, 'i, 'n, 's>,
    limits: BidiImplicitLimits,
) -> Result<BidiImplicit<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>, BidiImplicitError> {
    let n = n0.scalar_count();
    let a = n0.active_scalar_count();
    let r = n0.run_count();
    let s = n0.sequence_count();
    if n > limits.scalars.get() {
        return Err(BidiImplicitError::Scalars);
    }
    if a > limits.active_scalars.get() {
        return Err(BidiImplicitError::ActiveScalars);
    }
    if r > limits.runs.get() {
        return Err(BidiImplicitError::Runs);
    }
    if s > limits.sequences.get() {
        return Err(BidiImplicitError::Sequences);
    }
    let work = work_for(a, r, s, limits.work.get())?;
    bytes_for(a, limits.retained_bytes.get())?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(a)
        .map_err(|_| BidiImplicitError::Allocation)?;
    let retained_bytes = bytes_for(records.capacity(), limits.retained_bytes.get())?;
    initialize(n0, &mut records);
    neutrals(n0, &mut records);
    implicit_levels(n0.pairs().weak().runs(), &mut records);
    Ok(BidiImplicit {
        n0,
        records,
        retained_bytes,
        work,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{
        bidi_bracket_pairs::*, bidi_data::BidiClass, bidi_explicit::*, bidi_initial::*,
        bidi_isolates::*, bidi_n0::*, bidi_runs::*, bidi_weak::*,
    };
    use std::num::NonZeroUsize;
    fn max() -> NonZeroUsize {
        NonZeroUsize::new(usize::MAX).unwrap()
    }
    fn limits() -> BidiImplicitLimits {
        let n = max();
        BidiImplicitLimits {
            scalars: n,
            active_scalars: n,
            runs: n,
            sequences: n,
            retained_bytes: n,
            work: n,
        }
    }
    macro_rules! fixture {
        ($source:expr,$limits:expr,$r:ident=>$body:block) => {{
            let source = ($source).to_string();
            let n = max();
            let initial = evaluate_bidi_initial(
                &source,
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
            let n0 = resolve_bidi_n0(
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
            let $r = resolve_bidi_implicit(&n0, $limits);
            $body
        }};
    }
    #[test]
    fn actual_capacity_and_saved_cursor_suffixes() {
        for source in ["", "\u{ad}", "a  \u{2067}א\u{2069}  a", "א \n😀"] {
            fixture!(source,limits(),result=>{let r=result.unwrap();assert_eq!(r.records.len(),r.active_scalar_count());assert!(r.records.capacity()>=r.records.len());assert_eq!(r.retained_bytes(),r.records.capacity()*size_of::<ImplicitRecord>());assert_eq!(r.retained_bytes(),r.peak_retained_bytes());let runs=r.n0.pairs().weak().runs();for si in 0..runs.sequence_count(){let seq=runs.sequence(si).unwrap();let expected:Vec<_>=seq.scalars().map(|v|v.index()).collect();let mut cursor=Cursor{run:Some(seq.runs().next().unwrap().index()),local:0};for (start,&index) in expected.iter().enumerate(){assert_eq!(cursor.index(runs),Some(index));let mut saved=cursor;for &want in &expected[start..]{assert_eq!(saved.index(runs),Some(want));saved.advance(runs);}assert_eq!(saved.index(runs),None);saved.advance(runs);assert_eq!(saved.index(runs),None);cursor.advance(runs);}}});
        }
    }
    #[test]
    fn checked_budgets_ni_domain_and_all_implicit_deltas() {
        assert_eq!(
            bytes_for(usize::MAX, usize::MAX),
            Err(BidiImplicitError::Bytes)
        );
        assert_eq!(
            bytes_for(isize::MAX as usize + 1, usize::MAX),
            Err(BidiImplicitError::Bytes)
        );
        assert_eq!(
            bytes_for(1, size_of::<ImplicitRecord>() - 1),
            Err(BidiImplicitError::Bytes)
        );
        assert_eq!(bytes_for(0, usize::MAX), Ok(0));
        for (a, r, s) in [(usize::MAX, 0, 0), (0, usize::MAX, 0), (0, 0, usize::MAX)] {
            assert_eq!(work_for(a, r, s, usize::MAX), Err(BidiImplicitError::Work));
        }
        assert_eq!(work_for(0, 0, 0, 1), Ok(1));
        assert_eq!(work_for(3, 2, 1, 18), Ok(18));
        assert_eq!(work_for(3, 2, 1, 17), Err(BidiImplicitError::Work));
        use BidiClass::*;
        for c in [B, S, Ws, On, Fsi, Lri, Rli, Pdi] {
            assert!(is_ni(c));
        }
        for c in [
            L, R, En, An, Nsm, Bn, Al, Es, Et, Cs, Lre, Rle, Lro, Rlo, Pdf,
        ] {
            assert!(!is_ni(c));
        }
        // Actual explicit owners supply every valid level; Table 5 is the independent delta table.
        for number in 0..=125u8 {
            let source = if number.is_multiple_of(2) {
                format!("{}a", "\u{202a}".repeat(usize::from(number / 2)))
            } else {
                format!("{}a", "\u{202b}".repeat(usize::from(number / 2 + 1)))
            };
            fixture!(source,limits(),result=>{let r=result.unwrap();let original=r.n0.pairs().weak().runs().explicit().scalar(r.scalar_count()-1).unwrap();let BidiExplicitState::Retained{level,..}=original.state()else{panic!()};assert_eq!(level.number(),number);for (c,even,odd) in [(L,0,1),(R,1,0),(En,2,1),(An,2,1)]{let expected=number+if number.is_multiple_of(2){even}else{odd};assert_eq!(resolve_level(level,c).number(),expected);assert!(expected<=126);}});
        }
    }
}
