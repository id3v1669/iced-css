use iced_css_core::values::{Integer, NumericError, NumericLimits};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(1_000_000).unwrap(),
    }
}
fn integer(input: &str) -> Integer {
    Integer::parse(input, limits()).unwrap()
}

#[test]
fn integer_identity_retains_every_digit_and_canonicalizes_zero() {
    assert_eq!(integer("+000123"), integer("123"));
    assert_ne!(integer("16777216"), integer("16777217"));
    let huge = "123456789012345678901234567890123456789012345678901234567890";
    assert_eq!(integer(&format!("+000{huge}")), integer(huge));
    assert_ne!(integer(huge), integer(&format!("-{huge}")));
    for zero in ["0", "-0", "+000", "-000"] {
        assert_eq!(integer(zero), integer("0"));
        assert!(integer(zero).is_zero());
        assert!(!integer(zero).is_negative());
    }
    assert!(integer("-1").is_negative());
}
#[test]
fn mathematically_integral_decimals_are_not_css_integer_syntax() {
    assert!(Integer::parse("12", limits()).is_ok());
    for invalid in [
        "1.0", "1.", ".0", "-0.0", "1e2", "0x10", " 1", "1 ", "١", "", "+", "--1", "NaN",
    ] {
        assert_eq!(
            Integer::parse(invalid, limits()),
            Err(NumericError::InvalidNumber),
            "{invalid}"
        );
    }
}
#[test]
fn signed_addition_handles_multi_limb_carry_borrow_and_cancellation() {
    for (left, right, want) in [
        (
            "999999999999999999999999999",
            "1",
            "1000000000000000000000000000",
        ),
        (
            "1000000000000000000000000000",
            "-1",
            "999999999999999999999999999",
        ),
        (
            "-1000000000000000000000000000",
            "1",
            "-999999999999999999999999999",
        ),
        (
            "-999999999999999999999999999",
            "-1",
            "-1000000000000000000000000000",
        ),
        (
            "-999999999999999999999999999",
            "999999999999999999999999999",
            "0",
        ),
        ("-0", "-0", "0"),
        ("5", "-12", "-7"),
        ("-5", "12", "7"),
    ] {
        let a = integer(left);
        let b = integer(right);
        assert_eq!(a.checked_add(&b, limits()).unwrap(), integer(want));
        assert_eq!(b.checked_add(&a, limits()).unwrap(), integer(want));
        assert_eq!(a, integer(left));
        assert_eq!(b, integer(right));
    }
}
#[test]
fn additions_agree_with_independent_native_integer_oracles() {
    for a in [
        i128::from(i64::MIN),
        -1_000_000_001,
        -999_999_999,
        -1,
        0,
        1,
        999_999_999,
        1_000_000_001,
        i128::from(i64::MAX),
    ] {
        for b in [
            i128::from(i64::MIN),
            -1_000_000_001,
            -999_999_999,
            -1,
            0,
            1,
            999_999_999,
            1_000_000_001,
            i128::from(i64::MAX),
        ] {
            assert_eq!(
                integer(&a.to_string())
                    .checked_add(&integer(&b.to_string()), limits())
                    .unwrap(),
                integer(&(a + b).to_string())
            );
        }
    }
}
#[test]
fn integer_limits_reject_exhaustion_without_mutation_or_truncation() {
    let small = NumericLimits {
        digits: NonZeroUsize::new(3).unwrap(),
        ..limits()
    };
    assert!(Integer::parse("999", small).is_ok());
    assert_eq!(Integer::parse("0000", small), Err(NumericError::DigitLimit));
    let left = integer("999");
    let right = integer("1");
    assert_eq!(
        left.checked_add(&right, small),
        Err(NumericError::DigitLimit)
    );
    let no_work = NumericLimits {
        work: NonZeroUsize::new(1).unwrap(),
        ..limits()
    };
    assert_eq!(Integer::parse("123", no_work), Err(NumericError::WorkLimit));
    assert_eq!(
        left.checked_add(&right, no_work),
        Err(NumericError::WorkLimit)
    );
    assert_eq!(left, integer("999"));
    assert_eq!(left.checked_add(&right, limits()).unwrap(), integer("1000"));
}

#[test]
fn digit_limits_cover_whole_limb_carries_and_existing_inputs() {
    for (digits, left, right, expected) in [
        (9, "999999998", "1", Some("999999999")),
        (9, "999999999", "1", None),
        (10, "999999999", "1", Some("1000000000")),
        (18, "999999999999999999", "1", None),
        (19, "999999999999999999", "1", Some("1000000000000000000")),
        (1, "100", "-100", None),
    ] {
        let limited = NumericLimits {
            digits: NonZeroUsize::new(digits).unwrap(),
            ..limits()
        };
        let actual = integer(left).checked_add(&integer(right), limited);
        match expected {
            Some(value) => assert_eq!(actual, Ok(integer(value))),
            None => assert_eq!(actual, Err(NumericError::DigitLimit)),
        }
    }
}
