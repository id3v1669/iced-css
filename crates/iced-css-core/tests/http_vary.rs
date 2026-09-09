use iced_css_core::resources::vary::*;
use std::num::NonZeroUsize;
fn n(x: usize) -> NonZeroUsize {
    NonZeroUsize::new(x).unwrap()
}
fn limits() -> VaryLimits {
    VaryLimits {
        fields: n(10),
        bytes: n(1000),
        members: n(20),
        record_bytes: n(4096),
        work: n(10000),
    }
}
#[test]
fn field_lines_empty_members_and_original_locations_are_retained() {
    let vary = parse_vary_fields(
        &[
            b" , Accept-Encoding,,\t",
            b"accept-language, ACCEPT-encoding,",
        ],
        limits(),
    )
    .unwrap();
    assert!(vary.is_present());
    assert!(!vary.is_wildcard());
    assert_eq!(vary.members().len(), 3);
    let a = &vary.members()[0];
    assert_eq!(a.name(), b"Accept-Encoding");
    assert_eq!(a.field(), 0);
    assert_eq!(a.span(), 3..18);
    assert_eq!(vary.members()[1].field(), 1);
    assert!(vary.selects(b"ACCEPT-LANGUAGE", n(1000)).unwrap());
    assert!(!vary.selects(b"content-type", n(1000)).unwrap());
    assert!(!parse_vary_fields(&[], limits()).unwrap().is_present());
    assert!(
        parse_vary_fields(&[b" , , \t"], limits())
            .unwrap()
            .members()
            .is_empty()
    );
}
#[test]
fn wildcard_in_any_position_blocks_reuse_but_does_not_skip_invalid_tail() {
    for fields in [
        vec![b"*".as_slice()],
        vec![b"accept, *, x".as_slice()],
        vec![b"x".as_slice(), b"*,".as_slice()],
    ] {
        let vary = parse_vary_fields(&fields, limits()).unwrap();
        assert!(vary.is_wildcard());
    }
    for invalid in [
        b"*, @bad".as_slice(),
        b"accept encoding",
        b"\"accept\"",
        b"x;param",
        b"x\r\ny",
    ] {
        assert!(
            matches!(
                parse_vary_fields(&[invalid], limits()),
                Err(VaryError::Invalid { .. })
            ),
            "{invalid:?}"
        );
    }
    assert!(!parse_vary_fields(&[b"x*"], limits()).unwrap().is_wildcard());
}
#[test]
fn bounds_are_failures_and_field_name_grammar_is_ascii_token() {
    let mut cap = limits();
    cap.fields = n(1);
    assert!(matches!(
        parse_vary_fields(&[b"x", b"y"], cap),
        Err(VaryError::Fields)
    ));
    cap = limits();
    cap.bytes = n(1);
    assert!(matches!(
        parse_vary_fields(&[b"xy"], cap),
        Err(VaryError::Bytes)
    ));
    cap = limits();
    cap.members = n(1);
    assert!(matches!(
        parse_vary_fields(&[b"x,y"], cap),
        Err(VaryError::Members)
    ));
    cap = limits();
    cap.record_bytes = n(1);
    assert!(matches!(
        parse_vary_fields(&[b"x"], cap),
        Err(VaryError::Records)
    ));
    cap = limits();
    cap.work = n(1);
    assert!(matches!(
        parse_vary_fields(&[b",,,,,"], cap),
        Err(VaryError::Work)
    ));
    for c in 0u8..=255 {
        let bytes = [b'x', c, b'y'];
        let valid = c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c) || c == b',';
        assert_eq!(parse_vary_fields(&[&bytes], limits()).is_ok(), valid, "{c}");
    }
}
