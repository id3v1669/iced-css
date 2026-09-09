use iced_css_core::values::{
    CounterError, CounterLimits, CounterStyle, Integer, NumericLimits, format_counter,
    format_counters,
};
use std::num::NonZeroUsize;
fn limits() -> CounterLimits {
    CounterLimits {
        numeric: NumericLimits {
            digits: NonZeroUsize::new(1000).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
    }
}
fn number(text: &str) -> Integer {
    Integer::parse(text, limits().numeric).unwrap()
}
fn formatted(value: &str, style: &str) -> String {
    format_counter(
        &number(value),
        CounterStyle::parse(style).unwrap(),
        limits(),
    )
    .unwrap()
}

#[test]
fn decimal_sign_padding_and_large_fallback_preserve_exact_integers() {
    for (value, expected) in [
        ("-1", "-01"),
        ("0", "00"),
        ("1", "01"),
        ("100", "100"),
        ("-0", "00"),
    ] {
        assert_eq!(formatted(value, "decimal-leading-zero"), expected);
    }
    let huge = "-123456789012345678901234567890000000001";
    for style in [
        "decimal",
        "lower-roman",
        "upper-roman",
        "lower-latin",
        "upper-latin",
        "armenian",
        "georgian",
        "lower-greek",
    ] {
        assert_eq!(formatted(huge, style), huge);
    }
    assert_eq!(formatted("1000000000", "decimal"), "1000000000");
}
#[test]
fn finite_alphabets_and_aliases_use_decimal_outside_the_first_alphabet() {
    for style in ["lower-latin", "lower-alpha"] {
        assert_eq!(formatted("1", style), "a");
        assert_eq!(formatted("26", style), "z");
        assert_eq!(formatted("27", style), "27");
    }
    for style in ["upper-latin", "upper-alpha"] {
        assert_eq!(formatted("1", style), "A");
        assert_eq!(formatted("26", style), "Z");
        assert_eq!(formatted("0", style), "0");
    }
    for (value, expected) in [
        ("17", "ρ"),
        ("18", "σ"),
        ("24", "ω"),
        ("25", "25"),
        ("-1", "-1"),
    ] {
        assert_eq!(formatted(value, "lower-greek"), expected);
    }
    assert!(CounterStyle::parse("lower-hexadecimal").is_none());
    assert_eq!(
        CounterStyle::parse("LOWER-ALPHA"),
        CounterStyle::parse("lower-latin")
    );
}
#[test]
fn additive_styles_cover_subtractive_pairs_and_historic_symbols() {
    for (value, expected) in [
        ("4", "IV"),
        ("9", "IX"),
        ("40", "XL"),
        ("90", "XC"),
        ("400", "CD"),
        ("900", "CM"),
        ("3999", "MMMCMXCIX"),
        ("4000", "4000"),
    ] {
        assert_eq!(formatted(value, "upper-roman"), expected);
        assert_eq!(
            formatted(value, "lower-roman"),
            expected.to_ascii_lowercase()
        );
    }
    for (value, expected) in [
        ("1", "Ա"),
        ("1000", "Ռ"),
        ("9999", "ՔՋՂԹ"),
        ("10000", "10000"),
    ] {
        assert_eq!(formatted(value, "armenian"), expected);
    }
    for (value, expected) in [
        ("8", "ჱ"),
        ("60", "ჲ"),
        ("400", "ჳ"),
        ("7000", "ჴ"),
        ("10000", "ჵ"),
        ("19999", "ჵჰშჟთ"),
        ("20000", "20000"),
    ] {
        assert_eq!(formatted(value, "georgian"), expected);
    }
}
#[test]
fn symbols_none_and_scope_separators_do_not_gain_marker_punctuation() {
    for (style, expected) in [
        ("disc", "•"),
        ("circle", "◦"),
        ("square", "▪"),
        ("none", ""),
    ] {
        for value in ["-1", "0", "1", "99999999999999999999"] {
            assert_eq!(formatted(value, style), expected);
        }
    }
    let values = [number("1"), number("2"), number("3")];
    assert_eq!(
        format_counters(
            &values,
            " / ",
            CounterStyle::parse("decimal").unwrap(),
            limits()
        )
        .unwrap(),
        "1 / 2 / 3"
    );
    assert_eq!(
        format_counters(&values, ".", CounterStyle::parse("none").unwrap(), limits()).unwrap(),
        ".."
    );
}
#[test]
fn output_and_work_limits_reject_without_truncation_or_fallback() {
    let mut small = limits();
    small.retained_bytes = NonZeroUsize::new(2).unwrap();
    assert_eq!(
        format_counter(
            &number("3999"),
            CounterStyle::parse("upper-roman").unwrap(),
            small
        ),
        Err(CounterError::Bytes)
    );
    small = limits();
    small.numeric.work = NonZeroUsize::MIN;
    assert!(
        format_counter(
            &number("3999"),
            CounterStyle::parse("upper-roman").unwrap(),
            small
        )
        .is_err()
    );
    small = limits();
    small.numeric.digits = NonZeroUsize::MIN;
    assert!(
        format_counter(
            &number("123"),
            CounterStyle::parse("decimal").unwrap(),
            small
        )
        .is_err()
    );
}
