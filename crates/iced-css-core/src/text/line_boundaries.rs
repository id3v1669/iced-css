//! Unicode17 UAX14 revision55 default opportunities on one unchanged raw domain.
//! This performs no CSS tailoring, line fitting, normalization or caret selection.
use super::{
    line_break_aux_data::{EastAsianWidth, GeneralCategory, east_asian_width, general_category},
    line_break_data::{LineBreak, line_break},
    word_data::is_extended_pictographic,
};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineBreakKind {
    Prohibited,
    Optional,
    Mandatory,
}
/// A permitted position copied from an owner; no public constructor detaches an owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineOpportunity {
    offset: usize,
    kind: LineBreakKind,
}
impl LineOpportunity {
    pub fn offset(&self) -> usize {
        self.offset
    }
    pub fn kind(&self) -> LineBreakKind {
        self.kind
    }
}
#[derive(Debug, Clone, Copy)]
pub struct LineBoundaryLimits {
    pub input_bytes: NonZeroUsize,
    pub scalars: NonZeroUsize,
    /// Admitted successful actual cache/output payload; excludes borrowed/static
    /// storage, allocator overhead and rejected-reservation physical peak.
    pub retained_bytes: NonZeroUsize,
    /// Exact prepaid structural reservation: 1 + 4 * scalar count.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineBoundaryError {
    InputBytes,
    Scalars,
    Work,
    Bytes,
    Allocation,
}
impl std::fmt::Display for LineBoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "raw line boundaries: {self:?}")
    }
}
impl std::error::Error for LineBoundaryError {}
/// Immutable original-source opportunities, with mandatory breaks distinguished.
/// Empty input follows formal first-match LB2: prohibited zero, no stored endpoint.
/// This is an explicit default-precedence inference, not an official empty test case.
///
/// ```compile_fail,E0505
/// use iced_css_core::text::line_boundaries::*;
/// use std::num::NonZeroUsize;
/// let n=NonZeroUsize::new(1000).unwrap();
/// let source=String::from("text");
/// let result=evaluate_line_boundaries(&source,LineBoundaryLimits {
///     input_bytes:n,scalars:n,retained_bytes:n,work:n }).unwrap();
/// drop(source);
/// assert_eq!(result.source(),"text");
/// ```
#[must_use]
#[derive(Debug)]
pub struct LineBoundaries<'source> {
    source: &'source str,
    opportunities: Vec<LineOpportunity>,
    scalar_count: usize,
    retained_bytes: usize,
    peak_retained_bytes: usize,
    work: usize,
}
impl<'source> LineBoundaries<'source> {
    pub fn source(&self) -> &'source str {
        self.source
    }
    pub fn opportunities(&self) -> &[LineOpportunity] {
        &self.opportunities
    }
    pub fn scalar_count(&self) -> usize {
        self.scalar_count
    }
    /// Invalid beyond-end/interior UTF-8 positions return None; absence at a valid
    /// position returns Some(Prohibited). This gives no insertion/caret affinity.
    pub fn break_at(&self, offset: usize) -> Option<LineBreakKind> {
        self.source.is_char_boundary(offset).then(|| {
            self.opportunities
                .binary_search_by_key(&offset, |v| v.offset)
                .map_or(LineBreakKind::Prohibited, |i| self.opportunities[i].kind)
        })
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
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Facts {
    line_break: LineBreak,
    category: GeneralCategory,
    width: EastAsianWidth,
    extended_pictographic: bool,
    dotted_circle: bool,
}
impl Facts {
    fn original(c: char) -> Self {
        Self {
            line_break: line_break(c),
            category: general_category(c),
            width: east_asian_width(c),
            extended_pictographic: is_extended_pictographic(c),
            dotted_circle: c == '\u{25cc}',
        }
    }
    fn east_asian(self) -> bool {
        matches!(
            self.width,
            EastAsianWidth::Fullwidth | EastAsianWidth::Wide | EastAsianWidth::Halfwidth
        )
    }
    fn brahmic(self) -> bool {
        matches!(self.line_break, LineBreak::Ak | LineBreak::As) || self.dotted_circle
    }
    fn aks(self) -> bool {
        self.line_break == LineBreak::Ak || self.dotted_circle
    }
}
const A: Facts = Facts {
    line_break: LineBreak::Al,
    category: GeneralCategory::UppercaseLetter,
    width: EastAsianWidth::Narrow,
    extended_pictographic: false,
    dotted_circle: false,
};
struct Classified {
    end: usize,
    original: Facts,
    resolved: LineBreak,
    effective: Option<Facts>,
    next_effective: Option<usize>,
}
fn hard(c: LineBreak) -> bool {
    matches!(
        c,
        LineBreak::Bk | LineBreak::Cr | LineBreak::Lf | LineBreak::Nl
    )
}
fn excluded(c: LineBreak) -> bool {
    hard(c) || matches!(c, LineBreak::Sp | LineBreak::Zw)
}
fn resolve(f: Facts) -> LineBreak {
    use LineBreak::*;
    match f.line_break {
        Ai | Sg | Xx => Al,
        Cj => Ns,
        Sa if matches!(
            f.category,
            GeneralCategory::NonspacingMark | GeneralCategory::SpacingMark
        ) =>
        {
            Cm
        }
        Sa => Al,
        c => c,
    }
}
impl Classified {
    fn new(end: usize, original: Facts, previous: Option<LineBreak>) -> Self {
        // Derive fields locally from retained original facts before the single push.
        let mut entry = Self {
            end,
            original,
            resolved: original.line_break,
            effective: None,
            next_effective: None,
        };
        entry.resolved = resolve(entry.original);
        entry.effective = if matches!(entry.resolved, LineBreak::Cm | LineBreak::Zwj) {
            if previous.is_some_and(|c| !excluded(c)) {
                None
            } else {
                Some(A)
            }
        } else {
            Some(Facts {
                line_break: entry.resolved,
                ..entry.original
            })
        };
        entry
    }
}
fn class(f: Option<Facts>) -> Option<LineBreak> {
    f.map(|v| v.line_break)
}
fn letters(c: LineBreak) -> bool {
    matches!(c, LineBreak::Al | LineBreak::Hl)
}
fn hangul(c: LineBreak) -> bool {
    matches!(
        c,
        LineBreak::Jl | LineBreak::Jv | LineBreak::Jt | LineBreak::H2 | LineBreak::H3
    )
}
fn initial(c: LineBreak) -> bool {
    hard(c)
        || matches!(
            c,
            LineBreak::Op | LineBreak::Qu | LineBreak::Gl | LineBreak::Sp | LineBreak::Zw
        )
}
#[derive(Default)]
struct Prefix {
    left: Option<Facts>,
    before: Option<Facts>,
    non_space: Option<LineBreak>,
    quote: bool,
    numeric: bool,
    closed_numeric: bool,
    odd_ri: bool,
    physical_non_space: Option<LineBreak>,
}
impl Prefix {
    fn push(&mut self, f: Facts) {
        use LineBreak::*;
        let c = f.line_break;
        if c != Sp {
            self.non_space = Some(c);
            self.quote = c == Qu
                && f.category == GeneralCategory::InitialPunctuation
                && self.left.is_none_or(|l| initial(l.line_break));
        }
        self.closed_numeric = self.numeric && matches!(c, Cl | Cp);
        self.numeric = c == Nu || (matches!(c, Sy | Is) && self.numeric);
        self.odd_ri = c == Ri && !self.odd_ri;
        self.before = self.left;
        self.left = Some(f);
    }
    fn later(&self, right: Facts, next: Option<Facts>, next2: Option<Facts>) -> LineBreakKind {
        use LineBreak::*;
        use LineBreakKind::{Optional, Prohibited};
        let left = self
            .left
            .expect("later boundary has an effective predecessor");
        let l = left.line_break;
        let r = right.line_break;
        let prev = class(self.before);
        let n = class(next);
        let n2 = class(next2);
        if r == Wj || l == Wj {
            return Prohibited;
        } // LB11
        if l == Gl {
            return Prohibited;
        } // LB12
        if r == Gl && !matches!(l, Sp | Ba | Hy | Hh) {
            return Prohibited;
        } // LB12a
        if matches!(r, Cl | Cp | Ex | Sy) {
            return Prohibited;
        } // LB13
        if self.non_space == Some(Op) {
            return Prohibited;
        } // LB14
        if self.quote {
            return Prohibited;
        } // LB15a
        if r == Qu
            && right.category == GeneralCategory::FinalPunctuation
            && n.is_none_or(|v| {
                hard(v) || matches!(v, Sp | Gl | Wj | Cl | Qu | Cp | Ex | Is | Sy | Zw)
            })
        {
            return Prohibited;
        } // LB15b
        if l == Sp && r == Is && n == Some(Nu) {
            return Optional;
        } // LB15c
        if r == Is {
            return Prohibited;
        } // LB15d
        if matches!(self.non_space, Some(Cl | Cp)) && r == Ns {
            return Prohibited;
        } // LB16
        if self.non_space == Some(B2) && r == B2 {
            return Prohibited;
        } // LB17
        if l == Sp {
            return Optional;
        } // LB18
        if (r == Qu && right.category != GeneralCategory::InitialPunctuation)
            || (l == Qu && left.category != GeneralCategory::FinalPunctuation)
        {
            return Prohibited;
        } // LB19
        if (r == Qu && (!left.east_asian() || next.is_none_or(|v| !v.east_asian())))
            || (l == Qu && (!right.east_asian() || self.before.is_none_or(|v| !v.east_asian())))
        {
            return Prohibited;
        } // LB19a
        if r == Cb || l == Cb {
            return Optional;
        } // LB20
        if prev.is_none_or(|v| hard(v) || matches!(v, Sp | Zw | Cb | Gl))
            && matches!(l, Hy | Hh)
            && letters(r)
        {
            return Prohibited;
        } // LB20a
        if matches!(r, Ba | Hh | Hy | Ns) || l == Bb {
            return Prohibited;
        } // LB21
        if prev == Some(Hl) && matches!(l, Hy | Hh) && r != Hl {
            return Prohibited;
        } // LB21a
        if l == Sy && r == Hl {
            return Prohibited;
        } // LB21b
        if r == In {
            return Prohibited;
        } // LB22
        if (letters(l) && r == Nu) || (l == Nu && letters(r)) {
            return Prohibited;
        } // LB23
        if (l == Pr && matches!(r, Id | Eb | Em)) || (matches!(l, Id | Eb | Em) && r == Po) {
            return Prohibited;
        } // LB23a
        if (matches!(l, Pr | Po) && letters(r)) || (letters(l) && matches!(r, Pr | Po)) {
            return Prohibited;
        } // LB24
        if (matches!(r, Po | Pr) && (self.numeric || self.closed_numeric))
            || (matches!(l, Po | Pr)
                && (r == Nu || (r == Op && (n == Some(Nu) || (n == Some(Is) && n2 == Some(Nu))))))
            || (matches!(l, Hy | Is) && r == Nu)
            || (self.numeric && r == Nu)
        {
            return Prohibited;
        } // LB25
        if (l == Jl && matches!(r, Jl | Jv | H2 | H3))
            || (matches!(l, Jv | H2) && matches!(r, Jv | Jt))
            || (matches!(l, Jt | H3) && r == Jt)
        {
            return Prohibited;
        } // LB26
        if (hangul(l) && r == Po) || (l == Pr && hangul(r)) {
            return Prohibited;
        } // LB27
        if letters(l) && letters(r) {
            return Prohibited;
        } // LB28
        if (l == Ap && right.brahmic())
            || (left.brahmic() && matches!(r, Vf | Vi))
            || (self.before.is_some_and(Facts::brahmic) && l == Vi && right.aks())
            || (left.brahmic() && right.brahmic() && n == Some(Vf))
        {
            return Prohibited;
        } // LB28a
        if l == Is && letters(r) {
            return Prohibited;
        } // LB29
        if ((letters(l) || l == Nu) && r == Op && !right.east_asian())
            || (l == Cp && !left.east_asian() && (letters(r) || r == Nu))
        {
            return Prohibited;
        } // LB30
        if l == Ri && r == Ri && self.odd_ri {
            return Prohibited;
        } // LB30a
        if (l == Eb || (left.extended_pictographic && left.category == GeneralCategory::Unassigned))
            && r == Em
        {
            return Prohibited;
        } // LB30b
        Optional // LB31
    }
}
fn next_facts(cache: &[Classified], index: Option<usize>) -> Option<Facts> {
    index.map(|i| {
        cache[i]
            .effective
            .expect("next-effective link points to an effective record")
    })
}
fn decide(cache: &[Classified], i: usize, prefix: &Prefix) -> LineBreakKind {
    use LineBreak::*;
    use LineBreakKind::*;
    let entry = &cache[i];
    let l = cache[i - 1].resolved;
    let r = entry.resolved;
    if l == Bk {
        return Mandatory;
    } // LB4
    if l == Cr && r == Lf {
        return Prohibited;
    } // LB5 CRLF
    if matches!(l, Cr | Lf | Nl) {
        return Mandatory;
    } // LB5
    if hard(r) {
        return Prohibited;
    } // LB6
    if matches!(r, Sp | Zw) {
        return Prohibited;
    } // LB7
    if prefix.physical_non_space == Some(Zw) {
        return Optional;
    } // LB8
    if l == Zwj {
        return Prohibited;
    } // LB8a
    let Some(right) = entry.effective else {
        return Prohibited;
    }; // LB9
    let next = entry.next_effective;
    let next2 = next.and_then(|j| cache[j].next_effective);
    prefix.later(right, next_facts(cache, next), next_facts(cache, next2))
}
fn allocation_bytes<T>(capacity: usize) -> Result<usize, LineBoundaryError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|&b| b <= isize::MAX as usize)
        .ok_or(LineBoundaryError::Bytes)
}
fn combined_bytes(a: usize, b: usize, allowance: usize) -> Result<usize, LineBoundaryError> {
    a.checked_add(b)
        .filter(|&n| n <= allowance)
        .ok_or(LineBoundaryError::Bytes)
}
fn charge(work: usize, amount: usize, allowance: usize) -> Result<usize, LineBoundaryError> {
    work.checked_add(amount)
        .filter(|&n| n <= allowance)
        .ok_or(LineBoundaryError::Work)
}
/// Compute default ordered raw line-break opportunities without changing input.
/// Counting pays before each UTF-8 decode, followed by prepaid classification,
/// reverse links and forward decisions. No query rescans a skipped numeric/mark run.
/// The caller establishes a complete domain; source widgets and normalized owners
/// are not silently substituted. Empty input selects first-match formal LB2.
///
/// # Errors
/// Returns the corresponding input/scalar/work/byte limit error for excess or
/// arithmetic overflow. Bytes covers requested and actual cache/output capacity,
/// including per-allocation addressability. Allocation denotes fallible reservation
/// failure. All failures preserve source and publish no partial result.
pub fn evaluate_line_boundaries(
    source: &str,
    limits: LineBoundaryLimits,
) -> Result<LineBoundaries<'_>, LineBoundaryError> {
    let mut work = charge(0, 1, limits.work.get())?;
    if source.len() > limits.input_bytes.get() {
        return Err(LineBoundaryError::InputBytes);
    }
    let mut chars = source.chars();
    let mut count = 0usize;
    while !chars.as_str().is_empty() {
        let next = count
            .checked_add(1)
            .filter(|&n| n <= limits.scalars.get())
            .ok_or(LineBoundaryError::Scalars)?;
        work = charge(work, 1, limits.work.get())?;
        chars.next();
        count = next;
    }
    work = charge(
        work,
        count.checked_mul(3).ok_or(LineBoundaryError::Work)?,
        limits.work.get(),
    )?;
    let requested_cache = allocation_bytes::<Classified>(count)?;
    let requested_output = allocation_bytes::<LineOpportunity>(count)?;
    combined_bytes(
        requested_cache,
        requested_output,
        limits.retained_bytes.get(),
    )?;
    let mut cache = Vec::new();
    cache
        .try_reserve_exact(count)
        .map_err(|_| LineBoundaryError::Allocation)?;
    let cache_bytes = allocation_bytes::<Classified>(cache.capacity())?;
    combined_bytes(cache_bytes, requested_output, limits.retained_bytes.get())?;
    let mut opportunities = Vec::new();
    opportunities
        .try_reserve_exact(count)
        .map_err(|_| LineBoundaryError::Allocation)?;
    let retained_bytes = allocation_bytes::<LineOpportunity>(opportunities.capacity())?;
    let peak_retained_bytes =
        combined_bytes(cache_bytes, retained_bytes, limits.retained_bytes.get())?;
    let mut previous = None;
    for (start, c) in source.char_indices() {
        let entry = Classified::new(start + c.len_utf8(), Facts::original(c), previous);
        previous = Some(entry.resolved);
        cache.push(entry);
    }
    let mut following = None;
    for (i, entry) in cache.iter_mut().enumerate().rev() {
        entry.next_effective = following;
        if entry.effective.is_some() {
            following = Some(i);
        }
    }
    let mut prefix = Prefix::default();
    for (i, entry) in cache.iter().enumerate() {
        if i > 0 {
            let kind = decide(&cache, i, &prefix);
            if kind != LineBreakKind::Prohibited {
                opportunities.push(LineOpportunity {
                    offset: cache[i - 1].end,
                    kind,
                });
            }
        }
        if let Some(f) = entry.effective {
            prefix.push(f);
        }
        if entry.resolved != LineBreak::Sp {
            prefix.physical_non_space = Some(entry.resolved);
        }
        if i + 1 == count {
            opportunities.push(LineOpportunity {
                offset: source.len(),
                kind: LineBreakKind::Mandatory,
            });
        }
    }
    drop(cache);
    Ok(LineBoundaries {
        source,
        opportunities,
        scalar_count: count,
        retained_bytes,
        peak_retained_bytes,
        work,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_facts_survive_lb1_lb9_and_full_lb10() {
        for (c, category) in [
            ('\u{e31}', GeneralCategory::NonspacingMark),
            ('\u{17b6}', GeneralCategory::SpacingMark),
        ] {
            let raw = Facts::original(c);
            assert_eq!(raw.line_break, LineBreak::Sa);
            assert_eq!(raw.category, category);
            let ignored = Classified::new(c.len_utf8(), raw, Some(LineBreak::Al));
            assert_eq!(ignored.original, raw);
            assert_eq!(ignored.resolved, LineBreak::Cm);
            assert_eq!(ignored.effective, None);
            for previous in [
                None,
                Some(LineBreak::Sp),
                Some(LineBreak::Bk),
                Some(LineBreak::Zw),
            ] {
                let remaining = Classified::new(c.len_utf8(), raw, previous);
                assert_eq!(remaining.original, raw);
                assert_eq!(remaining.effective, Some(A));
            }
        }
        let zwj = Classified::new(3, Facts::original('\u{200d}'), None);
        assert_eq!(zwj.original.line_break, LineBreak::Zwj);
        assert_eq!(zwj.effective, Some(A));
        let dot = Classified::new(3, Facts::original('\u{25cc}'), None);
        assert!(dot.effective.unwrap().dotted_circle);
        // Explicit synthetic original facts exercise clearing every auxiliary bit
        // through the same constructor; not a claimed Unicode scalar assignment.
        let synthetic = Facts {
            line_break: LineBreak::Cm,
            category: GeneralCategory::InitialPunctuation,
            width: EastAsianWidth::Wide,
            extended_pictographic: true,
            dotted_circle: true,
        };
        let result = Classified::new(1, synthetic, None);
        assert_eq!(result.original, synthetic);
        assert_eq!(result.effective, Some(A));
        for (raw, want) in [
            (LineBreak::Ai, LineBreak::Al),
            (LineBreak::Sg, LineBreak::Al),
            (LineBreak::Xx, LineBreak::Al),
            (LineBreak::Cj, LineBreak::Ns),
            (LineBreak::Cb, LineBreak::Cb),
            (LineBreak::Sa, LineBreak::Al),
        ] {
            assert_eq!(
                resolve(Facts {
                    line_break: raw,
                    ..A
                }),
                want
            ); // Sg is schema-only, never a char.
        }
    }
    #[test]
    fn checked_work_payload_and_actual_layout() {
        assert_eq!(
            allocation_bytes::<Classified>(usize::MAX),
            Err(LineBoundaryError::Bytes)
        );
        assert_eq!(
            allocation_bytes::<LineOpportunity>(
                isize::MAX as usize / size_of::<LineOpportunity>() + 1
            ),
            Err(LineBoundaryError::Bytes)
        );
        assert_eq!(
            combined_bytes(usize::MAX, 1, usize::MAX),
            Err(LineBoundaryError::Bytes)
        );
        assert_eq!(
            combined_bytes(isize::MAX as usize, 1, usize::MAX),
            Ok(isize::MAX as usize + 1)
        );
        assert_eq!(
            charge(usize::MAX, 1, usize::MAX),
            Err(LineBoundaryError::Work)
        );
        assert_eq!(charge(1, 4, 4), Err(LineBoundaryError::Work));
        let n = NonZeroUsize::new(usize::MAX).unwrap();
        for source in ["", "abc", "a b", "😀"] {
            let r = evaluate_line_boundaries(
                source,
                LineBoundaryLimits {
                    input_bytes: n,
                    scalars: n,
                    retained_bytes: n,
                    work: n,
                },
            )
            .unwrap();
            assert_eq!(
                r.retained_bytes,
                r.opportunities.capacity() * size_of::<LineOpportunity>()
            );
            assert!(r.opportunities.capacity() >= r.scalar_count);
            let cache_bytes = r.peak_retained_bytes - r.retained_bytes;
            assert_eq!(cache_bytes % size_of::<Classified>(), 0);
            assert!(cache_bytes >= r.scalar_count * size_of::<Classified>());
            if source.is_empty() {
                assert_eq!(cache_bytes, 0);
                assert_eq!(r.retained_bytes, 0);
            }
        }
    }
}
