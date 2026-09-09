use iced_css_core::resources::{cache_control::*, cache_restrictions::*};
use std::num::NonZeroUsize;
fn n(x: usize) -> NonZeroUsize {
    NonZeroUsize::new(x).unwrap()
}
fn parsed(x: &[u8]) -> CacheControl<'_> {
    parse_cache_control_fields(
        &[x],
        CacheControlLimits {
            fields: n(10),
            bytes: n(10000),
            directives: n(100),
            retained_bytes: n(10000),
            work: n(100000),
        },
    )
    .unwrap()
}
fn limits() -> CacheEvaluationLimits {
    CacheEvaluationLimits {
        directives: n(100),
        work: n(100000),
    }
}
#[test]
fn restrictions_cannot_be_cleared_by_extensions_or_public_cache_hints() {
    let c = parsed(b"public, No-Store, must-understand, x-unknown=anything, max-age=999");
    let r = private_cache_restrictions(&c, limits()).unwrap();
    assert!(r.no_store);
    assert!(!r.validate_before_reuse);
    assert!(matches!(r.max_age,MaxAge::Seconds(x) if x.get()==999));
    let c = parsed(b"no-cache=\"etag\", must-revalidate, public");
    let r = private_cache_restrictions(&c, limits()).unwrap();
    assert!(r.validate_before_reuse);
    assert!(r.validate_when_stale);
    assert!(!r.no_store);
    let c = parsed(b"proxy-revalidate, s-maxage=0, private");
    let r = private_cache_restrictions(&c, limits()).unwrap();
    assert!(!r.no_store);
    assert!(!r.validate_before_reuse);
    assert!(!r.validate_when_stale);
    assert_eq!(r.max_age, MaxAge::Absent);
}
#[test]
fn duplicate_or_malformed_max_age_is_stale_without_falling_back_to_other_values() {
    for field in [
        b"max-age=10,max-age=10".as_slice(),
        b"max-age=bad,max-age=10",
        b"max-age=10,max-age=bad",
        b"max-age",
        b"max-age=\"\"",
        b"max-age=-1",
        b"max-age=1.0",
    ] {
        let r = private_cache_restrictions(&parsed(field), limits()).unwrap();
        assert!(matches!(r.max_age, MaxAge::Invalid { .. }), "{field:?}");
    }
    let r = private_cache_restrictions(&parsed(b"max-age=\"0\\5\""), limits()).unwrap();
    assert!(matches!(r.max_age,MaxAge::Seconds(x) if x.get()==5));
    let r = private_cache_restrictions(&parsed(b"max-age=999999999999999999999999999"), limits())
        .unwrap();
    assert!(matches!(r.max_age,MaxAge::Seconds(x) if x.get()==u64::MAX));
    let r = private_cache_restrictions(&parsed(b"x, max-age=1, max-age=2, max-age=3"), limits())
        .unwrap();
    assert_eq!(
        r.max_age,
        MaxAge::Invalid {
            first: 1,
            second: Some(2)
        }
    );
}
#[test]
fn malformed_restriction_arguments_are_reported_without_weakening_them() {
    let r =
        private_cache_restrictions(&parsed(b"no-store=0, must-revalidate=0"), limits()).unwrap();
    assert!(r.no_store);
    assert!(r.validate_when_stale);
    assert_eq!(r.malformed_restriction, Some(0));
}
#[test]
fn evaluation_is_bounded_including_ignored_names_and_numeric_scan() {
    let mut cap = limits();
    cap.directives = n(1);
    assert!(matches!(
        private_cache_restrictions(&parsed(b"x,y"), cap),
        Err(CacheEvaluationError::Directives)
    ));
    cap = limits();
    cap.work = n(1);
    assert!(matches!(
        private_cache_restrictions(&parsed(b"x-unknown"), cap),
        Err(CacheEvaluationError::Work)
    ));
    cap = limits();
    cap.work = n(32);
    assert!(matches!(
        private_cache_restrictions(
            &parsed(b"max-age=999999999999999999999999999999999999999999999"),
            cap
        ),
        Err(CacheEvaluationError::Work)
    ));
}
