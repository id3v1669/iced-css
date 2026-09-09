use iced_css_core::text::{
    lf_conversion::LfDecision,
    line_edges::WhiteSpace,
    newlines::{BoundaryAffinity, NewlineLimits, NormalizedText, normalize_default_newlines},
    preserved_spaces::PreservedSpace,
    space_collapse::SpaceAction,
    whitespace_preprocessing::{
        WhitespacePreprocessingError, WhitespacePreprocessingLimits, WhitespaceRendering,
        preprocess_whitespace,
    },
};
use std::num::NonZeroUsize;

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn normalized(source: &str) -> NormalizedText<'_> {
    normalize_default_newlines(
        source,
        NewlineLimits {
            input_bytes: nz(100000),
            retained_bytes: nz(1000000),
            work: nz(200000),
        },
    )
    .unwrap()
}
fn limits() -> WhitespacePreprocessingLimits {
    WhitespacePreprocessingLimits {
        scalars: nz(10000),
        retained_bytes: nz(1000000),
        work: nz(1000000),
    }
}

#[test]
fn collapsing_modes_compose_prior_removals_lf_spaces_tabs_and_controls() {
    use WhitespaceRendering::*;
    let text = normalized("漢 \t\n漢\n\nA\t \u{200e} B");
    for mode in [WhiteSpace::Normal, WhiteSpace::Nowrap, WhiteSpace::PreLine] {
        let result = preprocess_whitespace(&text, mode, limits()).unwrap();
        let expected = if mode == WhiteSpace::PreLine {
            [
                Keep,
                RemoveWhitespace,
                RemoveWhitespace,
                Keep,
                Keep,
                Keep,
                Keep,
                Keep,
                Space,
                RemoveAdjacentSpace,
                Keep,
                RemoveAdjacentSpace,
                Keep,
            ]
        } else {
            [
                Keep,
                RemoveWhitespace,
                RemoveWhitespace,
                RemoveLf,
                Keep,
                Space,
                RemoveAdjacentSpace,
                Keep,
                Space,
                RemoveAdjacentSpace,
                Keep,
                RemoveAdjacentSpace,
                Keep,
            ]
        };
        assert_eq!(result.scalar_count(), expected.len());
        for (index, expected) in expected.into_iter().enumerate() {
            assert_eq!(result.rendering(index), Some(expected), "{mode:?} {index}");
            assert_eq!(
                result.preserved_space(index),
                Some(&PreservedSpace::Ordinary)
            );
        }
        assert_eq!(result.lf_decision(1), Some(&LfDecision::RemoveWhitespace));
        assert_eq!(result.collapse_action(1), Some(&SpaceAction::Unchanged));
        assert_eq!(result.collapse_action(8), Some(&SpaceAction::TabToSpace));
        assert_eq!(result.collapse_action(10), Some(&SpaceAction::Unchanged));
        if mode != WhiteSpace::PreLine {
            assert_eq!(result.lf_decision(3), Some(&LfDecision::RemoveLf));
            assert_eq!(result.collapse_action(3), Some(&SpaceAction::Unchanged));
            assert_eq!(result.lf_decision(6), Some(&LfDecision::Space));
            assert_eq!(
                result.collapse_action(6),
                Some(&SpaceAction::RemovedAdjacentSpace)
            );
        }
        assert_eq!(result.mode(), mode);
        assert_eq!(result.work(), 132);
        assert_eq!(result.lf_conversion().work(), 79);
        assert!(std::ptr::eq(result.text(), &text));
        assert!(std::ptr::eq(result.lf_conversion().text(), &text));
    }
    assert_eq!(text.source(), "漢 \t\n漢\n\nA\t \u{200e} B");
}

#[test]
fn preserved_modes_keep_scalars_and_separate_sequence_metadata() {
    let text = normalized(" \u{202b} \u{202c}\t \n ");
    for mode in [WhiteSpace::Pre, WhiteSpace::PreWrap] {
        let result = preprocess_whitespace(&text, mode, limits()).unwrap();
        for index in 0..8 {
            assert_eq!(result.rendering(index), Some(WhitespaceRendering::Keep));
            assert_eq!(result.lf_decision(index), Some(&LfDecision::Keep));
            assert_eq!(result.collapse_action(index), Some(&SpaceAction::Unchanged));
            let expected = if [0, 2, 5, 7].contains(&index) {
                PreservedSpace::NonBreaking {
                    sequence_end_opportunity: mode == WhiteSpace::PreWrap && index != 0,
                }
            } else {
                PreservedSpace::Ordinary
            };
            assert_eq!(result.preserved_space(index), Some(&expected));
        }
    }
    // Distinct admitted element scopes retain their own step2 sequence ends.
    for source in ["a ", " b"] {
        let text = normalized(source);
        let result = preprocess_whitespace(&text, WhiteSpace::PreWrap, limits()).unwrap();
        let index = usize::from(source.starts_with('a'));
        assert_eq!(
            result.preserved_space(index),
            Some(&PreservedSpace::NonBreaking {
                sequence_end_opportunity: true,
            })
        );
    }
}

