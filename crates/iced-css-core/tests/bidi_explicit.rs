use iced_css_core::text::{
    bidi_data::BidiClass, bidi_explicit::*, bidi_initial::*, bidi_isolates::*,
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
fn limits() -> BidiExplicitLimits {
    let n = max();
    BidiExplicitLimits {
        scalars: n,
        paragraphs: n,
        retained_bytes: n,
        work: n,
    }
}
fn observed(state: BidiExplicitState) -> Option<(u8, BidiClass)> {
    match state {
        BidiExplicitState::RemovedByX9 => None,
        BidiExplicitState::Retained {
            level,
            current_type,
        } => Some((level.number(), current_type)),
    }
}
#[test]
fn literal_transitions_and_original_owner() {
    use BidiClass::*;
    for (s, mode, expected) in [
        (
            "\u{2068}\u{2069}",
            BidiParagraphMode::RightToLeft,
            vec![Some((1, Lri)), Some((1, Pdi))],
        ),
        (
            "\u{202e}\u{2068}a\u{2069}\u{202c}",
            BidiParagraphMode::LeftToRight,
            vec![None, Some((1, R)), Some((2, L)), Some((1, R)), None],
        ),
        (
            "\u{202a}a\r\nא",
            BidiParagraphMode::AutoLeftToRight,
            vec![None, Some((2, L)), Some((0, B)), Some((0, B)), Some((1, R))],
        ),
        (
            "\u{ad}\u{202b}\u{202a}\u{202d}\u{202e}\u{202c}",
            BidiParagraphMode::LeftToRight,
            vec![None; 6],
        ),
        (
            "\u{202c}\u{2069}😀",
            BidiParagraphMode::LeftToRight,
            vec![None, Some((0, Pdi)), Some((0, On))],
        ),
    ] {
        let i = initial(s, mode);
        let iso = isolates(&i);
        let r = resolve_bidi_explicit(&iso, limits()).unwrap();
        assert!(std::ptr::eq(r.isolates(), &iso));
        assert_eq!(r.scalar_count(), expected.len());
        for (j, e) in expected.into_iter().enumerate() {
            let v = r.scalar(j).unwrap();
            let copy = v;
            assert_eq!(copy.index(), j);
            assert_eq!(observed(v.state()), e);
            let original = v.original();
            assert_eq!(original.class(), i.scalar(j).unwrap().class());
            assert_eq!(original.source_range(), i.scalar(j).unwrap().source_range());
            assert_eq!(original.text(), i.scalar(j).unwrap().text());
            assert_eq!(
                v.isolate().matching_scalar_index(),
                iso.scalar(j).unwrap().matching_scalar_index()
            );
        }
        assert!(r.scalar(r.scalar_count()).is_none());
        assert!(r.scalar(usize::MAX).is_none());
    }
}
#[test]
fn overflow_and_constant_time_unwind() {
    use BidiClass::*;
    for depth in [124, 125, 126, 128, 1000] {
        let prefix: String = (1..=depth)
            .map(|x| if x % 2 == 1 { '\u{202b}' } else { '\u{202a}' })
            .collect();
        let s = format!("{prefix}a{}b", "\u{202c}".repeat(depth));
        let i = initial(&s, BidiParagraphMode::LeftToRight);
        let iso = isolates(&i);
        let r = resolve_bidi_explicit(&iso, limits()).unwrap();
        assert_eq!(
            observed(r.scalar(depth).unwrap().state()),
            Some((depth.min(125) as u8, L))
        );
        assert_eq!(
            observed(r.scalar(2 * depth + 1).unwrap().state()),
            Some((0, L))
        );
    }
    let s = format!("\u{2067}{}\u{2069}a", "\u{202a}".repeat(130));
    let i = initial(&s, BidiParagraphMode::LeftToRight);
    let iso = isolates(&i);
    let r = resolve_bidi_explicit(&iso, limits()).unwrap();
    assert_eq!(observed(r.scalar(131).unwrap().state()), Some((0, Pdi)));
    assert_eq!(observed(r.scalar(132).unwrap().state()), Some((0, L)));
}
#[test]
fn exact_tracked_resources_and_error_order() {
    let one = NonZeroUsize::new(1).unwrap();
    for s in ["", "a", "\u{2068}א\u{2069}", "\r\n\n"] {
        let i = initial(s, BidiParagraphMode::AutoLeftToRight);
        let iso = isolates(&i);
        let r = resolve_bidi_explicit(&iso, limits()).unwrap();
        let n = i.scalar_count();
        assert_eq!(
            r.work(),
            if n == 0 {
                1
            } else {
                1 + 127 + i.paragraph_count() + n
            }
        );
        assert_eq!(
            r.retained_bytes() + r.status_stack_bytes(),
            r.peak_retained_bytes()
        );
        assert_eq!(r.status_stack_bytes() == 0, n == 0);
        let mut l = limits();
        l.work = NonZeroUsize::new(r.work()).unwrap();
        l.retained_bytes = NonZeroUsize::new(r.peak_retained_bytes().max(1)).unwrap();
        assert!(resolve_bidi_explicit(&iso, l).is_ok());
        if n > 0 {
            l.work = NonZeroUsize::new(r.work() - 1).unwrap();
            assert_eq!(
                resolve_bidi_explicit(&iso, l).unwrap_err(),
                BidiExplicitError::Work
            );
            let mut l = limits();
            l.retained_bytes = NonZeroUsize::new(r.peak_retained_bytes() - 1).unwrap();
            assert_eq!(
                resolve_bidi_explicit(&iso, l).unwrap_err(),
                BidiExplicitError::Bytes
            );
        }
    }
    let l = BidiExplicitLimits {
        scalars: one,
        paragraphs: one,
        work: one,
        retained_bytes: one,
    };
    let i = initial("", BidiParagraphMode::RightToLeft);
    let iso = isolates(&i);
    let r = resolve_bidi_explicit(&iso, l).unwrap();
    assert_eq!(r.retained_bytes(), 0);
    let i = initial("\n\n", BidiParagraphMode::LeftToRight);
    let iso = isolates(&i);
    assert_eq!(
        resolve_bidi_explicit(&iso, l).unwrap_err(),
        BidiExplicitError::Scalars
    );
    assert_eq!(
        resolve_bidi_explicit(
            &iso,
            BidiExplicitLimits {
                scalars: max(),
                ..l
            }
        )
        .unwrap_err(),
        BidiExplicitError::Paragraphs
    );
    assert_eq!(
        resolve_bidi_explicit(
            &iso,
            BidiExplicitLimits {
                scalars: max(),
                paragraphs: max(),
                ..l
            }
        )
        .unwrap_err(),
        BidiExplicitError::Work
    );
}
