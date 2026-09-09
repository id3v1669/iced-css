use iced_css_core::syntax::decode::*;
use std::num::NonZeroUsize;
fn limits() -> DecodeLimits {
    DecodeLimits {
        entries: NonZeroUsize::new(1000).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(100000).unwrap(),
    }
}
#[test]
fn nul_replacement_preserves_original_offsets_and_scalar_columns() {
    let source = decode_utf8_source("é\0😀".as_bytes(), limits()).unwrap();
    assert_eq!(source.text(), "é�😀");
    assert_eq!(source.nul_repairs(), std::slice::from_ref(&(2..3)));
    for (output, original, column) in [(0, 0, 1), (2, 2, 2), (5, 3, 3), (9, 7, 4)] {
        assert_eq!(
            source.position(output),
            Some(SourcePosition {
                byte: original,
                line: 1,
                column
            })
        );
    }
    for offset in [1, 3, 4, 6, 7, 8, 10] {
        assert!(source.position(offset).is_none());
    }
}
#[test]
fn crlf_is_one_break_with_explicit_positions_for_each_code_point() {
    let source = decode_utf8_source(b"a\r\nb\rc\nd\x0ce", limits()).unwrap();
    for (byte, line, column) in [
        (1, 1, 2),
        (2, 1, 3),
        (3, 2, 1),
        (5, 3, 1),
        (7, 4, 1),
        (9, 5, 1),
        (10, 5, 2),
    ] {
        assert_eq!(
            source.position(byte),
            Some(SourcePosition { byte, line, column })
        );
    }
}
#[test]
fn invalid_utf8_is_an_error_before_css_replacement_or_escape_processing() {
    for bytes in [
        b"\xc0\x80".as_slice(),
        b"\xed\xa0\x80",
        b"\xf4\x90\x80\x80",
        b"\x80",
        b"\xe2\x82",
    ] {
        let mut input = b"ok ".to_vec();
        input.extend_from_slice(bytes);
        assert!(matches!(
            decode_utf8_source(&input, limits()),
            Err(DecodeError::Utf8 { valid_up_to: 3, .. })
        ));
    }
    let text = "\u{feff}\u{fdd0}\u{ffff}\u{10ffff}\\0";
    let source = decode_utf8_source(text.as_bytes(), limits()).unwrap();
    assert_eq!(source.text(), text);
    assert!(source.nul_repairs().is_empty());
}
#[test]
fn aggregate_capacity_entries_and_work_fail_explicitly() {
    let mut bounded = limits();
    bounded.work = NonZeroUsize::MIN;
    assert!(matches!(
        decode_utf8_source(b"x", bounded),
        Err(DecodeError::Work)
    ));
    bounded = limits();
    bounded.entries = NonZeroUsize::MIN;
    assert!(matches!(
        decode_utf8_source(b"x", bounded),
        Err(DecodeError::Entries)
    ));
    let mut success = false;
    let mut failure = false;
    for bytes in 1..1024 {
        bounded = limits();
        bounded.retained_bytes = NonZeroUsize::new(bytes).unwrap();
        match decode_utf8_source(b"a\0b", bounded) {
            Ok(source) => {
                assert!(source.retained_bytes() <= bytes);
                assert_eq!(source.text(), "a�b");
                success = true;
            }
            Err(DecodeError::Bytes) => failure = true,
            Err(error) => panic!("{error:?}"),
        }
    }
    assert!(success && failure);
    let empty = decode_utf8_source(b"", limits()).unwrap();
    assert_eq!(
        empty.position(0),
        Some(SourcePosition {
            byte: 0,
            line: 1,
            column: 1
        })
    );
}

#[test]
fn css_selector_processing_distinguishes_literal_nul_from_escaped_zero() {
    use iced_css_core::selectors::{SelectorLimits, SimpleSelector, parse_selector_list};
    let selector_limits = SelectorLimits {
        work: NonZeroUsize::new(10000).unwrap(),
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
        entries: NonZeroUsize::new(100).unwrap(),
    };
    let literal = decode_utf8_source(b".a\0", limits()).unwrap();
    let selectors = parse_selector_list(literal.text(), selector_limits).unwrap();
    assert!(
        selectors[0].compounds[0]
            .tests
            .contains(&SimpleSelector::Class("a�".into()))
    );
    assert_eq!(literal.nul_repairs(), std::slice::from_ref(&(2..3)));
    let escaped = decode_utf8_source(br".a\0", limits()).unwrap();
    assert!(escaped.nul_repairs().is_empty());
    assert!(parse_selector_list(escaped.text(), selector_limits).is_err());
}
