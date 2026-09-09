use iced_css_core::resources::entity_tag::*;
use std::num::NonZeroUsize;
fn n(x: usize) -> NonZeroUsize {
    NonZeroUsize::new(x).unwrap()
}
fn limits() -> EntityTagLimits {
    EntityTagLimits {
        bytes: n(1024),
        work: n(10000),
    }
}
fn tag(x: &[u8]) -> EntityTag<'_> {
    parse_entity_tag_fields(&[x], limits()).unwrap().unwrap()
}
#[test]
fn rfc_comparison_table_and_opaque_round_trip() {
    for (a, b, strong, weak) in [
        (b"W/\"1\"".as_slice(), b"W/\"1\"".as_slice(), false, true),
        (b"W/\"1\"", b"W/\"2\"", false, false),
        (b"W/\"1\"", b"\"1\"", false, true),
        (b"\"1\"", b"\"1\"", true, true),
    ] {
        let (a, b) = (tag(a), tag(b));
        assert_eq!(a.strong_eq(b, n(100)).unwrap(), strong);
        assert_eq!(a.weak_eq(b, n(100)).unwrap(), weak);
    }
    let t = tag(b" \tW/\"a\\b,\xff\" \t");
    assert!(t.is_weak());
    assert_eq!(t.opaque(), b"a\\b,\xff");
    assert_eq!(t.as_bytes(), b"W/\"a\\b,\xff\"");
    assert_eq!(tag(b"\"\"").opaque(), b"");
}
#[test]
fn every_byte_obeys_etagc_without_http_quoted_pair_decoding() {
    for c in 0u8..=255 {
        let data = [b'"', c, b'"'];
        let result = parse_entity_tag_fields(&[&data], limits());
        let valid = c == 0x21 || (0x23..=0x7e).contains(&c) || c >= 0x80;
        assert_eq!(result.is_ok(), valid, "{c}");
    }
    for invalid in [
        b"w/\"1\"".as_slice(),
        b"W/ \"1\"",
        b"\"a\\\"b\"",
        b"\"1\",\"2\"",
        b"*",
        b"",
        b"\"unterminated",
    ] {
        assert!(
            matches!(
                parse_entity_tag_fields(&[invalid], limits()),
                Err(EntityTagError::Invalid { .. })
            ),
            "{invalid:?}"
        );
    }
    assert!(parse_entity_tag_fields(&[], limits()).unwrap().is_none());
    assert!(matches!(
        parse_entity_tag_fields(&[b"\"1\"", b"\"1\""], limits()),
        Err(EntityTagError::MultipleFields { count: 2 })
    ));
}
#[test]
fn parsing_and_comparison_have_explicit_bounds() {
    let mut bound = limits();
    bound.bytes = n(2);
    assert!(matches!(
        parse_entity_tag_fields(&[b"\"a\""], bound),
        Err(EntityTagError::Bytes)
    ));
    bound = limits();
    bound.work = n(1);
    assert!(matches!(
        parse_entity_tag_fields(&[b"\"a\""], bound),
        Err(EntityTagError::Work)
    ));
    let a = tag(b"\"long\"");
    assert!(matches!(a.weak_eq(a, n(1)), Err(EntityTagError::Work)));
    assert!(matches!(a.strong_eq(a, n(1)), Err(EntityTagError::Work)));
}
