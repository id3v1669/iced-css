use iced_css_core::text::{bidi_initial::*, bidi_isolates::*};
use std::num::NonZeroUsize;
fn limits() -> BidiIsolateLimits {
    let n = NonZeroUsize::new(usize::MAX).unwrap();
    BidiIsolateLimits {
        scalars: n,
        paragraphs: n,
        retained_bytes: n,
        work: n,
    }
}
fn initial(s: &str, mode: BidiParagraphMode) -> BidiInitial<'_> {
    let n = NonZeroUsize::new(usize::MAX).unwrap();
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
#[test]
fn matching_and_fsi_literals() {
    for (s, expected) in [
        (
            "\u{2066}a\u{2069}",
            vec![(Some(2), None), (None, None), (Some(0), None)],
        ),
        (
            "\u{2067}א\u{2069}",
            vec![(Some(2), None), (None, None), (Some(0), None)],
        ),
        (
            "\u{2068}אa\u{2069}",
            vec![
                (Some(3), Some(1)),
                (None, None),
                (None, None),
                (Some(0), None),
            ],
        ),
        (
            "\u{2068}\u{2069}",
            vec![(Some(1), Some(0)), (Some(0), None)],
        ),
        ("\u{2069}\u{2068}", vec![(None, None), (None, Some(0))]),
        (
            "\u{2068}\u{2067}א",
            vec![(None, Some(0)), (None, None), (None, None)],
        ),
    ] {
        for mode in [
            BidiParagraphMode::LeftToRight,
            BidiParagraphMode::RightToLeft,
            BidiParagraphMode::AutoLeftToRight,
        ] {
            let initial = initial(s, mode);
            let r = prepare_bidi_isolates(&initial, limits()).unwrap();
            assert!(std::ptr::eq(r.initial(), &initial));
            assert_eq!(r.scalar_count(), expected.len());
            for (i, (m, d)) in expected.iter().enumerate() {
                let v = r.scalar(i).unwrap();
                let copy = v;
                assert_eq!(copy.index(), i);
                assert_eq!(v.matching_scalar_index(), *m);
                assert_eq!(v.fsi_direction().map(|b| b.level()), *d);
                let o = v.original();
                assert_eq!(o.text(), initial.scalar(i).unwrap().text());
                assert_eq!(o.source_range(), initial.scalar(i).unwrap().source_range());
            }
            assert!(r.scalar(r.scalar_count()).is_none());
            assert!(r.scalar(usize::MAX).is_none());
        }
    }
    for (s, d) in [
        ("\u{2068}\u{202e}א\u{202c}\u{2069}", 1),
        ("\u{2068}\u{2067}א\u{2069}a\u{2069}", 0),
        ("\u{2068}aא\u{2069}", 0),
        ("\u{2068}ا\u{2069}", 1),
    ] {
        let i = initial(s, BidiParagraphMode::RightToLeft);
        let r = prepare_bidi_isolates(&i, limits()).unwrap();
        assert_eq!(r.scalar(0).unwrap().fsi_direction().unwrap().level(), d);
    }
}
#[test]
fn paragraphs_original_ranges_and_deep_matching() {
    let i = initial(
        "😀\u{2068}א\r\n\u{2069}\u{2068}a\u{2069}\n\n",
        BidiParagraphMode::RightToLeft,
    );
    let r = prepare_bidi_isolates(&i, limits()).unwrap();
    assert_eq!(r.paragraph_count(), 3);
    assert_eq!(r.scalar(1).unwrap().matching_scalar_index(), None);
    assert_eq!(r.scalar(5).unwrap().matching_scalar_index(), None);
    assert_eq!(r.scalar(6).unwrap().matching_scalar_index(), Some(8));
    assert_eq!(r.scalar(8).unwrap().matching_scalar_index(), Some(6));
    assert_eq!(r.scalar(0).unwrap().original().source_range(), 0..4);
    for j in 0..r.scalar_count() {
        let v = r.scalar(j).unwrap();
        let o = v.original();
        assert_eq!(o.class(), i.scalar(j).unwrap().class());
        assert_eq!(o.text(), i.scalar(j).unwrap().text());
    }
    let s = format!("{}א{}", "\u{2068}".repeat(1000), "\u{2069}".repeat(1000));
    let i = initial(&s, BidiParagraphMode::RightToLeft);
    let r = prepare_bidi_isolates(&i, limits()).unwrap();
    for j in 0..1000 {
        assert_eq!(r.scalar(j).unwrap().matching_scalar_index(), Some(2000 - j));
        assert_eq!(r.scalar(2000 - j).unwrap().matching_scalar_index(), Some(j));
        assert_eq!(
            r.scalar(j).unwrap().fsi_direction().unwrap().level(),
            u8::from(j == 999)
        );
    }
}
#[test]
fn resource_limits_and_precedence() {
    for s in ["", "a", "\u{2068}א\u{2069}", "\n\n"] {
        let i = initial(s, BidiParagraphMode::AutoLeftToRight);
        let r = prepare_bidi_isolates(&i, limits()).unwrap();
        assert_eq!(r.work(), 1 + i.paragraph_count() + i.scalar_count());
        let mut l = limits();
        l.work = NonZeroUsize::new(r.work()).unwrap();
        l.retained_bytes = NonZeroUsize::new(r.peak_retained_bytes().max(1)).unwrap();
        assert!(prepare_bidi_isolates(&i, l).is_ok());
        if r.work() > 1 {
            l.work = NonZeroUsize::new(r.work() - 1).unwrap();
            assert_eq!(
                prepare_bidi_isolates(&i, l).unwrap_err(),
                BidiIsolateError::Work
            );
        }
        if r.peak_retained_bytes() > 1 {
            let mut l = limits();
            l.retained_bytes = NonZeroUsize::new(r.peak_retained_bytes() - 1).unwrap();
            assert_eq!(
                prepare_bidi_isolates(&i, l).unwrap_err(),
                BidiIsolateError::Bytes
            );
        }
    }
    let one = NonZeroUsize::new(1).unwrap();
    let l = BidiIsolateLimits {
        scalars: one,
        paragraphs: one,
        retained_bytes: one,
        work: one,
    };
    let empty = initial("", BidiParagraphMode::RightToLeft);
    let r = prepare_bidi_isolates(&empty, l).unwrap();
    assert_eq!(r.retained_bytes(), 0);
    assert_eq!(r.peak_retained_bytes(), 0);
    assert_eq!(r.work(), 1);
    let i = initial("\n\n", BidiParagraphMode::AutoLeftToRight);
    assert_eq!(
        prepare_bidi_isolates(&i, l).unwrap_err(),
        BidiIsolateError::Scalars
    );
    assert_eq!(
        prepare_bidi_isolates(
            &i,
            BidiIsolateLimits {
                scalars: limits().scalars,
                ..l
            }
        )
        .unwrap_err(),
        BidiIsolateError::Paragraphs
    );
    assert_eq!(
        prepare_bidi_isolates(
            &i,
            BidiIsolateLimits {
                scalars: limits().scalars,
                paragraphs: limits().paragraphs,
                ..l
            }
        )
        .unwrap_err(),
        BidiIsolateError::Work
    );
}
