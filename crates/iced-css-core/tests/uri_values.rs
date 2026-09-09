use iced_css_core::syntax::*;
use std::num::NonZeroUsize;
fn limits() -> StringLimits {
    StringLimits {
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(100000).unwrap(),
    }
}
#[test]
fn uri_decoding_preserves_payload_and_decodes_css_escapes() {
    for (input, expected) in [
        ("url(a.png)", "a.png"),
        ("url( 'a b.png' )", "a b.png"),
        (r"\75 rl(a\ b\(c\).png)", "a b(c).png"),
        ("url(/*x*/a%20b)", "/*x*/a%20b"),
        ("url()", ""),
        (r"url(a\20 )", "a "),
    ] {
        let actual = decode_uri(input, limits()).unwrap();
        assert_eq!(actual.text, expected, "{input}");
        assert_eq!(actual.consumed, input.len());
        assert_eq!(actual.closure, Closure::Explicit);
    }
}
#[test]
fn uri_eof_repairs_follow_token_boundaries() {
    for input in ["url(a", "url('a", "url('a'"] {
        let actual = decode_uri(input, limits()).unwrap();
        assert_eq!(actual.text, "a");
        assert_eq!(actual.closure, Closure::EndOfInput);
    }
    for input in [
        "url(a b)",
        "url(a\\",
        "url(\\0)",
        "url('a\nb')",
        "url(a)junk",
        "foo(a)",
    ] {
        assert!(decode_uri(input, limits()).is_err(), "{input}");
    }
}
#[test]
fn uri_repairs_and_limits_are_explicit() {
    let actual = decode_uri("url(a\0b)", limits()).unwrap();
    assert_eq!(actual.text, "a\u{fffd}b");
    assert_eq!(actual.repairs[0].start, 5);
    let mut small = limits();
    small.retained_bytes = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        decode_uri("url(abc)", small),
        Err(SyntaxError {
            fault: SyntaxFault::Resource(SyntaxResource::Bytes),
            ..
        })
    ));
    small = limits();
    small.work = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        decode_uri("url(abc)", small),
        Err(SyntaxError {
            fault: SyntaxFault::Resource(SyntaxResource::Work),
            ..
        })
    ));
}

#[test]
fn quoted_uri_eof_and_continuation_reuse_string_semantics() {
    let value = decode_uri("url('a\\", limits()).unwrap();
    assert_eq!(value.text, "a\\");
    assert_eq!(value.closure, Closure::EndOfInput);
    assert_eq!(decode_uri("url('a\\\r\nb')", limits()).unwrap().text, "ab");
    let value = decode_uri(r"\75 rl(\110000)", limits()).unwrap();
    assert_eq!(value.text, "\u{fffd}");
    assert_eq!(value.repairs[0].start, 7);
}
