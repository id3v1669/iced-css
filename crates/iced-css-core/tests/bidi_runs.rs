use iced_css_core::text::{bidi_explicit::*, bidi_initial::*, bidi_isolates::*, bidi_runs::*};
use std::num::NonZeroUsize;
fn max() -> NonZeroUsize {
    NonZeroUsize::new(usize::MAX).unwrap()
}
fn initial(s: &str) -> BidiInitial<'_> {
    let n = max();
    evaluate_bidi_initial(
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
fn limits() -> BidiRunLimits {
    let n = max();
    BidiRunLimits {
        scalars: n,
        paragraphs: n,
        active_scalars: n,
        runs: n,
        sequences: n,
        retained_bytes: n,
        work: n,
    }
}
#[test]
fn grouping_membership_and_raw_paragraphs() {
    for (s, expected) in [
        ("", vec![]),
        ("\u{ad}\u{202c}", vec![]),
        ("a\u{ad}b", vec![vec![0, 2]]),
        ("\r\n", vec![vec![0, 1]]),
        ("\u{202e}\r\na", vec![vec![1, 2], vec![3]]),
    ] {
        let i = initial(s);
        let iso = isolates(&i);
        let ex = explicit(&iso);
        let r = prepare_bidi_run_sequences(&ex, limits()).unwrap();
        assert!(std::ptr::eq(r.explicit(), &ex));
        assert_eq!(r.run_count(), expected.len());
        let mut active = 0;
        for (ri, indices) in expected.iter().enumerate() {
            let run = r.run(ri).unwrap();
            assert_eq!(run.index(), ri);
            assert_eq!(run.scalar_count(), indices.len());
            for (j, &index) in indices.iter().enumerate() {
                let v = run.scalar(j).unwrap();
                assert_eq!(v.index(), index);
                let m = v.membership().unwrap();
                assert_eq!(m.run_index(), ri);
                assert_eq!(m.active_index(), active);
                active += 1;
            }
            assert!(run.scalar(indices.len()).is_none());
            assert!(run.scalar(usize::MAX).is_none());
        }
        assert_eq!(r.active_scalar_count(), active);
        for j in 0..i.scalar_count() {
            let v = r.scalar(j).unwrap();
            assert_eq!(
                v.explicit().original().source_range(),
                i.scalar(j).unwrap().source_range()
            );
            assert_eq!(
                v.membership().is_none(),
                matches!(
                    ex.scalar(j).unwrap().state(),
                    BidiExplicitState::RemovedByX9
                )
            );
        }
        assert!(r.scalar(i.scalar_count()).is_none());
        assert!(r.scalar(usize::MAX).is_none());
        assert!(r.run(r.run_count()).is_none());
        assert!(r.run(usize::MAX).is_none());
        assert!(r.sequence(r.sequence_count()).is_none());
        assert!(r.sequence(usize::MAX).is_none());
    }
}
#[test]
fn noncontiguous_chains_directions_and_iterators() {
    // Original X10 second table, text spans represented by L.
    let s = "a\u{2067}a\u{2066}a\u{2069}a\u{2069}\u{2067}a\u{2069}a";
    let i = initial(s);
    let iso = isolates(&i);
    let ex = explicit(&iso);
    let r = prepare_bidi_run_sequences(&ex, limits()).unwrap();
    let expected = [
        (vec![0, 1, 7, 8, 10, 11], 0, "L", "L"),
        (vec![2, 3, 5, 6], 1, "R", "R"),
        (vec![4], 2, "L", "L"),
        (vec![9], 1, "R", "R"),
    ];
    assert_eq!(r.sequence_count(), expected.len());
    for (si, (indices, level, sos, eos)) in expected.into_iter().enumerate() {
        let seq = r.sequence(si).unwrap();
        let copied = seq;
        assert_eq!(copied.index(), si);
        assert_eq!(seq.level().number(), level);
        assert_eq!(format!("{:?}", seq.sos().bidi_class()), sos);
        assert_eq!(format!("{:?}", seq.eos().bidi_class()), eos);
        let mut iter = seq.scalars();
        let mut views = Vec::new();
        for (j, index) in indices.iter().enumerate() {
            assert_eq!(iter.len(), indices.len() - j);
            assert_eq!(
                iter.size_hint(),
                (indices.len() - j, Some(indices.len() - j))
            );
            let v = iter.next().unwrap();
            assert_eq!(v.index(), *index);
            views.push(v);
        }
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert_eq!(iter.len(), 0);
        assert_eq!(views[0].index(), indices[0]);
        let mut runs = seq.runs();
        let count = runs.len();
        let mut seen = 0;
        while let Some(run) = runs.next() {
            assert_eq!(run.sequence_index(), si);
            seen += 1;
            assert_eq!(runs.len(), count - seen);
        }
        assert_eq!(seen, seq.run_count());
        assert!(runs.next().is_none());
    }
    // Structural original controls retain links even when current types are overridden.
    let i = initial("\u{202e}\u{2066}a\u{2069}\u{202c}");
    let iso = isolates(&i);
    let ex = explicit(&iso);
    let r = prepare_bidi_run_sequences(&ex, limits()).unwrap();
    assert_eq!(r.run(0).unwrap().next_run_index(), Some(2));
    assert_eq!(
        r.scalar(1).unwrap().membership().unwrap().sequence_index(),
        r.scalar(3).unwrap().membership().unwrap().sequence_index()
    );
}
#[test]
fn exact_work_bytes_and_staged_errors() {
    for s in ["", "\u{ad}", "a", "\n\n", "a\u{2067}א\u{2069}"] {
        let i = initial(s);
        let iso = isolates(&i);
        let ex = explicit(&iso);
        let r = prepare_bidi_run_sequences(&ex, limits()).unwrap();
        assert_eq!(
            r.work(),
            1 + 2 * i.paragraph_count() + 2 * i.scalar_count() + 3 * r.run_count()
        );
        assert_eq!(r.retained_bytes(), r.peak_retained_bytes());
        let mut l = limits();
        l.work = NonZeroUsize::new(r.work()).unwrap();
        l.retained_bytes = NonZeroUsize::new(r.retained_bytes().max(1)).unwrap();
        assert!(prepare_bidi_run_sequences(&ex, l).is_ok());
        if r.work() > 1 {
            l.work = NonZeroUsize::new(r.work() - 1).unwrap();
            assert_eq!(
                prepare_bidi_run_sequences(&ex, l).unwrap_err(),
                BidiRunError::Work
            );
        }
        if r.retained_bytes() > 1 {
            let mut l = limits();
            l.retained_bytes = NonZeroUsize::new(r.retained_bytes() - 1).unwrap();
            assert_eq!(
                prepare_bidi_run_sequences(&ex, l).unwrap_err(),
                BidiRunError::Bytes
            );
        }
    }
    let one = NonZeroUsize::new(1).unwrap();
    let l = BidiRunLimits {
        scalars: one,
        paragraphs: one,
        active_scalars: one,
        runs: one,
        sequences: one,
        retained_bytes: one,
        work: one,
    };
    let i = initial("");
    let iso = isolates(&i);
    let ex = explicit(&iso);
    let r = prepare_bidi_run_sequences(&ex, l).unwrap();
    assert_eq!(r.retained_bytes(), 0);
    assert_eq!(r.work(), 1);
    let i = initial("\n\n");
    let iso = isolates(&i);
    let ex = explicit(&iso);
    assert_eq!(
        prepare_bidi_run_sequences(&ex, l).unwrap_err(),
        BidiRunError::Scalars
    );
    assert_eq!(
        prepare_bidi_run_sequences(
            &ex,
            BidiRunLimits {
                scalars: max(),
                ..l
            }
        )
        .unwrap_err(),
        BidiRunError::Paragraphs
    );
    assert_eq!(
        prepare_bidi_run_sequences(
            &ex,
            BidiRunLimits {
                scalars: max(),
                paragraphs: max(),
                ..l
            }
        )
        .unwrap_err(),
        BidiRunError::Work
    );
    for (l, e) in [
        (
            BidiRunLimits {
                active_scalars: one,
                ..limits()
            },
            BidiRunError::ActiveScalars,
        ),
        (
            BidiRunLimits {
                runs: one,
                ..limits()
            },
            BidiRunError::Runs,
        ),
        (
            BidiRunLimits {
                sequences: one,
                ..limits()
            },
            BidiRunError::Sequences,
        ),
    ] {
        assert_eq!(prepare_bidi_run_sequences(&ex, l).unwrap_err(), e);
    }
}
