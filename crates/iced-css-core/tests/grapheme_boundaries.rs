use iced_css_core::text::grapheme_boundaries::*;
use std::{mem::size_of, num::NonZeroUsize};
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> GraphemeBoundaryLimits {
    GraphemeBoundaryLimits {
        input_bytes: nz(usize::MAX),
        scalars: nz(usize::MAX),
        retained_bytes: nz(usize::MAX),
        work: nz(usize::MAX),
    }
}
#[test]
fn ordered_literal_rules_and_raw_crlf() {
    for (s, expected) in [
        ("", vec![0]),
        ("a\r\nb", vec![0, 1, 3, 4]),
        ("\r\u{308}", vec![0, 1, 3]),
        ("\u{600}\r\u{308}", vec![0, 2, 3, 5]),
        ("각", vec![0, 9]),
        ("각", vec![0, 6]),
        ("각ᆨ", vec![0, 6]),
        ("\u{16d63}\u{16d63}", vec![0, 8]),
        ("a\u{903}", vec![0, 4]),
        ("\u{600}a", vec![0, 3]),
        ("🇦\u{308}🇧", vec![0, 6, 10]),
        ("🇦\u{200d}🇧", vec![0, 7, 11]),
        ("😀\u{200d}\u{308}😀", vec![0, 9, 13]),
        ("😀\u{200d}\u{200d}😀", vec![0, 10, 14]),
        ("\u{94d}\u{915}", vec![0, 3, 6]),
        ("\u{915}\u{308}\u{915}", vec![0, 5, 8]),
        ("\u{915}\u{94d}\u{200c}\u{915}", vec![0, 9, 12]),
        ("\u{915}\u{94d}\u{200d}\u{915}", vec![0, 12]),
        ("\u{915}\u{94d}\u{915}\u{915}", vec![0, 9, 12]),
    ] {
        assert_eq!(
            evaluate_grapheme_boundaries(s, limits()).unwrap().offsets(),
            expected,
            "{s:?}"
        );
    }
}
#[test]
fn byte_queries_borrowing_and_exact_limits() {
    let source = String::from("é\u{308}😀");
    let r = evaluate_grapheme_boundaries(&source, limits()).unwrap();
    assert_eq!(r.source().as_ptr(), source.as_ptr());
    assert_eq!(r.source(), source);
    assert_eq!(r.offsets(), &[0, 4, 8]);
    assert_eq!(r.scalar_count(), 3);
    assert_eq!(r.work(), 7);
    assert_eq!(r.peak_retained_bytes(), r.retained_bytes());
    assert!(r.retained_bytes() >= 4 * size_of::<usize>());
    for b in 0..=source.len() {
        assert_eq!(
            r.is_boundary(b),
            source.is_char_boundary(b).then(|| [0, 4, 8].contains(&b))
        );
    }
    assert_eq!(r.is_boundary(usize::MAX), None);
    let mut exact = limits();
    exact.input_bytes = nz(8);
    exact.scalars = nz(3);
    exact.work = nz(7);
    exact.retained_bytes = nz(r.retained_bytes());
    assert!(evaluate_grapheme_boundaries(&source, exact).is_ok());
    for (field, error) in [
        (0, GraphemeBoundaryError::InputBytes),
        (1, GraphemeBoundaryError::Scalars),
        (2, GraphemeBoundaryError::Work),
        (3, GraphemeBoundaryError::Bytes),
    ] {
        let mut l = exact;
        match field {
            0 => l.input_bytes = nz(7),
            1 => l.scalars = nz(2),
            2 => l.work = nz(6),
            _ => l.retained_bytes = nz(r.retained_bytes() - 1),
        }
        assert_eq!(evaluate_grapheme_boundaries(&source, l).unwrap_err(), error);
    }
    let mut empty = limits();
    empty.work = nz(1);
    let r = evaluate_grapheme_boundaries("", empty).unwrap();
    assert_eq!(r.work(), 1);
    assert_eq!(r.scalar_count(), 0);
    assert_eq!(r.is_boundary(0), Some(true));
    assert_eq!(r.retained_bytes(), r.peak_retained_bytes());
    assert!(r.retained_bytes() >= size_of::<usize>());
    empty.retained_bytes = nz(size_of::<usize>() - 1);
    assert_eq!(
        evaluate_grapheme_boundaries("", empty).unwrap_err(),
        GraphemeBoundaryError::Bytes
    );
}
#[test]
fn long_original_suffixes_and_scalar_metering() {
    for s in [
        format!("😀{}\u{200d}😀", "\u{308}".repeat(512)),
        format!(
            "\u{915}{}\u{94d}{}\u{915}",
            "\u{308}".repeat(256),
            "\u{200d}".repeat(256)
        ),
    ] {
        let r = evaluate_grapheme_boundaries(&s, limits()).unwrap();
        assert_eq!(r.offsets(), &[0, s.len()]);
        assert_eq!(r.work(), 1 + 2 * s.chars().count());
    }
    let s = "🇦".repeat(513);
    let r = evaluate_grapheme_boundaries(&s, limits()).unwrap();
    let expected: Vec<_> = (0..s.len())
        .step_by(8)
        .chain(std::iter::once(s.len()))
        .collect();
    assert_eq!(r.offsets(), expected);
    let mut l = limits();
    l.work = nz(1);
    assert_eq!(
        evaluate_grapheme_boundaries("😀", l).unwrap_err(),
        GraphemeBoundaryError::Work
    );
    l.work = nz(3);
    assert!(evaluate_grapheme_boundaries("😀", l).is_ok());
    l.scalars = nz(1);
    assert_eq!(
        evaluate_grapheme_boundaries("ab", l).unwrap_err(),
        GraphemeBoundaryError::Scalars
    );
    let e = GraphemeBoundaryError::Work;
    assert!(e.to_string().contains("Work"));
    fn error_trait(_: &dyn std::error::Error) {}
    error_trait(&e);
}
