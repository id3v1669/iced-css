//! Original-context boundary join. No transformed output is a boundary input.
use super::*;
use iced_css_core::text::{
    grapheme_boundaries::{GraphemeBoundaryLimits, evaluate_grapheme_boundaries},
    newlines::NormalizedText,
    word_boundaries::{WordBoundaryLimits, evaluate_word_boundaries},
};

pub(super) fn upper_lane(mut source: SourceRecord) -> Option<usize> {
    source.operation = Some(SourceCasingOperation::Upper);
    lane(source)
}
pub(super) fn word_error(start: usize, error: WordBoundaryError) -> SourceCasingError {
    SourceCasingError::WordBoundaries { start, error }
}
pub(super) fn grapheme_error(start: usize, error: GraphemeBoundaryError) -> SourceCasingError {
    SourceCasingError::GraphemeBoundaries { start, error }
}

pub(super) fn analyze(
    out: &mut SourceCasing<'_, '_, '_, '_, '_, '_>,
    range: Range<usize>,
    normalized: &NormalizedText<'_>,
    ledger: &mut Ledger,
    h: usize,
    a: usize,
    #[cfg(test)] observer: &mut dyn FnMut(Receipt),
) -> Result<(u8, usize), SourceCasingError> {
    let start = range.start;
    let m = range.len();
    let text = normalized.text();
    let scalars = NonZeroUsize::new(m).expect("nonempty capitalize context");
    let input_bytes = NonZeroUsize::new(text.len()).expect("nonempty capitalize bytes");
    #[cfg(test)]
    let p = payload::<SourceRecord>(out.sources.capacity())?;
    #[cfg(test)]
    let r = payload::<ScalarRecord>(out.scalars.capacity())?;
    #[cfg(test)]
    let j = payload::<UnitRecord>(out.units.capacity())?;
    // h/a describe still-live caller-owned payload; used only in test receipts.
    #[cfg(not(test))]
    let _ = (h, a);
    let ww = cost(m, 4, 1)?;
    ledger.charge(ww)?;
    let bytes = ledger.forwarded()?;
    let words = evaluate_word_boundaries(
        text,
        WordBoundaryLimits {
            input_bytes,
            scalars,
            retained_bytes: bytes,
            work: NonZeroUsize::new(ww).expect("word work"),
        },
    )
    .map_err(|error| word_error(start, error))?;
    let wr = words.retained_bytes();
    let wp = words.peak_retained_bytes();
    ledger.admit(wp)?;
    // Qualified nested successful peak: the word helper's cache has already
    // dropped physically. Account that overlap before retaining only offsets.
    observe!(
        observer,
        ledger,
        "word_peak",
        start,
        None,
        [0, p, r, j, h, a, wr, wp - wr, 0, 0, 0],
        Some((bytes.get(), ww))
    );
    ledger.live -= wp - wr;
    observe!(
        observer,
        ledger,
        "word_retained",
        start,
        None,
        [0, p, r, j, h, a, wr, 0, 0, 0, 0],
        None
    );
    let gw = cost(m, 2, 1)?;
    ledger.charge(gw)?;
    let bytes = ledger.forwarded()?;
    let graphemes = evaluate_grapheme_boundaries(
        text,
        GraphemeBoundaryLimits {
            input_bytes,
            scalars,
            retained_bytes: bytes,
            work: NonZeroUsize::new(gw).expect("grapheme work"),
        },
    )
    .map_err(|error| grapheme_error(start, error))?;
    let g = graphemes.retained_bytes();
    ledger.admit(g)?;
    observe!(
        observer,
        ledger,
        "graphemes",
        start,
        None,
        [0, p, r, j, h, a, wr, 0, g, 0, 0],
        Some((bytes.get(), gw))
    );
    ledger.charge(cost(m, 1, 2)?)?;
    if words.scalar_count() != m || graphemes.scalar_count() != m {
        return Err(invariant(start, SourceCasingInvariantReason::Boundaries));
    }
    let selected = join(out, range, text, words.offsets(), graphemes.offsets())?;
    observe!(
        observer,
        ledger,
        "boundary_join",
        start,
        None,
        [0, p, r, j, h, a, wr, 0, g, 0, 0],
        None
    );
    drop(graphemes);
    ledger.live -= g;
    observe!(
        observer,
        ledger,
        "graphemes_drop",
        start,
        None,
        [0, p, r, j, h, a, wr, 0, 0, 0, 0],
        None
    );
    drop(words);
    ledger.live -= wr;
    observe!(
        observer,
        ledger,
        "words_drop",
        start,
        None,
        [0, p, r, j, h, a, 0, 0, 0, 0, 0],
        None
    );
    Ok(selected)
}

// Each offset cursor advances at most once per scalar boundary. A pending word
// retains just its first complete grapheme and whether any member operates.
pub(super) fn join(
    out: &mut SourceCasing<'_, '_, '_, '_, '_, '_>,
    range: Range<usize>,
    text: &str,
    words: &[usize],
    graphemes: &[usize],
) -> Result<(u8, usize), SourceCasingError> {
    let error = invariant(range.start, SourceCasingInvariantReason::Boundaries);
    if words.first() != Some(&0)
        || graphemes.first() != Some(&0)
        || words.last() != Some(&text.len())
        || graphemes.last() != Some(&text.len())
    {
        return Err(error);
    }
    let mut w = 1;
    let mut g = 1;
    let mut pending = UnitRecord {
        start: range.start,
        unit_end: range.end,
        word_end: range.end,
    };
    let mut first = true;
    let mut operative = false;
    let mut mask = 0u8;
    let mut targets = 0;
    let mut decoded = 0;
    for (local, (byte, _)) in text.char_indices().enumerate() {
        decoded = local + 1;
        let global = range.start + local;
        if global >= range.end {
            return Err(error);
        }
        if byte > 0 {
            let wb = *words.get(w).ok_or(error)?;
            let gb = *graphemes.get(g).ok_or(error)?;
            if wb < byte || gb < byte {
                return Err(error);
            }
            let word_cut = wb == byte;
            let grapheme_cut = gb == byte;
            if word_cut {
                w += 1;
            }
            if grapheme_cut {
                g += 1;
                if first {
                    pending.unit_end = global;
                    first = false;
                }
            }
            if word_cut && grapheme_cut {
                pending.word_end = global;
                if operative {
                    append(out, pending, error)?;
                }
                pending = UnitRecord {
                    start: global,
                    unit_end: range.end,
                    word_end: range.end,
                };
                first = true;
                operative = false;
            }
        }
        if first {
            let source = out.sources[out.scalars[global].source];
            if source.operation == Some(SourceCasingOperation::Capitalize) {
                operative = true;
                mask |= 1 << upper_lane(source).expect("upper lane");
                targets += 1;
            }
        }
    }
    // The only unconsumed offset in either stream is the final byte endpoint.
    if decoded != range.len()
        || words.get(w) != Some(&text.len())
        || w + 1 != words.len()
        || graphemes.get(g) != Some(&text.len())
        || g + 1 != graphemes.len()
    {
        return Err(error);
    }
    if operative {
        append(out, pending, error)?;
    }
    Ok((mask, targets))
}
fn append(
    out: &mut SourceCasing<'_, '_, '_, '_, '_, '_>,
    unit: UnitRecord,
    error: SourceCasingError,
) -> Result<(), SourceCasingError> {
    if out.units.len() >= out.capitalize_scalars || out.units.len() >= out.units.capacity() {
        return Err(error);
    }
    out.units.push(unit);
    Ok(())
}
