use iced_css_core::resources::age::*;
use std::num::NonZeroUsize;
fn n(x: usize) -> NonZeroUsize {
    NonZeroUsize::new(x).unwrap()
}
fn limits() -> AgeLimits {
    AgeLimits {
        bytes: n(1000),
        work: n(10000),
    }
}
#[test]
fn decimal_seconds_saturate_without_skipping_invalid_suffixes() {
    for (raw, want) in [
        (b"0".as_slice(), 0),
        (b"00015", 15),
        (b"18446744073709551615", u64::MAX),
        (b"18446744073709551616", u64::MAX),
        (b"99999999999999999999999999999999999999999999", u64::MAX),
    ] {
        assert_eq!(parse_delta_seconds(raw, limits()).unwrap().get(), want);
    }
    for raw in [
        b"".as_slice(),
        b"+1",
        b"-1",
        b"1.0",
        b" 1",
        b"1 ",
        b"999999999999999999999999999999999999x",
    ] {
        assert!(
            matches!(
                parse_delta_seconds(raw, limits()),
                Err(AgeError::Invalid { .. })
            ),
            "{raw:?}"
        );
    }
    let max = parse_delta_seconds(b"18446744073709551615", limits()).unwrap();
    let one = parse_delta_seconds(b"1", limits()).unwrap();
    assert_eq!(max.saturating_add(one).get(), u64::MAX);
    assert_eq!(one.elapsed_since(max).get(), 0);
    assert_eq!(max.elapsed_since(one).get(), u64::MAX - 1);
}
#[test]
fn age_uses_first_member_and_distinguishes_invalid_from_absent_and_zero() {
    assert!(matches!(
        parse_age_fields(&[], limits()).unwrap(),
        AgeField::Absent
    ));
    match parse_age_fields(&[b" \t0012 , ignored", b"99"], limits()).unwrap() {
        AgeField::Seconds { value, discarded } => {
            assert_eq!(value.get(), 12);
            assert!(discarded);
        }
        other => panic!("{other:?}"),
    }
    assert!(
        matches!(parse_age_fields(&[b"0"],limits()).unwrap(),AgeField::Seconds{value,discarded:false} if value.get()==0)
    );
    for field in [b"".as_slice(), b",12", b"bad,12", b"\"12\"", b"1 2"] {
        assert!(matches!(
            parse_age_fields(&[field], limits()).unwrap(),
            AgeField::Ignored { .. }
        ));
    }
    assert!(matches!(
        parse_age_fields(&[b"bad", b"12"], limits()).unwrap(),
        AgeField::Ignored {
            discarded: true,
            ..
        }
    ));
}
#[test]
fn field_and_number_limits_remain_explicit_errors() {
    let mut cap = limits();
    cap.bytes = n(1);
    assert!(matches!(
        parse_delta_seconds(b"12", cap),
        Err(AgeError::Bytes)
    ));
    assert!(matches!(
        parse_age_fields(&[b"1, junk"], cap),
        Err(AgeError::Bytes)
    ));
    cap = limits();
    cap.work = n(1);
    assert!(matches!(
        parse_delta_seconds(b"12", cap),
        Err(AgeError::Work)
    ));
    assert!(matches!(
        parse_age_fields(&[b"bad"], cap),
        Err(AgeError::Work)
    ));
}
