//! Original BD9 isolate links and X5c FSI directions within an admitted initial owner.
//! This stage neither chooses paragraphs nor executes explicit levels or X9 removal.
use super::{
    bidi_data::BidiClass,
    bidi_initial::{BidiInitial, BidiParagraphBase, BidiScalar},
};
use std::{mem::size_of, num::NonZeroUsize};
#[derive(Debug, Clone, Copy)]
pub struct BidiIsolateLimits {
    pub scalars: NonZeroUsize,
    pub paragraphs: NonZeroUsize,
    /// Successful output plus scratch payload; excludes the borrowed initial owner,
    /// allocator metadata and physical peaks of rejected reservations.
    pub retained_bytes: NonZeroUsize,
    /// Exact structural reservation: 1 + paragraph count + scalar count.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiIsolateError {
    Scalars,
    Paragraphs,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for BidiIsolateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bidi isolate preparation: {self:?}")
    }
}
impl std::error::Error for BidiIsolateError {}
#[derive(Debug, Clone, Copy)]
struct IsolateRecord {
    matching: Option<usize>,
    fsi_direction: Option<BidiParagraphBase>,
}
#[derive(Clone, Copy)]
struct Frame {
    opening: usize,
    first_seen: bool,
}
/// Immutable records borrow the exact initial owner and its original source.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(1000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// drop(source);
/// assert_eq!(result.initial().source(),"text");
/// ```
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(1000).unwrap();
/// let initial=evaluate_bidi_initial("text",BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// drop(initial);
/// assert_eq!(result.scalar_count(),4);
/// ```
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(1000).unwrap();
/// let initial=evaluate_bidi_initial("text",BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let view=result.scalar(0).unwrap();
/// drop(result);
/// assert_eq!(view.original().text(),"t");
/// ```
#[must_use]
#[derive(Debug)]
pub struct BidiIsolates<'initial, 'source> {
    initial: &'initial BidiInitial<'source>,
    records: Vec<IsolateRecord>,
    retained_bytes: usize,
    peak_retained_bytes: usize,
    work: usize,
}
impl<'initial, 'source> BidiIsolates<'initial, 'source> {
    pub fn initial(&self) -> &'initial BidiInitial<'source> {
        self.initial
    }
    pub fn scalar_count(&self) -> usize {
        self.initial.scalar_count()
    }
    pub fn paragraph_count(&self) -> usize {
        self.initial.paragraph_count()
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
    pub fn scalar(&self, index: usize) -> Option<BidiIsolateScalar<'_, 'initial, 'source>> {
        self.records.get(index)?;
        Some(BidiIsolateScalar { owner: self, index })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiIsolateScalar<'view, 'initial, 'source> {
    owner: &'view BidiIsolates<'initial, 'source>,
    index: usize,
}
impl BidiIsolateScalar<'_, '_, '_> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn original(&self) -> BidiScalar<'_> {
        self.owner
            .initial
            .scalar(self.index)
            .expect("isolate record has an original scalar")
    }
    pub fn matching_scalar_index(&self) -> Option<usize> {
        self.owner.records[self.index].matching
    }
    pub fn fsi_direction(&self) -> Option<BidiParagraphBase> {
        self.owner.records[self.index].fsi_direction
    }
}
fn bytes_for<T>(capacity: usize) -> Result<usize, BidiIsolateError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&b| b <= isize::MAX as usize)
        .ok_or(BidiIsolateError::Bytes)
}
fn combined(a: usize, b: usize, allowance: usize) -> Result<usize, BidiIsolateError> {
    a.checked_add(b)
        .filter(|&b| b <= allowance)
        .ok_or(BidiIsolateError::Bytes)
}
fn work_for(
    scalars: usize,
    paragraphs: usize,
    allowance: usize,
) -> Result<usize, BidiIsolateError> {
    1usize
        .checked_add(paragraphs)
        .and_then(|w| w.checked_add(scalars))
        .filter(|&w| w <= allowance)
        .ok_or(BidiIsolateError::Work)
}
/// Match every original initiator, without explicit-depth truncation. Only the top
/// FSI frame captures a first strong class; nested isolate contents never propagate
/// outward. FSI's fallback is LTR regardless of the initial paragraph base.
///
/// # Errors
/// Scalars, then Paragraphs, then checked Work are admitted before payload checks.
/// Requested output/scratch capacities each obey isize addressability and together
/// obey the caller allowance. Actual output plus requested scratch is checked before
/// scratch reservation; both actual capacities are admitted before filling. Bytes
/// includes arithmetic overflow; Allocation denotes a fallible reservation failure.
/// Failure preserves the initial owner and source and publishes no partial result.
/// Borrowed predecessor payload is excluded and requires separate wrapper accounting.
pub fn prepare_bidi_isolates<'initial, 'source>(
    initial: &'initial BidiInitial<'source>,
    limits: BidiIsolateLimits,
) -> Result<BidiIsolates<'initial, 'source>, BidiIsolateError> {
    let n = initial.scalar_count();
    let p = initial.paragraph_count();
    if n > limits.scalars.get() {
        return Err(BidiIsolateError::Scalars);
    }
    if p > limits.paragraphs.get() {
        return Err(BidiIsolateError::Paragraphs);
    }
    let work = work_for(n, p, limits.work.get())?;
    let scratch_requested = bytes_for::<Frame>(n)?;
    combined(
        bytes_for::<IsolateRecord>(n)?,
        scratch_requested,
        limits.retained_bytes.get(),
    )?;
    let mut records = Vec::<IsolateRecord>::new();
    records
        .try_reserve_exact(n)
        .map_err(|_| BidiIsolateError::Allocation)?;
    let retained_bytes = bytes_for::<IsolateRecord>(records.capacity())?;
    combined(
        retained_bytes,
        scratch_requested,
        limits.retained_bytes.get(),
    )?;
    let mut stack = Vec::<Frame>::new();
    stack
        .try_reserve_exact(n)
        .map_err(|_| BidiIsolateError::Allocation)?;
    let peak_retained_bytes = combined(
        retained_bytes,
        bytes_for::<Frame>(stack.capacity())?,
        limits.retained_bytes.get(),
    )?;
    for paragraph_index in 0..p {
        let paragraph = initial
            .paragraph(paragraph_index)
            .expect("admitted paragraph index");
        for index in paragraph.scalar_range() {
            let class = initial
                .scalar(index)
                .expect("admitted paragraph scalar")
                .class();
            records.push(IsolateRecord {
                matching: None,
                fsi_direction: (class == BidiClass::Fsi).then_some(BidiParagraphBase::LeftToRight),
            });
            match class {
                BidiClass::Lri | BidiClass::Rli | BidiClass::Fsi => stack.push(Frame {
                    opening: index,
                    first_seen: false,
                }),
                BidiClass::Pdi => {
                    if let Some(frame) = stack.pop() {
                        records[frame.opening].matching = Some(index);
                        records[index].matching = Some(frame.opening);
                    }
                }
                BidiClass::L | BidiClass::R | BidiClass::Al => {
                    if let Some(frame) = stack.last_mut() {
                        let opening = &mut records[frame.opening];
                        if opening.fsi_direction.is_some() && !frame.first_seen {
                            opening.fsi_direction = Some(if class == BidiClass::L {
                                BidiParagraphBase::LeftToRight
                            } else {
                                BidiParagraphBase::RightToLeft
                            });
                            frame.first_seen = true;
                        }
                    }
                }
                _ => {}
            }
        }
        // Frame is trivial: discard unmatched initiators without a cleanup scan.
        stack.clear();
    }
    drop(stack);
    Ok(BidiIsolates {
        initial,
        records,
        retained_bytes,
        peak_retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::bidi_initial::{BidiInitialLimits, BidiParagraphMode, evaluate_bidi_initial};
    #[test]
    fn arithmetic_and_actual_capacities() {
        assert_eq!(
            bytes_for::<IsolateRecord>(usize::MAX),
            Err(BidiIsolateError::Bytes)
        );
        assert_eq!(
            bytes_for::<Frame>(isize::MAX as usize / size_of::<Frame>() + 1),
            Err(BidiIsolateError::Bytes)
        );
        assert_eq!(
            combined(usize::MAX, 1, usize::MAX),
            Err(BidiIsolateError::Bytes)
        );
        assert_eq!(
            combined(isize::MAX as usize, 1, usize::MAX),
            Ok(isize::MAX as usize + 1)
        );
        assert_eq!(
            work_for(usize::MAX, 0, usize::MAX),
            Err(BidiIsolateError::Work)
        );
        assert_eq!(
            work_for(0, usize::MAX, usize::MAX),
            Err(BidiIsolateError::Work)
        );
        assert_eq!(work_for(2, 1, 3), Err(BidiIsolateError::Work));
        let max = NonZeroUsize::new(usize::MAX).unwrap();
        for s in ["", "abc", "\u{2068}\u{2067}א\u{2069}\u{2069}", "\n\n"] {
            let initial = evaluate_bidi_initial(
                s,
                BidiParagraphMode::AutoLeftToRight,
                BidiInitialLimits {
                    input_bytes: max,
                    scalars: max,
                    paragraphs: max,
                    retained_bytes: max,
                    work: max,
                },
            )
            .unwrap();
            let r = prepare_bidi_isolates(
                &initial,
                BidiIsolateLimits {
                    scalars: max,
                    paragraphs: max,
                    retained_bytes: max,
                    work: max,
                },
            )
            .unwrap();
            assert_eq!(r.records.len(), initial.scalar_count());
            assert!(r.records.capacity() >= r.records.len());
            assert_eq!(
                r.retained_bytes,
                r.records.capacity() * size_of::<IsolateRecord>()
            );
            let scratch_bytes = r.peak_retained_bytes - r.retained_bytes;
            assert_eq!(scratch_bytes % size_of::<Frame>(), 0);
            assert!(scratch_bytes / size_of::<Frame>() >= initial.scalar_count());
        }
    }
}
