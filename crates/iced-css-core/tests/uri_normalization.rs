use iced_css_core::resources::uri::*;
use std::num::NonZeroUsize;
fn limits() -> UriLimits {
    UriLimits {
        bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(100000).unwrap(),
    }
}
#[test]
fn generic_syntax_normalization_matches_retained_examples_and_is_idempotent() {
    for (input, expected) in [
        (
            "eXAMPLE://a/./b/../b/%63/%7bfoo%7d",
            "example://a/b/c/%7Bfoo%7D",
        ),
        ("example://a/b/c/%7Bfoo%7D", "example://a/b/c/%7Bfoo%7D"),
        ("HTTP://%45XAMPLE.COM/a/%2e%2E/b", "http://example.com/b"),
        ("HTTP://[2001:DB8::A]:80/", "http://[2001:db8::a]:80/"),
        (
            "FiLe://LOCALHOST/Upper/Lower",
            "file://localhost/Upper/Lower",
        ),
        ("MAILTO:Joe@Example.COM", "mailto:Joe@Example.COM"),
    ] {
        let result = normalize_syntax(input, limits()).unwrap();
        assert_eq!(result, expected);
        assert_eq!(normalize_syntax(&result, limits()).unwrap(), result);
    }
}
#[test]
fn syntax_normalization_preserves_reserved_components_and_scheme_specific_choices() {
    for (input, expected) in [
        (
            "HTTP://User:Pa%3ass@EXAMPLE.COM:080/a%2fb?q=%2f%7e#%41",
            "http://User:Pa%3Ass@example.com:080/a%2Fb?q=%2F~#A",
        ),
        ("HTTP://EXAMPLE.COM", "http://example.com"),
        ("HTTP://EXAMPLE.COM:", "http://example.com:"),
        ("HTTP://EXAMPLE.COM/?", "http://example.com/?"),
        ("HTTP://EXAMPLE.COM/#", "http://example.com/#"),
        ("HTTP://@EXAMPLE.COM/?#", "http://@example.com/?#"),
        ("file:/Upper", "file:/Upper"),
        ("file:///Upper", "file:///Upper"),
    ] {
        assert_eq!(normalize_syntax(input, limits()).unwrap(), expected);
    }
}
#[test]
fn normalization_rejects_invalid_inputs_and_exhausted_limits() {
    assert_eq!(
        normalize_syntax("relative", limits()),
        Err(UriError::RelativeBase)
    );
    assert_eq!(
        normalize_syntax("http://h/%GG", limits()),
        Err(UriError::Syntax)
    );
    assert_eq!(
        normalize_syntax("http://h/é", limits()),
        Err(UriError::NonAscii)
    );
    let mut tiny = limits();
    tiny.bytes = NonZeroUsize::MIN;
    assert_eq!(normalize_syntax("http://h/", tiny), Err(UriError::Bytes));
    tiny = limits();
    tiny.work = NonZeroUsize::MIN;
    assert_eq!(normalize_syntax("http://h/", tiny), Err(UriError::Work));
    tiny = limits();
    tiny.bytes = NonZeroUsize::new(usize::MAX).unwrap();
    assert_eq!(normalize_syntax("http://h/", tiny), Err(UriError::Bytes));
}

#[test]
fn dot_removal_must_not_create_an_authority() {
    for input in [
        "asset:a/..//bundle/x",
        "asset:/a/..//bundle/x",
        "asset:/%2e//bundle/x",
    ] {
        assert_eq!(normalize_syntax(input, limits()), Err(UriError::Syntax));
    }
    assert_eq!(
        normalize_syntax("asset://host/a/..//bundle/x", limits()).unwrap(),
        "asset://host//bundle/x"
    );
}
