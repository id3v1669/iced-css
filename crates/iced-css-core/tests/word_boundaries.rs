use iced_css_core::text::word_boundaries::*;
use std::{mem::size_of, num::NonZeroUsize};
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> WordBoundaryLimits {
    WordBoundaryLimits {
        input_bytes: nz(usize::MAX),
        scalars: nz(usize::MAX),
        retained_bytes: nz(usize::MAX),
        work: nz(usize::MAX),
    }
}
#[test]
fn literal_priority_and_effective_context() {
    for (source, expected) in [
        ("", vec![0]),
        ("a\r\nb", vec![0, 1, 3, 4]),
        ("a'b", vec![0, 3]),
        ("12,34", vec![0, 5]),
        (" \u{301} ", vec![0, 3, 4]),
        ("\u{200d}\u{301}©", vec![0, 5, 7]),
        ("\u{301}A", vec![0, 2, 3]),
        ("\n\u{301}A", vec![0, 1, 3, 4]),
        ("a🇦\u{200d}🇧🇨b", vec![0, 1, 12, 16, 17]),
    ] {
        let result = evaluate_word_boundaries(source, limits()).unwrap();
        assert_eq!(result.offsets(), expected, "{source:?}");
    }
    for c in [
        '\u{2139}',
        '\u{24c2}',
        '\u{1f170}',
        '\u{1f171}',
        '\u{1f17e}',
        '\u{1f17f}',
    ] {
        let s = format!("\u{200d}{c}");
        assert_eq!(
            evaluate_word_boundaries(&s, limits()).unwrap().offsets(),
            &[0, s.len()]
        );
    }
}
#[test]
fn owner_byte_queries_and_accounting() {
    let source = String::from("é𐐀abc");
    let result = evaluate_word_boundaries(&source, limits()).unwrap();
    assert_eq!(result.source().as_ptr(), source.as_ptr());
    assert_eq!(result.source(), source);
    assert_eq!(result.scalar_count(), 5);
    assert_eq!(result.work(), 21);
    assert_eq!(result.offsets(), &[0, source.len()]);
    for i in 0..=source.len() {
        assert_eq!(
            result.is_boundary(i),
            if source.is_char_boundary(i) {
                Some(i == 0 || i == source.len())
            } else {
                None
            }
        );
    }
    assert_eq!(result.is_boundary(usize::MAX), None);
    assert!(result.retained_bytes() >= 6 * size_of::<usize>());
    assert!(result.peak_retained_bytes() > result.retained_bytes());
    let mut exact = limits();
    exact.input_bytes = nz(source.len());
    exact.scalars = nz(5);
    exact.work = nz(21);
    exact.retained_bytes = nz(result.peak_retained_bytes());
    assert!(evaluate_word_boundaries(&source, exact).is_ok());
    for (field, error) in [
        (0, WordBoundaryError::InputBytes),
        (1, WordBoundaryError::Scalars),
        (2, WordBoundaryError::Work),
        (3, WordBoundaryError::Bytes),
    ] {
        let mut l = exact;
        match field {
            0 => l.input_bytes = nz(source.len() - 1),
            1 => l.scalars = nz(4),
            2 => l.work = nz(20),
            _ => l.retained_bytes = nz(result.peak_retained_bytes() - 1),
        }
        assert_eq!(evaluate_word_boundaries(&source, l).unwrap_err(), error);
    }
    let mut empty = limits();
    empty.work = nz(1);
    let r = evaluate_word_boundaries("", empty).unwrap();
    assert_eq!(r.work(), 1);
    assert_eq!(r.scalar_count(), 0);
    assert_eq!(r.is_boundary(0), Some(true));
    assert_eq!(r.retained_bytes(), r.peak_retained_bytes());
    assert!(r.retained_bytes() >= size_of::<usize>());
    empty.retained_bytes = nz(size_of::<usize>() - 1);
    assert_eq!(
        evaluate_word_boundaries("", empty).unwrap_err(),
        WordBoundaryError::Bytes
    );
}

#[test]
fn long_ignored_context_and_ri_parity() {
    let source = format!("A{}'{}B", "\u{308}".repeat(256), "\u{308}".repeat(256));
    let r = evaluate_word_boundaries(&source, limits()).unwrap();
    assert_eq!(r.offsets(), &[0, source.len()]);
    assert_eq!(r.work(), 1 + 4 * 515);
    let source = "🇦\u{200d}".repeat(257);
    let r = evaluate_word_boundaries(&source, limits()).unwrap();
    let expected: Vec<_> = std::iter::once(0)
        .chain((14..source.len()).step_by(14))
        .chain(std::iter::once(source.len()))
        .collect();
    assert_eq!(r.offsets(), expected);
    for s in ["אב", "א'", "א\"ב", "カタ", "a_カ", "a1", "1a", "1.2", "a:b"] {
        assert_eq!(
            evaluate_word_boundaries(s, limits()).unwrap().offsets(),
            &[0, s.len()]
        );
    }
}

#[test]
fn counting_is_admitted_by_scalars_before_decode() {
    let mut l = limits();
    l.work = nz(1);
    assert_eq!(
        evaluate_word_boundaries("𐐀", l).unwrap_err(),
        WordBoundaryError::Work
    );
    l.work = nz(5);
    assert!(evaluate_word_boundaries("𐐀", l).is_ok());
    l.scalars = nz(1);
    assert_eq!(
        evaluate_word_boundaries("ab", l).unwrap_err(),
        WordBoundaryError::Scalars
    );
    let e = WordBoundaryError::Work;
    assert!(e.to_string().contains("Work"));
    fn error_trait(_: &dyn std::error::Error) {}
    error_trait(&e);
}
