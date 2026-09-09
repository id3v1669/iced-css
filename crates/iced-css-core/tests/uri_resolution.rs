use iced_css_core::resources::uri::*;
use std::num::NonZeroUsize;
fn limits() -> UriLimits {
    UriLimits {
        bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
const RFC_CASES: &[(&str, &str)] = &[
    ("g:h", "g:h"),
    ("g", "http://a/b/c/g"),
    ("./g", "http://a/b/c/g"),
    ("g/", "http://a/b/c/g/"),
    ("/g", "http://a/g"),
    ("//g", "http://g"),
    ("?y", "http://a/b/c/d;p?y"),
    ("g?y", "http://a/b/c/g?y"),
    ("#s", "http://a/b/c/d;p?q#s"),
    ("g#s", "http://a/b/c/g#s"),
    ("g?y#s", "http://a/b/c/g?y#s"),
    (";x", "http://a/b/c/;x"),
    ("g;x", "http://a/b/c/g;x"),
    ("g;x?y#s", "http://a/b/c/g;x?y#s"),
    ("", "http://a/b/c/d;p?q"),
    (".", "http://a/b/c/"),
    ("./", "http://a/b/c/"),
    ("..", "http://a/b/"),
    ("../", "http://a/b/"),
    ("../g", "http://a/b/g"),
    ("../..", "http://a/"),
    ("../../", "http://a/"),
    ("../../g", "http://a/g"),
    ("../../../g", "http://a/g"),
    ("../../../../g", "http://a/g"),
    ("/./g", "http://a/g"),
    ("/../g", "http://a/g"),
    ("g.", "http://a/b/c/g."),
    (".g", "http://a/b/c/.g"),
    ("g..", "http://a/b/c/g.."),
    ("..g", "http://a/b/c/..g"),
    ("./../g", "http://a/b/g"),
    ("./g/.", "http://a/b/c/g/"),
    ("g/./h", "http://a/b/c/g/h"),
    ("g/../h", "http://a/b/c/h"),
    ("g;x=1/./y", "http://a/b/c/g;x=1/y"),
    ("g;x=1/../y", "http://a/b/c/y"),
    ("g?y/./x", "http://a/b/c/g?y/./x"),
    ("g?y/../x", "http://a/b/c/g?y/../x"),
    ("g#s/./x", "http://a/b/c/g#s/./x"),
    ("g#s/../x", "http://a/b/c/g#s/../x"),
    ("http:g", "http:g"),
];

#[test]
fn all_retained_rfc3986_section5_examples_resolve_strictly() {
    assert_eq!(RFC_CASES.len(), 42);
    for (reference, expected) in RFC_CASES {
        assert_eq!(
            resolve("http://a/b/c/d;p?q", reference, limits()).unwrap(),
            *expected,
            "{reference}"
        );
    }
}
#[test]
fn optional_component_presence_and_retrieval_fragment_are_preserved() {
    let reference = UriReference::parse("x://?#", limits()).unwrap();
    assert_eq!(reference.scheme(), Some("x"));
    assert_eq!(reference.authority(), Some(""));
    assert_eq!(reference.path(), "");
    assert_eq!(reference.query(), Some(""));
    assert_eq!(reference.fragment(), Some(""));
    assert_eq!(reference.without_fragment(), "x://?");
    for (reference, expected) in [
        ("", "http://a/b?q"),
        ("?", "http://a/b?"),
        ("#", "http://a/b?q#"),
        ("g?", "http://a/g?"),
        ("//", "http://"),
        ("HTTP:g", "HTTP:g"),
    ] {
        assert_eq!(
            resolve("http://a/b?q#old", reference, limits()).unwrap(),
            expected
        );
    }
}
#[test]
fn explicit_bases_merge_paths_without_filesystem_or_web_scheme_guesses() {
    for (base, reference, expected) in [
        ("asset:root", "child", "asset:child"),
        ("asset://bundle", "child", "asset://bundle/child"),
        (
            "file:///styles/main.css",
            "../images/a.png",
            "file:///images/a.png",
        ),
        ("http://a/x/../b", "", "http://a/x/../b"),
        ("http://a/b", "%2e%2e/g", "http://a/%2e%2e/g"),
        ("http://a/b", "./this:that", "http://a/this:that"),
    ] {
        assert_eq!(resolve(base, reference, limits()).unwrap(), expected);
    }
    assert_eq!(
        resolve("relative/base", "child", limits()),
        Err(UriError::RelativeBase)
    );
}
#[test]
fn authority_grammar_accepts_generic_hosts_ipv6_and_ipvfuture() {
    for uri in [
        "http://",
        "x://user:pass@host:",
        "x://host:99999999999999999999",
        "x://999.1.2.3",
        "x://%41.example",
        "x://[::]",
        "x://[::1]",
        "x://[1:2:3:4:5:6:7:8]",
        "x://[1::8]",
        "x://[1:2:3:4:5:6:192.0.2.1]",
        "x://[::ffff:192.0.2.1]",
        "x://[Vf.a:b!]:80",
    ] {
        assert!(UriReference::parse(uri, limits()).is_ok(), "{uri}");
    }
}
#[test]
fn malformed_generic_uris_are_rejected_before_resolution() {
    for uri in [
        "1x:y",
        ":x",
        "x://a@b@c",
        "x://host:port",
        "x://[1:2:3]",
        "x://[1:2:3:4:5:6:7:8:9]",
        "x://[1::2::3]",
        "x://[1:2:3:4:5:6:7:8::]",
        "x://[::ffff:192.00.2.1]",
        "x://[192.0.2.1::]",
        "x://[v1.]",
        "x://[v.abc]",
        "x://[::1]junk",
        "x://[::1",
        "x://a]",
        "x://a b",
        "x:/%",
        "x:/%GG",
        "x:/[x]",
        "x:?bad#fragment#again",
        "x:\n",
    ] {
        assert_eq!(
            UriReference::parse(uri, limits()).unwrap_err(),
            UriError::Syntax,
            "{uri}"
        );
    }
    assert_eq!(
        UriReference::parse("x:/é", limits()).unwrap_err(),
        UriError::NonAscii
    );
}
#[test]
fn byte_and_work_exhaustion_are_explicit_and_leave_input_unchanged() {
    let base = String::from("http://a/b/c");
    let reference = String::from("../x");
    let mut small = limits();
    small.work = NonZeroUsize::MIN;
    assert_eq!(resolve(&base, &reference, small), Err(UriError::Work));
    small = limits();
    small.bytes = NonZeroUsize::MIN;
    assert_eq!(resolve(&base, &reference, small), Err(UriError::Bytes));
    assert_eq!(base, "http://a/b/c");
    assert_eq!(reference, "../x");
}

#[test]
fn dot_removal_rejects_an_unserializable_authority_free_path() {
    for (base, reference) in [
        ("asset:a", "a/..//bundle/x"),
        ("asset:/a", "/a/..//bundle/x"),
        ("http://host/a", "asset:a/..//bundle/x"),
    ] {
        assert_eq!(resolve(base, reference, limits()), Err(UriError::Syntax));
    }
    assert_eq!(
        resolve("asset://host/a", "/a/..//bundle/x", limits()).unwrap(),
        "asset://host//bundle/x"
    );
}
