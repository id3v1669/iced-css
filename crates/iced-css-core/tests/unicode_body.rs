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
fn retained_unicode_examples_decode_in_both_explicit_byte_orders() {
    for (encoding, input, source_end) in [
        (
            UnicodeBodyEncoding::Utf16Be,
            b"\0M\x04\x30\x4e\x8c\xd8\0\xdf\x02".as_slice(),
            10,
        ),
        (
            UnicodeBodyEncoding::Utf16Le,
            b"M\0\x30\x04\x8c\x4e\0\xd8\x02\xdf",
            10,
        ),
        (
            UnicodeBodyEncoding::Utf32Be,
            b"\0\0\0M\0\0\x04\x30\0\0\x4e\x8c\0\x01\x03\x02",
            16,
        ),
        (
            UnicodeBodyEncoding::Utf32Le,
            b"M\0\0\0\x30\x04\0\0\x8c\x4e\0\0\x02\x03\x01\0",
            16,
        ),
    ] {
        let decoded = decode_unicode_source(input, encoding, limits()).unwrap();
        assert_eq!(decoded.text(), "Mа二𐌂");
        assert_eq!(
            decoded.position(10),
            Some(SourcePosition {
                byte: source_end,
                line: 1,
                column: 5
            })
        );
    }
}
#[test]
fn nul_repairs_and_supplementary_columns_use_original_unit_ranges() {
    let utf16 = [0x0041u16, 0, 0xd83d, 0xde00, 13, 10, 0x0042]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    let utf32 = [0x0041u32, 0, 0x1f600, 13, 10, 0x0042]
        .into_iter()
        .flat_map(u32::to_be_bytes)
        .collect::<Vec<_>>();
    for (input, encoding, repair, emoji, b_position) in [
        (&utf16, UnicodeBodyEncoding::Utf16Be, 2..4, 4, 12),
        (&utf32, UnicodeBodyEncoding::Utf32Be, 4..8, 8, 20),
    ] {
        let decoded = decode_unicode_source(input, encoding, limits()).unwrap();
        assert_eq!(decoded.text(), "A�😀\r\nB");
        assert_eq!(decoded.nul_repairs(), &[repair]);
        assert_eq!(
            decoded.position(4),
            Some(SourcePosition {
                byte: emoji,
                line: 1,
                column: 3
            })
        );
        assert_eq!(
            decoded.position(10),
            Some(SourcePosition {
                byte: b_position,
                line: 2,
                column: 1
            })
        );
    }
}
#[test]
fn malformed_units_fail_at_original_offsets_instead_of_becoming_characters() {
    for (input, kind, offset) in [
        (b"\0".as_slice(), UnicodeFault::TruncatedUnit, 0),
        (b"\xd8\0", UnicodeFault::UnpairedHighSurrogate, 0),
        (b"\xdc\0", UnicodeFault::UnpairedLowSurrogate, 0),
        (b"\xd8\0\0A", UnicodeFault::UnpairedHighSurrogate, 0),
        (b"\0A\xdc\0", UnicodeFault::UnpairedLowSurrogate, 2),
    ] {
        assert!(
            matches!(decode_unicode_source(input, UnicodeBodyEncoding::Utf16Be, limits()), Err(DecodeError::Unicode { offset: actual_offset, fault, .. }) if actual_offset == offset && fault == kind)
        );
    }
    for (input, fault) in [
        (b"\0\0".as_slice(), UnicodeFault::TruncatedUnit),
        (b"\0\0\xd8\0", UnicodeFault::InvalidScalar(0xd800)),
        (b"\0\x11\0\0", UnicodeFault::InvalidScalar(0x110000)),
    ] {
        assert!(
            matches!(decode_unicode_source(input, UnicodeBodyEncoding::Utf32Be, limits()), Err(DecodeError::Unicode { offset: 0, fault: actual, .. }) if actual == fault)
        );
    }
}
#[test]
fn explicit_body_decoding_preserves_feff_noncharacters_and_resource_limits() {
    for encoding in [
        UnicodeBodyEncoding::Utf16Be,
        UnicodeBodyEncoding::Utf16Le,
        UnicodeBodyEncoding::Utf32Be,
        UnicodeBodyEncoding::Utf32Le,
    ] {
        let text = "\u{feff}\u{fdd0}\u{ffff}\u{10ffff}";
        let input: Vec<u8> = match encoding {
            UnicodeBodyEncoding::Utf16Be => {
                text.encode_utf16().flat_map(u16::to_be_bytes).collect()
            }
            UnicodeBodyEncoding::Utf16Le => {
                text.encode_utf16().flat_map(u16::to_le_bytes).collect()
            }
            UnicodeBodyEncoding::Utf32Be => text
                .chars()
                .flat_map(|value| u32::from(value).to_be_bytes())
                .collect(),
            UnicodeBodyEncoding::Utf32Le => text
                .chars()
                .flat_map(|value| u32::from(value).to_le_bytes())
                .collect(),
        };
        assert_eq!(
            decode_unicode_source(&input, encoding, limits())
                .unwrap()
                .text(),
            text
        );
        let bounded = DecodeLimits {
            retained_bytes: NonZeroUsize::MIN,
            ..limits()
        };
        assert!(matches!(
            decode_unicode_source(&input, encoding, bounded),
            Err(DecodeError::Bytes)
        ));
    }
}
