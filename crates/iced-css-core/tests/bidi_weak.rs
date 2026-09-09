use iced_css_core::text::{
    bidi_data::BidiClass, bidi_explicit::*, bidi_initial::*, bidi_isolates::*, bidi_runs::*,
    bidi_weak::*,
};
use std::num::NonZeroUsize;
fn max() -> NonZeroUsize {
    NonZeroUsize::new(usize::MAX).unwrap()
}
fn initial(s: &str, mode: BidiParagraphMode) -> BidiInitial<'_> {
    let n = max();
    evaluate_bidi_initial(
        s,
        mode,
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
fn isolates<'a, 's>(i: &'a BidiInitial<'s>) -> BidiIsolates<'a, 's> {
    let n = max();
    prepare_bidi_isolates(
        i,
        BidiIsolateLimits {
            scalars: n,
            paragraphs: n,
            retained_bytes: n,
            work: n,
        },
    )
    .unwrap()
}
fn explicit<'a, 'i, 's>(i: &'a BidiIsolates<'i, 's>) -> BidiExplicit<'a, 'i, 's> {
    let n = max();
    resolve_bidi_explicit(
        i,
        BidiExplicitLimits {
            scalars: n,
            paragraphs: n,
            retained_bytes: n,
            work: n,
        },
    )
    .unwrap()
}
fn runs<'a, 'e, 'i, 's>(i: &'a BidiExplicit<'e, 'i, 's>) -> BidiRunSequences<'a, 'e, 'i, 's> {
    let n = max();
    prepare_bidi_run_sequences(
        i,
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
    .unwrap()
}
fn limits() -> BidiWeakLimits {
    let n = max();
    BidiWeakLimits {
        scalars: n,
        active_scalars: n,
        runs: n,
        sequences: n,
        retained_bytes: n,
        work: n,
    }
}
#[test]
fn literal_weak_rules_and_current_not_original() {
    use BidiClass::*;
    for (s, mode, expected) in [
        (
            "ا1",
            BidiParagraphMode::LeftToRight,
            vec![Some(R), Some(An)],
        ),
        (
            "1+1",
            BidiParagraphMode::RightToLeft,
            vec![Some(En), Some(En), Some(En)],
        ),
        (
            "1++1",
            BidiParagraphMode::RightToLeft,
            vec![Some(En), Some(On), Some(On), Some(En)],
        ),
        (
            "$1$",
            BidiParagraphMode::RightToLeft,
            vec![Some(En), Some(En), Some(En)],
        ),
        (
            "١$",
            BidiParagraphMode::RightToLeft,
            vec![Some(An), Some(On)],
        ),
        ("a1", BidiParagraphMode::RightToLeft, vec![Some(L), Some(L)]),
        (
            "\u{2067}\u{2069}\u{301}",
            BidiParagraphMode::LeftToRight,
            vec![Some(Rli), Some(Pdi), Some(On)],
        ),
        (
            "\u{202d}1\u{2066}\u{2069}\u{301}\u{202c}",
            BidiParagraphMode::RightToLeft,
            vec![None, Some(L), Some(L), Some(L), Some(L), None],
        ),
    ] {
        let i = initial(s, mode);
        let iso = isolates(&i);
        let ex = explicit(&iso);
        let r = runs(&ex);
        let w = resolve_bidi_weak(&r, limits()).unwrap();
        assert!(std::ptr::eq(w.runs(), &r));
        for (j, e) in expected.into_iter().enumerate() {
            let v = w.scalar(j).unwrap();
            let copy = v;
            assert_eq!(copy.index(), j);
            assert_eq!(v.current_type(), e);
            let parent = v.run_scalar();
            assert_eq!(
                parent.explicit().original().class(),
                i.scalar(j).unwrap().class()
            );
            assert_eq!(
                parent.explicit().original().source_range(),
                i.scalar(j).unwrap().source_range()
            );
        }
    }
}
#[test]
fn owner_sequences_and_exact_fused_iterator() {
    let i = initial(
        "😀\u{2067}ا1\u{2069}1\r\n\u{202d}1\u{202c}",
        BidiParagraphMode::AutoLeftToRight,
    );
    let iso = isolates(&i);
    let ex = explicit(&iso);
    let r = runs(&ex);
    let w = resolve_bidi_weak(&r, limits()).unwrap();
    assert_eq!(w.scalar_count(), r.scalar_count());
    assert_eq!(w.active_scalar_count(), r.active_scalar_count());
    assert_eq!(w.run_count(), r.run_count());
    assert_eq!(w.sequence_count(), r.sequence_count());
    for si in 0..w.sequence_count() {
        let seq = w.sequence(si).unwrap();
        let copy = seq;
        assert_eq!(copy.index(), si);
        let parent = r.sequence(si).unwrap();
        assert_eq!(seq.level(), parent.level());
        assert_eq!(seq.sos(), parent.sos());
        assert_eq!(seq.eos(), parent.eos());
        assert_eq!(seq.run_count(), parent.run_count());
        let indices: Vec<_> = parent.scalars().map(|v| v.index()).collect();
        let mut it = seq.scalars();
        let mut held = Vec::new();
        for (j, index) in indices.iter().enumerate() {
            assert_eq!(it.len(), indices.len() - j);
            assert_eq!(it.size_hint(), (indices.len() - j, Some(indices.len() - j)));
            let v = it.next().unwrap();
            assert_eq!(v.index(), *index);
            held.push(v);
        }
        assert!(it.next().is_none());
        assert!(it.next().is_none());
        assert_eq!(it.len(), 0);
        for v in held {
            assert_eq!(
                v.current_type(),
                w.scalar(v.index()).unwrap().current_type()
            );
        }
    }
    assert!(w.scalar(w.scalar_count()).is_none());
    assert!(w.scalar(usize::MAX).is_none());
    assert!(w.sequence(w.sequence_count()).is_none());
    assert!(w.sequence(usize::MAX).is_none());
}
#[test]
fn exact_resources_removed_and_error_order() {
    for s in ["", "\u{ad}\u{202c}", "a1", "\n\n"] {
        let i = initial(s, BidiParagraphMode::RightToLeft);
        let iso = isolates(&i);
        let ex = explicit(&iso);
        let r = runs(&ex);
        let w = resolve_bidi_weak(&r, limits()).unwrap();
        assert_eq!(
            w.work(),
            1 + 2 * r.run_count() + 9 * r.active_scalar_count() + 8 * r.sequence_count()
        );
        let mut l = limits();
        l.work = NonZeroUsize::new(w.work()).unwrap();
        l.retained_bytes = NonZeroUsize::new(w.peak_retained_bytes().max(1)).unwrap();
        assert!(resolve_bidi_weak(&r, l).is_ok());
        if w.work() > 1 {
            l.work = NonZeroUsize::new(w.work() - 1).unwrap();
            assert_eq!(resolve_bidi_weak(&r, l).unwrap_err(), BidiWeakError::Work);
        }
        if w.peak_retained_bytes() > 1 {
            let mut l = limits();
            l.retained_bytes = NonZeroUsize::new(w.peak_retained_bytes() - 1).unwrap();
            assert_eq!(resolve_bidi_weak(&r, l).unwrap_err(), BidiWeakError::Bytes);
        }
        if r.active_scalar_count() == 0 {
            assert_eq!(w.work(), 1);
            assert_eq!(w.retained_bytes(), 0);
            assert_eq!(w.peak_retained_bytes(), 0);
            for j in 0..w.scalar_count() {
                assert_eq!(w.scalar(j).unwrap().current_type(), None);
            }
        }
    }
    let one = NonZeroUsize::new(1).unwrap();
    let mut l = BidiWeakLimits {
        scalars: one,
        active_scalars: one,
        runs: one,
        sequences: one,
        retained_bytes: one,
        work: one,
    };
    for source in ["", "\u{ad}"] {
        let initial = initial(source, BidiParagraphMode::RightToLeft);
        let isolates = isolates(&initial);
        let explicit = explicit(&isolates);
        let runs = runs(&explicit);
        let weak = resolve_bidi_weak(&runs, l).unwrap();
        assert_eq!(weak.work(), 1);
        assert_eq!(weak.retained_bytes(), 0);
        assert_eq!(weak.peak_retained_bytes(), 0);
    }
    let i = initial("\n\n", BidiParagraphMode::LeftToRight);
    let iso = isolates(&i);
    let ex = explicit(&iso);
    let r = runs(&ex);
    for error in [
        BidiWeakError::Scalars,
        BidiWeakError::ActiveScalars,
        BidiWeakError::Runs,
        BidiWeakError::Sequences,
        BidiWeakError::Work,
    ] {
        assert_eq!(resolve_bidi_weak(&r, l).unwrap_err(), error);
        match error {
            BidiWeakError::Scalars => l.scalars = max(),
            BidiWeakError::ActiveScalars => l.active_scalars = max(),
            BidiWeakError::Runs => l.runs = max(),
            BidiWeakError::Sequences => l.sequences = max(),
            _ => {}
        }
    }
}
