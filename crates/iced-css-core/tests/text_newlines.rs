use iced_css_core::text::newlines::{
    BoundaryAffinity, NewlineError, NewlineLimits, normalize_default_newlines,
};
use std::num::NonZeroUsize;
fn limits() -> NewlineLimits {
    let nz = |v| NonZeroUsize::new(v).unwrap();
    NewlineLimits {
        input_bytes: nz(10000),
        retained_bytes: nz(100000),
        work: nz(20000),
    }
}
#[test]
fn css_default_normalizes_cr_crlf_and_preserves_other_scalars() {
    let input = "a\r\nb\rc\n\r\r\n\t\u{000c}\u{0085}\u{2028}\u{2029}中🦀";
    let normalized = normalize_default_newlines(input, limits()).unwrap();
    assert_eq!(normalized.source(), input);
    assert_eq!(
        normalized.text(),
        "a\nb\nc\n\n\n\t\u{000c}\u{0085}\u{2028}\u{2029}中🦀"
    );
    for span in normalized.scalars() {
        let original = &input[span.source.clone()];
        let expected = if original == "\r" || original == "\r\n" {
            "\n"
        } else {
            original
        };
        assert_eq!(&normalized.text()[span.normalized.clone()], expected);
    }
}
#[test]
fn crlf_interior_boundary_requires_explicit_affinity() {
    let text = normalize_default_newlines("é\r\n中", limits()).unwrap();
    assert_eq!(text.to_normalized(3, BoundaryAffinity::Before), Some(2));
    assert_eq!(text.to_normalized(3, BoundaryAffinity::After), Some(3));
    for (original, normalized) in [(0, 0), (2, 2), (4, 3), (7, 6)] {
        for affinity in [BoundaryAffinity::Before, BoundaryAffinity::After] {
            assert_eq!(text.to_normalized(original, affinity), Some(normalized));
        }
        assert_eq!(text.to_source(normalized), Some(original));
    }
    for invalid in [1, 5, 6, 8, usize::MAX] {
        assert_eq!(text.to_normalized(invalid, BoundaryAffinity::Before), None);
    }
    for invalid in [1, 4, 5, 7, usize::MAX] {
        assert_eq!(text.to_source(invalid), None);
    }
}
#[test]
fn unchanged_unicode_boundaries_round_trip_and_maps_partition_both_strings() {
    let source = "aé中🦀\u{301}\r\nx\r";
    let text = normalize_default_newlines(source, limits()).unwrap();
    let (mut original, mut normalized) = (0, 0);
    for span in text.scalars() {
        assert_eq!(
            (span.source.start, span.normalized.start),
            (original, normalized)
        );
        (original, normalized) = (span.source.end, span.normalized.end);
        assert_eq!(text.to_source(normalized), Some(original));
        assert_eq!(
            text.to_normalized(original, BoundaryAffinity::After),
            Some(normalized)
        );
    }
    assert_eq!((original, normalized), (source.len(), text.text().len()));
}
#[test]
fn limits_and_empty_input_have_defined_results() {
    let empty = normalize_default_newlines("", limits()).unwrap();
    assert_eq!(empty.text(), "");
    assert!(empty.scalars().is_empty());
    assert_eq!(empty.to_source(0), Some(0));
    assert_eq!(empty.to_normalized(0, BoundaryAffinity::Before), Some(0));
    assert_eq!(empty.to_source(1), None);
    let mut cap = limits();
    cap.input_bytes = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        normalize_default_newlines("é", cap),
        Err(NewlineError::InputBytes)
    ));
    cap = limits();
    cap.retained_bytes = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        normalize_default_newlines("a", cap),
        Err(NewlineError::Bytes)
    ));
    cap = limits();
    cap.work = NonZeroUsize::new(3).unwrap();
    assert!(matches!(
        normalize_default_newlines("\r\n", cap),
        Err(NewlineError::Work)
    ));
    cap.work = NonZeroUsize::new(4).unwrap();
    assert_eq!(
        normalize_default_newlines("\r\n", cap).unwrap().text(),
        "\n"
    );
}
