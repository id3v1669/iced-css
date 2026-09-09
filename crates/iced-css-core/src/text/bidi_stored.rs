//! Stored raw bidi lines: owned initial metadata and detached final values.
//! Existing bidi stages run once in a borrowing scope; no UBA algorithm is duplicated.
use super::{
    bidi_bracket_pairs::*,
    bidi_explicit::*,
    bidi_implicit::*,
    bidi_initial::{BidiInitial, BidiParagraphBase, BidiScalar},
    bidi_isolates::*,
    bidi_lines::*,
    bidi_n0::*,
    bidi_runs::*,
    bidi_weak::*,
};
use std::{iter::FusedIterator, mem::size_of, num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy)]
pub struct BidiStoredLimits {
    pub scalars: NonZeroUsize,
    pub active_scalars: NonZeroUsize,
    pub paragraphs: NonZeroUsize,
    pub runs: NonZeroUsize,
    pub sequences: NonZeroUsize,
    pub pairs: NonZeroUsize,
    pub lines: NonZeroUsize,
    /// Aggregate live owner/copy payload; includes consumed initial, excludes borrowed source.
    pub retained_bytes: NonZeroUsize,
    /// New invocation work only; excludes historical initial construction work.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiStoredError {
    Scalars,
    Paragraphs,
    Lines,
    Work,
    Bytes,
    Allocation,
    Isolates(BidiIsolateError),
    Explicit(BidiExplicitError),
    Runs(BidiRunError),
    Weak(BidiWeakError),
    BracketPairs(BidiBracketPairError),
    N0(BidiN0Error),
    Implicit(BidiImplicitError),
    LinesStage(BidiLineError),
}
impl std::fmt::Display for BidiStoredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stored bidi lines: {self:?}")
    }
}
impl std::error::Error for BidiStoredError {}
/// A failed composition returns the unchanged initial owner for recovery.
#[must_use]
#[derive(Debug)]
pub struct BidiStoredFailure<'s> {
    initial: BidiInitial<'s>,
    error: BidiStoredError,
}
impl<'s> BidiStoredFailure<'s> {
    pub fn initial(&self) -> &BidiInitial<'s> {
        &self.initial
    }
    pub fn error(&self) -> BidiStoredError {
        self.error
    }
    pub fn into_parts(self) -> (BidiInitial<'s>, BidiStoredError) {
        (self.initial, self.error)
    }
}
impl std::fmt::Display for BidiStoredFailure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for BidiStoredFailure<'_> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
#[derive(Debug)]
struct StoredLineRecord {
    original: Range<usize>,
    visual: Range<usize>,
    paragraph: usize,
}
/// Movable final values retaining owned raw metadata and borrowed caller text.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_stored::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(100000).unwrap();
/// let source=String::from("abc");
/// let raw=evaluate_bidi_initial(&source,BidiParagraphMode::LeftToRight,BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let limits=BidiStoredLimits{scalars:n,active_scalars:n,paragraphs:n,runs:n,sequences:n,pairs:n,lines:n,retained_bytes:n,work:n};
/// let result=store_bidi_lines(raw,&[3],limits).unwrap();
/// drop(source);
/// assert_eq!(result.scalar_count(),3);
/// ```
///
/// ```compile_fail,E0382
/// use iced_css_core::text::{bidi_initial::*,bidi_stored::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(100000).unwrap();
/// let source=String::from("abc");
/// let raw=evaluate_bidi_initial(&source,BidiParagraphMode::LeftToRight,BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let limits=BidiStoredLimits{scalars:n,active_scalars:n,paragraphs:n,runs:n,sequences:n,pairs:n,lines:n,retained_bytes:n,work:n};
/// let result=store_bidi_lines(raw,&[3],limits).unwrap();
/// assert_eq!(raw.scalar_count(),result.scalar_count());
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_stored::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(100000).unwrap();
/// let source=String::from("abc");
/// let raw=evaluate_bidi_initial(&source,BidiParagraphMode::LeftToRight,BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let limits=BidiStoredLimits{scalars:n,active_scalars:n,paragraphs:n,runs:n,sequences:n,pairs:n,lines:n,retained_bytes:n,work:n};
/// let result=store_bidi_lines(raw,&[3],limits).unwrap();
/// let view=result.scalar(0).unwrap();
/// drop(result);
/// assert_eq!(view.index(),0);
/// ```
///
/// ```compile_fail,E0597
/// use iced_css_core::text::{bidi_initial::*,bidi_stored::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(100000).unwrap();
/// let source=String::from("abc");
/// let raw=evaluate_bidi_initial(&source,BidiParagraphMode::LeftToRight,BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let limits=BidiStoredLimits{scalars:n,active_scalars:n,paragraphs:n,runs:n,sequences:n,pairs:n,lines:n,retained_bytes:n,work:n};
/// let result=store_bidi_lines(raw,&[3],limits).unwrap();
/// let mut iter;
/// {let line=result.line(0).unwrap();iter=line.scalars();}
/// assert!(iter.next().is_some());
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_stored::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(100000).unwrap();
/// let source=String::from("abc");
/// let raw=evaluate_bidi_initial(&source,BidiParagraphMode::LeftToRight,BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let limits=BidiStoredLimits{scalars:n,active_scalars:n,paragraphs:n,runs:n,sequences:n,pairs:n,lines:n,retained_bytes:n,work:n};
/// let failure=store_bidi_lines(raw,&[],limits).unwrap_err();
/// drop(source);
/// assert_eq!(failure.initial().scalar_count(),3);
/// ```
#[must_use]
#[derive(Debug)]
pub struct BidiStoredLines<'s> {
    initial: BidiInitial<'s>,
    headers: Vec<StoredLineRecord>,
    levels: Vec<Option<BidiLineLevel>>,
    visual: Vec<usize>,
    retained_bytes: usize,
    work: usize,
    peak_retained_bytes: usize,
}
impl<'s> BidiStoredLines<'s> {
    pub fn initial(&self) -> &BidiInitial<'s> {
        &self.initial
    }
    pub fn scalar_count(&self) -> usize {
        self.initial.scalar_count()
    }
    pub fn active_scalar_count(&self) -> usize {
        self.visual.len()
    }
    pub fn paragraph_count(&self) -> usize {
        self.initial().paragraph_count()
    }
    pub fn line_count(&self) -> usize {
        self.headers.len()
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
    pub fn line(&self, index: usize) -> Option<BidiStoredLine<'_, 's>> {
        self.headers.get(index)?;
        Some(BidiStoredLine { owner: self, index })
    }
    /// Checked original scalar lookup; finds the containing line in O(log line_count).
    pub fn scalar(&self, index: usize) -> Option<BidiStoredScalar<'_, 's>> {
        if index >= self.scalar_count() {
            return None;
        }
        let line = self.headers.partition_point(|h| h.original.end <= index);
        Some(BidiStoredScalar {
            owner: self,
            index,
            line,
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiStoredScalar<'v, 's> {
    owner: &'v BidiStoredLines<'s>,
    index: usize,
    line: usize,
}
impl<'v, 's> BidiStoredScalar<'v, 's> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn line_index(&self) -> usize {
        self.line
    }
    pub fn line_scalar_index(&self) -> usize {
        self.index - self.owner.headers[self.line].original.start
    }
    pub fn original(&self) -> BidiScalar<'_> {
        self.owner
            .initial
            .scalar(self.index)
            .expect("stored original scalar")
    }
    pub fn level(&self) -> Option<BidiLineLevel> {
        self.owner.levels[self.index]
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiStoredLine<'v, 's> {
    owner: &'v BidiStoredLines<'s>,
    index: usize,
}
impl<'v, 's> BidiStoredLine<'v, 's> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn paragraph_index(&self) -> usize {
        self.owner.headers[self.index].paragraph
    }
    pub fn scalar_range(&self) -> Range<usize> {
        self.owner.headers[self.index].original.clone()
    }
    pub fn source_range(&self) -> Range<usize> {
        let range = &self.owner.headers[self.index].original;
        let initial = self.owner.initial();
        initial
            .scalar(range.start)
            .expect("nonempty original line")
            .source_range()
            .start
            ..initial
                .scalar(range.end - 1)
                .expect("original line end")
                .source_range()
                .end
    }
    pub fn scalar_count(&self) -> usize {
        self.owner.headers[self.index].original.len()
    }
    pub fn active_scalar_count(&self) -> usize {
        self.owner.headers[self.index].visual.len()
    }
    pub fn base(&self) -> BidiParagraphBase {
        self.owner
            .initial()
            .paragraph(self.paragraph_index())
            .expect("line paragraph")
            .base()
    }
    pub fn scalar(&self, local: usize) -> Option<BidiStoredScalar<'_, 's>> {
        if local >= self.scalar_count() {
            return None;
        }
        Some(BidiStoredScalar {
            owner: self.owner,
            index: self.owner.headers[self.index].original.start + local,
            line: self.index,
        })
    }
    pub fn visual_scalar(&self, local: usize) -> Option<BidiStoredScalar<'_, 's>> {
        if local >= self.active_scalar_count() {
            return None;
        }
        Some(BidiStoredScalar {
            owner: self.owner,
            index: self.owner.visual[self.owner.headers[self.index].visual.start + local],
            line: self.index,
        })
    }
    pub fn scalars(&self) -> BidiStoredScalars<'_, 's> {
        let range = &self.owner.headers[self.index].original;
        BidiStoredScalars {
            owner: self.owner,
            line: self.index,
            next: range.start,
            end: range.end,
        }
    }
    pub fn visual_scalars(&self) -> BidiStoredVisualScalars<'_, 's> {
        let range = &self.owner.headers[self.index].visual;
        BidiStoredVisualScalars {
            owner: self.owner,
            line: self.index,
            next: range.start,
            end: range.end,
        }
    }
}
#[derive(Debug)]
pub struct BidiStoredScalars<'v, 's> {
    owner: &'v BidiStoredLines<'s>,
    line: usize,
    next: usize,
    end: usize,
}
impl<'v, 's> Iterator for BidiStoredScalars<'v, 's> {
    type Item = BidiStoredScalar<'v, 's>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.end {
            return None;
        }
        let index = self.next;
        self.next += 1;
        Some(BidiStoredScalar {
            owner: self.owner,
            index,
            line: self.line,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.end - self.next;
        (n, Some(n))
    }
}
impl ExactSizeIterator for BidiStoredScalars<'_, '_> {
    fn len(&self) -> usize {
        self.end - self.next
    }
}
impl FusedIterator for BidiStoredScalars<'_, '_> {}
#[derive(Debug)]
pub struct BidiStoredVisualScalars<'v, 's> {
    owner: &'v BidiStoredLines<'s>,
    line: usize,
    next: usize,
    end: usize,
}
impl<'v, 's> Iterator for BidiStoredVisualScalars<'v, 's> {
    type Item = BidiStoredScalar<'v, 's>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.end {
            return None;
        }
        let index = self.owner.visual[self.next];
        self.next += 1;
        Some(BidiStoredScalar {
            owner: self.owner,
            index,
            line: self.line,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.end - self.next;
        (n, Some(n))
    }
}
impl ExactSizeIterator for BidiStoredVisualScalars<'_, '_> {
    fn len(&self) -> usize {
        self.end - self.next
    }
}
impl FusedIterator for BidiStoredVisualScalars<'_, '_> {}
#[cfg(test)]
#[derive(Debug, Clone, Copy, Default)]
struct StageReceipt {
    before: usize,
    retained: usize,
    peak: usize,
    work: usize,
    work_allowance: usize,
    byte_allowance: usize,
}
#[derive(Debug)]
struct Ledger {
    used: usize,
    live: usize,
    peak: usize,
    work_limit: usize,
    byte_limit: usize,
    #[cfg(test)]
    receipts: [StageReceipt; 8],
    #[cfg(test)]
    count: usize,
}
impl Ledger {
    fn new(initial: usize, work_limit: usize, byte_limit: usize) -> Result<Self, BidiStoredError> {
        if work_limit < 1 {
            return Err(BidiStoredError::Work);
        }
        if initial > byte_limit {
            return Err(BidiStoredError::Bytes);
        }
        Ok(Self {
            used: 1,
            live: initial,
            peak: initial,
            work_limit,
            byte_limit,
            #[cfg(test)]
            receipts: [StageReceipt::default(); 8],
            #[cfg(test)]
            count: 0,
        })
    }
    fn allowances(&self) -> Result<(NonZeroUsize, NonZeroUsize), BidiStoredError> {
        let work = self
            .work_limit
            .checked_sub(self.used)
            .and_then(NonZeroUsize::new)
            .ok_or(BidiStoredError::Work)?;
        let bytes = self
            .byte_limit
            .checked_sub(self.live)
            .and_then(NonZeroUsize::new)
            .ok_or(BidiStoredError::Bytes)?;
        Ok((work, bytes))
    }
    fn charge(&mut self, work: usize) -> Result<(), BidiStoredError> {
        self.used = self
            .used
            .checked_add(work)
            .filter(|&v| v <= self.work_limit)
            .ok_or(BidiStoredError::Work)?;
        Ok(())
    }
    fn admit_payload(&self, bytes: usize) -> Result<usize, BidiStoredError> {
        self.live
            .checked_add(bytes)
            .filter(|&v| v <= self.byte_limit)
            .ok_or(BidiStoredError::Bytes)
    }
    fn stage(&mut self, work: usize, retained: usize, peak: usize) -> Result<(), BidiStoredError> {
        assert!(
            peak >= retained,
            "qualified stage peak includes retained payload"
        );
        #[cfg(test)]
        {
            self.receipts[self.count] = StageReceipt {
                before: self.live,
                retained,
                peak,
                work,
                work_allowance: self.work_limit - self.used,
                byte_allowance: self.byte_limit - self.live,
            };
            self.count += 1;
        }
        self.charge(work)?;
        self.peak = self.peak.max(self.admit_payload(peak)?);
        self.live = self.admit_payload(retained)?;
        Ok(())
    }
}
#[derive(Debug)]
struct Detached {
    headers: Vec<StoredLineRecord>,
    levels: Vec<Option<BidiLineLevel>>,
    visual: Vec<usize>,
    payload: usize,
    ledger: Ledger,
}
fn allocation_bytes<T>(capacity: usize) -> Result<usize, BidiStoredError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&v| v <= isize::MAX as usize)
        .ok_or(BidiStoredError::Bytes)
}
fn payload(h: usize, n: usize, a: usize) -> Result<usize, BidiStoredError> {
    allocation_bytes::<StoredLineRecord>(h)?
        .checked_add(allocation_bytes::<Option<BidiLineLevel>>(n)?)
        .and_then(|v| {
            allocation_bytes::<usize>(a)
                .ok()
                .and_then(|a| v.checked_add(a))
        })
        .ok_or(BidiStoredError::Bytes)
}
fn copy_lines(
    lines: &BidiLines<'_, '_, '_, '_, '_, '_, '_, '_, '_>,
    mut ledger: Ledger,
) -> Result<Detached, BidiStoredError> {
    let l = lines.line_count();
    let n = lines.scalar_count();
    let a = lines.active_scalar_count();
    let copy_work = l
        .checked_add(n)
        .and_then(|v| v.checked_add(a))
        .ok_or(BidiStoredError::Work)?;
    ledger.charge(copy_work)?;
    ledger.admit_payload(payload(l, n, a)?)?;
    let mut headers = Vec::new();
    headers
        .try_reserve_exact(l)
        .map_err(|_| BidiStoredError::Allocation)?;
    ledger.admit_payload(payload(headers.capacity(), n, a)?)?;
    let mut levels = Vec::new();
    levels
        .try_reserve_exact(n)
        .map_err(|_| BidiStoredError::Allocation)?;
    ledger.admit_payload(payload(headers.capacity(), levels.capacity(), a)?)?;
    let mut visual = Vec::new();
    visual
        .try_reserve_exact(a)
        .map_err(|_| BidiStoredError::Allocation)?;
    let payload = payload(headers.capacity(), levels.capacity(), visual.capacity())?;
    ledger.peak = ledger.peak.max(ledger.admit_payload(payload)?);
    for index in 0..l {
        let line = lines.line(index).expect("qualified line");
        let original = line.scalar_range();
        let start = visual.len();
        assert_eq!(original.start, levels.len());
        for scalar in line.scalars() {
            assert_eq!(scalar.index(), levels.len());
            levels.push(scalar.level());
        }
        for scalar in line.visual_scalars() {
            visual.push(scalar.index());
        }
        assert_eq!(original.end, levels.len());
        assert_eq!(visual.len() - start, line.active_scalar_count());
        headers.push(StoredLineRecord {
            original,
            visual: start..visual.len(),
            paragraph: line.paragraph_index(),
        });
    }
    Ok(Detached {
        headers,
        levels,
        visual,
        payload,
        ledger,
    })
}
fn build(
    initial: &BidiInitial<'_>,
    cuts: &[usize],
    limits: BidiStoredLimits,
) -> Result<Detached, BidiStoredError> {
    if initial.scalar_count() > limits.scalars.get() {
        return Err(BidiStoredError::Scalars);
    }
    if initial.paragraph_count() > limits.paragraphs.get() {
        return Err(BidiStoredError::Paragraphs);
    }
    if cuts.len() > limits.lines.get() {
        return Err(BidiStoredError::Lines);
    }
    let mut ledger = Ledger::new(
        initial.retained_bytes(),
        limits.work.get(),
        limits.retained_bytes.get(),
    )?;
    if initial.scalar_count() == 0 {
        if !cuts.is_empty() {
            return Err(BidiStoredError::LinesStage(BidiLineError::InvalidLineEnds));
        }
        return Ok(Detached {
            headers: Vec::new(),
            levels: Vec::new(),
            visual: Vec::new(),
            payload: 0,
            ledger,
        });
    }
    let (work, retained_bytes) = ledger.allowances()?;
    let isolates = prepare_bidi_isolates(
        initial,
        BidiIsolateLimits {
            scalars: limits.scalars,
            paragraphs: limits.paragraphs,
            work,
            retained_bytes,
        },
    )
    .map_err(BidiStoredError::Isolates)?;
    ledger.stage(
        isolates.work(),
        isolates.retained_bytes(),
        isolates.peak_retained_bytes(),
    )?;
    let (work, retained_bytes) = ledger.allowances()?;
    let explicit = resolve_bidi_explicit(
        &isolates,
        BidiExplicitLimits {
            scalars: limits.scalars,
            paragraphs: limits.paragraphs,
            work,
            retained_bytes,
        },
    )
    .map_err(BidiStoredError::Explicit)?;
    ledger.stage(
        explicit.work(),
        explicit.retained_bytes(),
        explicit.peak_retained_bytes(),
    )?;
    let (work, retained_bytes) = ledger.allowances()?;
    let runs = prepare_bidi_run_sequences(
        &explicit,
        BidiRunLimits {
            scalars: limits.scalars,
            paragraphs: limits.paragraphs,
            active_scalars: limits.active_scalars,
            runs: limits.runs,
            sequences: limits.sequences,
            work,
            retained_bytes,
        },
    )
    .map_err(BidiStoredError::Runs)?;
    ledger.stage(
        runs.work(),
        runs.retained_bytes(),
        runs.peak_retained_bytes(),
    )?;
    let (work, retained_bytes) = ledger.allowances()?;
    let weak = resolve_bidi_weak(
        &runs,
        BidiWeakLimits {
            scalars: limits.scalars,
            active_scalars: limits.active_scalars,
            runs: limits.runs,
            sequences: limits.sequences,
            work,
            retained_bytes,
        },
    )
    .map_err(BidiStoredError::Weak)?;
    ledger.stage(
        weak.work(),
        weak.retained_bytes(),
        weak.peak_retained_bytes(),
    )?;
    let (work, retained_bytes) = ledger.allowances()?;
    let pairs = identify_bidi_bracket_pairs(
        &weak,
        BidiBracketPairLimits {
            scalars: limits.scalars,
            active_scalars: limits.active_scalars,
            sequences: limits.sequences,
            pairs: limits.pairs,
            work,
            retained_bytes,
        },
    )
    .map_err(BidiStoredError::BracketPairs)?;
    ledger.stage(
        pairs.work(),
        pairs.retained_bytes(),
        pairs.peak_retained_bytes(),
    )?;
    let (work, retained_bytes) = ledger.allowances()?;
    let n0 = resolve_bidi_n0(
        &pairs,
        BidiN0Limits {
            scalars: limits.scalars,
            active_scalars: limits.active_scalars,
            runs: limits.runs,
            sequences: limits.sequences,
            pairs: limits.pairs,
            work,
            retained_bytes,
        },
    )
    .map_err(BidiStoredError::N0)?;
    ledger.stage(n0.work(), n0.retained_bytes(), n0.peak_retained_bytes())?;
    let (work, retained_bytes) = ledger.allowances()?;
    let implicit = resolve_bidi_implicit(
        &n0,
        BidiImplicitLimits {
            scalars: limits.scalars,
            active_scalars: limits.active_scalars,
            runs: limits.runs,
            sequences: limits.sequences,
            work,
            retained_bytes,
        },
    )
    .map_err(BidiStoredError::Implicit)?;
    ledger.stage(
        implicit.work(),
        implicit.retained_bytes(),
        implicit.peak_retained_bytes(),
    )?;
    let (work, retained_bytes) = ledger.allowances()?;
    let lines = reorder_bidi_lines(
        &implicit,
        cuts,
        BidiLineLimits {
            scalars: limits.scalars,
            active_scalars: limits.active_scalars,
            paragraphs: limits.paragraphs,
            lines: limits.lines,
            work,
            retained_bytes,
        },
    )
    .map_err(BidiStoredError::LinesStage)?;
    ledger.stage(
        lines.work(),
        lines.retained_bytes(),
        lines.peak_retained_bytes(),
    )?;
    copy_lines(&lines, ledger)
}
/// Consume initial, compose qualified stages, and retain detached final observations.
/// Historical initial work and borrowed source/cut payload are excluded from the new
/// work budget; initial's live retained payload is included in aggregate bytes.
///
/// # Errors
/// Wrapper Scalars, Paragraphs, Lines precede setup Work and initial Bytes admission.
/// Remaining positive work/bytes are forwarded to each qualified stage in order;
/// typed nested errors preserve its admission order. Copy work and requested/actual
/// capacities are admitted before filling. Failure returns unchanged initial after
/// all intermediates drop. Aggregate peaks count admitted payload, not failed allocator
/// physical peaks. Empty initial with empty cuts takes work1 and no nested stages.
pub fn store_bidi_lines<'s>(
    initial: BidiInitial<'s>,
    line_ends: &[usize],
    limits: BidiStoredLimits,
) -> Result<BidiStoredLines<'s>, BidiStoredFailure<'s>> {
    match build(&initial, line_ends, limits) {
        Ok(data) => {
            let retained_bytes = initial
                .retained_bytes()
                .checked_add(data.payload)
                .expect("final retained subset of admitted copy peak");
            Ok(BidiStoredLines {
                initial,
                headers: data.headers,
                levels: data.levels,
                visual: data.visual,
                retained_bytes,
                peak_retained_bytes: data.ledger.peak,
                work: data.ledger.used,
            })
        }
        Err(error) => Err(BidiStoredFailure { initial, error }),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::bidi_initial::{BidiInitialLimits, BidiParagraphMode, evaluate_bidi_initial};
    fn max() -> NonZeroUsize {
        NonZeroUsize::new(usize::MAX).unwrap()
    }
    fn limits() -> BidiStoredLimits {
        let n = max();
        BidiStoredLimits {
            scalars: n,
            active_scalars: n,
            paragraphs: n,
            runs: n,
            sequences: n,
            pairs: n,
            lines: n,
            retained_bytes: n,
            work: n,
        }
    }
    fn initial(source: &str) -> BidiInitial<'_> {
        let n = max();
        evaluate_bidi_initial(
            source,
            BidiParagraphMode::AutoLeftToRight,
            BidiInitialLimits {
                input_bytes: n,
                scalars: n,
                paragraphs: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap()
    }
    #[test]
    fn same_invocation_stage_ledger_and_actual_copy_capacities() {
        for source in ["", "\u{ad}", "(a)", "\u{2067}א\u{2069}a", "\r\na"] {
            let initial = initial(source);
            let cuts: Vec<_> = (0..initial.paragraph_count())
                .map(|i| initial.paragraph(i).unwrap().scalar_range().end)
                .collect();
            let data = build(&initial, &cuts, limits()).unwrap();
            let ledger = &data.ledger;
            assert_eq!(data.headers.len(), cuts.len());
            assert_eq!(data.levels.len(), initial.scalar_count());
            assert_eq!(
                data.payload,
                data.headers.capacity() * size_of::<StoredLineRecord>()
                    + data.levels.capacity() * size_of::<Option<BidiLineLevel>>()
                    + data.visual.capacity() * size_of::<usize>()
            );
            let mut live = initial.retained_bytes();
            let mut peak = live;
            let mut used = 1;
            for receipt in &ledger.receipts[..ledger.count] {
                assert_eq!(receipt.before, live);
                assert_eq!(receipt.work_allowance, usize::MAX - used);
                assert_eq!(receipt.byte_allowance, usize::MAX - live);
                assert!(receipt.work <= receipt.work_allowance);
                assert!(receipt.peak <= receipt.byte_allowance);
                assert!(receipt.retained <= receipt.peak);
                peak = peak.max(live + receipt.peak);
                live += receipt.retained;
                used += receipt.work;
            }
            assert_eq!(live, ledger.live);
            peak = peak.max(live + data.payload);
            assert_eq!(peak, ledger.peak);
            assert_eq!(
                ledger.used,
                used + cuts.len() + initial.scalar_count() + data.visual.len()
            );
            assert_eq!(ledger.count, if source.is_empty() { 0 } else { 8 });
            assert!(initial.retained_bytes() + data.payload <= peak);
            if source == "\u{ad}" {
                for index in [3, 4, 5, 6] {
                    assert_eq!(ledger.receipts[index].retained, 0);
                    assert_eq!(ledger.receipts[index].peak, 0);
                    assert!(ledger.receipts[index].byte_allowance > 0);
                }
                assert!(data.payload > 0);
            }
        }
    }
    #[test]
    fn copy_overlap_failure_and_exact_aggregate_admission() {
        let source = "a".repeat(256);
        let initial = initial(&source);
        let cuts = [256];
        let data = build(&initial, &cuts, limits()).unwrap();
        let ledger = &data.ledger;
        assert_eq!(ledger.peak, ledger.live + data.payload);
        assert!(ledger.peak > initial.retained_bytes() + data.payload);
        let exact = BidiStoredLimits {
            work: NonZeroUsize::new(ledger.used).unwrap(),
            retained_bytes: NonZeroUsize::new(ledger.peak).unwrap(),
            ..limits()
        };
        assert!(build(&initial, &cuts, exact).is_ok());
        assert_eq!(
            build(
                &initial,
                &cuts,
                BidiStoredLimits {
                    work: NonZeroUsize::new(ledger.used - 1).unwrap(),
                    ..exact
                }
            )
            .unwrap_err(),
            BidiStoredError::Work
        );
        assert_eq!(
            build(
                &initial,
                &cuts,
                BidiStoredLimits {
                    retained_bytes: NonZeroUsize::new(ledger.peak - 1).unwrap(),
                    ..exact
                }
            )
            .unwrap_err(),
            BidiStoredError::Bytes
        );
        let reduced = BidiStoredLimits {
            work: NonZeroUsize::new(
                ledger.used - cuts.len() - initial.scalar_count() - data.visual.len(),
            )
            .unwrap(),
            ..limits()
        };
        assert_eq!(
            build(&initial, &cuts, reduced).unwrap_err(),
            BidiStoredError::Work
        );
    }
    #[test]
    fn ledger_zero_overflow_and_staged_actual_products() {
        assert_eq!(Ledger::new(0, 0, 1).unwrap_err(), BidiStoredError::Work);
        assert_eq!(Ledger::new(2, 1, 1).unwrap_err(), BidiStoredError::Bytes);
        assert_eq!(
            Ledger::new(0, 1, 1).unwrap().allowances().unwrap_err(),
            BidiStoredError::Work
        );
        assert_eq!(
            Ledger::new(1, 2, 1).unwrap().allowances().unwrap_err(),
            BidiStoredError::Bytes
        );
        let mut ledger = Ledger::new(0, usize::MAX, usize::MAX).unwrap();
        assert_eq!(ledger.charge(usize::MAX), Err(BidiStoredError::Work));
        let ledger = Ledger::new(usize::MAX, 2, usize::MAX).unwrap();
        assert_eq!(ledger.admit_payload(1), Err(BidiStoredError::Bytes));
        assert_eq!(
            allocation_bytes::<usize>(usize::MAX),
            Err(BidiStoredError::Bytes)
        );
        assert_eq!(
            allocation_bytes::<Option<BidiLineLevel>>(isize::MAX as usize + 1),
            Err(BidiStoredError::Bytes)
        );
        assert_eq!(payload(usize::MAX, 0, 0), Err(BidiStoredError::Bytes));
        assert_eq!(payload(0, usize::MAX, 0), Err(BidiStoredError::Bytes));
        assert_eq!(payload(0, 0, usize::MAX), Err(BidiStoredError::Bytes));
        let h = isize::MAX as usize / size_of::<StoredLineRecord>();
        let n = isize::MAX as usize / size_of::<Option<BidiLineLevel>>();
        assert!(payload(h, n, 0).unwrap() > isize::MAX as usize);
        assert_eq!(
            payload(h, n, isize::MAX as usize / size_of::<usize>()),
            Err(BidiStoredError::Bytes)
        );
        let request = payload(1, 2, 2).unwrap();
        let ledger = Ledger::new(10, 100, 10 + request).unwrap();
        assert_eq!(ledger.admit_payload(request), Ok(10 + request));
        assert_eq!(
            ledger.admit_payload(payload(2, 2, 2).unwrap()),
            Err(BidiStoredError::Bytes)
        );
        assert_eq!(
            ledger.admit_payload(payload(1, 3, 2).unwrap()),
            Err(BidiStoredError::Bytes)
        );
        assert_eq!(
            ledger.admit_payload(payload(1, 2, 3).unwrap()),
            Err(BidiStoredError::Bytes)
        );
    }
}
