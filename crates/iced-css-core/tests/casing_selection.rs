use iced_css_core::text::{casing_contexts::*, casing_data::*, casing_selection::*, newlines::*};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> CasingSelectionLimits {
    CasingSelectionLimits {
        scalars: nz(1000),
        retained_bytes: nz(100000),
        work: nz(100000),
    }
}
fn normalized(s: &str) -> NormalizedText<'_> {
    normalize_default_newlines(
        s,
        NewlineLimits {
            input_bytes: nz(10000),
            retained_bytes: nz(100000),
            work: nz(100000),
        },
    )
    .unwrap()
}
fn contexts<'a, 'b>(s: &'a NormalizedText<'b>) -> CasingContexts<'a, 'b> {
    evaluate_casing_contexts(
        s,
        CasingContextLimits {
            scalars: nz(10000),
            retained_bytes: nz(100000),
            work: nz(100000),
        },
    )
    .unwrap()
}
#[test]
fn base_values_and_none_locale_sigma_keep_different_origins() {
    let text = normalized("ßﬃİ漢ΑΣ😀");
    let ctx = contexts(&text);
    let upper = select_case_mappings(&ctx, CasingOperation::Upper, None, limits()).unwrap();
    assert_eq!(
        upper.mapping(0),
        Some(CaseMapping::Replacement(&['S', 'S']))
    );
    assert_eq!(
        upper.mapping(1),
        Some(CaseMapping::Replacement(&['F', 'F', 'I']))
    );
    assert_eq!(upper.mapping(3), Some(CaseMapping::Identity));
    assert_eq!(upper.origin(3), Some(CasingSelectionOrigin::Base));
    assert_eq!(upper.mapping(6), Some(CaseMapping::Identity));
    assert_eq!(upper.origin(6), Some(CasingSelectionOrigin::Base));
    let Some(CasingSelectionOrigin::Conditional(row)) = upper.origin(5) else {
        panic!()
    };
    assert!(std::ptr::eq(row, &conditional_mappings('Σ')[0]));
    assert_eq!(upper.mapping(5), Some(CaseMapping::Replacement(&['Σ'])));
    let lower = select_case_mappings(&ctx, CasingOperation::Lower, None, limits()).unwrap();
    assert_eq!(
        lower.mapping(2),
        Some(CaseMapping::Replacement(&['i', '\u{307}']))
    );
    assert_eq!(lower.mapping(5), Some(CaseMapping::Replacement(&['ς'])));
    assert_eq!(lower.mapping(6), Some(CaseMapping::Identity));
    assert_eq!(lower.origin(6), Some(CasingSelectionOrigin::Base));
    let text = normalized("ΑΣΑ");
    let ctx = contexts(&text);
    assert_eq!(
        select_case_mappings(&ctx, CasingOperation::Lower, None, limits())
            .unwrap()
            .origin(1),
        Some(CasingSelectionOrigin::Base)
    );
}
#[test]
fn every_conditional_row_has_real_context_and_exact_static_provenance() {
    use CasingLocale::*;
    type ConditionalCase = (
        &'static str,
        usize,
        Option<CasingLocale>,
        &'static str,
        &'static [char],
        &'static [char],
    );
    let cases: &[ConditionalCase] = &[
        ("ΑΣ", 1, None, "Final_Sigma", &['ς'], &['Σ']),
        (
            "i\u{307}",
            1,
            Some(Lithuanian),
            "lt After_Soft_Dotted",
            &['\u{307}'],
            &[],
        ),
        (
            "I\u{301}",
            0,
            Some(Lithuanian),
            "lt More_Above",
            &['i', '\u{307}'],
            &['I'],
        ),
        (
            "J\u{301}",
            0,
            Some(Lithuanian),
            "lt More_Above",
            &['j', '\u{307}'],
            &['J'],
        ),
        (
            "Į\u{301}",
            0,
            Some(Lithuanian),
            "lt More_Above",
            &['į', '\u{307}'],
            &['Į'],
        ),
        (
            "Ì",
            0,
            Some(Lithuanian),
            "lt",
            &['i', '\u{307}', '\u{300}'],
            &['Ì'],
        ),
        (
            "Í",
            0,
            Some(Lithuanian),
            "lt",
            &['i', '\u{307}', '\u{301}'],
            &['Í'],
        ),
        (
            "Ĩ",
            0,
            Some(Lithuanian),
            "lt",
            &['i', '\u{307}', '\u{303}'],
            &['Ĩ'],
        ),
        ("İ", 0, Some(Turkish), "tr", &['i'], &['İ']),
        ("İ", 0, Some(Azeri), "az", &['i'], &['İ']),
        (
            "I\u{307}",
            1,
            Some(Turkish),
            "tr After_I",
            &[],
            &['\u{307}'],
        ),
        ("I\u{307}", 1, Some(Azeri), "az After_I", &[], &['\u{307}']),
        ("I", 0, Some(Turkish), "tr Not_Before_Dot", &['ı'], &['I']),
        ("I", 0, Some(Azeri), "az Not_Before_Dot", &['ı'], &['I']),
        ("i", 0, Some(Turkish), "tr", &['i'], &['İ']),
        ("i", 0, Some(Azeri), "az", &['i'], &['İ']),
    ];
    for &(s, index, locale, expression, lower, upper) in cases {
        let text = normalized(s);
        let ctx = contexts(&text);
        let scalar = text.text().chars().nth(index).unwrap();
        let expected = conditional_mappings(scalar)
            .iter()
            .find(|r| r.expression() == expression)
            .unwrap();
        for (operation, mapping) in [
            (CasingOperation::Lower, lower),
            (CasingOperation::Upper, upper),
        ] {
            let got = select_case_mappings(&ctx, operation, locale, limits()).unwrap();
            assert_eq!(
                got.mapping(index),
                Some(CaseMapping::Replacement(mapping)),
                "{s:?} {expression}"
            );
            let Some(CasingSelectionOrigin::Conditional(row)) = got.origin(index) else {
                panic!("{expression}")
            };
            assert!(std::ptr::eq(row, expected));
            assert_eq!(got.locale(), locale);
            assert_eq!(got.operation(), operation);
        }
    }
}
#[test]
fn negation_and_deletion_read_unchanged_original_context() {
    for locale in [CasingLocale::Turkish, CasingLocale::Azeri] {
        for (s, before) in [
            ("I\u{323}\u{307}", true),
            ("I\u{301}\u{307}", false),
            ("IA\u{307}", false),
        ] {
            let text = normalized(s);
            let ctx = contexts(&text);
            let got =
                select_case_mappings(&ctx, CasingOperation::Lower, Some(locale), limits()).unwrap();
            assert_eq!(
                got.mapping(0),
                Some(if before {
                    CaseMapping::Replacement(&['i'])
                } else {
                    CaseMapping::Replacement(&['ı'])
                })
            );
            assert_eq!(
                got.mapping(2),
                Some(if before {
                    CaseMapping::Replacement(&[])
                } else {
                    CaseMapping::Identity
                })
            );
            assert_eq!(ctx.matches(0, CasingContext::BeforeDot), Some(before));
            assert_eq!(text.source(), s);
        }
    }
}
#[test]
fn owner_ranges_checked_indices_empty_and_exact_limits() {
    let text = normalized("A\r\nΣ");
    let ctx = contexts(&text);
    let got = select_case_mappings(&ctx, CasingOperation::Lower, None, limits()).unwrap();
    assert!(std::ptr::eq(got.contexts(), &ctx));
    assert!(std::ptr::eq(got.text(), &text));
    assert_eq!(got.text().scalars()[1].source, 1..3);
    assert_eq!(got.scalar_count(), 3);
    assert_eq!(got.work(), 4);
    for index in [3, usize::MAX] {
        assert_eq!(got.mapping(index), None);
        assert_eq!(got.origin(index), None);
    }
    let mut cap = limits();
    cap.scalars = nz(3);
    cap.work = nz(4);
    cap.retained_bytes = nz(got.retained_bytes());
    assert!(select_case_mappings(&ctx, CasingOperation::Lower, None, cap).is_ok());
    let mut bad = cap;
    bad.work = nz(3);
    assert!(matches!(
        select_case_mappings(&ctx, CasingOperation::Lower, None, bad),
        Err(CasingSelectionError::Work)
    ));
    bad = cap;
    bad.scalars = nz(2);
    assert!(matches!(
        select_case_mappings(&ctx, CasingOperation::Lower, None, bad),
        Err(CasingSelectionError::Scalars)
    ));
    bad = cap;
    bad.retained_bytes = nz(got.retained_bytes() - 1);
    assert!(matches!(
        select_case_mappings(&ctx, CasingOperation::Lower, None, bad),
        Err(CasingSelectionError::Bytes)
    ));
    cap.retained_bytes = NonZeroUsize::MAX;
    assert!(select_case_mappings(&ctx, CasingOperation::Lower, None, cap).is_ok());
    let empty = normalized("");
    let ctx = contexts(&empty);
    for op in [CasingOperation::Lower, CasingOperation::Upper] {
        for locale in [
            None,
            Some(CasingLocale::Lithuanian),
            Some(CasingLocale::Turkish),
            Some(CasingLocale::Azeri),
        ] {
            let got = select_case_mappings(
                &ctx,
                op,
                locale,
                CasingSelectionLimits {
                    scalars: nz(1),
                    work: nz(1),
                    retained_bytes: nz(1),
                },
            )
            .unwrap();
            assert_eq!(
                (got.work(), got.retained_bytes(), got.scalar_count()),
                (1, 0, 0)
            );
            assert_eq!(got.mapping(0), None);
        }
    }
}
#[test]
fn published_conditional_groups_are_unique_for_all_1280_flag_cases() {
    let scalars = ['I', 'J', 'i', 'Ì', 'Í', 'Ĩ', 'Į', 'İ', '\u{307}', 'Σ'];
    let contexts = [
        CasingContext::FinalSigma,
        CasingContext::AfterSoftDotted,
        CasingContext::MoreAbove,
        CasingContext::BeforeDot,
        CasingContext::AfterI,
    ];
    let mut matched = 0;
    for c in scalars {
        for locale in [
            None,
            Some(CasingLocale::Lithuanian),
            Some(CasingLocale::Turkish),
            Some(CasingLocale::Azeri),
        ] {
            for bits in 0..32 {
                let count = conditional_mappings(c)
                    .iter()
                    .filter(|row| {
                        row.conditions().iter().all(|condition| match condition {
                            CasingCondition::Locale(wanted) => locale == Some(*wanted),
                            CasingCondition::Context { context, negated } => {
                                ((bits
                                    & (1 << contexts.iter().position(|c| c == context).unwrap()))
                                    != 0)
                                    != *negated
                            }
                        })
                    })
                    .count();
                assert!(count <= 1);
                matched += count;
            }
        }
    }
    assert_eq!(matched, 416);
}

