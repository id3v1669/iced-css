//! Original UAX9 X1–X9: explicit levels/current types and X9 membership.
//! These are not final bidi levels. Paragraph construction and original isolate
//! preparation remain in the exact borrowed predecessor chain.
use super::{
    bidi_data::BidiClass,
    bidi_initial::{BidiParagraphBase, BidiScalar},
    bidi_isolates::{BidiIsolateScalar, BidiIsolates},
};
use std::{mem::size_of, num::NonZeroUsize};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BidiExplicitLevel(u8);
impl BidiExplicitLevel {
    pub fn number(self) -> u8 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiExplicitState {
    RemovedByX9,
    Retained {
        level: BidiExplicitLevel,
        current_type: BidiClass,
    },
}
#[derive(Debug, Clone, Copy)]
pub struct BidiExplicitLimits {
    pub scalars: NonZeroUsize,
    pub paragraphs: NonZeroUsize,
    /// Tracked successful output capacity plus the fixed status array while alive.
    /// Excludes borrowed predecessors, other locals, allocator overhead and physical
    /// peaks of rejected reservations; this is not a thread-stack peak bound.
    pub retained_bytes: NonZeroUsize,
    /// Empty: 1. Nonempty: 1 + 127 + paragraphs + scalars.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiExplicitError {
    Scalars,
    Paragraphs,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for BidiExplicitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "explicit bidi resolution: {self:?}")
    }
}
impl std::error::Error for BidiExplicitError {}
/// Immutable explicit-stage observations; removed records have no invented level.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// drop(source);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// drop(initial);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// drop(isolates);
/// assert_eq!(result.scalar_count(),4);
/// ```
///
/// ```compile_fail,E0505
/// use iced_css_core::text::{bidi_initial::*,bidi_isolates::*,bidi_explicit::*};
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(10000).unwrap();
/// let source=String::from("text");
/// let initial=evaluate_bidi_initial(&source,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let isolates=prepare_bidi_isolates(&initial,BidiIsolateLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let result=resolve_bidi_explicit(&isolates,BidiExplicitLimits{scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let view=result.scalar(0).unwrap();
/// drop(result);
/// assert_eq!(view.original().text(),"t");
/// ```
#[must_use]
#[derive(Debug)]
pub struct BidiExplicit<'isolates, 'initial, 'source> {
    isolates: &'isolates BidiIsolates<'initial, 'source>,
    states: Vec<BidiExplicitState>,
    retained_bytes: usize,
    status_stack_bytes: usize,
    peak_retained_bytes: usize,
    work: usize,
}
impl<'isolates, 'initial, 'source> BidiExplicit<'isolates, 'initial, 'source> {
    pub fn isolates(&self) -> &'isolates BidiIsolates<'initial, 'source> {
        self.isolates
    }
    pub fn scalar_count(&self) -> usize {
        self.isolates.scalar_count()
    }
    pub fn paragraph_count(&self) -> usize {
        self.isolates.paragraph_count()
    }
    /// Actual retained enum-vector payload, excluding predecessors.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Tracked vector plus fixed status-array payload, excluding other stack locals.
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
    /// Actual fixed array layout for nonempty input; zero for empty input.
    pub fn status_stack_bytes(&self) -> usize {
        self.status_stack_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
    pub fn scalar(
        &self,
        index: usize,
    ) -> Option<BidiExplicitScalar<'_, 'isolates, 'initial, 'source>> {
        self.states.get(index)?;
        Some(BidiExplicitScalar { owner: self, index })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiExplicitScalar<'view, 'isolates, 'initial, 'source> {
    owner: &'view BidiExplicit<'isolates, 'initial, 'source>,
    index: usize,
}
impl<'view, 'isolates, 'initial, 'source> BidiExplicitScalar<'view, 'isolates, 'initial, 'source> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn state(&self) -> BidiExplicitState {
        self.owner.states[self.index]
    }
    pub fn original(&self) -> BidiScalar<'_> {
        self.owner
            .isolates
            .initial()
            .scalar(self.index)
            .expect("explicit record has an original scalar")
    }
    pub fn isolate(&self) -> BidiIsolateScalar<'_, 'initial, 'source> {
        self.owner
            .isolates
            .scalar(self.index)
            .expect("explicit record has an isolate record")
    }
}
#[derive(Clone, Copy)]
enum Override {
    Neutral,
    Left,
    Right,
}
#[derive(Clone, Copy)]
struct Status {
    level: u8,
    override_type: Override,
    isolate: bool,
    nearest_isolate: Option<usize>,
}
const EMPTY_STATUS: Status = Status {
    level: 0,
    override_type: Override::Neutral,
    isolate: false,
    nearest_isolate: None,
};
const STATUS_SLOTS: usize = 127;
fn observation(top: Status, class: BidiClass) -> BidiExplicitState {
    assert!(top.level <= 125, "explicit status level is bounded");
    BidiExplicitState::Retained {
        level: BidiExplicitLevel(top.level),
        current_type: match top.override_type {
            Override::Neutral => class,
            Override::Left => BidiClass::L,
            Override::Right => BidiClass::R,
        },
    }
}
fn increment(counter: usize) -> Result<usize, BidiExplicitError> {
    counter.checked_add(1).ok_or(BidiExplicitError::Scalars)
}
fn bytes_for(capacity: usize) -> Result<usize, BidiExplicitError> {
    capacity
        .checked_mul(size_of::<BidiExplicitState>())
        .filter(|&b| b <= isize::MAX as usize)
        .ok_or(BidiExplicitError::Bytes)
}
fn combined(a: usize, b: usize, allowance: usize) -> Result<usize, BidiExplicitError> {
    a.checked_add(b)
        .filter(|&b| b <= allowance)
        .ok_or(BidiExplicitError::Bytes)
}
fn work_for(n: usize, p: usize, allowance: usize) -> Result<usize, BidiExplicitError> {
    1usize
        .checked_add(if n == 0 { 0 } else { STATUS_SLOTS })
        .and_then(|w| w.checked_add(p))
        .and_then(|w| w.checked_add(n))
        .filter(|&w| w <= allowance)
        .ok_or(BidiExplicitError::Work)
}
fn process_nonempty(
    isolates: &BidiIsolates<'_, '_>,
    states: &mut Vec<BidiExplicitState>,
) -> Result<(), BidiExplicitError> {
    let mut stack = [EMPTY_STATUS; STATUS_SLOTS];
    for paragraph_index in 0..isolates.paragraph_count() {
        let paragraph = isolates
            .initial()
            .paragraph(paragraph_index)
            .expect("admitted paragraph index");
        let base = paragraph.base().level();
        stack[0] = Status {
            level: base,
            ..EMPTY_STATUS
        };
        let mut len = 1usize;
        let mut overflow_isolates = 0usize;
        let mut overflow_embeddings = 0usize;
        let mut valid_isolates = 0usize;
        for index in paragraph.scalar_range() {
            let isolate = isolates.scalar(index).expect("admitted paragraph scalar");
            let original = isolate.original().class();
            let class = if original == BidiClass::Fsi {
                match isolate
                    .fsi_direction()
                    .expect("original FSI has prepared direction")
                {
                    BidiParagraphBase::LeftToRight => BidiClass::Lri,
                    BidiParagraphBase::RightToLeft => BidiClass::Rli,
                }
            } else {
                original
            };
            let state = match class {
                BidiClass::Rle
                | BidiClass::Lre
                | BidiClass::Rlo
                | BidiClass::Lro
                | BidiClass::Rli
                | BidiClass::Lri => {
                    let top = stack[len - 1];
                    let entering_isolate = matches!(class, BidiClass::Rli | BidiClass::Lri);
                    let old = if entering_isolate {
                        observation(top, class)
                    } else {
                        BidiExplicitState::RemovedByX9
                    };
                    let odd = matches!(class, BidiClass::Rle | BidiClass::Rlo | BidiClass::Rli);
                    let candidate =
                        (u16::from(top.level) + if odd { 1 } else { 2 }) & !1 | u16::from(odd);
                    if candidate <= 125 && overflow_isolates == 0 && overflow_embeddings == 0 {
                        let override_type = match class {
                            BidiClass::Rlo => Override::Right,
                            BidiClass::Lro => Override::Left,
                            _ => Override::Neutral,
                        };
                        stack[len] = Status {
                            level: candidate as u8,
                            override_type,
                            isolate: entering_isolate,
                            nearest_isolate: if entering_isolate {
                                Some(len)
                            } else {
                                top.nearest_isolate
                            },
                        };
                        len += 1;
                        if entering_isolate {
                            valid_isolates = increment(valid_isolates)?;
                        }
                    } else if entering_isolate {
                        overflow_isolates = increment(overflow_isolates)?;
                    } else if overflow_isolates == 0 {
                        overflow_embeddings = increment(overflow_embeddings)?;
                    }
                    old
                }
                BidiClass::Pdi => {
                    if overflow_isolates > 0 {
                        overflow_isolates -= 1;
                    } else if valid_isolates > 0 {
                        let nearest = stack[len - 1]
                            .nearest_isolate
                            .expect("valid isolate has nearest status");
                        assert!(
                            nearest > 0 && nearest < len,
                            "nearest isolate lies above base"
                        );
                        len = nearest;
                        valid_isolates -= 1;
                        overflow_embeddings = 0;
                    }
                    observation(stack[len - 1], BidiClass::Pdi)
                }
                BidiClass::Pdf => {
                    if overflow_isolates == 0 {
                        if overflow_embeddings > 0 {
                            overflow_embeddings -= 1;
                        } else if len > 1 && !stack[len - 1].isolate {
                            len -= 1;
                        }
                    }
                    BidiExplicitState::RemovedByX9
                }
                BidiClass::Bn => BidiExplicitState::RemovedByX9,
                BidiClass::B => observation(
                    Status {
                        level: base,
                        ..EMPTY_STATUS
                    },
                    BidiClass::B,
                ),
                _ => observation(stack[len - 1], class),
            };
            states.push(state);
        }
    }
    Ok(())
}
/// Execute X1–X9 without changing original scalar records or paragraph boundaries.
/// Fixed status initialization is paid once, not once per paragraph.
///
/// # Errors
/// Scalars precedes Paragraphs, then checked Work and requested Bytes admission.
/// Allocation denotes fallible output reservation failure. Actual output capacity
/// plus fixed status bytes is checked before status initialization or filling.
/// Overflowing counters report Scalars; failures publish no owner and preserve all
/// predecessors. Metrics exclude predecessor buffers and physical allocator peaks.
pub fn resolve_bidi_explicit<'isolates, 'initial, 'source>(
    isolates: &'isolates BidiIsolates<'initial, 'source>,
    limits: BidiExplicitLimits,
) -> Result<BidiExplicit<'isolates, 'initial, 'source>, BidiExplicitError> {
    let n = isolates.scalar_count();
    let p = isolates.paragraph_count();
    if n > limits.scalars.get() {
        return Err(BidiExplicitError::Scalars);
    }
    if p > limits.paragraphs.get() {
        return Err(BidiExplicitError::Paragraphs);
    }
    let work = work_for(n, p, limits.work.get())?;
    let status_stack_bytes = if n == 0 {
        0
    } else {
        size_of::<[Status; STATUS_SLOTS]>()
    };
    combined(
        bytes_for(n)?,
        status_stack_bytes,
        limits.retained_bytes.get(),
    )?;
    let mut states = Vec::new();
    states
        .try_reserve_exact(n)
        .map_err(|_| BidiExplicitError::Allocation)?;
    let retained_bytes = bytes_for(states.capacity())?;
    let peak_retained_bytes = combined(
        retained_bytes,
        status_stack_bytes,
        limits.retained_bytes.get(),
    )?;
    if n > 0 {
        process_nonempty(isolates, &mut states)?;
    }
    Ok(BidiExplicit {
        isolates,
        states,
        retained_bytes,
        status_stack_bytes,
        peak_retained_bytes,
        work,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{bidi_initial::*, bidi_isolates::*};
    #[test]
    fn arithmetic_and_actual_layout() {
        assert_eq!(bytes_for(usize::MAX), Err(BidiExplicitError::Bytes));
        assert_eq!(
            bytes_for(isize::MAX as usize / size_of::<BidiExplicitState>() + 1),
            Err(BidiExplicitError::Bytes)
        );
        assert_eq!(
            combined(usize::MAX, 1, usize::MAX),
            Err(BidiExplicitError::Bytes)
        );
        assert_eq!(
            combined(isize::MAX as usize, 1, usize::MAX),
            Ok(isize::MAX as usize + 1)
        );
        assert_eq!(increment(usize::MAX), Err(BidiExplicitError::Scalars));
        assert_eq!(
            work_for(usize::MAX, 1, usize::MAX),
            Err(BidiExplicitError::Work)
        );
        assert_eq!(
            work_for(1, usize::MAX, usize::MAX),
            Err(BidiExplicitError::Work)
        );
        let n = NonZeroUsize::new(usize::MAX).unwrap();
        for s in ["", "abc", "\u{2068}א\u{2069}", "\n\n"] {
            let initial = evaluate_bidi_initial(
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
                &initial,
                BidiIsolateLimits {
                    scalars: n,
                    paragraphs: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            let r = resolve_bidi_explicit(
                &iso,
                BidiExplicitLimits {
                    scalars: n,
                    paragraphs: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            assert_eq!(r.states.len(), initial.scalar_count());
            assert!(r.states.capacity() >= r.states.len());
            assert_eq!(
                r.retained_bytes,
                r.states.capacity() * size_of::<BidiExplicitState>()
            );
            assert_eq!(
                r.status_stack_bytes,
                if s.is_empty() {
                    0
                } else {
                    size_of::<[Status; 127]>()
                }
            );
            assert_eq!(
                r.peak_retained_bytes,
                r.retained_bytes + r.status_stack_bytes
            );
        }
    }
}
