use iced_css_core::syntax::*;
use std::num::NonZeroUsize;
fn limits() -> StringLimits {
    StringLimits {
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(10000).unwrap(),
    }
}
#[test]
fn escaped_names_preserve_case_and_are_not_retokenized() {
    for (input, expected) in [
        (r"\63 olor", "color"),
        (r"-\66 oo", "-foo"),
        (r"\31 23", "123"),
        (r"a\;b", "a;b"),
        (r"\52 ED", "RED"),
        ("é", "é"),
        ("\u{80}", "\u{80}"),
        (r"\000026B", "&B"),
    ] {
        assert_eq!(
            decode_identifier(input, limits()).unwrap().text,
            expected,
            "{input:?}"
        );
    }
}
#[test]
fn whole_css22_identifier_grammar_must_match_before_semantic_decode() {
    for input in [
        "", "1foo", "--foo", "-", "a b", "a\\", "a\\\n", "\u{7f}", "a(",
    ] {
        assert!(decode_identifier(input, limits()).is_err(), "{input:?}");
    }
}
#[test]
fn scalar_repairs_and_invalid_escapes_keep_original_offsets() {
    let value = decode_identifier("a\0\\110000", limits()).unwrap();
    assert_eq!(value.text, "a\u{fffd}\u{fffd}");
    assert_eq!(
        value.repairs,
        vec![
            Repair {
                start: 1,
                end: 2,
                kind: RepairKind::LiteralNul
            },
            Repair {
                start: 2,
                end: 9,
                kind: RepairKind::OutOfRangeEscape
            }
        ]
    );
    for input in [r"a\0", r"a\d800"] {
        assert!(matches!(
            decode_identifier(input, limits()),
            Err(SyntaxError {
                offset: 1,
                fault: SyntaxFault::InvalidNumericEscape(_)
            })
        ));
    }
}
#[test]
fn work_and_output_capacity_remain_bounded() {
    let budget = StringLimits {
        retained_bytes: NonZeroUsize::MIN,
        ..limits()
    };
    assert_eq!(decode_identifier("a", budget).unwrap().text, "a");
    assert!(decode_identifier("ab", budget).is_err());
    assert!(
        decode_identifier(
            "abc",
            StringLimits {
                work: NonZeroUsize::MIN,
                ..limits()
            }
        )
        .is_err()
    );
}
