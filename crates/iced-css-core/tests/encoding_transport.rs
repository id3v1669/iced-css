use iced_css_core::syntax::{encoding::*, encoding_transport::*};

fn boms() -> Vec<(&'static [u8], EncodingHint)> {
    vec![
        (b"\0\0\xfe\xff", EncodingHint::Utf32(ByteOrder::Big)),
        (b"\xff\xfe\0\0", EncodingHint::Utf32(ByteOrder::Little)),
        (b"\xfe\xff", EncodingHint::Utf16(ByteOrder::Big)),
        (b"\xff\xfe", EncodingHint::Utf16(ByteOrder::Little)),
        (b"\xef\xbb\xbf", EncodingHint::Utf8),
    ]
}
#[test]
fn generic_unicode_transport_encodings_follow_every_override_row() {
    for class in [
        TransportEncoding::Utf8,
        TransportEncoding::Utf16,
        TransportEncoding::Utf32,
    ] {
        for (bytes, encoding) in boms() {
            let result = transport_bom_override(Some(class), bytes).unwrap();
            assert_eq!(result.encoding, encoding);
            assert_eq!(result.bom_bytes, bytes.len());
            assert!(!result.explicit_endian_error);
        }
    }
}
#[test]
fn explicit_endian_transport_labels_report_error_even_when_bom_agrees() {
    for class in [
        TransportEncoding::Utf16Be,
        TransportEncoding::Utf16Le,
        TransportEncoding::Utf32Be,
        TransportEncoding::Utf32Le,
    ] {
        for (bytes, encoding) in boms() {
            let result = transport_bom_override(Some(class), bytes).unwrap();
            assert_eq!(result.encoding, encoding);
            assert!(result.explicit_endian_error);
        }
    }
}
#[test]
fn absent_or_other_transport_encoding_does_not_activate_override_rule() {
    for class in [None, Some(TransportEncoding::Other)] {
        for (bytes, _) in boms() {
            assert!(transport_bom_override(class, bytes).is_none());
        }
    }
    for input in [b"".as_slice(), b"\xef\xbb", b"\xfe", b"@charset \"UTF-8\";"] {
        assert!(transport_bom_override(Some(TransportEncoding::Utf8), input).is_none());
    }
}
#[test]
fn override_table_does_not_borrow_extra_orders_from_detection_table() {
    assert!(transport_bom_override(Some(TransportEncoding::Utf32), b"\0\0\xff\xfe").is_none());
    let result = transport_bom_override(Some(TransportEncoding::Utf32), b"\xfe\xff\0\0").unwrap();
    assert_eq!(result.encoding, EncodingHint::Utf16(ByteOrder::Big));
    assert_eq!(result.bom_bytes, 2);
}
