use iced_css_core::syntax::*;
use std::num::NonZeroUsize;
fn limits() -> StringLimits {
    StringLimits {
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(1_000_000).unwrap(),
    }
}
#[test]
fn retained_eof_probe_success_cases_preserve_values_repairs_and_closure() {
    for (input, text, virtual_close, repairs) in [
        ("'Hello", "Hello", true, 0),
        ("'Hello'", "Hello", false, 0),
        ("'Hello\\", "Hello\\", true, 0),
        ("'Hello\\\\'", "Hello\\", false, 0),
        ("'a\\\nb", "ab", true, 0),
        ("'a\\\r\nb'", "ab", false, 0),
        ("'\\26 B'", "&B", false, 0),
        ("'\\26  B'", "& B", false, 0),
        ("'\\26\r\nB'", "&B", false, 0),
        ("'\\1f600'", "😀", false, 0),
        ("'\\10ffff'", "\u{10ffff}", false, 0),
        ("'\\110000'", "\u{fffd}", false, 1),
        ("'\0'", "\u{fffd}", false, 1),
        ("'\\fdd0'", "\u{fdd0}", false, 0),
    ] {
        let actual = decode_string(input, limits()).unwrap();
        assert_eq!(actual.text, text, "{input:?}");
        assert_eq!(actual.consumed, input.len());
        assert_eq!(
            actual.closure,
            if virtual_close {
                Closure::EndOfInput
            } else {
                Closure::Explicit
            }
        );
        assert_eq!(actual.repairs.len(), repairs);
    }
}
#[test]
fn invalid_escapes_and_newlines_are_not_repaired_into_valid_strings() {
    assert!(decode_string("''", limits()).is_ok());
    for (input, fault, offset) in [
        ("x", SyntaxFault::ExpectedQuote, 0),
        ("'a\nb'", SyntaxFault::UnexpectedNewline, 2),
        ("'a\rb'", SyntaxFault::UnexpectedNewline, 2),
        ("'a\u{c}b'", SyntaxFault::UnexpectedNewline, 2),
        ("'\\0'", SyntaxFault::InvalidNumericEscape(0), 1),
        ("'\\d800'", SyntaxFault::InvalidNumericEscape(0xd800), 1),
        ("'\\dfff'", SyntaxFault::InvalidNumericEscape(0xdfff), 1),
    ] {
        assert_eq!(
            decode_string(input, limits()),
            Err(SyntaxError { offset, fault }),
            "{input:?}"
        );
    }
}
#[test]
fn explicit_close_leaves_following_declarations_unconsumed() {
    let input = "'a\\\\';color:red";
    let value = decode_string(input, limits()).unwrap();
    assert_eq!(value.text, "a\\");
    assert_eq!(&input[value.consumed..], ";color:red");
}
#[test]
fn repair_locations_are_original_decoded_input_byte_ranges() {
    let value = decode_string("'é\0\\110000'", limits()).unwrap();
    assert_eq!(value.text, "é\u{fffd}\u{fffd}");
    assert_eq!(
        value.repairs,
        vec![
            Repair {
                start: 3,
                end: 4,
                kind: RepairKind::LiteralNul
            },
            Repair {
                start: 4,
                end: 11,
                kind: RepairKind::OutOfRangeEscape
            }
        ]
    );
    assert_eq!(decode_string("'é\n'", limits()).unwrap_err().offset, 3);
}
#[test]
fn work_and_retained_capacity_failures_do_not_return_partial_strings() {
    let tight = StringLimits {
        retained_bytes: NonZeroUsize::new(2).unwrap(),
        ..limits()
    };
    assert_eq!(decode_string("'ab'", tight).unwrap().text, "ab");
    assert!(matches!(
        decode_string("'abc'", tight),
        Err(SyntaxError {
            fault: SyntaxFault::Resource(SyntaxResource::Bytes),
            ..
        })
    ));
    let work = StringLimits {
        work: NonZeroUsize::new(1).unwrap(),
        ..limits()
    };
    assert!(matches!(
        decode_string("'a'", work),
        Err(SyntaxError {
            fault: SyntaxFault::Resource(SyntaxResource::Work),
            ..
        })
    ));
    assert!(matches!(
        decode_string("'\0'", tight),
        Err(SyntaxError {
            fault: SyntaxFault::Resource(SyntaxResource::Bytes),
            ..
        })
    ));
}

#[test]
fn escape_terminators_and_six_digit_limit_preserve_following_characters() {
    for (input, expected) in [
        ("\"a'b\"", "a'b"),
        ("'\\000026B'", "&B"),
        ("'\\26\t B'", "& B"),
        ("'a\\\u{c}b'", "ab"),
        ("'a\\\rb'", "ab"),
        ("'\\\\n'", "\\n"),
        ("'\\\"'", "\""),
        ("'\\é'", "é"),
        ("'\\110000\r\nB'", "\u{fffd}B"),
    ] {
        assert_eq!(
            decode_string(input, limits()).unwrap().text,
            expected,
            "{input:?}"
        );
    }
    let escaped_nul = decode_string("'\\\0'", limits()).unwrap();
    assert_eq!(
        escaped_nul.repairs,
        vec![Repair {
            start: 2,
            end: 3,
            kind: RepairKind::LiteralNul
        }]
    );
    let out_of_range = decode_string("'\\110000\r\nB'", limits()).unwrap();
    assert_eq!(
        out_of_range.repairs,
        vec![Repair {
            start: 1,
            end: 8,
            kind: RepairKind::OutOfRangeEscape
        }]
    );
}

#[test]
fn all_small_budgets_are_obeyed_for_growth_and_repair_storage() {
    for input in ["''", "'abcdefghijk'", "'é😀é'", "'\0\0\0'", "'\\110000\0a'"] {
        for bytes in 1..256 {
            let budget = StringLimits {
                retained_bytes: NonZeroUsize::new(bytes).unwrap(),
                ..limits()
            };
            match decode_string(input, budget) {
                Ok(value) => assert!(
                    value.text.capacity() + value.repairs.capacity() * size_of::<Repair>() <= bytes
                ),
                Err(error) => assert!(matches!(
                    error.fault,
                    SyntaxFault::Resource(SyntaxResource::Bytes)
                )),
            }
        }
    }
}
