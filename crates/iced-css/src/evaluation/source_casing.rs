//! Mapped lower/upper/none casing over the exact source-whitespace owner.
//!
//! Original normalized context crosses admitted inline boundaries and stops at
//! Block boundaries. Per-target language and operation select pinned Unicode17
//! mappings. Temporary context strings are algorithm inputs, never application
//! source identities. This does not implement capitalize, shaping or line edges.
use super::{
    source_participants::{SourceOuterCategory, SourceParticipant, SourceParticipants},
    source_structural_events::SourceStructuralEventKind,
    source_whitespace::{SourceWhitespace, SourceWhitespaceScalar},
};
use crate::capture::{ContentCapability, source::ConstructionSourceError};
use iced_css_core::{
    selectors::SelectorSource,
    source::selector_tree::SelectorTreeLimits,
    text::{
        casing_contexts::{CasingContextError, CasingContextLimits, evaluate_casing_contexts},
        casing_data::{CaseMapping, CasingLocale, ConditionalCaseMapping},
        casing_selection::{
            CasingOperation, CasingSelectionError, CasingSelectionLimits, CasingSelectionOrigin,
            select_case_mappings,
        },
        newlines::{NewlineError, NewlineLimits, normalize_default_newlines},
    },
};
use std::{iter::FusedIterator, mem::size_of, num::NonZeroUsize, ops::Range};
#[path = "source_casing_language.rs"]
mod language;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCasingOperation {
    None,
    Lower,
    Upper,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCasingLanguageStatus {
    Unknown,
    Malformed,
    WellFormed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCasingProfile {
    Default,
    Lithuanian,
    Turkish,
    Azeri,
}
#[derive(Debug, Clone, Copy)]
pub enum SourceCasingOrigin {
    Untransformed,
    Base,
    Conditional(&'static ConditionalCaseMapping),
}
#[derive(Debug, Clone, Copy)]
pub enum SourceCasingPresentation {
    SuppressedByWhitespace,
    Scalar(char),
    Replacement(&'static [char]),
}
impl SourceCasingPresentation {
    pub fn characters(self) -> SourceCasingCharacters {
        let (scalar, rest) = match self {
            Self::SuppressedByWhitespace => (None, &[][..]),
            Self::Scalar(c) => (Some(c), &[][..]),
            Self::Replacement(s) => (None, s),
        };
        SourceCasingCharacters { scalar, rest }
    }
}
#[derive(Debug)]
pub struct SourceCasingCharacters {
    scalar: Option<char>,
    rest: &'static [char],
}
impl Iterator for SourceCasingCharacters {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        if let Some(c) = self.scalar.take() {
            return Some(c);
        }
        let (&first, rest) = self.rest.split_first()?;
        self.rest = rest;
        Some(first)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}
impl ExactSizeIterator for SourceCasingCharacters {
    fn len(&self) -> usize {
        usize::from(self.scalar.is_some()) + self.rest.len()
    }
}
impl FusedIterator for SourceCasingCharacters {}
#[derive(Debug, Clone, Copy)]
pub struct SourceCasingLimits {
    /// All captured nodes, including suppressed sources.
    pub sources: NonZeroUsize,
    /// Every mapped normalized scalar, including whitespace-removed scalars.
    pub scalars: NonZeroUsize,
    /// Resolved known language bytes, counted once per eligible source occurrence.
    pub language_bytes: NonZeroUsize,
    /// Original normalized UTF8 bytes across active context segments.
    pub context_bytes: NonZeroUsize,
    /// Maximum admitted live owned payload capacities. Borrowed owners, static
    /// data, headers, allocator internals and failed-operation physical peaks excluded.
    pub retained_bytes: NonZeroUsize,
    /// Exact prepaid structural work including the existing view's possibly
    /// quadratic duplicate-ID checks; only the newly added stages are linear.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCasingTransformReason {
    Capitalize,
    InvalidKeyword,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCasingInvariantReason {
    Source,
    OriginalCr,
    Alignment,
    TargetCoverage,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCasingError {
    Sources,
    Scalars,
    LanguageBytes {
        source: usize,
    },
    ContextBytes {
        start: usize,
    },
    Work,
    Bytes,
    Allocation,
    LanguageView(ConstructionSourceError),
    Transform {
        source: usize,
        reason: SourceCasingTransformReason,
    },
    Invariant {
        start: usize,
        reason: SourceCasingInvariantReason,
    },
    Newlines {
        start: usize,
        error: NewlineError,
    },
    Contexts {
        start: usize,
        error: CasingContextError,
    },
    Selection {
        start: usize,
        operation: SourceCasingOperation,
        profile: SourceCasingProfile,
        error: CasingSelectionError,
    },
}
impl std::fmt::Display for SourceCasingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mapped source casing: {self:?}")
    }
}
impl std::error::Error for SourceCasingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LanguageView(e) => Some(e),
            Self::Newlines { error, .. } => Some(error),
            Self::Contexts { error, .. } => Some(error),
            Self::Selection { error, .. } => Some(error),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy)]
struct SourceRecord {
    status: SourceCasingLanguageStatus,
    profile: SourceCasingProfile,
    operation: Option<SourceCasingOperation>,
}
#[derive(Debug, Clone, Copy)]
struct ScalarRecord {
    source: usize,
    mapping: CaseMapping,
    origin: SourceCasingOrigin,
}
/// Immutable selections borrow exact whitespace provenance. Returned scalar/source
/// views borrow this owner; copied mappings and presentations contain static values.
#[must_use]
pub struct SourceCasing<'w, 'e, 'd, 's, 'b, 'r> {
    whitespace: &'w SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    sources: Vec<SourceRecord>,
    scalars: Vec<ScalarRecord>,
    captured_sources: usize,
    language_bytes: usize,
    known_languages: usize,
    context_bytes: usize,
    active_contexts: usize,
    selection_passes: usize,
    view_work: usize,
    work: usize,
    retained_bytes: usize,
    peak: usize,
}
impl std::fmt::Debug for SourceCasing<'_, '_, '_, '_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceCasing")
            .field("sources", &self.sources.len())
            .field("scalars", &self.scalars.len())
            .field("work", &self.work)
            .finish_non_exhaustive()
    }
}
impl<'w, 'e, 'd, 's, 'b, 'r> SourceCasing<'w, 'e, 'd, 's, 'b, 'r> {
    pub fn whitespace(&self) -> &'w SourceWhitespace<'e, 'd, 's, 'b, 'r> {
        self.whitespace
    }
    pub fn captured_source_count(&self) -> usize {
        self.captured_sources
    }
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
    pub fn scalar_count(&self) -> usize {
        self.scalars.len()
    }
    pub fn language_bytes(&self) -> usize {
        self.language_bytes
    }
    pub fn known_language_count(&self) -> usize {
        self.known_languages
    }
    pub fn context_bytes(&self) -> usize {
        self.context_bytes
    }
    pub fn active_context_count(&self) -> usize {
        self.active_contexts
    }
    pub fn selection_pass_count(&self) -> usize {
        self.selection_passes
    }
    pub fn language_view_work(&self) -> usize {
        self.view_work
    }
    pub fn work(&self) -> usize {
        self.work
    }
    /// Actual two-vector retained payload, excluding borrowed/static/fixed storage.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak
    }
    pub fn source(&self, index: usize) -> Option<SourceCasingSource<'_, 'w, 'e, 'd, 's, 'b, 'r>> {
        let ordinal = self.whitespace.eligibility().ordinal(index)?;
        self.sources.get(ordinal)?;
        Some(SourceCasingSource {
            owner: self,
            ordinal,
        })
    }
    pub fn scalar(&self, index: usize) -> Option<SourceCasingScalar<'_, 'w, 'e, 'd, 's, 'b, 'r>> {
        self.scalars.get(index)?;
        Some(SourceCasingScalar { owner: self, index })
    }
    pub fn sources(&self) -> SourceCasingSources<'_, 'w, 'e, 'd, 's, 'b, 'r> {
        SourceCasingSources {
            owner: self,
            range: 0..self.sources.len(),
        }
    }
    pub fn scalars(&self) -> SourceCasingScalars<'_, 'w, 'e, 'd, 's, 'b, 'r> {
        SourceCasingScalars {
            owner: self,
            range: 0..self.scalars.len(),
        }
    }
}
#[derive(Debug)]
pub struct SourceCasingSource<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceCasing<'w, 'e, 'd, 's, 'b, 'r>,
    ordinal: usize,
}
impl<'e, 'd, 's, 'b, 'r> SourceCasingSource<'_, '_, 'e, 'd, 's, 'b, 'r> {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn source_index(&self) -> usize {
        self.owner.whitespace.eligibility().ordered()[self.ordinal].source
    }
    pub fn language_status(&self) -> SourceCasingLanguageStatus {
        self.owner.sources[self.ordinal].status
    }
    pub fn profile(&self) -> SourceCasingProfile {
        self.owner.sources[self.ordinal].profile
    }
    pub fn operation(&self) -> Option<SourceCasingOperation> {
        self.owner.sources[self.ordinal].operation
    }
    pub fn participant(&self) -> SourceParticipant<'e, 'd, 's, 'b, 'r> {
        SourceParticipants::new(self.owner.whitespace.eligibility())
            .get(self.source_index())
            .expect("validated eligible source")
    }
}
#[derive(Debug)]
pub struct SourceCasingScalar<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceCasing<'w, 'e, 'd, 's, 'b, 'r>,
    index: usize,
}
impl<'o, 'w, 'e, 'd, 's, 'b, 'r> SourceCasingScalar<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn source(&self) -> SourceCasingSource<'o, 'w, 'e, 'd, 's, 'b, 'r> {
        SourceCasingSource {
            owner: self.owner,
            ordinal: self.owner.scalars[self.index].source,
        }
    }
    pub fn whitespace_scalar(&self) -> SourceWhitespaceScalar<'w, 'e, 'd, 's, 'b, 'r> {
        self.owner
            .whitespace
            .scalar(self.index)
            .expect("validated scalar ordinal")
    }
    pub fn mapping(&self) -> CaseMapping {
        self.owner.scalars[self.index].mapping
    }
    pub fn origin(&self) -> SourceCasingOrigin {
        self.owner.scalars[self.index].origin
    }
    pub fn presentation(&self) -> SourceCasingPresentation {
        let Some(scalar) = self.whitespace_scalar().rendered_scalar() else {
            return SourceCasingPresentation::SuppressedByWhitespace;
        };
        match self.mapping() {
            CaseMapping::Identity => SourceCasingPresentation::Scalar(scalar),
            CaseMapping::Replacement(s) => SourceCasingPresentation::Replacement(s),
        }
    }
}
#[derive(Debug)]
pub struct SourceCasingSources<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceCasing<'w, 'e, 'd, 's, 'b, 'r>,
    range: Range<usize>,
}
impl<'o, 'w, 'e, 'd, 's, 'b, 'r> Iterator for SourceCasingSources<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    type Item = SourceCasingSource<'o, 'w, 'e, 'd, 's, 'b, 'r>;
    fn next(&mut self) -> Option<Self::Item> {
        let ordinal = self.range.next()?;
        Some(SourceCasingSource {
            owner: self.owner,
            ordinal,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl ExactSizeIterator for SourceCasingSources<'_, '_, '_, '_, '_, '_, '_> {}
impl FusedIterator for SourceCasingSources<'_, '_, '_, '_, '_, '_, '_> {}
#[derive(Debug)]
pub struct SourceCasingScalars<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    owner: &'o SourceCasing<'w, 'e, 'd, 's, 'b, 'r>,
    range: Range<usize>,
}
impl<'o, 'w, 'e, 'd, 's, 'b, 'r> Iterator for SourceCasingScalars<'o, 'w, 'e, 'd, 's, 'b, 'r> {
    type Item = SourceCasingScalar<'o, 'w, 'e, 'd, 's, 'b, 'r>;
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.range.next()?;
        Some(SourceCasingScalar {
            owner: self.owner,
            index,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl ExactSizeIterator for SourceCasingScalars<'_, '_, '_, '_, '_, '_, '_> {}
impl FusedIterator for SourceCasingScalars<'_, '_, '_, '_, '_, '_, '_> {}
fn invariant(start: usize, reason: SourceCasingInvariantReason) -> SourceCasingError {
    SourceCasingError::Invariant { start, reason }
}
fn newline_error(start: usize, error: NewlineError) -> SourceCasingError {
    SourceCasingError::Newlines { start, error }
}
fn context_error(start: usize, error: CasingContextError) -> SourceCasingError {
    SourceCasingError::Contexts { start, error }
}
fn selection_error(
    start: usize,
    operation: SourceCasingOperation,
    profile: SourceCasingProfile,
    error: CasingSelectionError,
) -> SourceCasingError {
    SourceCasingError::Selection {
        start,
        operation,
        profile,
        error,
    }
}
fn cost(n: usize, factor: usize, base: usize) -> Result<usize, SourceCasingError> {
    n.checked_mul(factor)
        .and_then(|v| v.checked_add(base))
        .ok_or(SourceCasingError::Work)
}
fn payload<T>(n: usize) -> Result<usize, SourceCasingError> {
    n.checked_mul(size_of::<T>())
        .filter(|&b| b <= isize::MAX as usize)
        .ok_or(SourceCasingError::Bytes)
}
fn admitted(live: usize, extra: usize, allowance: usize) -> Result<usize, SourceCasingError> {
    live.checked_add(extra)
        .filter(|&b| b <= allowance)
        .ok_or(SourceCasingError::Bytes)
}
struct Ledger {
    work: usize,
    allowance: usize,
    live: usize,
    peak: usize,
}
impl Ledger {
    fn charge(&mut self, n: usize) -> Result<(), SourceCasingError> {
        self.work = self.work.checked_sub(n).ok_or(SourceCasingError::Work)?;
        Ok(())
    }
    fn admit(&mut self, extra: usize) -> Result<(), SourceCasingError> {
        self.live = admitted(self.live, extra, self.allowance)?;
        self.peak = self.peak.max(self.live);
        Ok(())
    }
    fn remaining(&self) -> usize {
        self.allowance - self.live
    }
    fn forwarded(&self) -> Result<NonZeroUsize, SourceCasingError> {
        NonZeroUsize::new(self.remaining().min(isize::MAX as usize)).ok_or(SourceCasingError::Bytes)
    }
}
fn reserve<T>(n: usize, ledger: &mut Ledger) -> Result<Vec<T>, SourceCasingError> {
    let requested = payload::<T>(n)?;
    admitted(ledger.live, requested, ledger.allowance)?;
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| SourceCasingError::Allocation)?;
    ledger.admit(payload::<T>(v.capacity())?)?;
    Ok(v)
}
fn operation(value: &str, source: usize) -> Result<SourceCasingOperation, SourceCasingError> {
    match value {
        "none" => Ok(SourceCasingOperation::None),
        "lowercase" => Ok(SourceCasingOperation::Lower),
        "uppercase" => Ok(SourceCasingOperation::Upper),
        _ => Err(SourceCasingError::Transform {
            source,
            reason: if value == "capitalize" {
                SourceCasingTransformReason::Capitalize
            } else {
                SourceCasingTransformReason::InvalidKeyword
            },
        }),
    }
}
fn lane(record: SourceRecord) -> Option<usize> {
    let offset = match record.operation? {
        SourceCasingOperation::None => return None,
        SourceCasingOperation::Lower => 0,
        SourceCasingOperation::Upper => 4,
    };
    Some(
        offset
            + match record.profile {
                SourceCasingProfile::Default => 0,
                SourceCasingProfile::Lithuanian => 1,
                SourceCasingProfile::Turkish => 2,
                SourceCasingProfile::Azeri => 3,
            },
    )
}
fn lane_values(
    bit: usize,
) -> (
    SourceCasingOperation,
    SourceCasingProfile,
    CasingOperation,
    Option<CasingLocale>,
) {
    let (op, core) = if bit < 4 {
        (SourceCasingOperation::Lower, CasingOperation::Lower)
    } else {
        (SourceCasingOperation::Upper, CasingOperation::Upper)
    };
    let (profile, locale) = match bit % 4 {
        0 => (SourceCasingProfile::Default, None),
        1 => (
            SourceCasingProfile::Lithuanian,
            Some(CasingLocale::Lithuanian),
        ),
        2 => (SourceCasingProfile::Turkish, Some(CasingLocale::Turkish)),
        _ => (SourceCasingProfile::Azeri, Some(CasingLocale::Azeri)),
    };
    (op, profile, core, locale)
}
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
struct Receipt {
    phase: &'static str,
    start: usize,
    lane: Option<usize>,
    components: [usize; 7],
    live: usize,
    peak: usize,
    remaining_work: usize,
    remaining_bytes: usize,
    forwarded: Option<(usize, usize)>,
}
macro_rules! observe {
    ($observer:ident,$ledger:ident,$phase:expr,$start:expr,$lane:expr,$components:expr,$forwarded:expr) => {
        #[cfg(test)]
        $observer(Receipt {
            phase: $phase,
            start: $start,
            lane: $lane,
            components: $components,
            live: $ledger.live,
            peak: $ledger.peak,
            remaining_work: $ledger.work,
            remaining_bytes: $ledger.remaining(),
            forwarded: $forwarded,
        });
    };
}
/// Evaluate the narrow admitted source domain without consuming its owner.
///
/// The result retains two vectors (eligible-source profiles and scalar mappings)
/// and borrows the exact whitespace owner for application provenance. Temporary
/// original-context strings and their normalized/context/selection owners drop
/// before publication; these strings are never application source identities.
///
/// # Errors
/// Checks captured `sources` before mapped `scalars`, then forwards remaining
/// work/bytes to the complete language view. At each eligible source, rejects
/// unsupported content transforms before language byte/scanner work; managed
/// sources have no content operation. Language-byte bounds precede tag work.
/// Active-context byte bounds precede assembly. Other work and requested/actual
/// capacity checks follow the documented stage order; nested errors preserve
/// their helper and original source index or global scalar start as applicable.
/// Malformed tags retain default-fallback diagnostics rather than failing.
/// `capitalize` on actual content (even empty content) returns `Transform`.
/// Failure publishes no partial result and leaves all borrowed owners usable.
pub fn evaluate_source_casing<'w, 'e, 'd, 's, 'b, 'r>(
    whitespace: &'w SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    limits: SourceCasingLimits,
) -> Result<SourceCasing<'w, 'e, 'd, 's, 'b, 'r>, SourceCasingError> {
    build(
        whitespace,
        limits,
        #[cfg(test)]
        &mut |_| {},
    )
}
fn build<'w, 'e, 'd, 's, 'b, 'r>(
    whitespace: &'w SourceWhitespace<'e, 'd, 's, 'b, 'r>,
    limits: SourceCasingLimits,
    #[cfg(test)] observer: &mut dyn FnMut(Receipt),
) -> Result<SourceCasing<'w, 'e, 'd, 's, 'b, 'r>, SourceCasingError> {
    let eligibility = whitespace.eligibility();
    let snapshot = eligibility.display().snapshot();
    let captured = snapshot.construction().nodes().len();
    let n = whitespace.scalar_count();
    let k = whitespace.source_count();
    let mut ledger = Ledger {
        work: limits.work.get(),
        allowance: limits.retained_bytes.get(),
        live: 0,
        peak: 0,
    };
    ledger.charge(1)?;
    if captured > limits.sources.get() {
        return Err(SourceCasingError::Sources);
    }
    if n > limits.scalars.get() {
        return Err(SourceCasingError::Scalars);
    }
    let mut out = SourceCasing {
        whitespace,
        sources: Vec::new(),
        scalars: Vec::new(),
        captured_sources: captured,
        language_bytes: 0,
        known_languages: 0,
        context_bytes: 0,
        active_contexts: 0,
        selection_passes: 0,
        view_work: 0,
        work: 1,
        retained_bytes: 0,
        peak: 0,
    };
    if k == 0 {
        observe!(observer, ledger, "publish", 0, None, [0; 7], None);
        return Ok(out);
    }
    let view_work = NonZeroUsize::new(ledger.work).ok_or(SourceCasingError::Work)?;
    let view_bytes = ledger.forwarded()?;
    let view = snapshot
        .selector_source(SelectorTreeLimits {
            entries: limits.sources,
            retained_bytes: view_bytes,
            work: view_work,
        })
        .map_err(SourceCasingError::LanguageView)?;
    out.view_work = view.work();
    ledger.charge(out.view_work)?;
    let l = view.retained_bytes();
    ledger.admit(l)?;
    observe!(
        observer,
        ledger,
        "view",
        0,
        None,
        [l, 0, 0, 0, 0, 0, 0],
        Some((view_bytes.get(), view_work.get()))
    );
    ledger.charge(k)?;
    out.sources = reserve(k, &mut ledger)?;
    #[cfg(test)]
    let p = payload::<SourceRecord>(out.sources.capacity())?;
    observe!(
        observer,
        ledger,
        "profiles",
        0,
        None,
        [l, p, 0, 0, 0, 0, 0],
        None
    );
    let participants = SourceParticipants::new(eligibility);
    for entry in eligibility.ordered() {
        let source = entry.source;
        let participant = participants
            .get(source)
            .ok_or(invariant(0, SourceCasingInvariantReason::Source))?;
        let op = if matches!(
            participant.content(),
            ContentCapability::Text(_) | ContentCapability::RichText { .. }
        ) {
            Some(operation(
                participant
                    .text_keywords()
                    .text_transform()
                    .keyword()
                    .as_str(),
                source,
            )?)
        } else {
            None
        };
        let facts = view
            .facts(source)
            .ok_or(invariant(0, SourceCasingInvariantReason::Source))?;
        let (status, profile) = if let Some(tag) = facts.language {
            out.language_bytes = out
                .language_bytes
                .checked_add(tag.len())
                .filter(|&v| v <= limits.language_bytes.get())
                .ok_or(SourceCasingError::LanguageBytes { source })?;
            ledger.charge(cost(tag.len(), 1, 219)?)?;
            out.known_languages += 1;
            language::classify(tag)
        } else {
            (
                SourceCasingLanguageStatus::Unknown,
                SourceCasingProfile::Default,
            )
        };
        out.sources.push(SourceRecord {
            status,
            profile,
            operation: op,
        });
    }
    drop(view);
    ledger.live -= l;
    observe!(
        observer,
        ledger,
        "view_drop",
        0,
        None,
        [0, p, 0, 0, 0, 0, 0],
        None
    );
    ledger.charge(n)?;
    out.scalars = reserve(n, &mut ledger)?;
    #[cfg(test)]
    let r = payload::<ScalarRecord>(out.scalars.capacity())?;
    observe!(
        observer,
        ledger,
        "scalars",
        0,
        None,
        [0, p, r, 0, 0, 0, 0],
        None
    );
    for scalar in whitespace.scalars() {
        let source = eligibility
            .ordinal(scalar.participant().source_index())
            .ok_or(invariant(
                scalar.index(),
                SourceCasingInvariantReason::Source,
            ))?;
        out.scalars.push(ScalarRecord {
            source,
            mapping: CaseMapping::Identity,
            origin: SourceCasingOrigin::Untransformed,
        });
    }
    ledger.charge(whitespace.event_count())?;
    let mut start = 0;
    for event in whitespace.events() {
        if event.kind() != SourceStructuralEventKind::Content
            && event.participant().outer_category() == SourceOuterCategory::Block
        {
            let end = event.scalar_range().start;
            segment(
                &mut out,
                start..end,
                &mut ledger,
                limits,
                #[cfg(test)]
                observer,
            )?;
            start = end;
        }
    }
    segment(
        &mut out,
        start..n,
        &mut ledger,
        limits,
        #[cfg(test)]
        observer,
    )?;
    out.work = limits.work.get() - ledger.work;
    out.retained_bytes = ledger.live;
    out.peak = ledger.peak;
    observe!(
        observer,
        ledger,
        "publish",
        n,
        None,
        [0, p, r, 0, 0, 0, 0],
        None
    );
    Ok(out)
}
fn segment(
    out: &mut SourceCasing<'_, '_, '_, '_, '_, '_>,
    range: Range<usize>,
    ledger: &mut Ledger,
    limits: SourceCasingLimits,
    #[cfg(test)] observer: &mut dyn FnMut(Receipt),
) -> Result<(), SourceCasingError> {
    let start = range.start;
    let m = range.len();
    if m == 0 {
        return Ok(());
    }
    ledger.charge(m)?;
    let mut u = 0usize;
    let mut mask = 0u8;
    let mut targets = 0usize;
    for i in range.clone() {
        let scalar = out
            .whitespace
            .scalar(i)
            .ok_or(invariant(start, SourceCasingInvariantReason::Source))?
            .normalized();
        if scalar == '\r' {
            return Err(invariant(start, SourceCasingInvariantReason::OriginalCr));
        }
        u = u
            .checked_add(scalar.len_utf8())
            .ok_or(SourceCasingError::ContextBytes { start })?;
        if let Some(bit) = lane(out.sources[out.scalars[i].source]) {
            mask |= 1 << bit;
            targets += 1;
        }
    }
    if mask == 0 {
        return Ok(());
    }
    ledger.charge(1)?;
    out.context_bytes = out
        .context_bytes
        .checked_add(u)
        .filter(|&v| v <= limits.context_bytes.get())
        .ok_or(SourceCasingError::ContextBytes { start })?;
    ledger.charge(m)?;
    let requested = payload::<u8>(u)?;
    admitted(ledger.live, requested, ledger.allowance)?;
    let mut text = String::new();
    text.try_reserve_exact(u)
        .map_err(|_| SourceCasingError::Allocation)?;
    let h = payload::<u8>(text.capacity())?;
    ledger.admit(h)?;
    #[cfg(test)]
    let p = payload::<SourceRecord>(out.sources.capacity())?;
    #[cfg(test)]
    let r = payload::<ScalarRecord>(out.scalars.capacity())?;
    observe!(
        observer,
        ledger,
        "string",
        start,
        None,
        [0, p, r, h, 0, 0, 0],
        None
    );
    for i in range.clone() {
        text.push(
            out.whitespace
                .scalar(i)
                .expect("checked segment ordinal")
                .normalized(),
        );
    }
    let nw = cost(u, 2, 0)?;
    ledger.charge(nw)?;
    let bytes = ledger.forwarded()?;
    let normalized = normalize_default_newlines(
        &text,
        NewlineLimits {
            input_bytes: NonZeroUsize::new(u).expect("active nonempty bytes"),
            retained_bytes: bytes,
            work: NonZeroUsize::new(nw).expect("active normalization work"),
        },
    )
    .map_err(|error| newline_error(start, error))?;
    let a = normalized.retained_bytes();
    ledger.admit(a)?;
    observe!(
        observer,
        ledger,
        "normalized",
        start,
        None,
        [0, p, r, h, a, 0, 0],
        Some((bytes.get(), nw))
    );
    ledger.charge(m)?;
    if normalized.scalars().len() != m {
        return Err(invariant(start, SourceCasingInvariantReason::Alignment));
    }
    let mut byte = 0;
    for (local, c) in normalized.text().chars().enumerate() {
        let expected = out
            .whitespace
            .scalar(start + local)
            .ok_or(invariant(start, SourceCasingInvariantReason::Alignment))?
            .normalized();
        let end = byte + c.len_utf8();
        let map = &normalized.scalars()[local];
        if c != expected || map.source != (byte..end) || map.normalized != (byte..end) {
            return Err(invariant(start, SourceCasingInvariantReason::Alignment));
        }
        byte = end;
    }
    let cw = cost(m, 3, 1)?;
    ledger.charge(cw)?;
    let bytes = ledger.forwarded()?;
    let contexts = evaluate_casing_contexts(
        &normalized,
        CasingContextLimits {
            scalars: NonZeroUsize::new(m).expect("nonempty context"),
            retained_bytes: bytes,
            work: NonZeroUsize::new(cw).expect("context admission"),
        },
    )
    .map_err(|error| context_error(start, error))?;
    let c = contexts.retained_bytes();
    ledger.admit(c)?;
    observe!(
        observer,
        ledger,
        "contexts",
        start,
        None,
        [0, p, r, h, a, c, 0],
        Some((bytes.get(), cw))
    );
    out.active_contexts += 1;
    let mut writes = 0;
    for bit in 0..8 {
        if mask & (1 << bit) == 0 {
            continue;
        }
        ledger.charge(1)?;
        let sw = cost(m, 1, 1)?;
        ledger.charge(sw)?;
        let bytes = ledger.forwarded()?;
        let (op, profile, core, locale) = lane_values(bit);
        let selected = select_case_mappings(
            &contexts,
            core,
            locale,
            CasingSelectionLimits {
                scalars: NonZeroUsize::new(m).expect("nonempty selection"),
                retained_bytes: bytes,
                work: NonZeroUsize::new(sw).expect("selection admission"),
            },
        )
        .map_err(|error| selection_error(start, op, profile, error))?;
        let q = selected.retained_bytes();
        ledger.admit(q)?;
        observe!(
            observer,
            ledger,
            "selection",
            start,
            Some(bit),
            [0, p, r, h, a, c, q],
            Some((bytes.get(), sw))
        );
        ledger.charge(m)?;
        for (local, i) in range.clone().enumerate() {
            let record = &mut out.scalars[i];
            if lane(out.sources[record.source]) == Some(bit) {
                record.mapping = selected
                    .mapping(local)
                    .ok_or(invariant(start, SourceCasingInvariantReason::Alignment))?;
                record.origin = match selected
                    .origin(local)
                    .ok_or(invariant(start, SourceCasingInvariantReason::Alignment))?
                {
                    CasingSelectionOrigin::Base => SourceCasingOrigin::Base,
                    CasingSelectionOrigin::Conditional(row) => SourceCasingOrigin::Conditional(row),
                };
                writes += 1;
            }
        }
        out.selection_passes += 1;
        drop(selected);
        ledger.live -= q;
        observe!(
            observer,
            ledger,
            "selection_drop",
            start,
            Some(bit),
            [0, p, r, h, a, c, 0],
            None
        );
    }
    if writes != targets {
        return Err(invariant(
            start,
            SourceCasingInvariantReason::TargetCoverage,
        ));
    }
    drop(contexts);
    ledger.live -= c;
    observe!(
        observer,
        ledger,
        "contexts_drop",
        start,
        None,
        [0, p, r, h, a, 0, 0],
        None
    );
    drop(normalized);
    ledger.live -= a;
    observe!(
        observer,
        ledger,
        "normalized_drop",
        start,
        None,
        [0, p, r, h, 0, 0, 0],
        None
    );
    drop(text);
    ledger.live -= h;
    observe!(
        observer,
        ledger,
        "string_drop",
        start,
        None,
        [0, p, r, 0, 0, 0, 0],
        None
    );
    Ok(())
}
#[cfg(test)]
#[path = "source_casing_tests.rs"]
mod tests;
