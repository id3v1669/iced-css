use iced_css_core::resources::{
    location::*,
    uri::{UriError, UriLimits},
};
use std::num::NonZeroUsize;
fn n(x: usize) -> NonZeroUsize {
    NonZeroUsize::new(x).unwrap()
}
fn limits() -> UriLimits {
    UriLimits {
        bytes: n(4096),
        work: n(100000),
    }
}
#[test]
fn rfc_redirects_inherit_only_an_absent_location_fragment() {
    for (target, reference, location, want, acquire) in [
        (
            "http://www.example.org/~tim",
            "http://www.example.org/~tim",
            b"/People.html#tim".as_slice(),
            "http://www.example.org/People.html#tim",
            "http://www.example.org/People.html",
        ),
        (
            "http://www.example.org/index.html",
            "http://www.example.org/index.html#larry",
            b"http://www.example.net/index.html",
            "http://www.example.net/index.html#larry",
            "http://www.example.net/index.html",
        ),
        (
            "https://x/a/b?old",
            "https://x/a/b?old#prior",
            b"../c?new#",
            "https://x/c?new#",
            "https://x/c?new",
        ),
        (
            "https://x/a",
            "https://x/a#",
            b"b",
            "https://x/b#",
            "https://x/b",
        ),
    ] {
        let result = resolve_redirect_location(target, reference, &[location], limits())
            .unwrap()
            .unwrap();
        assert_eq!(result.reference_uri(), want);
        assert_eq!(result.acquisition_uri(), acquire);
    }
}
#[test]
fn singleton_uri_data_and_actual_target_base_are_preserved() {
    assert!(
        resolve_redirect_location("https://x/a", "https://x/a", &[], limits())
            .unwrap()
            .is_none()
    );
    let r = resolve_redirect_location(
        "https://x/normalized/a",
        "https://x/old/../normalized/a#f",
        &[b" \t../b,c? \t"],
        limits(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(r.reference_uri(), "https://x/b,c?#f");
    assert_eq!(r.location_span(), 2..9);
    let r = resolve_redirect_location("https://x/a?old", "https://x/a?old#f", &[b""], limits())
        .unwrap()
        .unwrap();
    assert_eq!(r.reference_uri(), "https://x/a?old#f");
    assert!(matches!(
        resolve_redirect_location("https://x/a", "https://x/a", &[b"b", b"c"], limits()),
        Err(LocationError::MultipleFields { count: 2 })
    ));
    for invalid in [b"with space".as_slice(), b"%xx", b"\xff"] {
        assert!(matches!(
            resolve_redirect_location("https://x/a", "https://x/a", &[invalid], limits()),
            Err(LocationError::Uri(_))
        ));
    }
}
#[test]
fn inherited_fragment_storage_and_shared_work_are_bounded() {
    let mut cap = limits();
    cap.bytes = n(15);
    assert!(matches!(
        resolve_redirect_location("https://x/a", "https://x/a#abc", &[b"long"], cap),
        Err(LocationError::Uri(UriError::Bytes))
    ));
    cap = limits();
    cap.work = n(1);
    assert!(matches!(
        resolve_redirect_location("https://x/a", "https://x/a#f", &[b"b"], cap),
        Err(LocationError::Uri(UriError::Work))
    ));
    assert!(matches!(
        resolve_redirect_location("https://x/a#bad", "https://x/a#bad", &[b"b"], limits()),
        Err(LocationError::TargetFragment)
    ));
}
