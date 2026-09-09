use iced_css_core::text::{
    casing_contexts::*, casing_data::*, casing_output::*, casing_selection::*, newlines::*,
};
use std::{num::NonZeroUsize, ops::Range};
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> CasingOutputLimits {
    CasingOutputLimits {
        scalars: nz(usize::MAX),
        output_bytes: nz(usize::MAX),
        retained_bytes: nz(usize::MAX),
        work: nz(usize::MAX),
    }
}
fn normalized(s: &str) -> NormalizedText<'_> {
    normalize_default_newlines(
        s,
        NewlineLimits {
            input_bytes: nz(100000),
            retained_bytes: nz(1000000),
            work: nz(1000000),
        },
    )
    .unwrap()
}
fn contexts<'a, 'b>(t: &'a NormalizedText<'b>) -> CasingContexts<'a, 'b> {
    evaluate_casing_contexts(
        t,
        CasingContextLimits {
            scalars: nz(100000),
            retained_bytes: nz(1000000),
            work: nz(1000000),
        },
    )
    .unwrap()
}
fn selected<'a, 'b, 'c>(
    c: &'a CasingContexts<'b, 'c>,
    op: CasingOperation,
    locale: Option<CasingLocale>,
) -> CasingSelection<'a, 'b, 'c> {
    select_case_mappings(
        c,
        op,
        locale,
        CasingSelectionLimits {
            scalars: nz(100000),
            retained_bytes: nz(1000000),
            work: nz(1000000),
        },
    )
    .unwrap()
}
#[test]
fn expansion_and_multibyte_identity_have_complete_byte_origins() {
    let text = normalized("ßﬃ漢😀a");
    let ctx = contexts(&text);
    let selection = selected(&ctx, CasingOperation::Upper, None);
    let got = materialize_case_mappings(&selection, limits()).unwrap();
    assert_eq!(got.output(), "SSFFI漢😀A");
    assert_eq!(got.output_scalar_count(), 8);
    assert_eq!(got.work(), 27);
    for (i, r) in [0..2, 2..5, 5..8, 8..12, 12..13].iter().enumerate() {
        assert_eq!(got.range(i), Some(r));
        for byte in r.clone() {
            assert_eq!(got.source_scalar_at_output_byte(byte), Some(i));
        }
    }
    assert_eq!(got.source_scalar_at_output_byte(13), None);
    assert_eq!(got.source_scalar_at_output_byte(usize::MAX), None);
    assert_eq!(got.range(5), None);
    assert_eq!(got.range(usize::MAX), None);
}
fn check_ranges(got: &CasingOutput<'_, '_, '_, '_>, expected: &[Range<usize>]) {
    assert_eq!(got.scalar_count(), expected.len());
    for (i, r) in expected.iter().enumerate() {
        assert_eq!(got.range(i), Some(r));
        for byte in r.clone() {
            assert_eq!(got.source_scalar_at_output_byte(byte), Some(i));
        }
    }
    assert_eq!(got.source_scalar_at_output_byte(got.output().len()), None);
    assert_eq!(got.range(expected.len()), None);
}
#[test]
fn exact_static_expansions_deletions_and_conditional_identity() {
    use CasingLocale::*;
    use CasingOperation::*;
    for (source, op, locale, output, u, ranges) in [
        (
            "İ",
            Lower,
            None,
            "i\u{307}",
            2,
            std::iter::once(0..3).collect(),
        ),
        (
            "I\u{307}x",
            Lower,
            Some(Turkish),
            "ix",
            2,
            vec![0..1, 1..1, 1..2],
        ),
        ("I\u{307}", Lower, Some(Azeri), "i", 1, vec![0..1, 1..1]),
        (
            "i\u{307}\u{307}",
            Upper,
            Some(Lithuanian),
            "I\u{307}",
            2,
            vec![0..1, 1..1, 1..3],
        ),
        (
            "I\u{301}",
            Lower,
            Some(Lithuanian),
            "i\u{307}\u{301}",
            3,
            vec![0..3, 3..5],
        ),
        (
            "i",
            Lower,
            Some(Turkish),
            "i",
            1,
            std::iter::once(0..1).collect(),
        ),
        ("ΑΣ", Lower, None, "ας", 2, vec![0..2, 2..4]),
    ] {
        let text = normalized(source);
        let ctx = contexts(&text);
        let selection = selected(&ctx, op, locale);
        let got = materialize_case_mappings(&selection, limits()).unwrap();
        assert_eq!(got.output(), output, "{source:?} {op:?} {locale:?}");
        assert_eq!(got.output_scalar_count(), u);
        check_ranges(&got, &ranges);
        assert_eq!(got.work(), 1 + 2 * selection.scalar_count() + 2 * u);
        assert_eq!(got.selection().operation(), op);
        assert_eq!(got.selection().locale(), locale);
    }
    let text = normalized("i");
    let ctx = contexts(&text);
    let selection = selected(&ctx, Lower, Some(Turkish));
    let got = materialize_case_mappings(&selection, limits()).unwrap();
    assert_eq!(
        got.selection().mapping(0),
        Some(CaseMapping::Replacement(&['i']))
    );
    let Some(CasingSelectionOrigin::Conditional(row)) = got.selection().origin(0) else {
        panic!("conditional identity lost")
    };
    assert!(
        conditional_mappings('i')
            .iter()
            .any(|candidate| std::ptr::eq(candidate, row))
    );
}
#[test]
fn original_chain_and_crlf_provenance_are_unchanged_after_success_and_rejection() {
    let source = String::from("ß\r\nI\u{307}😀");
    let text = normalized(&source);
    let ctx = contexts(&text);
    let selection = selected(&ctx, CasingOperation::Lower, Some(CasingLocale::Turkish));
    let before: Vec<_> = (0..selection.scalar_count())
        .map(|i| (selection.mapping(i), selection.origin(i)))
        .collect();
    let got = materialize_case_mappings(&selection, limits()).unwrap();
    assert!(std::ptr::eq(got.selection(), &selection));
    assert!(std::ptr::eq(got.selection().contexts(), &ctx));
    assert!(std::ptr::eq(got.text(), &text));
    assert_eq!(got.output(), "ß\ni😀");
    check_ranges(&got, &[0..2, 2..3, 3..4, 4..4, 4..8]);
    assert_eq!(got.text().source(), source);
    assert_eq!(got.text().text(), "ß\nI\u{307}😀");
    assert_eq!(got.text().scalars()[1].source, 2..4);
    assert_eq!(got.text().scalars()[1].normalized, 2..3);
    assert_eq!(
        materialize_case_mappings(
            &selection,
            CasingOutputLimits {
                output_bytes: nz(1),
                ..limits()
            }
        )
        .unwrap_err(),
        CasingOutputError::OutputBytes
    );
    assert_eq!(
        materialize_case_mappings(
            &selection,
            CasingOutputLimits {
                retained_bytes: nz(1),
                ..limits()
            }
        )
        .unwrap_err(),
        CasingOutputError::Bytes
    );
    assert_eq!(got.output(), "ß\ni😀");
    for (i, expected) in before.into_iter().enumerate() {
        assert_eq!((selection.mapping(i), selection.origin(i)), expected);
    }
    assert_eq!(text.source(), source);
}
#[test]
fn exact_dynamic_work_bytes_count_and_empty_limits() {
    for (source, op, locale, work, output_bytes) in [
        ("漢😀", CasingOperation::Upper, None, 9, 7),
        (
            "I\u{307}",
            CasingOperation::Lower,
            Some(CasingLocale::Turkish),
            7,
            1,
        ),
        ("ßﬃ", CasingOperation::Upper, None, 15, 5),
    ] {
        let text = normalized(source);
        let ctx = contexts(&text);
        let selection = selected(&ctx, op, locale);
        let initial = materialize_case_mappings(&selection, limits()).unwrap();
        let exact = CasingOutputLimits {
            scalars: nz(2),
            output_bytes: nz(output_bytes),
            retained_bytes: nz(initial.retained_bytes()),
            work: nz(work),
        };
        let got = materialize_case_mappings(&selection, exact).unwrap();
        assert_eq!(got.work(), work);
        for (low, error) in [
            (
                CasingOutputLimits {
                    scalars: nz(1),
                    ..exact
                },
                CasingOutputError::Scalars,
            ),
            (
                CasingOutputLimits {
                    work: nz(work - 1),
                    ..exact
                },
                CasingOutputError::Work,
            ),
            (
                CasingOutputLimits {
                    work: nz(4),
                    ..exact
                },
                CasingOutputError::Work,
            ),
            (
                CasingOutputLimits {
                    retained_bytes: nz(initial.retained_bytes() - 1),
                    ..exact
                },
                CasingOutputError::Bytes,
            ),
        ] {
            assert_eq!(
                materialize_case_mappings(&selection, low).unwrap_err(),
                error
            );
        }
        if output_bytes > 1 {
            assert_eq!(
                materialize_case_mappings(
                    &selection,
                    CasingOutputLimits {
                        output_bytes: nz(output_bytes - 1),
                        ..exact
                    }
                )
                .unwrap_err(),
                CasingOutputError::OutputBytes
            );
        }
    }
    let text = normalized("");
    let ctx = contexts(&text);
    for op in [CasingOperation::Lower, CasingOperation::Upper] {
        for locale in [
            None,
            Some(CasingLocale::Turkish),
            Some(CasingLocale::Azeri),
            Some(CasingLocale::Lithuanian),
        ] {
            let selection = selected(&ctx, op, locale);
            let got = materialize_case_mappings(
                &selection,
                CasingOutputLimits {
                    scalars: nz(1),
                    output_bytes: nz(1),
                    retained_bytes: nz(1),
                    work: nz(1),
                },
            )
            .unwrap();
            assert_eq!(
                (
                    got.scalar_count(),
                    got.output_scalar_count(),
                    got.retained_bytes(),
                    got.work()
                ),
                (0, 0, 0, 1)
            );
            assert_eq!(got.output(), "");
            assert_eq!(got.range(0), None);
            assert_eq!(got.source_scalar_at_output_byte(0), None);
        }
    }
}
