use iced_css_core::text::{
    casing_contexts::{CasingContextError, CasingContextLimits, evaluate_casing_contexts},
    casing_data::{CasingContext::*, *},
    newlines::{NewlineLimits, NormalizedText, normalize_default_newlines},
};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> CasingContextLimits {
    CasingContextLimits {
        scalars: nz(usize::MAX),
        retained_bytes: nz(usize::MAX),
        work: nz(usize::MAX),
    }
}
fn normalized(s: &str) -> NormalizedText<'_> {
    normalize_default_newlines(
        s,
        NewlineLimits {
            input_bytes: nz(1_000_000),
            retained_bytes: nz(10_000_000),
            work: nz(10_000_000),
        },
    )
    .unwrap()
}
#[test]
fn literals_exclude_current_and_respect_possessive_ignorable_overlap() {
    for (s, i, context, expected) in [
        ("I", 0, AfterI, false),
        ("i", 0, AfterSoftDotted, false),
        ("\u{307}", 0, BeforeDot, false),
        ("\u{301}", 0, MoreAbove, false),
        ("\u{345}Σ", 1, FinalSigma, false),
        ("α\u{345}Σ\u{345}", 2, FinalSigma, true),
        ("αΣ\u{345}α", 1, FinalSigma, false),
        ("i\u{323}X", 2, AfterSoftDotted, true),
        ("i\u{301}X", 2, AfterSoftDotted, false),
        ("I\u{323}X", 2, AfterI, true),
        ("IaX", 2, AfterI, false),
        ("X\u{323}\u{307}", 0, BeforeDot, true),
        ("X\u{301}\u{307}", 0, BeforeDot, false),
        ("X\u{323}\u{301}", 0, MoreAbove, true),
        ("Xa\u{301}", 0, MoreAbove, false),
        ("\u{2b2}X", 1, AfterSoftDotted, true),
        ("α\u{2b0}Σ", 2, FinalSigma, true),
    ] {
        let text = normalized(s);
        let got = evaluate_casing_contexts(&text, limits()).unwrap();
        assert_eq!(
            got.matches(i, context),
            Some(expected),
            "{s:?} {i} {context:?}"
        );
    }
}
const CONTEXTS: [CasingContext; 5] = [FinalSigma, AfterSoftDotted, MoreAbove, BeforeDot, AfterI];
// Direct formal witnesses over original slices, independent of rolling scan state.
fn oracle(s: &[char], i: usize, context: CasingContext) -> bool {
    let permitted = |c: &char| !matches!(canonical_combining_class(*c), 0 | 230);
    match context {
        FinalSigma => {
            s[..i]
                .iter()
                .rev()
                .find(|c| !is_case_ignorable(**c))
                .is_some_and(|c| is_cased(*c))
                && !s[i + 1..]
                    .iter()
                    .find(|c| !is_case_ignorable(**c))
                    .is_some_and(|c| is_cased(*c))
        }
        AfterSoftDotted | AfterI => (0..i).any(|j| {
            (if context == AfterI {
                s[j] == 'I'
            } else {
                is_soft_dotted(s[j])
            }) && s[j + 1..i].iter().all(permitted)
        }),
        MoreAbove | BeforeDot => (i + 1..s.len()).any(|j| {
            (if context == BeforeDot {
                s[j] == '\u{307}'
            } else {
                canonical_combining_class(s[j]) == 230
            }) && s[i + 1..j].iter().all(permitted)
        }),
    }
}
#[test]
fn formal_oracle_all_twelve_property_classes_through_length_four() {
    let alphabet = [
        '\n', '\u{34f}', 'Σ', 'I', 'i', '\u{2b0}', '\u{2b2}', '\u{301}', '\u{307}', '\u{323}',
        '\u{345}', '\u{1715}',
    ];
    let mut strings = 0;
    let mut comparisons = 0;
    for len in 0..=4u32 {
        for mut code in 0..12usize.pow(len) {
            let chars: Vec<_> = (0..len)
                .map(|_| {
                    let c = alphabet[code % 12];
                    code /= 12;
                    c
                })
                .collect();
            let source: String = chars.iter().collect();
            let text = normalized(&source);
            let got = evaluate_casing_contexts(&text, limits()).unwrap();
            for i in 0..chars.len() {
                for context in CONTEXTS {
                    assert_eq!(
                        got.matches(i, context),
                        Some(oracle(&chars, i, context)),
                        "{chars:?} {i} {context:?}"
                    );
                    comparisons += 1;
                }
            }
            assert_eq!(got.work(), 1 + 3 * chars.len());
            strings += 1;
        }
    }
    assert_eq!((strings, comparisons), (22621, 442140));
}
#[test]
fn unicode17_boundary_assumptions_and_supplementary_witnesses() {
    assert_eq!(canonical_combining_class('I'), 0);
    assert_eq!(canonical_combining_class('\u{307}'), 230);
    for scalar in (0..=0x10ffff).filter_map(char::from_u32) {
        if is_soft_dotted(scalar) {
            assert_eq!(canonical_combining_class(scalar), 0, "{scalar:?}");
        }
    }
    let text = normalized("\u{10400}Σ\u{1d185}");
    let got = evaluate_casing_contexts(&text, limits()).unwrap();
    let chars: Vec<_> = text.text().chars().collect();
    for i in 0..chars.len() {
        for context in CONTEXTS {
            assert_eq!(got.matches(i, context), Some(oracle(&chars, i, context)));
        }
    }
}
#[test]
fn original_owner_maps_and_checked_indexes_survive_success_and_failure() {
    let source = String::from("I\r\nαΣ\rI\u{307}");
    let text = normalized(&source);
    let got = evaluate_casing_contexts(&text, limits()).unwrap();
    assert!(std::ptr::eq(got.text(), &text));
    assert_eq!(got.text().source(), source);
    assert_eq!(got.text().text(), "I\nαΣ\nI\u{307}");
    assert_eq!(got.text().scalars()[1].source, 1..3);
    assert_eq!(got.text().scalars()[1].normalized, 1..2);
    assert_eq!(got.matches(3, FinalSigma), Some(true));
    assert_eq!(got.matches(5, BeforeDot), Some(true));
    assert_eq!(got.matches(6, AfterI), Some(true));
    for context in CONTEXTS {
        assert_eq!(got.matches(got.scalar_count(), context), None);
        assert_eq!(got.matches(usize::MAX, context), None);
    }
    let mut low = limits();
    low.work = nz(1);
    assert_eq!(
        evaluate_casing_contexts(&text, low).unwrap_err(),
        CasingContextError::Work
    );
    assert_eq!(got.matches(3, FinalSigma), Some(true));
    assert_eq!(text.source(), source);
    assert!(evaluate_casing_contexts(&text, limits()).is_ok());
}
#[test]
fn exact_one_less_empty_and_long_run_accounts() {
    let text = normalized("αΣI\u{307}");
    let initial = evaluate_casing_contexts(&text, limits()).unwrap();
    let exact = CasingContextLimits {
        scalars: nz(4),
        retained_bytes: nz(initial.retained_bytes()),
        work: nz(13),
    };
    assert_eq!(evaluate_casing_contexts(&text, exact).unwrap().work(), 13);
    for (low, expected) in [
        (
            CasingContextLimits {
                scalars: nz(3),
                ..exact
            },
            CasingContextError::Scalars,
        ),
        (
            CasingContextLimits {
                retained_bytes: nz(initial.retained_bytes() - 1),
                ..exact
            },
            CasingContextError::Bytes,
        ),
        (
            CasingContextLimits {
                work: nz(12),
                ..exact
            },
            CasingContextError::Work,
        ),
    ] {
        assert_eq!(evaluate_casing_contexts(&text, low).unwrap_err(), expected);
    }
    let empty = normalized("");
    let got = evaluate_casing_contexts(
        &empty,
        CasingContextLimits {
            scalars: nz(1),
            retained_bytes: nz(1),
            work: nz(1),
        },
    )
    .unwrap();
    assert_eq!(
        (got.scalar_count(), got.retained_bytes(), got.work()),
        (0, 0, 1)
    );
    assert_eq!(got.matches(0, FinalSigma), None);
    let source = format!("α{}Σ{}", "\u{345}".repeat(10000), "\u{345}".repeat(10000));
    let text = normalized(&source);
    let got = evaluate_casing_contexts(&text, limits()).unwrap();
    assert_eq!(got.matches(10001, FinalSigma), Some(true));
    assert_eq!(got.work(), 60007);
}
