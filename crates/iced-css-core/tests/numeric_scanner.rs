use iced_css_core::syntax::*;
use std::num::NonZeroUsize;
fn scan(input: &str) -> Option<NumericToken> {
    scan_numeric_prefix(input, NonZeroUsize::new(10000).unwrap()).unwrap()
}
#[test]
fn retained_numeric_competition_uses_longest_match_then_table_priority() {
    use NumericTokenKind::*;
    for (input, kind, consumed) in [
        ("1e2", Number, 3),
        ("1E+2", Number, 4),
        ("1e-2", Number, 4),
        ("1e2px", Dimension, 5),
        ("1em", Dimension, 3),
        (".5", Number, 2),
        ("-0", Number, 2),
        ("1.", Number, 1),
    ] {
        assert_eq!(
            scan(input),
            Some(NumericToken { kind, consumed }),
            "{input:?}"
        );
    }
}
#[test]
fn percentage_and_ambiguous_dimension_suffixes_preserve_raw_spelling() {
    use NumericTokenKind::*;
    for (input, kind, consumed) in [
        ("1e2%", Percentage, 4),
        ("1e+2px;", Dimension, 6),
        ("1e+px", Dimension, 2),
        ("1e-2px", Dimension, 6),
        ("1-foo", Dimension, 5),
        ("1--foo", Number, 1),
        ("1.2.3", Number, 3),
        ("+1.5%foo", Percentage, 5),
        ("1e2e3", Dimension, 5),
        ("12\\70 x", Dimension, 7),
    ] {
        assert_eq!(
            scan(input),
            Some(NumericToken { kind, consumed }),
            "{input:?}"
        );
    }
}
#[test]
fn identifier_suffix_boundaries_follow_css22_not_css3() {
    use NumericTokenKind::*;
    for (input, kind, consumed) in [
        ("1\u{7f}", Number, 1),
        ("1\u{80}", Dimension, 3),
        ("1\0", Dimension, 2),
        ("1\\80", Dimension, 4),
        ("1\\0", Dimension, 3),
        ("1\\d800", Dimension, 6),
        ("1\\\n", Number, 1),
        ("1\\", Number, 1),
        ("1_", Dimension, 2),
    ] {
        assert_eq!(
            scan(input),
            Some(NumericToken { kind, consumed }),
            "{input:?}"
        );
    }
    for input in ["", "+", "-", ".", "-.a", "--1", "é", "NaN", "inf"] {
        assert_eq!(scan(input), None, "{input:?}");
    }
}
#[test]
fn generic_number_acceptance_does_not_rewrite_property_decimal_spelling() {
    use iced_css_core::values::{Decimal, NumericLimits};
    let limits = NumericLimits {
        digits: NonZeroUsize::new(100).unwrap(),
        work: NonZeroUsize::new(10000).unwrap(),
    };
    for input in ["1e2", "1E+2", "1e-2"] {
        assert_eq!(scan(input).unwrap().kind, NumericTokenKind::Number);
        assert!(Decimal::parse(input, limits).is_err());
    }
}
#[test]
fn exhaustion_is_a_resource_error_not_a_shorter_token() {
    assert!(matches!(
        scan_numeric_prefix("123456", NonZeroUsize::new(1).unwrap()),
        Err(SyntaxError {
            fault: SyntaxFault::Resource(SyntaxResource::Work),
            ..
        })
    ));
}
