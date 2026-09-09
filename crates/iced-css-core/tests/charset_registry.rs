use iced_css_core::syntax::{charset::*, encoding_transport::TransportEncoding};
use std::num::NonZeroUsize;
fn lookup(label: &[u8]) -> Option<&'static Charset> {
    lookup_iana_charset(label, NonZeroUsize::new(10000).unwrap()).unwrap()
}
#[test]
fn iana_names_and_aliases_resolve_with_ascii_case_insensitivity() {
    for label in [b"UTF-8".as_slice(), b"utf-8", b"CsUtF8"] {
        let charset = lookup(label).unwrap();
        assert_eq!(charset.mib(), 106);
        assert_eq!(charset.name(), "UTF-8");
        assert_eq!(
            charset.unicode_transport_class(),
            Some(TransportEncoding::Utf8)
        );
    }
    let latin = lookup(b"LaTiN1").unwrap();
    assert_eq!(latin.mib(), 4);
    assert_eq!(latin.name(), "ISO_8859-1:1987");
    assert_eq!(latin.preferred_mime_name(), Some("ISO-8859-1"));
    assert_eq!(latin.unicode_transport_class(), None);
    assert_ne!(latin.mib(), lookup(b"windows-1252").unwrap().mib());
}
#[test]
fn lookup_does_not_trim_invent_aliases_or_decode_invalid_bytes() {
    for input in [
        b"utf8".as_slice(),
        b" UTF-8",
        b"UTF-8 ",
        b"\xff",
        b"UTF-8\0",
        b"",
    ] {
        assert!(lookup(input).is_none(), "{input:?}");
    }
    assert!(lookup_iana_charset(b"UTF-8", NonZeroUsize::MIN).is_err());
}
#[test]
fn unicode_transport_aliases_preserve_generic_and_explicit_endian_identity() {
    for (label, mib, class) in [
        (b"csUTF16".as_slice(), 1015, TransportEncoding::Utf16),
        (b"csUTF16BE", 1013, TransportEncoding::Utf16Be),
        (b"csUTF16LE", 1014, TransportEncoding::Utf16Le),
        (b"csUTF32", 1017, TransportEncoding::Utf32),
        (b"csUTF32BE", 1018, TransportEncoding::Utf32Be),
        (b"csUTF32LE", 1019, TransportEncoding::Utf32Le),
    ] {
        let charset = lookup(label).unwrap();
        assert_eq!(charset.mib(), mib);
        assert_eq!(charset.unicode_transport_class(), Some(class));
    }
}
#[test]
fn separately_verified_mib_alias_and_long_registered_names_remain_available() {
    assert_eq!(lookup(b"csAmiga1251").unwrap().mib(), 2104);
    assert!(lookup(b"csAmiga1251\n(Aliases are provided for historical reasons and should not be used) [Malyshev]").is_none());
    assert_eq!(
        lookup(b"Extended_UNIX_Code_Packed_Format_for_Japanese")
            .unwrap()
            .mib(),
        18
    );
}
