use iced_css_core::resources::content_codings::*;
use std::num::NonZeroUsize;
fn n(x: usize) -> NonZeroUsize {
    NonZeroUsize::new(x).unwrap()
}
fn limits() -> ContentCodingLimits {
    ContentCodingLimits {
        fields: n(10),
        bytes: n(1000),
        members: n(10),
        record_bytes: n(1000),
        work: n(10000),
    }
}
#[test]
fn application_order_and_reverse_decoding_order_preserve_provenance() {
    let fields: &[&[u8]] = &[b" , GZip,", b" \tdeflate , x-custom "];
    let list = parse_content_coding_fields(fields, limits()).unwrap();
    assert!(list.is_present());
    assert_eq!(
        list.codings().iter().map(|c| c.name()).collect::<Vec<_>>(),
        vec![b"GZip".as_slice(), b"deflate", b"x-custom"]
    );
    assert_eq!(
        list.decoding_order().map(|c| c.name()).collect::<Vec<_>>(),
        vec![b"x-custom".as_slice(), b"deflate", b"GZip"]
    );
    for coding in list.codings() {
        assert_eq!(&fields[coding.field()][coding.span()], coding.name());
    }
}
#[test]
fn empty_lists_and_unknown_tokens_do_not_invent_decoder_support() {
    let absent = parse_content_coding_fields(&[], limits()).unwrap();
    assert!(!absent.is_present());
    assert!(absent.codings().is_empty());
    for fields in [&[b"".as_slice()][..], &[b" , , ".as_slice(), b"\t"][..]] {
        let list = parse_content_coding_fields(fields, limits()).unwrap();
        assert!(list.is_present());
        assert!(list.codings().is_empty());
    }
    let list = parse_content_coding_fields(&[b"identity, *, unknown"], limits()).unwrap();
    assert_eq!(list.codings().len(), 3);
}
#[test]
fn malformed_coding_lists_and_all_budgets_are_rejected() {
    for bad in [
        b"\"gzip\"".as_slice(),
        b"gzip;q=1",
        b"gzip deflate",
        b"gzip\r\n",
        b"\xff",
    ] {
        assert!(matches!(
            parse_content_coding_fields(&[b"br", bad], limits()),
            Err(ContentCodingError::Invalid { field: 1, .. })
        ));
    }
    assert!(matches!(
        parse_content_coding_fields(
            &[b"gzip".as_slice(), b"br"],
            ContentCodingLimits {
                fields: n(1),
                ..limits()
            }
        ),
        Err(ContentCodingError::Fields)
    ));
    assert!(matches!(
        parse_content_coding_fields(
            &[b"gzip"],
            ContentCodingLimits {
                bytes: n(1),
                ..limits()
            }
        ),
        Err(ContentCodingError::Bytes)
    ));
    assert!(matches!(
        parse_content_coding_fields(
            &[b"gzip,br"],
            ContentCodingLimits {
                members: n(1),
                ..limits()
            }
        ),
        Err(ContentCodingError::Members)
    ));
    assert!(matches!(
        parse_content_coding_fields(
            &[b"gzip"],
            ContentCodingLimits {
                record_bytes: n(1),
                ..limits()
            }
        ),
        Err(ContentCodingError::Records)
    ));
    assert!(matches!(
        parse_content_coding_fields(
            &[b" , , , "],
            ContentCodingLimits {
                work: n(1),
                ..limits()
            }
        ),
        Err(ContentCodingError::Work)
    ));
    assert!(matches!(
        parse_content_coding_fields(
            &[],
            ContentCodingLimits {
                record_bytes: n(usize::MAX),
                ..limits()
            }
        ),
        Err(ContentCodingError::Records)
    ));
}
