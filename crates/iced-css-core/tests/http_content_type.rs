use iced_css_core::resources::http_metadata::*;
use std::num::NonZeroUsize;
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn limits() -> ContentTypeLimits {
    ContentTypeLimits {
        parameters: n(100),
        retained_bytes: n(100),
        work: n(10000),
    }
}
#[test]
fn actual_charset_presence_spelling_and_http_quoted_pairs_are_preserved() {
    assert!(parse_content_type_fields(&[], limits()).unwrap().is_none());
    let plain = parse_content_type_fields(&[b"text/css"], limits())
        .unwrap()
        .unwrap();
    assert!(plain.is_css());
    assert!(plain.charset().is_none());
    let raw = b" \tText/CSS; CHARSET=\"UtF\\-8\"; x=\"a;b,c\"; ; \t";
    let content = parse_content_type_fields(&[raw], limits())
        .unwrap()
        .unwrap();
    assert!(content.is_css());
    assert_eq!(content.type_name, b"Text");
    assert_eq!(content.subtype, b"CSS");
    assert_eq!(content.charset(), Some(b"UtF-8".as_slice()));
    assert_eq!(&raw[content.charset_span().unwrap()], b"\"UtF\\-8\"");
    let empty = parse_content_type_fields(&[b"text/css;charset=\"\""], limits())
        .unwrap()
        .unwrap();
    assert_eq!(empty.charset(), Some(b"".as_slice()));
    let content = parse_content_type_fields(&[b"application/octet-stream;charset=utf-8"], limits())
        .unwrap()
        .unwrap();
    assert!(!content.is_css());
    assert_eq!(content.charset(), Some(b"utf-8".as_slice()));
}
#[test]
fn singleton_and_charset_ambiguity_are_explicit_without_defaulting() {
    assert!(matches!(
        parse_content_type_fields(&[b"text/css", b"text/css"], limits()),
        Err(ContentTypeError::MultipleFields { count: 2 })
    ));
    assert!(matches!(
        parse_content_type_fields(&[b"text/css;charset=utf-8;CHARSET=utf-8"], limits()),
        Err(ContentTypeError::DuplicateCharset { .. })
    ));
    let content = parse_content_type_fields(&[b"text/css;x=a;x=b;charset=utf-8"], limits())
        .unwrap()
        .unwrap();
    assert_eq!(content.charset(), Some(b"utf-8".as_slice()));
    for invalid in [
        b"text/css, text/plain".as_slice(),
        b"text /css",
        b"text/ css",
        b"text/css;charset =utf-8",
        b"text/css;charset= utf-8",
        b"text/css;charset=",
        b"text/css;x=\"bad\r\nvalue\"",
        b"text/css;x=\"unterminated",
        b"text/css;comment",
        b"text/css(comment)",
    ] {
        assert!(
            matches!(
                parse_content_type_fields(&[invalid], limits()),
                Err(ContentTypeError::Invalid { .. })
            ),
            "{invalid:?}"
        );
    }
}
#[test]
fn byte_values_and_empty_parameter_slots_obey_their_own_limits() {
    let content = parse_content_type_fields(&[b"text/css;charset=\"\xff\""], limits())
        .unwrap()
        .unwrap();
    assert_eq!(content.charset(), Some(b"\xff".as_slice()));
    let mut bounded = limits();
    bounded.parameters = NonZeroUsize::MIN;
    assert!(matches!(
        parse_content_type_fields(&[b"text/css;;"], bounded),
        Err(ContentTypeError::Parameters)
    ));
    bounded = limits();
    bounded.retained_bytes = NonZeroUsize::MIN;
    assert!(parse_content_type_fields(&[b"text/css;charset=utf-8"], bounded).is_ok());
    assert!(matches!(
        parse_content_type_fields(&[b"text/css;charset=\"ut\\f-8\""], bounded),
        Err(ContentTypeError::Bytes)
    ));
    bounded = limits();
    bounded.work = NonZeroUsize::MIN;
    assert!(matches!(
        parse_content_type_fields(&[b"text/css"], bounded),
        Err(ContentTypeError::Work)
    ));
}

#[test]
fn parsed_charset_drives_css_transport_precedence_without_a_synthesized_default() {
    use iced_css_core::syntax::{
        decode::DecodeLimits,
        encoding_selection::{EncodingMetadata, EncodingOrigin, SelectionLimits},
        unicode_intake::{IntakeError, IntakeLimits, decode_separate_unicode},
    };
    let intake = IntakeLimits {
        selection: SelectionLimits {
            registry_work: n(100000),
            prefix_work: n(100000),
        },
        decode: DecodeLimits {
            entries: n(100000),
            retained_bytes: n(100000),
            work: n(100000),
        },
        charset_work: n(100000),
    };
    for header in [None, Some(b"text/css".as_slice())] {
        let fields: Vec<_> = header.into_iter().collect();
        let content = parse_content_type_fields(&fields, limits()).unwrap();
        let source = decode_separate_unicode(
            b"a{}",
            EncodingMetadata {
                transport: content.as_ref().and_then(HttpContentType::charset),
                ..Default::default()
            },
            intake,
        )
        .unwrap();
        assert_eq!(source.origin(), EncodingOrigin::Default);
        assert_eq!(source.text(), "a{}");
    }
    let content = parse_content_type_fields(&[b"text/css;charset=\"csUTF8\""], limits())
        .unwrap()
        .unwrap();
    let source = decode_separate_unicode(
        b"a{}",
        EncodingMetadata {
            transport: content.charset(),
            ..Default::default()
        },
        intake,
    )
    .unwrap();
    assert_eq!(source.origin(), EncodingOrigin::Transport);
    let content = parse_content_type_fields(&[b"text/css;charset=latin1"], limits())
        .unwrap()
        .unwrap();
    assert!(matches!(
        decode_separate_unicode(
            b"a{}",
            EncodingMetadata {
                transport: content.charset(),
                ..Default::default()
            },
            intake
        ),
        Err(IntakeError::UnimplementedCharset { mib: 4 })
    ));
    let content = parse_content_type_fields(&[b"text/css;charset=\"\""], limits())
        .unwrap()
        .unwrap();
    assert!(matches!(
        decode_separate_unicode(
            b"a{}",
            EncodingMetadata {
                transport: content.charset(),
                ..Default::default()
            },
            intake
        ),
        Err(IntakeError::UnknownCharset)
    ));
}
