//! Original bidi classes and initial paragraph bases over a caller-owned raw str.
//! Original B separates paragraphs, with CRLF grouped; empty input has no paragraph.
//! This selected input contract does not choose CSS paragraph construction or execute
//! resolved levels, reordering, bracket resolution or FSI direction.
use super::bidi_data::{BidiClass, bidi_class};
use std::{mem::size_of, num::NonZeroUsize, ops::Range};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiParagraphMode {
    LeftToRight,
    RightToLeft,
    AutoLeftToRight,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiParagraphBase {
    LeftToRight,
    RightToLeft,
}
impl BidiParagraphBase {
    pub fn level(self) -> u8 {
        match self {
            Self::LeftToRight => 0,
            Self::RightToLeft => 1,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct BidiInitialLimits {
    pub input_bytes: NonZeroUsize,
    pub scalars: NonZeroUsize,
    pub paragraphs: NonZeroUsize,
    /// Actual successful vector payload; excludes borrowed source and allocator overhead.
    pub retained_bytes: NonZeroUsize,
    /// Exact structural reservation: 1 + 3 * scalar count.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiInitialError {
    InputBytes,
    Scalars,
    Paragraphs,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for BidiInitialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "initial bidi paragraphs: {self:?}")
    }
}
impl std::error::Error for BidiInitialError {}
#[derive(Debug)]
struct ScalarRecord {
    end: usize,
    class: BidiClass,
    paragraph_end: bool,
}
#[derive(Debug)]
struct ParagraphRecord {
    scalars: Range<usize>,
    source: Range<usize>,
    base: BidiParagraphBase,
}
/// An immutable original-source owner; views also borrow its private records.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::bidi_initial::*;
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(1000).unwrap();
/// let s=String::from("abc");
/// let r=evaluate_bidi_initial(&s,BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// drop(s);
/// assert_eq!(r.source(),"abc");
/// ```
/// ```compile_fail,E0505
/// use iced_css_core::text::bidi_initial::*;
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(1000).unwrap();
/// let r=evaluate_bidi_initial("abc",BidiParagraphMode::AutoLeftToRight,
/// BidiInitialLimits{input_bytes:n,scalars:n,paragraphs:n,retained_bytes:n,work:n}).unwrap();
/// let view=r.scalar(0).unwrap();
/// drop(r);
/// assert_eq!(view.text(),"a");
/// ```
#[must_use]
#[derive(Debug)]
pub struct BidiInitial<'source> {
    source: &'source str,
    mode: BidiParagraphMode,
    scalars: Vec<ScalarRecord>,
    paragraphs: Vec<ParagraphRecord>,
    work: usize,
    retained_bytes: usize,
}
impl<'source> BidiInitial<'source> {
    pub fn source(&self) -> &'source str {
        self.source
    }
    pub fn mode(&self) -> BidiParagraphMode {
        self.mode
    }
    pub fn scalar_count(&self) -> usize {
        self.scalars.len()
    }
    pub fn paragraph_count(&self) -> usize {
        self.paragraphs.len()
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    /// Both vectors remain owned; there is no heap scratch. This is admitted payload,
    /// not a guarantee about physical peaks of rejected allocator reservations.
    pub fn peak_retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn work(&self) -> usize {
        self.work
    }
    pub fn scalar(&self, index: usize) -> Option<BidiScalar<'_>> {
        let record = self.scalars.get(index)?;
        let start = if index == 0 {
            0
        } else {
            self.scalars.get(index - 1)?.end
        };
        Some(BidiScalar {
            record,
            source: self.source,
            start,
        })
    }
    pub fn paragraph(&self, index: usize) -> Option<BidiParagraph<'_, 'source>> {
        self.paragraphs.get(index)?;
        Some(BidiParagraph { owner: self, index })
    }
}
#[derive(Debug)]
pub struct BidiScalar<'owner> {
    record: &'owner ScalarRecord,
    source: &'owner str,
    start: usize,
}
impl BidiScalar<'_> {
    pub fn class(&self) -> BidiClass {
        self.record.class
    }
    pub fn source_range(&self) -> Range<usize> {
        self.start..self.record.end
    }
    pub fn text(&self) -> &str {
        &self.source[self.source_range()]
    }
}
#[derive(Debug)]
pub struct BidiParagraph<'owner, 'source> {
    owner: &'owner BidiInitial<'source>,
    index: usize,
}
impl BidiParagraph<'_, '_> {
    fn record(&self) -> &ParagraphRecord {
        &self.owner.paragraphs[self.index]
    }
    pub fn source_range(&self) -> Range<usize> {
        self.record().source.clone()
    }
    pub fn scalar_range(&self) -> Range<usize> {
        self.record().scalars.clone()
    }
    pub fn text(&self) -> &str {
        &self.owner.source[self.source_range()]
    }
    pub fn base(&self) -> BidiParagraphBase {
        self.record().base
    }
    pub fn scalar_count(&self) -> usize {
        self.record().scalars.len()
    }
    pub fn scalar(&self, index: usize) -> Option<BidiScalar<'_>> {
        if index >= self.scalar_count() {
            return None;
        }
        self.owner
            .scalar(self.record().scalars.start.checked_add(index)?)
    }
}
fn bytes_for<T>(capacity: usize) -> Result<usize, BidiInitialError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&b| b <= isize::MAX as usize)
        .ok_or(BidiInitialError::Bytes)
}
fn combined(a: usize, b: usize, allowance: usize) -> Result<usize, BidiInitialError> {
    a.checked_add(b)
        .filter(|&x| x <= allowance)
        .ok_or(BidiInitialError::Bytes)
}
fn charge(work: usize, amount: usize, allowance: usize) -> Result<usize, BidiInitialError> {
    work.checked_add(amount)
        .filter(|&x| x <= allowance)
        .ok_or(BidiInitialError::Work)
}
/// Classify unchanged scalars and evaluate P2/P3 or the explicit bounded HL1 base.
/// Counting is metered before each decode; two further scalar visits are prepaid.
///
/// # Errors
/// Distinct variants report input/count/work/payload limits or their arithmetic
/// overflow, and fallible reservation failure. No partial owner is published.
pub fn evaluate_bidi_initial(
    source: &str,
    mode: BidiParagraphMode,
    limits: BidiInitialLimits,
) -> Result<BidiInitial<'_>, BidiInitialError> {
    let mut work = charge(0, 1, limits.work.get())?;
    if source.len() > limits.input_bytes.get() {
        return Err(BidiInitialError::InputBytes);
    }
    let mut chars = source.chars();
    let mut count = 0usize;
    while !chars.as_str().is_empty() {
        let next = count
            .checked_add(1)
            .filter(|&n| n <= limits.scalars.get())
            .ok_or(BidiInitialError::Scalars)?;
        work = charge(work, 1, limits.work.get())?;
        chars.next();
        count = next;
    }
    work = charge(
        work,
        count.checked_mul(2).ok_or(BidiInitialError::Work)?,
        limits.work.get(),
    )?;
    combined(
        bytes_for::<ScalarRecord>(count)?,
        0,
        limits.retained_bytes.get(),
    )?;
    let mut scalars = Vec::new();
    scalars
        .try_reserve_exact(count)
        .map_err(|_| BidiInitialError::Allocation)?;
    let scalar_bytes = bytes_for::<ScalarRecord>(scalars.capacity())?;
    combined(scalar_bytes, 0, limits.retained_bytes.get())?;
    let mut paragraph_count = 0usize;
    for (start, c) in source.char_indices() {
        let end = start + c.len_utf8();
        let class = bidi_class(c);
        let paragraph_end = (class == BidiClass::B
            && !(c == '\r' && source.as_bytes().get(end) == Some(&b'\n')))
            || end == source.len();
        if paragraph_end {
            paragraph_count = paragraph_count
                .checked_add(1)
                .filter(|&p| p <= limits.paragraphs.get())
                .ok_or(BidiInitialError::Paragraphs)?;
        }
        scalars.push(ScalarRecord {
            end,
            class,
            paragraph_end,
        });
    }
    combined(
        scalar_bytes,
        bytes_for::<ParagraphRecord>(paragraph_count)?,
        limits.retained_bytes.get(),
    )?;
    let mut paragraphs = Vec::new();
    paragraphs
        .try_reserve_exact(paragraph_count)
        .map_err(|_| BidiInitialError::Allocation)?;
    let retained_bytes = combined(
        scalar_bytes,
        bytes_for::<ParagraphRecord>(paragraphs.capacity())?,
        limits.retained_bytes.get(),
    )?;
    let mut depth = 0usize;
    let mut first = None;
    let mut scalar_start = 0;
    let mut byte_start = 0;
    for (i, record) in scalars.iter().enumerate() {
        if mode == BidiParagraphMode::AutoLeftToRight {
            match record.class {
                BidiClass::Lri | BidiClass::Rli | BidiClass::Fsi => depth += 1,
                BidiClass::Pdi => depth = depth.saturating_sub(1),
                BidiClass::L if depth == 0 && first.is_none() => {
                    first = Some(BidiParagraphBase::LeftToRight)
                }
                BidiClass::R | BidiClass::Al if depth == 0 && first.is_none() => {
                    first = Some(BidiParagraphBase::RightToLeft)
                }
                _ => {}
            }
        }
        if record.paragraph_end {
            let base = match mode {
                BidiParagraphMode::LeftToRight => BidiParagraphBase::LeftToRight,
                BidiParagraphMode::RightToLeft => BidiParagraphBase::RightToLeft,
                BidiParagraphMode::AutoLeftToRight => {
                    first.unwrap_or(BidiParagraphBase::LeftToRight)
                }
            };
            paragraphs.push(ParagraphRecord {
                scalars: scalar_start..i + 1,
                source: byte_start..record.end,
                base,
            });
            scalar_start = i + 1;
            byte_start = record.end;
            depth = 0;
            first = None;
        }
    }
    Ok(BidiInitial {
        source,
        mode,
        scalars,
        paragraphs,
        work,
        retained_bytes,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn payload_and_overflow() {
        assert_eq!(
            bytes_for::<ScalarRecord>(usize::MAX),
            Err(BidiInitialError::Bytes)
        );
        assert_eq!(
            bytes_for::<ParagraphRecord>(isize::MAX as usize / size_of::<ParagraphRecord>() + 1),
            Err(BidiInitialError::Bytes)
        );
        assert_eq!(
            combined(usize::MAX, 1, usize::MAX),
            Err(BidiInitialError::Bytes)
        );
        assert_eq!(
            combined(isize::MAX as usize, 1, usize::MAX),
            Ok(isize::MAX as usize + 1)
        );
        assert_eq!(
            charge(usize::MAX, 1, usize::MAX),
            Err(BidiInitialError::Work)
        );
        let n = NonZeroUsize::new(usize::MAX).unwrap();
        for s in ["", "abc", "a\r\nא", "\n\n"] {
            let r = evaluate_bidi_initial(
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
            assert_eq!(
                r.retained_bytes,
                r.scalars.capacity() * size_of::<ScalarRecord>()
                    + r.paragraphs.capacity() * size_of::<ParagraphRecord>()
            );
            assert!(r.scalars.capacity() >= r.scalar_count());
            assert!(r.paragraphs.capacity() >= r.paragraph_count());
        }
    }
}
