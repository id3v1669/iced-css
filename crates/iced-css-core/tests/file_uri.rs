use iced_css_core::resources::{
    file_uri::{FileAuthority, FileUri, FileUriError},
    uri::{UriError, UriLimits, UriReference},
};
use std::num::NonZeroUsize;
fn limits() -> UriLimits {
    UriLimits {
        bytes: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(100_000).unwrap(),
    }
}

#[test]
fn local_forms_preserve_native_name_spelling_before_mapping() {
    for (input, authority, path) in [
        ("file:/", FileAuthority::Local, "/"),
        (
            "FILE:///Tmp/a%3Ab%2Fc%00%FF",
            FileAuthority::Local,
            "/Tmp/a%3Ab%2Fc%00%FF",
        ),
        (
            "file://LoCaLhOsT/tmp/~User/%2e%2e/a",
            FileAuthority::Local,
            "/tmp/~User/%2e%2e/a",
        ),
        ("file:/a%3Fb%23c", FileAuthority::Local, "/a%3Fb%23c"),
        (
            "file:///c:/WINDOWS/name",
            FileAuthority::Local,
            "/c:/WINDOWS/name",
        ),
    ] {
        let parsed = FileUri::parse(input, limits()).unwrap();
        assert_eq!(parsed.as_str(), input);
        assert_eq!(parsed.authority(), authority);
        assert_eq!(parsed.path(), path);
    }
}

#[test]
fn hosts_require_external_locality_without_ports_or_credentials() {
    for host in [
        "example.org",
        "LOCALHOST.",
        "127.0.0.1",
        "[::1]",
        "[2001:db8::1]",
    ] {
        let input = format!("file://{host}/a");
        assert_eq!(
            FileUri::parse(&input, limits()).unwrap().authority(),
            FileAuthority::Unresolved(host)
        );
    }
    for input in [
        "file://user@host/a",
        "file://localhost:/a",
        "file://host:80/a",
        "file://[::1]:/a",
        "file://[::1]:80/a",
    ] {
        assert_eq!(
            FileUri::parse(input, limits()),
            Err(FileUriError::Authority),
            "{input}"
        );
    }
}

#[test]
fn acquisition_boundary_rejects_generic_only_and_extension_forms() {
    for input in [
        "file:",
        "file:a",
        "file:c:/a",
        "file://localhost",
        "file:////host/a",
        "file://localhost//a",
    ] {
        assert_eq!(
            FileUri::parse(input, limits()),
            Err(FileUriError::Path),
            "{input}"
        );
    }
    for input in ["/a", "https://localhost/a", "files:///a"] {
        assert_eq!(
            FileUri::parse(input, limits()),
            Err(FileUriError::Scheme),
            "{input}"
        );
    }
    for input in ["file:///a?", "file:///a?query"] {
        assert_eq!(FileUri::parse(input, limits()), Err(FileUriError::Query));
    }
    for input in ["file:///a#", "file:///a#fragment"] {
        assert_eq!(FileUri::parse(input, limits()), Err(FileUriError::Fragment));
        let reference = UriReference::parse(input, limits()).unwrap();
        assert_eq!(
            FileUri::parse(reference.without_fragment(), limits())
                .unwrap()
                .path(),
            "/a"
        );
    }
    for input in [
        "file:///a%",
        "file:///c|/a",
        "file:///a\\b",
        "file://[fe80::1%25eth0]/a",
    ] {
        assert!(matches!(
            FileUri::parse(input, limits()),
            Err(FileUriError::Uri(UriError::Syntax))
        ));
    }
    assert_eq!(
        FileUri::parse("file:///é", limits()),
        Err(FileUriError::Uri(UriError::NonAscii))
    );
}

#[test]
fn syntax_and_scheme_checks_share_bounded_work() {
    let mut bound = limits();
    bound.bytes = NonZeroUsize::new(7).unwrap();
    assert_eq!(
        FileUri::parse("file:///", bound),
        Err(FileUriError::Uri(UriError::Bytes))
    );
    bound = limits();
    bound.work = NonZeroUsize::new(161).unwrap(); // Generic eight-byte URI parse alone costs 161.
    assert_eq!(
        FileUri::parse("file:///", bound),
        Err(FileUriError::Uri(UriError::Work))
    );
}
