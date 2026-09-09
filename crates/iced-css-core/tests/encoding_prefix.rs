use iced_css_core::syntax::encoding::*;
use std::num::NonZeroUsize;
fn work() -> NonZeroUsize {
    NonZeroUsize::new(100_000).unwrap()
}

#[test]
fn css_table_prefers_a_complete_declaration_over_a_bom_only_row() {
    let input = b"\xef\xbb\xbf@charset \"UTF-8\";.a{}";
    let result = detect_unicode_prefix(input, work()).unwrap().unwrap();
    assert_eq!(
        result.hint,
        EncodingHint::Declared {
            default_order: None
        }
    );
    assert_eq!(result.name.unwrap().bytes().collect::<Vec<_>>(), b"UTF-8");
    assert_eq!(result.consumed, 20);
    let result = detect_unicode_prefix(b"\xef\xbb\xbf@charset \"unterminated", work())
        .unwrap()
        .unwrap();
    assert_eq!(result.hint, EncodingHint::Utf8);
    assert_eq!(result.consumed, 3);
}

#[test]
fn literal_detection_prefix_and_shortest_terminating_suffix_are_preserved() {
    for input in [
        b" @charset \"UTF-8\";".as_slice(),
        b"@CHARSET \"UTF-8\";",
        b"@charset 'UTF-8';",
        b"@char\\set \"UTF-8\";",
        b"@charset  \"UTF-8\";",
    ] {
        assert!(detect_unicode_prefix(input, work()).unwrap().is_none());
    }
    let input = b"@charset \"a\";b\";";
    let result = detect_unicode_prefix(input, work()).unwrap().unwrap();
    assert_eq!(result.name.unwrap().bytes().collect::<Vec<_>>(), b"a");
    assert_eq!(&input[result.consumed..], b"b\";");
}

#[test]
fn all_unicode_bom_only_rows_distinguish_overlapping_orders() {
    for (input, hint) in [
        (b"\xef\xbb\xbf".as_slice(), EncodingHint::Utf8),
        (b"\xfe\xff", EncodingHint::Utf16(ByteOrder::Big)),
        (b"\xff\xfe", EncodingHint::Utf16(ByteOrder::Little)),
        (b"\0\0\xfe\xff", EncodingHint::Utf32(ByteOrder::Big)),
        (b"\xff\xfe\0\0", EncodingHint::Utf32(ByteOrder::Little)),
        (b"\0\0\xff\xfe", EncodingHint::Utf32(ByteOrder::Order2143)),
        (b"\xfe\xff\0\0", EncodingHint::Utf32(ByteOrder::Order3412)),
    ] {
        let result = detect_unicode_prefix(input, work()).unwrap().unwrap();
        assert_eq!(result.hint, hint);
        assert_eq!(result.consumed, input.len());
        assert!(result.name.is_none());
    }
}

#[test]
fn utf16_and_utf32_name_bytes_keep_the_declared_default_order() {
    for (width, ascii_index, order) in [
        (2, 0, ByteOrder::Little),
        (2, 1, ByteOrder::Big),
        (4, 0, ByteOrder::Little),
        (4, 3, ByteOrder::Big),
        (4, 2, ByteOrder::Order2143),
        (4, 1, ByteOrder::Order3412),
    ] {
        let mut input = Vec::new();
        for byte in b"@charset \"UTF-16\";" {
            let mut unit = vec![0; width];
            unit[ascii_index] = *byte;
            input.extend(unit);
        }
        let result = detect_unicode_prefix(&input, work()).unwrap().unwrap();
        assert_eq!(
            result.hint,
            EncodingHint::Declared {
                default_order: Some(order)
            }
        );
        assert_eq!(result.name.unwrap().bytes().collect::<Vec<_>>(), b"UTF-16");
        assert_eq!(result.consumed, input.len());
    }
}

#[test]
fn prefix_work_is_bounded_without_lossy_decoding_or_label_validation() {
    assert!(detect_unicode_prefix(b"@charset \"UTF-8\";", NonZeroUsize::MIN).is_err());
    let result = detect_unicode_prefix(b"@charset \"\xff\n\";", work())
        .unwrap()
        .unwrap();
    assert_eq!(result.name.unwrap().bytes().collect::<Vec<_>>(), b"\xff\n");
    // Matching XX bytes does not certify ASCII names, valid IANA labels, or the
    // appropriate post-decoding @charset required by the following CSS clause.
}