#[test]
fn bounded_real_context_dispatch_matches_raw_expression_oracle() {
    // This oracle parses retained expressions, not the production typed-condition
    // matcher. Context computation is the separately qualified dependency.
    let alphabet = [
        'I', 'i', '\u{307}', '\u{323}', '\u{301}', 'A', 'Σ', 'Α', ' ',
    ];
    for len in 0..=3_u32 {
        for mut code in 0..alphabet.len().pow(len) {
            let source: String = (0..len)
                .map(|_| {
                    let c = alphabet[code % alphabet.len()];
                    code /= alphabet.len();
                    c
                })
                .collect();
            let text = normalized(&source);
            let ctx = contexts(&text);
            for locale in [
                None,
                Some(CasingLocale::Lithuanian),
                Some(CasingLocale::Turkish),
                Some(CasingLocale::Azeri),
            ] {
                for op in [CasingOperation::Lower, CasingOperation::Upper] {
                    let got = select_case_mappings(&ctx, op, locale, limits()).unwrap();
                    for (index, c) in text.text().chars().enumerate() {
                        let matching: Vec<_> = conditional_mappings(c)
                            .iter()
                            .filter(|row| {
                                row.expression()
                                    .split_whitespace()
                                    .all(|token| match token {
                                        "lt" => locale == Some(CasingLocale::Lithuanian),
                                        "tr" => locale == Some(CasingLocale::Turkish),
                                        "az" => locale == Some(CasingLocale::Azeri),
                                        context => {
                                            let (name, negated) = context
                                                .strip_prefix("Not_")
                                                .map_or((context, false), |name| (name, true));
                                            let key = match name {
                                                "Final_Sigma" => CasingContext::FinalSigma,
                                                "After_Soft_Dotted" => {
                                                    CasingContext::AfterSoftDotted
                                                }
                                                "More_Above" => CasingContext::MoreAbove,
                                                "Before_Dot" => CasingContext::BeforeDot,
                                                "After_I" => CasingContext::AfterI,
                                                _ => panic!("unknown retained context"),
                                            };
                                            ctx.matches(index, key).unwrap() != negated
                                        }
                                    })
                            })
                            .collect();
                        assert!(matching.len() <= 1);
                        let expected = if let Some(row) = matching.first() {
                            let Some(CasingSelectionOrigin::Conditional(actual)) =
                                got.origin(index)
                            else {
                                panic!()
                            };
                            assert!(std::ptr::eq(actual, *row));
                            CaseMapping::Replacement(match op {
                                CasingOperation::Lower => row.lower(),
                                CasingOperation::Upper => row.upper(),
                            })
                        } else {
                            assert_eq!(got.origin(index), Some(CasingSelectionOrigin::Base));
                            match op {
                                CasingOperation::Lower => base_lower_mapping(c),
                                CasingOperation::Upper => base_upper_mapping(c),
                            }
                        };
                        assert_eq!(
                            got.mapping(index),
                            Some(expected),
                            "{source:?} {index} {op:?} {locale:?}"
                        );
                    }
                    assert_eq!(got.work(), 1 + len as usize);
                }
            }
        }
    }
}
