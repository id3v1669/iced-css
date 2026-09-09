use iced_css_core::values::{Decimal, LengthTarget, NumericError, NumericLimits};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(1_000_000).unwrap(),
    }
}
fn length(s: &str) -> LengthTarget {
    LengthTarget::parse(s, limits()).unwrap()
}
#[test]
fn absolute_units_have_exact_common_identity() {
    for equivalent in [
        "1in",
        "2.54cm",
        "25.4mm",
        "72pt",
        "6pc",
        "96px",
        "00096.000PX",
    ] {
        assert_eq!(length(equivalent), length("96px"), "{equivalent}");
    }
    assert_eq!(length(".75pt"), length("1px"));
    assert_ne!(length("1cm"), length("38px"));
}
#[test]
fn relative_percent_and_keyword_targets_remain_distinct() {
    assert_ne!(length("100%"), length("100px"));
    assert_ne!(length("1em"), length("1ex"));
    assert_ne!(length("auto"), length("0px"));
    assert_eq!(length("+00.000"), length("0px"));
    assert_eq!(length("-0cm"), length("0px"));
}
#[test]
fn decimals_do_not_lose_digits_to_float_rounding() {
    assert_ne!(length("16777216px"), length("16777217px"));
    let huge = "123456789012345678901234567890123456789012345678901234567890";
    assert_eq!(
        length(&format!("{huge}px")),
        length(&format!("{huge}.000px"))
    );
    assert_eq!(
        length("100000000000000000000000000000000000000000000000000000000000in"),
        length("9600000000000000000000000000000000000000000000000000000000000px")
    );
}
#[test]
fn sign_is_validated_exactly_before_any_future_rounding() {
    let tiny = Decimal::parse(
        "-.000000000000000000000000000000000000000000000000000000000001",
        limits(),
    )
    .unwrap();
    assert!(tiny.is_negative());
    assert!(Decimal::parse("-0", limits()).unwrap().authored_negative());
    assert!(!Decimal::parse("-0", limits()).unwrap().is_negative());
    assert_eq!(
        Decimal::parse("-0", limits()).unwrap(),
        Decimal::parse("0.000", limits()).unwrap()
    );
}
#[test]
fn unsupported_number_grammar_and_units_are_rejected() {
    for invalid in [
        "", "+", "-", ".", "1.", "1e2", "NaN", "inf", " 1", "1 ", "--1", "1.2.3", "١",
    ] {
        assert!(Decimal::parse(invalid, limits()).is_err(), "{invalid}");
    }
    for invalid in ["1rem", "1vh", "1e2px", "1", "1é", "1 px"] {
        assert!(LengthTarget::parse(invalid, limits()).is_err(), "{invalid}");
    }
}
#[test]
fn explicit_budgets_reject_instead_of_truncating() {
    let short = NumericLimits {
        digits: NonZeroUsize::new(3).unwrap(),
        ..limits()
    };
    assert!(Decimal::parse("123", short).is_ok());
    assert_eq!(Decimal::parse("1234", short), Err(NumericError::DigitLimit));
    let work = NumericLimits {
        work: NonZeroUsize::new(1).unwrap(),
        ..limits()
    };
    assert_eq!(Decimal::parse("123", work), Err(NumericError::WorkLimit));
}

#[test]
fn limb_carries_and_reduction_agree_with_independent_integer_arithmetic() {
    for n in [
        0i128,
        1,
        -1,
        123,
        999_999_999,
        1_000_000_001,
        i128::from(i64::MAX),
    ] {
        assert_eq!(length(&format!("{n}in")), length(&format!("{}px", n * 96)));
        assert_eq!(
            length(&format!("{}cm", n * 254)),
            length(&format!("{}px", n * 9600))
        );
        assert_eq!(
            length(&format!("{}pt", n * 3)),
            length(&format!("{}px", n * 4))
        );
    }
}
#[test]
fn rational_conversion_charges_work_after_decimal_admission() {
    let budget = NumericLimits {
        work: NonZeroUsize::new(3).unwrap(),
        ..limits()
    };
    assert!(Decimal::parse("1", budget).is_ok());
    assert_eq!(
        LengthTarget::parse("1px", budget),
        Err(NumericError::WorkLimit)
    );
}
