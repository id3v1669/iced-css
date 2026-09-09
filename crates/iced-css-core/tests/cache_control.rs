use iced_css_core::resources::cache_control::*;
use std::num::NonZeroUsize;
fn n(x: usize) -> NonZeroUsize {
    NonZeroUsize::new(x).unwrap()
}
fn limits() -> CacheControlLimits {
    CacheControlLimits {
        fields: n(10),
        bytes: n(1000),
        directives: n(20),
        retained_bytes: n(10000),
        work: n(100000),
    }
}
#[test]
fn directives_preserve_duplicates_unknowns_locations_and_decoded_arguments() {
    let parsed = parse_cache_control_fields(
        &[
            b" , No-Store, max-age=5,",
            b"max-age=\"\x35\", X=\"a,b\\\"c\", no-cache=\"\"",
        ],
        limits(),
    )
    .unwrap();
    assert!(parsed.is_present());
    assert_eq!(parsed.directives().len(), 5);
    let d = &parsed.directives()[0];
    assert_eq!(d.name(), b"No-Store");
    assert_eq!(d.argument(), None);
    assert_eq!(d.field(), 0);
    assert_eq!(d.span(), 3..11);
    assert_eq!(parsed.directives()[1].argument(), Some(b"5".as_slice()));
    assert_eq!(parsed.directives()[2].argument(), Some(b"5".as_slice()));
    assert_eq!(
        parsed.directives()[3].argument(),
        Some(b"a,b\"c".as_slice())
    );
    assert_eq!(parsed.directives()[4].argument(), Some(b"".as_slice()));
    assert_eq!(parsed.directives()[3].field(), 1);
    assert_eq!(parsed.directives()[3].name(), b"X");
    assert!(
        !parse_cache_control_fields(&[], limits())
            .unwrap()
            .is_present()
    );
    assert!(
        parse_cache_control_fields(&[b", ,"], limits())
            .unwrap()
            .directives()
            .is_empty()
    );
}
#[test]
fn invalid_later_directives_and_quoted_controls_do_not_return_a_prefix() {
    for invalid in [
        b"no-store, max-age =5".as_slice(),
        b"x= 5",
        b"x=",
        b"x=\"unterminated",
        b"x=\"a\r\"",
        b"x=\"a\\\n\"",
        b"x=5 junk",
        b"x; y",
        b"x=(comment)",
    ] {
        assert!(
            matches!(
                parse_cache_control_fields(&[invalid], limits()),
                Err(CacheControlError::Invalid { .. })
            ),
            "{invalid:?}"
        );
    }
    let d = parse_cache_control_fields(&[b"x=\"\xff\\\xff\""], limits()).unwrap();
    assert_eq!(d.directives()[0].argument(), Some(b"\xff\xff".as_slice()));
}
#[test]
fn shared_field_record_argument_and_work_bounds_fail_explicitly() {
    let mut cap = limits();
    cap.fields = n(1);
    assert!(matches!(
        parse_cache_control_fields(&[b"x", b"y"], cap),
        Err(CacheControlError::Fields)
    ));
    cap = limits();
    cap.bytes = n(1);
    assert!(matches!(
        parse_cache_control_fields(&[b"xy"], cap),
        Err(CacheControlError::Bytes)
    ));
    cap = limits();
    cap.directives = n(1);
    assert!(matches!(
        parse_cache_control_fields(&[b"x,y"], cap),
        Err(CacheControlError::Directives)
    ));
    cap = limits();
    cap.retained_bytes = n(1);
    assert!(matches!(
        parse_cache_control_fields(&[b"x"], cap),
        Err(CacheControlError::Bytes)
    ));
    cap = limits();
    cap.work = n(1);
    assert!(matches!(
        parse_cache_control_fields(&[b",,,,,"], cap),
        Err(CacheControlError::Work)
    ));
    let parsed = parse_cache_control_fields(&[b"x"], limits()).unwrap();
    let base = parsed.retained_bytes();
    cap = limits();
    cap.retained_bytes = n(base);
    assert!(parse_cache_control_fields(&[b"x=\"abc\""], cap).is_ok());
    assert!(matches!(
        parse_cache_control_fields(&[b"x=\"a\\b\""], cap),
        Err(CacheControlError::Bytes)
    ));
}

#[test]
fn quoted_text_and_quoted_pairs_use_the_complete_http_byte_grammar() {
    for byte in 0u8..=255 {
        let raw = [b'x', b'=', b'"', byte, b'"'];
        let raw_valid = matches!(byte,b'\t'|b' '|0x21|0x23..=0x5b|0x5d..=0x7e|0x80..=0xff);
        assert_eq!(
            parse_cache_control_fields(&[&raw], limits()).is_ok(),
            raw_valid,
            "raw {byte}"
        );
        let escaped = [b'x', b'=', b'"', b'\\', byte, b'"'];
        let escaped_valid = matches!(byte,b'\t'|b' '..=b'~'|0x80..=0xff);
        let parsed = parse_cache_control_fields(&[&escaped], limits());
        assert_eq!(parsed.is_ok(), escaped_valid, "escaped {byte}");
        if let Ok(parsed) = parsed {
            assert_eq!(parsed.directives()[0].argument(), Some([byte].as_slice()));
        }
    }
}
