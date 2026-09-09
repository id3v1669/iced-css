use iced_css_core::text::{bidi_data::BidiClass, bidi_initial::*};
use std::num::NonZeroUsize;
fn limits() -> BidiInitialLimits {
    let n = NonZeroUsize::new(usize::MAX).unwrap();
    BidiInitialLimits {
        input_bytes: n,
        scalars: n,
        paragraphs: n,
        retained_bytes: n,
        work: n,
    }
}
#[test]
fn raw_paragraphs_and_original_views() {
    let s = "😀\r\nא\u{2028}a\u{2029}\n";
    let r = evaluate_bidi_initial(s, BidiParagraphMode::AutoLeftToRight, limits()).unwrap();
    assert_eq!(r.source(), s);
    assert_eq!(r.paragraph_count(), 3);
    for (i, (text, range, base)) in [
        ("😀\r\n", 0..3, 0),
        ("א\u{2028}a\u{2029}", 3..7, 1),
        ("\n", 7..8, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let p = r.paragraph(i).unwrap();
        assert_eq!(p.text(), text);
        assert_eq!(p.scalar_range(), range);
        assert_eq!(p.base().level(), base);
        assert_eq!(&s[p.source_range()], text);
        assert!(p.scalar(p.scalar_count()).is_none());
        assert!(p.scalar(usize::MAX).is_none());
        for j in 0..p.scalar_count() {
            let c = p.scalar(j).unwrap();
            assert_eq!(&s[c.source_range()], c.text());
        }
    }
    assert_eq!(r.scalar(1).unwrap().class(), BidiClass::B);
    assert_eq!(r.scalar(2).unwrap().class(), BidiClass::B);
    assert_eq!(r.scalar(0).unwrap().source_range(), 0..4);
    assert!(r.scalar(usize::MAX).is_none());
    assert!(r.paragraph(usize::MAX).is_none());
    for s in [
        "\n", "\r", "\u{1c}", "\u{1d}", "\u{1e}", "\u{85}", "\u{2029}",
    ] {
        assert_eq!(
            evaluate_bidi_initial(&s.repeat(2), BidiParagraphMode::AutoLeftToRight, limits())
                .unwrap()
                .paragraph_count(),
            2
        );
    }
    for s in ["a\u{b}א", "a\u{c}א", "a\u{2028}א"] {
        assert_eq!(
            evaluate_bidi_initial(s, BidiParagraphMode::AutoLeftToRight, limits())
                .unwrap()
                .paragraph_count(),
            1
        );
    }
}
#[test]
fn first_strong_isolates_and_modes() {
    for (s, expected) in [
        ("", None),
        ("\u{2069}א", Some(1)),
        ("\u{202a}א\u{202c}a", Some(1)),
        ("\u{2067}א\u{2069}a", Some(0)),
        ("\u{2068}ا", Some(0)),
        ("\u{301}ا", Some(1)),
    ] {
        for mode in [
            BidiParagraphMode::AutoLeftToRight,
            BidiParagraphMode::LeftToRight,
            BidiParagraphMode::RightToLeft,
        ] {
            let r = evaluate_bidi_initial(s, mode, limits()).unwrap();
            assert_eq!(r.mode(), mode);
            assert_eq!(
                r.paragraph(0).map(|p| p.base().level()),
                expected.map(|x| match mode {
                    BidiParagraphMode::AutoLeftToRight => x,
                    BidiParagraphMode::LeftToRight => 0,
                    BidiParagraphMode::RightToLeft => 1,
                })
            );
        }
    }
    let s = format!(
        "{}א{}a\nא",
        "\u{2066}".repeat(1000),
        "\u{2069}".repeat(1000)
    );
    let r = evaluate_bidi_initial(&s, BidiParagraphMode::AutoLeftToRight, limits()).unwrap();
    assert_eq!(r.paragraph(0).unwrap().base().level(), 0);
    assert_eq!(r.paragraph(1).unwrap().base().level(), 1);
    let r = evaluate_bidi_initial("\u{2067}א\ra", BidiParagraphMode::AutoLeftToRight, limits())
        .unwrap();
    assert_eq!(r.paragraph(1).unwrap().base().level(), 0);
}
#[test]
fn exact_resources_and_errors() {
    let mode = BidiParagraphMode::AutoLeftToRight;
    for s in ["", "a", "😀\r\nא", "\n\n"] {
        let r = evaluate_bidi_initial(s, mode, limits()).unwrap();
        let n = s.chars().count();
        assert_eq!(r.work(), 1 + 3 * n);
        assert_eq!(r.scalar_count(), n);
        assert_eq!(r.retained_bytes(), r.peak_retained_bytes());
        let mut l = limits();
        l.work = NonZeroUsize::new(r.work()).unwrap();
        l.retained_bytes = NonZeroUsize::new(r.retained_bytes().max(1)).unwrap();
        assert!(evaluate_bidi_initial(s, mode, l).is_ok());
        if n > 0 {
            l.work = NonZeroUsize::new(r.work() - 1).unwrap();
            assert_eq!(
                evaluate_bidi_initial(s, mode, l).unwrap_err(),
                BidiInitialError::Work
            );
        }
        if r.retained_bytes() > 1 {
            let mut l = limits();
            l.retained_bytes = NonZeroUsize::new(r.retained_bytes() - 1).unwrap();
            assert_eq!(
                evaluate_bidi_initial(s, mode, l).unwrap_err(),
                BidiInitialError::Bytes
            );
        }
    }
    let one = NonZeroUsize::new(1).unwrap();
    let l = BidiInitialLimits {
        input_bytes: one,
        scalars: one,
        paragraphs: one,
        retained_bytes: one,
        work: one,
    };
    let e = evaluate_bidi_initial("", mode, l).unwrap();
    assert_eq!(e.paragraph_count(), 0);
    assert_eq!(e.retained_bytes(), 0);
    for (l, error) in [
        (
            BidiInitialLimits {
                input_bytes: one,
                ..limits()
            },
            BidiInitialError::InputBytes,
        ),
        (
            BidiInitialLimits {
                scalars: one,
                ..limits()
            },
            BidiInitialError::Scalars,
        ),
        (
            BidiInitialLimits {
                paragraphs: one,
                ..limits()
            },
            BidiInitialError::Paragraphs,
        ),
    ] {
        assert_eq!(evaluate_bidi_initial("\n\n", mode, l).unwrap_err(), error);
    }
}
