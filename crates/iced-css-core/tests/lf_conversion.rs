use iced_css_core::text::{
    lf_conversion::{LfConversionError, LfConversionLimits, LfDecision::*, convert_line_feeds},
    line_edges::WhiteSpace,
    newlines::{NewlineLimits, normalize_default_newlines},
    unicode::is_lf_cjk_ideograph,
};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> LfConversionLimits {
    LfConversionLimits {
        scalars: nz(10000),
        retained_bytes: nz(1000000),
        work: nz(1000000),
    }
}
fn normalized(s: &str) -> iced_css_core::text::newlines::NormalizedText<'_> {
    normalize_default_newlines(
        s,
        NewlineLimits {
            input_bytes: nz(1000000),
            retained_bytes: nz(1000000),
            work: nz(1000000),
        },
    )
    .unwrap()
}
#[test]
fn original_neighbors_control_conversion_and_preserved_modes() {
    let text = normalized("漢 \t\n漢\n\n漢");
    for mode in [WhiteSpace::Normal, WhiteSpace::Nowrap] {
        let got = convert_line_feeds(&text, mode, limits()).unwrap();
        assert_eq!(
            got.decisions(),
            &[
                Keep,
                RemoveWhitespace,
                RemoveWhitespace,
                RemoveLf,
                Keep,
                Space,
                Space,
                Keep
            ]
        );
        assert!(std::ptr::eq(got.text(), &text));
        assert_eq!(got.work(), 49);
    }
    let got = convert_line_feeds(&text, WhiteSpace::PreLine, limits()).unwrap();
    assert_eq!(
        got.decisions(),
        &[
            Keep,
            RemoveWhitespace,
            RemoveWhitespace,
            Keep,
            Keep,
            Keep,
            Keep,
            Keep
        ]
    );
    for mode in [WhiteSpace::Pre, WhiteSpace::PreWrap] {
        assert!(
            convert_line_feeds(&text, mode, limits())
                .unwrap()
                .decisions()
                .iter()
                .all(|d| *d == Keep)
        );
    }
}
#[test]
fn classifier_is_policy_specific() {
    for c in [
        '\u{3007}',
        '\u{3021}',
        '漢',
        '\u{F900}',
        '\u{16FF2}',
        '\u{33479}',
    ] {
        assert!(is_lf_cjk_ideograph(c));
    }
    for c in [
        '\u{3005}',
        '\u{3006}',
        '\u{17000}',
        '\u{1B170}',
        'あ',
        '가',
        '\u{3347A}',
    ] {
        assert!(!is_lf_cjk_ideograph(c));
    }
}
#[test]
fn all_bidi_controls_are_transparent_but_other_controls_are_neighbors() {
    for c in [
        '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
        '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
    ] {
        let input = format!("漢{c}\n{c}漢");
        let text = normalized(&input);
        assert_eq!(
            convert_line_feeds(&text, WhiteSpace::Normal, limits())
                .unwrap()
                .decisions(),
            &[Keep, Keep, RemoveLf, Keep, Keep]
        );
    }
    for c in [
        'A',
        '\u{0301}',
        '\u{fe0f}',
        '\u{e0100}',
        '\u{200c}',
        '\u{200d}',
        '\u{3006}',
    ] {
        for input in [format!("漢{c}\n漢"), format!("漢\n{c}漢")] {
            let text = normalized(&input);
            let got = convert_line_feeds(&text, WhiteSpace::Normal, limits()).unwrap();
            let lf = text.text().chars().position(|v| v == '\n').unwrap();
            assert_eq!(got.decision(lf), Some(&Space));
        }
    }
}
#[test]
fn edges_empty_controls_and_original_crlf_mapping() {
    for input in ["\n漢", "漢\n", "A\n漢", "漢\nA"] {
        let text = normalized(input);
        let got = convert_line_feeds(&text, WhiteSpace::Normal, limits()).unwrap();
        assert!(got.decisions().contains(&Space));
        assert!(!got.decisions().contains(&RemoveLf));
    }
    for input in ["", "\u{200e}\u{2066}"] {
        let text = normalized(input);
        let got = convert_line_feeds(&text, WhiteSpace::Normal, limits()).unwrap();
        assert!(got.decisions().iter().all(|v| *v == Keep));
        assert_eq!(got.decision(got.decisions().len()), None);
        assert_eq!(got.decision(usize::MAX), None);
    }
    let text = normalized("漢\r\n漢\rA");
    let got = convert_line_feeds(&text, WhiteSpace::Normal, limits()).unwrap();
    assert_eq!(got.decisions(), &[Keep, RemoveLf, Keep, Space, Keep]);
    assert_eq!(got.text().source(), "漢\r\n漢\rA");
    assert_eq!(got.text().text(), "漢\n漢\nA");
    assert_eq!(got.text().scalars()[1].source, 3..5);
    assert_eq!(got.text().scalars()[3].source, 8..9);
}
#[test]
fn exact_admission_and_one_less_preserve_owners() {
    let text = normalized("漢\n漢");
    let base = convert_line_feeds(&text, WhiteSpace::Normal, limits()).unwrap();
    assert_eq!(base.work(), 19);
    assert!(base.peak_retained_bytes() > base.retained_bytes());
    let mut exact = limits();
    exact.scalars = nz(3);
    exact.work = nz(19);
    exact.retained_bytes = nz(base.peak_retained_bytes());
    assert!(convert_line_feeds(&text, WhiteSpace::Normal, exact).is_ok());
    let mut cap = exact;
    cap.work = nz(18);
    assert!(matches!(
        convert_line_feeds(&text, WhiteSpace::Normal, cap),
        Err(LfConversionError::Work)
    ));
    cap = exact;
    cap.retained_bytes = nz(base.peak_retained_bytes() - 1);
    assert!(matches!(
        convert_line_feeds(&text, WhiteSpace::Normal, cap),
        Err(LfConversionError::Bytes)
    ));
    cap = exact;
    cap.scalars = nz(2);
    assert!(matches!(
        convert_line_feeds(&text, WhiteSpace::Normal, cap),
        Err(LfConversionError::Scalars)
    ));
    cap = exact;
    cap.retained_bytes = NonZeroUsize::MAX;
    assert!(convert_line_feeds(&text, WhiteSpace::Normal, cap).is_ok());
    let empty = normalized("");
    cap.work = nz(1);
    cap.retained_bytes = nz(1);
    let got = convert_line_feeds(&empty, WhiteSpace::Normal, cap).unwrap();
    assert_eq!(
        (got.work(), got.retained_bytes(), got.peak_retained_bytes()),
        (1, 0, 0)
    );
    assert_eq!(text.source(), "漢\n漢");
}
#[test]
fn collapse_consumes_decisions_without_changing_original_lf_barriers() {
    use iced_css_core::text::space_collapse::*;
    let text = normalized("漢\n\n漢");
    let got = convert_line_feeds(&text, WhiteSpace::Normal, limits()).unwrap();
    let items: Vec<_> = text
        .text()
        .chars()
        .zip(got.decisions())
        .map(|(c, d)| SpaceItem {
            unit: match d {
                Keep => SpaceUnit::Scalar(c),
                Space => SpaceUnit::Scalar(' '),
                RemoveLf | RemoveWhitespace => SpaceUnit::Removed,
            },
            white_space: WhiteSpace::Normal,
        })
        .collect();
    assert_eq!(
        collapse_adjacent_spaces(
            &items,
            SpaceCollapseLimits {
                items: nz(4),
                retained_bytes: nz(100),
                work: nz(4)
            }
        )
        .unwrap(),
        [
            SpaceAction::Unchanged,
            SpaceAction::Unchanged,
            SpaceAction::RemovedAdjacentSpace,
            SpaceAction::Unchanged
        ]
    );
    assert_eq!(got.decisions(), &[Keep, Space, Space, Keep]);
}