#[test]
fn normalized_crlf_ranges_and_original_owner_are_preserved() {
    let source = String::from("漢\r\n漢\rA");
    let text = normalized(&source);
    let result = preprocess_whitespace(&text, WhiteSpace::Normal, limits()).unwrap();
    assert_eq!(result.scalar_count(), 5);
    assert_eq!(result.rendering(1), Some(WhitespaceRendering::RemoveLf));
    assert_eq!(result.rendering(3), Some(WhitespaceRendering::Space));
    assert_eq!(result.text().source(), source);
    assert_eq!(result.text().text(), "漢\n漢\nA");
    assert!(std::ptr::eq(result.text().source(), source.as_str()));
    assert!(std::ptr::eq(result.text().scalars(), text.scalars()));
    assert_eq!(result.text().scalars()[1].source, 3..5);
    assert_eq!(result.text().scalars()[1].normalized, 3..4);
    assert_eq!(
        result.text().to_normalized(4, BoundaryAffinity::Before),
        Some(3)
    );
    assert_eq!(
        result.text().to_normalized(4, BoundaryAffinity::After),
        Some(4)
    );
    assert_eq!(result.text().to_source(4), Some(5));
    for index in [result.scalar_count(), usize::MAX] {
        assert_eq!(result.rendering(index), None);
        assert_eq!(result.lf_decision(index), None);
        assert_eq!(result.preserved_space(index), None);
        assert_eq!(result.collapse_action(index), None);
    }
}

#[test]
fn all_bidi_controls_remain_visible_to_provenance_but_transparent_to_stages() {
    for control in [
        '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
        '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
    ] {
        let source = format!("漢{control} \n{control}漢\t{control} ");
        let text = normalized(&source);
        let result = preprocess_whitespace(&text, WhiteSpace::Normal, limits()).unwrap();
        for index in [1, 4, 7] {
            assert_eq!(result.rendering(index), Some(WhitespaceRendering::Keep));
        }
        assert_eq!(
            result.rendering(2),
            Some(WhitespaceRendering::RemoveWhitespace)
        );
        assert_eq!(result.rendering(3), Some(WhitespaceRendering::RemoveLf));
        assert_eq!(result.rendering(6), Some(WhitespaceRendering::Space));
        assert_eq!(
            result.rendering(8),
            Some(WhitespaceRendering::RemoveAdjacentSpace)
        );
    }
    for source in [
        "A\n漢",
        "漢\nA",
        "漢\u{fe0f}\n漢",
        "漢\n\u{0301}漢",
        "\n漢",
        "漢\n",
    ] {
        let text = normalized(source);
        let index = text.text().chars().position(|c| c == '\n').unwrap();
        let result = preprocess_whitespace(&text, WhiteSpace::Normal, limits()).unwrap();
        assert_eq!(result.rendering(index), Some(WhitespaceRendering::Space));
    }
}

#[test]
fn exact_peak_count_and_work_admission_and_one_less_boundaries() {
    let text = normalized("漢\n\n漢");
    let base = preprocess_whitespace(&text, WhiteSpace::Normal, limits()).unwrap();
    assert_eq!(base.work(), 42);
    assert!(base.peak_retained_bytes() > base.retained_bytes());
    let exact = WhitespacePreprocessingLimits {
        scalars: nz(4),
        retained_bytes: nz(base.peak_retained_bytes()),
        work: nz(42),
    };
    let result = preprocess_whitespace(&text, WhiteSpace::Normal, exact).unwrap();
    assert_eq!(result.retained_bytes(), base.retained_bytes());
    assert_eq!(result.peak_retained_bytes(), base.peak_retained_bytes());
    let mut cap = exact;
    cap.scalars = nz(3);
    assert!(matches!(
        preprocess_whitespace(&text, WhiteSpace::Normal, cap),
        Err(WhitespacePreprocessingError::Scalars)
    ));
    cap = exact;
    cap.work = nz(41);
    assert!(matches!(
        preprocess_whitespace(&text, WhiteSpace::Normal, cap),
        Err(WhitespacePreprocessingError::Work)
    ));
    cap = exact;
    cap.retained_bytes = nz(base.peak_retained_bytes() - 1);
    assert!(matches!(
        preprocess_whitespace(&text, WhiteSpace::Normal, cap),
        Err(WhitespacePreprocessingError::Bytes)
    ));
    cap = exact;
    cap.retained_bytes = NonZeroUsize::MAX;
    let large = preprocess_whitespace(&text, WhiteSpace::Normal, cap).unwrap();
    assert_eq!(large.retained_bytes(), base.retained_bytes());
    assert_eq!(text.text(), "漢\n\n漢");
}

#[test]
fn empty_owner_costs_two_entries_without_payload_in_every_mode() {
    let text = normalized("");
    for mode in [
        WhiteSpace::Normal,
        WhiteSpace::Nowrap,
        WhiteSpace::Pre,
        WhiteSpace::PreWrap,
        WhiteSpace::PreLine,
    ] {
        let mut cap = WhitespacePreprocessingLimits {
            scalars: nz(1),
            retained_bytes: nz(1),
            work: nz(2),
        };
        let result = preprocess_whitespace(&text, mode, cap).unwrap();
        assert_eq!(
            (
                result.work(),
                result.retained_bytes(),
                result.peak_retained_bytes()
            ),
            (2, 0, 0)
        );
        assert_eq!(result.scalar_count(), 0);
        assert_eq!(result.mode(), mode);
        assert_eq!(result.lf_conversion().work(), 1);
        assert_eq!(result.rendering(0), None);
        assert_eq!(result.preserved_space(usize::MAX), None);
        cap.work = nz(1);
        assert!(matches!(
            preprocess_whitespace(&text, mode, cap),
            Err(WhitespacePreprocessingError::Work)
        ));
    }
}
