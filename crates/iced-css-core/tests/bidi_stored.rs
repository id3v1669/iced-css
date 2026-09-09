use iced_css_core::text::{bidi_initial::*, bidi_lines::BidiLineError, bidi_stored::*};
use std::num::NonZeroUsize;
fn max() -> NonZeroUsize {
    NonZeroUsize::new(usize::MAX).unwrap()
}
fn initial(source: &str) -> BidiInitial<'_> {
    let n = max();
    evaluate_bidi_initial(
        source,
        BidiParagraphMode::LeftToRight,
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
#[test]
fn movable_detached_lines_and_recoverable_failure() {
    let source = String::from("😀אב\r\n\u{ad}");
    let mut cuts = vec![5, 6];
    let result = {
        let raw = initial(&source);
        store_bidi_lines(raw, &cuts, limits()).unwrap()
    };
    cuts.clear();
    drop(cuts);
    let moved = result;
    assert_eq!(moved.initial().source().as_ptr(), source.as_ptr());
    assert_eq!(moved.line_count(), 2);
    assert_eq!(moved.line(0).unwrap().source_range(), 0..10);
    assert_eq!(moved.line(1).unwrap().source_range(), 10..12);
    assert_eq!(moved.scalar(5).unwrap().level(), None);
    assert_eq!(moved.line(1).unwrap().active_scalar_count(), 0);
    assert_eq!(
        moved
            .line(0)
            .unwrap()
            .visual_scalars()
            .map(|v| v.index())
            .collect::<Vec<_>>(),
        [0, 2, 1, 3, 4]
    );
    let failure = store_bidi_lines(initial("\r\n"), &[1, 2], limits()).unwrap_err();
    assert_eq!(
        failure.error(),
        BidiStoredError::LinesStage(BidiLineError::InvalidLineEnds)
    );
    let (raw, error) = failure.into_parts();
    assert_eq!(raw.source(), "\r\n");
    assert_eq!(
        error,
        BidiStoredError::LinesStage(BidiLineError::InvalidLineEnds)
    );
    assert!(store_bidi_lines(raw, &[2], limits()).is_ok());
}
#[test]
fn empty_modes_and_nested_failure_recovery() {
    let one = NonZeroUsize::new(1).unwrap();
    let tiny = BidiStoredLimits {
        scalars: one,
        active_scalars: one,
        paragraphs: one,
        runs: one,
        sequences: one,
        pairs: one,
        lines: one,
        retained_bytes: one,
        work: one,
    };
    let empty = store_bidi_lines(initial(""), &[], tiny).unwrap();
    assert_eq!(empty.work(), 1);
    assert_eq!(empty.retained_bytes(), 0);
    assert_eq!(empty.peak_retained_bytes(), 0);
    assert_eq!(empty.line_count(), 0);
    let source = "(a)(b)\nא";
    let cuts = [7, 8];
    let base = limits();
    let bytes = initial(source).retained_bytes();
    for (limited, want) in [
        (
            BidiStoredLimits {
                scalars: one,
                ..base
            },
            BidiStoredError::Scalars,
        ),
        (
            BidiStoredLimits {
                paragraphs: one,
                ..base
            },
            BidiStoredError::Paragraphs,
        ),
        (
            BidiStoredLimits { lines: one, ..base },
            BidiStoredError::Lines,
        ),
        (
            BidiStoredLimits { work: one, ..base },
            BidiStoredError::Work,
        ),
        (
            BidiStoredLimits {
                retained_bytes: NonZeroUsize::new(bytes - 1).unwrap(),
                ..base
            },
            BidiStoredError::Bytes,
        ),
        (
            BidiStoredLimits {
                retained_bytes: NonZeroUsize::new(bytes).unwrap(),
                ..base
            },
            BidiStoredError::Bytes,
        ),
        (
            BidiStoredLimits {
                retained_bytes: NonZeroUsize::new(bytes + 1).unwrap(),
                ..base
            },
            BidiStoredError::Isolates(iced_css_core::text::bidi_isolates::BidiIsolateError::Bytes),
        ),
        (
            BidiStoredLimits {
                work: NonZeroUsize::new(2).unwrap(),
                ..base
            },
            BidiStoredError::Isolates(iced_css_core::text::bidi_isolates::BidiIsolateError::Work),
        ),
        (
            BidiStoredLimits {
                active_scalars: one,
                ..base
            },
            BidiStoredError::Runs(iced_css_core::text::bidi_runs::BidiRunError::ActiveScalars),
        ),
        (
            BidiStoredLimits { runs: one, ..base },
            BidiStoredError::Runs(iced_css_core::text::bidi_runs::BidiRunError::Runs),
        ),
        (
            BidiStoredLimits {
                sequences: one,
                ..base
            },
            BidiStoredError::Runs(iced_css_core::text::bidi_runs::BidiRunError::Sequences),
        ),
        (
            BidiStoredLimits { pairs: one, ..base },
            BidiStoredError::BracketPairs(
                iced_css_core::text::bidi_bracket_pairs::BidiBracketPairError::Pairs,
            ),
        ),
    ] {
        let raw = initial(source);
        let work = raw.work();
        let failure = store_bidi_lines(raw, &cuts, limited).unwrap_err();
        assert_eq!(failure.error(), want);
        assert_eq!(failure.initial().source().as_ptr(), source.as_ptr());
        assert_eq!(failure.initial().retained_bytes(), bytes);
        assert_eq!(failure.initial().work(), work);
        assert!(std::error::Error::source(&failure).is_some());
        assert_eq!(failure.to_string(), want.to_string());
        let (raw, error) = failure.into_parts();
        assert_eq!(error, want);
        assert!(store_bidi_lines(raw, &cuts, base).is_ok());
    }
    for mode in [
        BidiParagraphMode::LeftToRight,
        BidiParagraphMode::RightToLeft,
        BidiParagraphMode::AutoLeftToRight,
    ] {
        let n = max();
        let raw = evaluate_bidi_initial(
            "א\na",
            mode,
            BidiInitialLimits {
                input_bytes: n,
                scalars: n,
                paragraphs: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let expected = [
            raw.paragraph(0).unwrap().base(),
            raw.paragraph(1).unwrap().base(),
        ];
        let r = store_bidi_lines(raw, &[2, 3], base).unwrap();
        assert_eq!(r.initial().mode(), mode);
        assert_eq!(
            [r.line(0).unwrap().base(), r.line(1).unwrap().base()],
            expected
        );
    }
}
#[test]
fn exact_work_and_live_checked_iterators() {
    let source = "😀\u{ad}אב\r\na";
    let r = store_bidi_lines(initial(source), &[2, 6, 7], limits()).unwrap();
    let exact = BidiStoredLimits {
        work: NonZeroUsize::new(r.work()).unwrap(),
        retained_bytes: NonZeroUsize::new(r.peak_retained_bytes()).unwrap(),
        ..limits()
    };
    assert!(store_bidi_lines(initial(source), &[2, 6, 7], exact).is_ok());
    let failure = store_bidi_lines(
        initial(source),
        &[2, 6, 7],
        BidiStoredLimits {
            work: NonZeroUsize::new(r.work() - 1).unwrap(),
            ..limits()
        },
    )
    .unwrap_err();
    assert_eq!(failure.error(), BidiStoredError::Work);
    for li in 0..r.line_count() {
        let line = r.line(li).unwrap();
        let range = line.scalar_range();
        let mut it = line.scalars();
        let mut held = Vec::new();
        for (local, index) in range.clone().enumerate() {
            assert_eq!(it.len(), range.end - index);
            assert_eq!(it.size_hint(), (range.end - index, Some(range.end - index)));
            let v = it.next().unwrap();
            assert_eq!(v.index(), index);
            assert_eq!(v.line_index(), li);
            assert_eq!(v.line_scalar_index(), local);
            assert_eq!(line.scalar(local).unwrap().index(), index);
            assert_eq!(
                v.original().class(),
                r.initial().scalar(index).unwrap().class()
            );
            held.push(v);
        }
        assert!(it.next().is_none());
        assert!(it.next().is_none());
        for v in held {
            assert_eq!(v.level(), r.scalar(v.index()).unwrap().level());
        }
        let mut it = line.visual_scalars();
        let mut held = Vec::new();
        for local in 0..line.active_scalar_count() {
            assert_eq!(it.len(), line.active_scalar_count() - local);
            let v = it.next().unwrap();
            assert!(v.level().is_some());
            assert_eq!(line.visual_scalar(local).unwrap().index(), v.index());
            held.push(v);
        }
        assert!(it.next().is_none());
        assert!(it.next().is_none());
        for v in held {
            assert_eq!(v.level(), r.scalar(v.index()).unwrap().level());
        }
        assert!(line.scalar(line.scalar_count()).is_none());
        assert!(line.scalar(usize::MAX).is_none());
        assert!(line.visual_scalar(line.active_scalar_count()).is_none());
        assert!(line.visual_scalar(usize::MAX).is_none());
    }
    assert!(r.scalar(r.scalar_count()).is_none());
    assert!(r.scalar(usize::MAX).is_none());
    assert!(r.line(r.line_count()).is_none());
    assert!(r.line(usize::MAX).is_none());
}
