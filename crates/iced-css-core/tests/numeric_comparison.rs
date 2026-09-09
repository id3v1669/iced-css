use iced_css_core::values::{Decimal, Integer, NumericError, NumericLimits};
use std::{
    cmp::Ordering::{self, Equal, Greater, Less},
    num::NonZeroUsize,
};
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn decimal(s: &str) -> Decimal {
    Decimal::parse(s, limits()).unwrap()
}
fn integer(s: &str) -> Integer {
    Integer::parse(s, limits()).unwrap()
}
#[test]
fn decimal_order_preserves_sign_scale_and_implicit_trailing_zeros() {
    for (a, b, want) in [
        ("-0", "0", Equal),
        ("0", "0.0001", Less),
        ("0", "-0.0001", Greater),
        ("1.20", "1.2", Equal),
        ("1.2", "1.2001", Less),
        ("10", "9.999", Greater),
        ("0.001", "0.00099", Greater),
        ("1200", "1200.001", Less),
        ("-0.0002", "-0.0001", Less),
        ("-1.2", "-1.23", Greater),
        ("0.1000000000000000000001", "0.1", Greater),
    ] {
        let a = decimal(a);
        let b = decimal(b);
        let before = (a.clone(), b.clone());
        assert_eq!(a.checked_cmp(&b, limits()).unwrap(), want);
        assert_eq!(b.checked_cmp(&a, limits()).unwrap(), want.reverse());
        assert_eq!((a, b), before);
    }
    let a = decimal(&format!("0.{}1", "0".repeat(499)));
    let b = decimal(&format!("0.{}9", "0".repeat(500)));
    assert_eq!(a.checked_cmp(&b, limits()).unwrap(), Greater);
    let a = decimal(&format!("1{}", "0".repeat(500)));
    let b = decimal(&"9".repeat(500));
    assert_eq!(a.checked_cmp(&b, limits()).unwrap(), Greater);
}
#[test]
fn integer_order_never_narrows_wide_values() {
    let wide = "9".repeat(200);
    let near = format!("{}8", "9".repeat(199));
    for (a, b, want) in [
        ("-0".to_owned(), "0".to_owned(), Equal),
        (wide.clone(), near.clone(), Greater),
        (format!("-{wide}"), format!("-{near}"), Less),
        (wide, format!("1{}", "0".repeat(200)), Less),
    ] {
        let a = integer(&a);
        let b = integer(&b);
        let before = (a.clone(), b.clone());
        assert_eq!(a.checked_cmp(&b, limits()).unwrap(), want);
        assert_eq!(b.checked_cmp(&a, limits()).unwrap(), want.reverse());
        assert_eq!((a, b), before);
    }
}
#[test]
fn resource_exhaustion_is_an_error_instead_of_an_ordering() {
    let mut tiny = limits();
    tiny.work = NonZeroUsize::MIN;
    assert_eq!(
        decimal("123.45").checked_cmp(&decimal("123.45"), tiny),
        Err(NumericError::WorkLimit)
    );
    assert_eq!(
        integer("12345678901").checked_cmp(&integer("12345678901"), tiny),
        Err(NumericError::WorkLimit)
    );
    assert_eq!(
        decimal("0").checked_cmp(&decimal("-0"), tiny).unwrap(),
        Ordering::Equal
    );
    tiny = limits();
    tiny.digits = NonZeroUsize::new(3).unwrap();
    assert_eq!(
        integer("1000").checked_cmp(&integer("1"), tiny),
        Err(NumericError::DigitLimit)
    );
    assert_eq!(
        decimal("0.0001").checked_cmp(&decimal("1"), tiny),
        Err(NumericError::DigitLimit)
    );
    assert_eq!(
        decimal("1").checked_cmp(&decimal("1000"), tiny),
        Err(NumericError::DigitLimit)
    );
}