#[test]
fn bounded_original_neighbor_oracle_covers_mixed_sequences() {
    // The oracle independently removes maximal surrounding SP/HT runs, then
    // selects immediate retained neighbors. It never uses converter decisions.
    let alphabet = ['漢', 'A', '\n', ' ', '\t', '\u{200e}', '\u{fe0f}'];
    for len in 0..=4_u32 {
        for mut code in 0..alphabet.len().pow(len) {
            let chars: Vec<_> = (0..len)
                .map(|_| {
                    let c = alphabet[code % alphabet.len()];
                    code /= alphabet.len();
                    c
                })
                .collect();
            let input: String = chars.iter().collect();
            let text = normalized(&input);
            for mode in [
                WhiteSpace::Normal,
                WhiteSpace::Nowrap,
                WhiteSpace::Pre,
                WhiteSpace::PreWrap,
                WhiteSpace::PreLine,
            ] {
                let mut expected = vec![Keep; chars.len()];
                for (i, c) in chars.iter().enumerate() {
                    if mode.collapses_spaces() && matches!(c, ' ' | '\t') {
                        let left = chars[..i]
                            .iter()
                            .rev()
                            .find(|c| !matches!(c, ' ' | '\t' | '\u{200e}'));
                        let right = chars[i + 1..]
                            .iter()
                            .find(|c| !matches!(c, ' ' | '\t' | '\u{200e}'));
                        if left == Some(&'\n') || right == Some(&'\n') {
                            expected[i] = RemoveWhitespace;
                        }
                    }
                }
                let retained: Vec<_> = (0..chars.len())
                    .filter(|i| expected[*i] != RemoveWhitespace && chars[*i] != '\u{200e}')
                    .collect();
                for (k, i) in retained.iter().copied().enumerate() {
                    if chars[i] == '\n' && !mode.preserves_newlines() {
                        expected[i] = if k > 0
                            && k + 1 < retained.len()
                            && chars[retained[k - 1]] == '漢'
                            && chars[retained[k + 1]] == '漢'
                        {
                            RemoveLf
                        } else {
                            Space
                        };
                    }
                }
                let got = convert_line_feeds(&text, mode, limits()).unwrap();
                assert_eq!(got.decisions(), expected, "{input:?} {mode:?}");
                assert_eq!(got.work(), 1 + 6 * chars.len());
            }
        }
    }
}
