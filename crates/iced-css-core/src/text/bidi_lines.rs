//! Generic caller-supplied raw scalar lines: L1 logical levels and L2 visual order.
//! This service does not choose CSS cuts or apply L3/L4 glyph processing.
use super::{
    bidi_data::BidiClass,
    bidi_implicit::{BidiImplicit, BidiImplicitScalar},
    bidi_initial::{BidiInitial, BidiParagraphBase},
    bidi_runs::BidiRunSequences,
};
use std::{iter::FusedIterator, mem::size_of, num::NonZeroUsize, ops::Range};
/// A line-adjusted level in 0..=126, distinct from explicit and implicit levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BidiLineLevel(u8);
impl BidiLineLevel {
    pub fn number(self) -> u8 {
        self.0
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiLineLimits {
    pub scalars: NonZeroUsize,
    pub active_scalars: NonZeroUsize,
    pub paragraphs: NonZeroUsize,
    pub lines: NonZeroUsize,
    /// Actual payload of all three owned capacities, excluding borrowed owners and allocator metadata.
    pub retained_bytes: NonZeroUsize,
    /// Reserved upper-bound schedule: 1 + 2p + n + 130l + 192a.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiLineError {
    Scalars,
    ActiveScalars,
    Paragraphs,
    Lines,
    Work,
    InvalidLineEnds,
    Bytes,
    Allocation,
}
impl std::fmt::Display for BidiLineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bidi line processing: {self:?}")
    }
}
impl std::error::Error for BidiLineError {}
#[derive(Debug)]
struct LineRecord {
    original: Range<usize>,
    active: Range<usize>,
    paragraph: usize,
}
/// Immutable L1 levels and L2 order borrowing the exact implicit owner.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*,bidi_implicit::*,bidi_lines::*};
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
/// let implicit=resolve_bidi_implicit(&n0,BidiImplicitLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=reorder_bidi_lines(&implicit,&[3],BidiLineLimits{scalars:n,active_scalars:n,paragraphs:n,lines:n,retained_bytes:n,work:n}).unwrap();
/// drop(source);
/// assert_eq!(result.scalar_count(),3);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*,bidi_implicit::*,bidi_lines::*};
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
/// let implicit=resolve_bidi_implicit(&n0,BidiImplicitLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=reorder_bidi_lines(&implicit,&[3],BidiLineLimits{scalars:n,active_scalars:n,paragraphs:n,lines:n,retained_bytes:n,work:n}).unwrap();
/// drop(implicit);
/// assert_eq!(result.scalar_count(),3);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*,bidi_implicit::*,bidi_lines::*};
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
/// let implicit=resolve_bidi_implicit(&n0,BidiImplicitLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=reorder_bidi_lines(&implicit,&[3],BidiLineLimits{scalars:n,active_scalars:n,paragraphs:n,lines:n,retained_bytes:n,work:n}).unwrap();
/// drop(pairs);
/// assert_eq!(result.scalar_count(),3);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*,bidi_implicit::*,bidi_lines::*};
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
/// let implicit=resolve_bidi_implicit(&n0,BidiImplicitLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=reorder_bidi_lines(&implicit,&[3],BidiLineLimits{scalars:n,active_scalars:n,paragraphs:n,lines:n,retained_bytes:n,work:n}).unwrap();
/// let view=result.scalar(0).unwrap();
/// drop(result);
/// assert_eq!(view.index(),0);
/// ```
///
/// ```compile_fail,E0597
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*,bidi_runs::*,bidi_weak::*,bidi_bracket_pairs::*,bidi_n0::*,bidi_implicit::*,bidi_lines::*};
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
/// let implicit=resolve_bidi_implicit(&n0,BidiImplicitLimits{scalars:n,active_scalars:n,runs:n,sequences:n,retained_bytes:n,work:n}).unwrap();
/// let result=reorder_bidi_lines(&implicit,&[3],BidiLineLimits{scalars:n,active_scalars:n,paragraphs:n,lines:n,retained_bytes:n,work:n}).unwrap();
/// let mut iterator;
/// { let sequence=result.line(0).unwrap(); iterator=sequence.scalars(); }
/// assert!(iterator.next().is_some());
/// ```
#[must_use]
#[derive(Debug)]
pub struct BidiLines<'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    implicit: &'b BidiImplicit<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>,
    headers: Vec<LineRecord>,
    levels: Vec<BidiLineLevel>,
    visual: Vec<usize>,
    retained_bytes: usize,
    work: usize,
}
impl<'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> BidiLines<'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    pub fn implicit(&self) -> &'b BidiImplicit<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
        self.implicit
    }
    pub fn scalar_count(&self) -> usize {
        self.implicit.scalar_count()
    }
    pub fn active_scalar_count(&self) -> usize {
        self.implicit.active_scalar_count()
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
        self.retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
    fn initial(&self) -> &BidiInitial<'s> {
        self.implicit
            .n0()
            .pairs()
            .weak()
            .runs()
            .explicit()
            .isolates()
            .initial()
    }
    pub fn line(&self, index: usize) -> Option<BidiLine<'_, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>> {
        self.headers.get(index)?;
        Some(BidiLine { owner: self, index })
    }
    /// Checked original scalar lookup; finds the containing line in O(log line_count).
    pub fn scalar(
        &self,
        index: usize,
    ) -> Option<BidiLineScalar<'_, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>> {
        if index >= self.scalar_count() {
            return None;
        }
        let line = self.headers.partition_point(|h| h.original.end <= index);
        Some(BidiLineScalar {
            owner: self,
            index,
            line,
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiLineScalar<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiLines<'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>,
    index: usize,
    line: usize,
}
impl<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>
    BidiLineScalar<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>
{
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn line_index(&self) -> usize {
        self.line
    }
    pub fn line_scalar_index(&self) -> usize {
        self.index - self.owner.headers[self.line].original.start
    }
    pub fn implicit_scalar(&self) -> BidiImplicitScalar<'_, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
        self.owner
            .implicit
            .scalar(self.index)
            .expect("line scalar has implicit predecessor")
    }
    pub fn level(&self) -> Option<BidiLineLevel> {
        self.implicit_scalar()
            .n0_scalar()
            .weak_scalar()
            .run_scalar()
            .membership()
            .map(|m| self.owner.levels[m.active_index()])
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiLine<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiLines<'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>,
    index: usize,
}
impl<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> BidiLine<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
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
        self.owner.headers[self.index].active.len()
    }
    pub fn base(&self) -> BidiParagraphBase {
        self.owner
            .initial()
            .paragraph(self.paragraph_index())
            .expect("line paragraph")
            .base()
    }
    pub fn scalar(
        &self,
        local: usize,
    ) -> Option<BidiLineScalar<'_, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>> {
        if local >= self.scalar_count() {
            return None;
        }
        Some(BidiLineScalar {
            owner: self.owner,
            index: self.owner.headers[self.index].original.start + local,
            line: self.index,
        })
    }
    pub fn visual_scalar(
        &self,
        local: usize,
    ) -> Option<BidiLineScalar<'_, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>> {
        if local >= self.active_scalar_count() {
            return None;
        }
        Some(BidiLineScalar {
            owner: self.owner,
            index: self.owner.visual[self.owner.headers[self.index].active.start + local],
            line: self.index,
        })
    }
    pub fn scalars(&self) -> BidiLineScalars<'_, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
        let range = &self.owner.headers[self.index].original;
        BidiLineScalars {
            owner: self.owner,
            line: self.index,
            next: range.start,
            end: range.end,
        }
    }
    pub fn visual_scalars(&self) -> BidiLineVisualScalars<'_, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
        let range = &self.owner.headers[self.index].active;
        BidiLineVisualScalars {
            owner: self.owner,
            line: self.index,
            next: range.start,
            end: range.end,
        }
    }
}
#[derive(Debug)]
pub struct BidiLineScalars<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiLines<'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>,
    line: usize,
    next: usize,
    end: usize,
}
impl<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> Iterator
    for BidiLineScalars<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>
{
    type Item = BidiLineScalar<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.end {
            return None;
        }
        let index = self.next;
        self.next += 1;
        Some(BidiLineScalar {
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
impl ExactSizeIterator for BidiLineScalars<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_> {
    fn len(&self) -> usize {
        self.end - self.next
    }
}
impl FusedIterator for BidiLineScalars<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_> {}
#[derive(Debug)]
pub struct BidiLineVisualScalars<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> {
    owner: &'v BidiLines<'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>,
    line: usize,
    next: usize,
    end: usize,
}
impl<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's> Iterator
    for BidiLineVisualScalars<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>
{
    type Item = BidiLineScalar<'v, 'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.end {
            return None;
        }
        let index = self.owner.visual[self.next];
        self.next += 1;
        Some(BidiLineScalar {
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
impl ExactSizeIterator for BidiLineVisualScalars<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_> {
    fn len(&self) -> usize {
        self.end - self.next
    }
}
impl FusedIterator for BidiLineVisualScalars<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_> {}
fn validate_cuts(initial: &BidiInitial<'_>, cuts: &[usize]) -> Result<(), BidiLineError> {
    let n = initial.scalar_count();
    if n == 0 {
        return if cuts.is_empty() {
            Ok(())
        } else {
            Err(BidiLineError::InvalidLineEnds)
        };
    }
    let mut paragraph = 0;
    let mut paragraph_end = initial
        .paragraph(0)
        .expect("nonempty source paragraph")
        .scalar_range()
        .end;
    let mut previous = 0;
    for &end in cuts {
        if end <= previous || end > n || end > paragraph_end {
            return Err(BidiLineError::InvalidLineEnds);
        }
        if end < n
            && initial.scalar(end - 1).expect("validated end").text() == "\r"
            && initial
                .scalar(end)
                .expect("following original scalar")
                .text()
                == "\n"
        {
            return Err(BidiLineError::InvalidLineEnds);
        }
        previous = end;
        if end == paragraph_end {
            paragraph += 1;
            if paragraph < initial.paragraph_count() {
                paragraph_end = initial
                    .paragraph(paragraph)
                    .expect("next paragraph")
                    .scalar_range()
                    .end;
            }
        }
    }
    if previous == n && paragraph == initial.paragraph_count() {
        Ok(())
    } else {
        Err(BidiLineError::InvalidLineEnds)
    }
}
fn active(runs: &BidiRunSequences<'_, '_, '_, '_>, index: usize) -> usize {
    runs.scalar(index)
        .expect("visual original index")
        .membership()
        .expect("visual member active")
        .active_index()
}
fn initialize(
    implicit: &BidiImplicit<'_, '_, '_, '_, '_, '_, '_, '_>,
    cuts: &[usize],
    headers: &mut Vec<LineRecord>,
    levels: &mut Vec<BidiLineLevel>,
    visual: &mut Vec<usize>,
) {
    let runs = implicit.n0().pairs().weak().runs();
    let initial = runs.explicit().isolates().initial();
    let mut paragraph = 0;
    let mut paragraph_end = initial.paragraph(0).map(|p| p.scalar_range().end);
    let mut start = 0;
    for &end in cuts {
        let active_start = levels.len();
        for index in start..end {
            if let Some(state) = implicit
                .scalar(index)
                .expect("admitted original member")
                .state()
            {
                assert_eq!(active(runs, index), levels.len());
                levels.push(BidiLineLevel(state.level().number()));
                visual.push(index);
            }
        }
        headers.push(LineRecord {
            original: start..end,
            active: active_start..levels.len(),
            paragraph,
        });
        start = end;
        if Some(end) == paragraph_end {
            paragraph += 1;
            paragraph_end = initial.paragraph(paragraph).map(|p| p.scalar_range().end);
        }
    }
}
fn trailing(c: BidiClass) -> bool {
    matches!(
        c,
        BidiClass::Ws | BidiClass::Fsi | BidiClass::Lri | BidiClass::Rli | BidiClass::Pdi
    )
}
fn reset_line(
    initial: &BidiInitial<'_>,
    header: &LineRecord,
    levels: &mut [BidiLineLevel],
    visual: &[usize],
) {
    let base = BidiLineLevel(
        initial
            .paragraph(header.paragraph)
            .expect("line paragraph")
            .base()
            .level(),
    );
    let mut pending = None;
    // Before L2, each visual offset is the corresponding logical active offset.
    for position in header.active.clone() {
        let class = initial
            .scalar(visual[position])
            .expect("active original scalar")
            .class();
        if matches!(class, BidiClass::B | BidiClass::S) {
            let start = pending.take().unwrap_or(position);
            for level in &mut levels[start..=position] {
                *level = base;
            }
        } else if trailing(class) {
            if pending.is_none() {
                pending = Some(position);
            }
        } else {
            pending = None;
        }
    }
    if let Some(start) = pending {
        for level in &mut levels[start..header.active.end] {
            *level = base;
        }
    }
}
fn reverse_span(visual: &mut [usize], start: usize, end: usize) {
    let mut left = start;
    let mut right = end;
    while right > left {
        right -= 1;
        if left >= right {
            break;
        }
        visual.swap(left, right);
        left += 1;
    }
}
fn reorder_line(
    runs: &BidiRunSequences<'_, '_, '_, '_>,
    levels: &[BidiLineLevel],
    visual: &mut [usize],
) {
    let mut maximum = 0;
    let mut lowest_odd = None;
    // Discover bounds only after L1 has finished all pending resets.
    for &index in visual.iter() {
        let number = levels[active(runs, index)].number();
        maximum = maximum.max(number);
        if !number.is_multiple_of(2) {
            lowest_odd = Some(lowest_odd.map_or(number, |old: u8| old.min(number)));
        }
    }
    let Some(minimum) = lowest_odd else {
        return;
    };
    // Visit absent intermediate thresholds as required by L2.
    for threshold in (minimum..=maximum).rev() {
        let mut start = None;
        for position in 0..visual.len() {
            if levels[active(runs, visual[position])].number() >= threshold {
                if start.is_none() {
                    start = Some(position);
                }
            } else if let Some(first) = start.take() {
                reverse_span(visual, first, position);
            }
        }
        if let Some(first) = start {
            reverse_span(visual, first, visual.len());
        }
    }
}
fn allocation_bytes<T>(capacity: usize) -> Result<usize, BidiLineError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&b| b <= isize::MAX as usize)
        .ok_or(BidiLineError::Bytes)
}
fn payload(
    headers: usize,
    levels: usize,
    visual: usize,
    allowance: usize,
) -> Result<usize, BidiLineError> {
    let h = allocation_bytes::<LineRecord>(headers)?;
    let t = allocation_bytes::<BidiLineLevel>(levels)?;
    let u = allocation_bytes::<usize>(visual)?;
    h.checked_add(t)
        .and_then(|s| s.checked_add(u))
        .filter(|&s| s <= allowance)
        .ok_or(BidiLineError::Bytes)
}
fn work_for(
    n: usize,
    a: usize,
    p: usize,
    l: usize,
    allowance: usize,
) -> Result<usize, BidiLineError> {
    p.checked_mul(2)
        .and_then(|w| w.checked_add(n))
        .and_then(|w| l.checked_mul(130).and_then(|l| w.checked_add(l)))
        .and_then(|w| a.checked_mul(192).and_then(|a| w.checked_add(a)))
        .and_then(|w| w.checked_add(1))
        .filter(|&w| w <= allowance)
        .ok_or(BidiLineError::Work)
}
/// Construct a complete caller-supplied partition in original scalar coordinates.
/// Cuts need not outlive the result. Original CRLF cannot be split; each paragraph
/// end must be included. No CSS, shaping or grapheme cut admissibility is implied.
///
/// # Errors
/// Scalars, ActiveScalars, Paragraphs and Lines precede Work admission, which precedes
/// InvalidLineEnds validation. Requested payloads are checked before reservation,
/// and each actual capacity is checked with remaining requested payloads before the
/// next allocation. Bytes includes overflow and per-allocation isize excess;
/// Allocation denotes fallible reservation failure. No partial owner escapes.
/// Retained and peak metrics count only the three actual capacity payloads, excluding
/// borrowed owners, locals, allocator metadata and physical failed-allocation peaks.
pub fn reorder_bidi_lines<'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>(
    implicit: &'b BidiImplicit<'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>,
    line_ends: &[usize],
    limits: BidiLineLimits,
) -> Result<BidiLines<'b, 'z, 'p, 'w, 'r, 'e, 'i, 'n, 's>, BidiLineError> {
    let runs = implicit.n0().pairs().weak().runs();
    let initial = runs.explicit().isolates().initial();
    let n = implicit.scalar_count();
    let a = implicit.active_scalar_count();
    let p = initial.paragraph_count();
    let l = line_ends.len();
    if n > limits.scalars.get() {
        return Err(BidiLineError::Scalars);
    }
    if a > limits.active_scalars.get() {
        return Err(BidiLineError::ActiveScalars);
    }
    if p > limits.paragraphs.get() {
        return Err(BidiLineError::Paragraphs);
    }
    if l > limits.lines.get() {
        return Err(BidiLineError::Lines);
    }
    let work = work_for(n, a, p, l, limits.work.get())?;
    validate_cuts(initial, line_ends)?;
    let allowance = limits.retained_bytes.get();
    payload(l, a, a, allowance)?;
    let mut headers = Vec::new();
    headers
        .try_reserve_exact(l)
        .map_err(|_| BidiLineError::Allocation)?;
    payload(headers.capacity(), a, a, allowance)?;
    let mut levels = Vec::new();
    levels
        .try_reserve_exact(a)
        .map_err(|_| BidiLineError::Allocation)?;
    payload(headers.capacity(), levels.capacity(), a, allowance)?;
    let mut visual = Vec::new();
    visual
        .try_reserve_exact(a)
        .map_err(|_| BidiLineError::Allocation)?;
    let retained_bytes = payload(
        headers.capacity(),
        levels.capacity(),
        visual.capacity(),
        allowance,
    )?;
    initialize(implicit, line_ends, &mut headers, &mut levels, &mut visual);
    for header in &headers {
        reset_line(initial, header, &mut levels, &visual);
        reorder_line(runs, &levels, &mut visual[header.active.clone()]);
    }
    Ok(BidiLines {
        implicit,
        headers,
        levels,
        visual,
        retained_bytes,
        work,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{
        bidi_bracket_pairs::*, bidi_explicit::*, bidi_implicit::*, bidi_initial::*,
        bidi_isolates::*, bidi_n0::*, bidi_runs::*, bidi_weak::*,
    };
    use std::num::NonZeroUsize;
    fn max() -> NonZeroUsize {
        NonZeroUsize::new(usize::MAX).unwrap()
    }
    fn limits() -> BidiLineLimits {
        let n = max();
        BidiLineLimits {
            scalars: n,
            active_scalars: n,
            paragraphs: n,
            lines: n,
            retained_bytes: n,
            work: n,
        }
    }
    macro_rules! fixture {
        ($source:expr,$cuts:expr,$limits:expr,$r:ident=>$body:block) => {{
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
            let implicit = resolve_bidi_implicit(
                &n0,
                BidiImplicitLimits {
                    scalars: n,
                    active_scalars: n,
                    runs: n,
                    sequences: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            let $r = reorder_bidi_lines(&implicit, $cuts, $limits);
            $body
        }};
    }
    #[test]
    fn actual_three_capacities_and_header_only_payload() {
        for (source, cuts) in [
            ("", vec![]),
            ("\u{ad}", vec![1]),
            ("😀אב\r\na", vec![1, 5, 6]),
        ] {
            fixture!(source,&cuts,limits(),result=>{let r=result.unwrap();assert_eq!(r.headers.len(),cuts.len());assert_eq!(r.levels.len(),r.active_scalar_count());assert_eq!(r.visual.len(),r.active_scalar_count());assert_eq!(r.retained_bytes(),r.headers.capacity()*size_of::<LineRecord>()+r.levels.capacity()*size_of::<BidiLineLevel>()+r.visual.capacity()*size_of::<usize>());assert_eq!(r.peak_retained_bytes(),r.retained_bytes());assert!(r.headers.capacity()>=r.headers.len());assert!(r.levels.capacity()>=r.levels.len());assert!(r.visual.capacity()>=r.visual.len());if r.active_scalar_count()==0{assert_eq!(r.retained_bytes(),r.headers.capacity()*size_of::<LineRecord>());}});
        }
    }
    #[test]
    fn checked_resource_products_and_sums() {
        assert_eq!(
            allocation_bytes::<usize>(usize::MAX),
            Err(BidiLineError::Bytes)
        );
        assert_eq!(
            allocation_bytes::<BidiLineLevel>(isize::MAX as usize + 1),
            Err(BidiLineError::Bytes)
        );
        let exact = size_of::<LineRecord>() + size_of::<BidiLineLevel>() + size_of::<usize>();
        assert_eq!(payload(1, 1, 1, exact), Ok(exact));
        assert_eq!(payload(1, 1, 1, exact - 1), Err(BidiLineError::Bytes));
        assert_eq!(payload(0, 0, 0, 1), Ok(0));
        // Each allocation must fit isize; the checked combined payload may exceed it.
        let count = isize::MAX as usize / size_of::<LineRecord>();
        let other = isize::MAX as usize;
        assert!(payload(count, other, 0, usize::MAX).unwrap() > isize::MAX as usize);
        assert_eq!(
            payload(
                count,
                other,
                isize::MAX as usize / size_of::<usize>(),
                usize::MAX
            ),
            Err(BidiLineError::Bytes)
        );
        for (n, a, p, l) in [
            (usize::MAX, 0, 0, 0),
            (0, usize::MAX, 0, 0),
            (0, 0, usize::MAX, 0),
            (0, 0, 0, usize::MAX),
        ] {
            assert_eq!(work_for(n, a, p, l, usize::MAX), Err(BidiLineError::Work));
        }
        assert_eq!(work_for(0, 0, 0, 0, 1), Ok(1));
        assert_eq!(work_for(3, 2, 1, 1, 520), Ok(520));
        assert_eq!(work_for(3, 2, 1, 1, 519), Err(BidiLineError::Work));
    }
    #[test]
    fn synthetic_level_permutations_include_absent_thresholds() {
        // Synthetic line levels only, using real ASCII source membership. No public owner is forged.
        fixture!("abc",&[3],limits(),result=>{let r=result.unwrap();let runs=r.implicit.n0().pairs().weak().runs();for a in 0..=6{for b in 0..=6{for c in 0..=6{let original=[a,b,c];let levels=original.map(BidiLineLevel);let mut actual=vec![0,1,2];reorder_line(runs,&levels,&mut actual);let mut expected=vec![0,1,2];if let Some(lowest)=original.iter().copied().filter(|v|v%2==1).min(){for threshold in (lowest..=*original.iter().max().unwrap()).rev(){let mut next=Vec::new();let mut i=0;while i<expected.len(){if original[expected[i]]<threshold{next.push(expected[i]);i+=1;}else{let start=i;while i<expected.len()&&original[expected[i]]>=threshold{i+=1;}next.extend(expected[start..i].iter().rev());}}expected=next;}}assert_eq!(actual,expected,"{original:?}");}}}let mut actual=vec![0,1,2];reorder_line(runs,&[BidiLineLevel(1),BidiLineLevel(3),BidiLineLevel(3)],&mut actual);assert_eq!(actual,[2,1,0]);let mut high=vec![0,1,2];reorder_line(runs,&[BidiLineLevel(1),BidiLineLevel(126),BidiLineLevel(126)],&mut high);assert_eq!(high,[1,2,0]);});
        for n in 0..=16 {
            for start in 0..=n {
                for end in start..=n {
                    let mut actual: Vec<_> = (0..n).collect();
                    let mut expected = actual.clone();
                    expected[start..end].reverse();
                    reverse_span(&mut actual, start, end);
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}
