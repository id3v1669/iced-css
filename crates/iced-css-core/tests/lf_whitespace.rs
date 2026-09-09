use iced_css_core::text::{lf_whitespace::*, line_edges::*, newlines::*, space_collapse::*};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn normalized(text: &str) -> NormalizedText<'_> {
    normalize_default_newlines(
        text,
        NewlineLimits {
            input_bytes: nz(10000),
            retained_bytes: nz(100000),
            work: nz(20000),
        },
    )
    .unwrap()
}
fn limits() -> LfWhitespaceLimits {
    LfWhitespaceLimits {
        scalars: nz(1000),
        retained_bytes: nz(10000),
        work: nz(3000),
    }
}
#[test]
fn all_modes_remove_only_prescribed_surrounding_spaces_and_tabs() {
    let text = normalized("x \t\n \ty");
    for mode in [WhiteSpace::Normal, WhiteSpace::Nowrap, WhiteSpace::PreLine] {
        assert_eq!(
            remove_whitespace_around_lf(&text, mode, limits()).unwrap(),
            [
                LfWhitespaceAction::Keep,
                LfWhitespaceAction::Remove,
                LfWhitespaceAction::Remove,
                LfWhitespaceAction::Keep,
                LfWhitespaceAction::Remove,
                LfWhitespaceAction::Remove,
                LfWhitespaceAction::Keep
            ]
        );
    }
    for mode in [WhiteSpace::Pre, WhiteSpace::PreWrap] {
        assert_eq!(
            remove_whitespace_around_lf(&text, mode, limits()).unwrap(),
            vec![LfWhitespaceAction::Keep; 7]
        );
    }
}
#[test]
fn formatting_controls_are_transparent_but_nbsp_and_other_separators_are_not() {
    let text = normalized(" \u{202b}\n\u{202c}\t");
    assert_eq!(
        remove_whitespace_around_lf(&text, WhiteSpace::Normal, limits()).unwrap(),
        [
            LfWhitespaceAction::Remove,
            LfWhitespaceAction::Keep,
            LfWhitespaceAction::Keep,
            LfWhitespaceAction::Keep,
            LfWhitespaceAction::Remove
        ]
    );
    for c in ['\u{a0}', '\u{200b}', '\u{2028}', '\u{000c}', 'x'] {
        let source = format!(" {c}\n{c} ");
        let text = normalized(&source);
        assert_eq!(
            remove_whitespace_around_lf(&text, WhiteSpace::Normal, limits()).unwrap(),
            vec![LfWhitespaceAction::Keep; 5]
        );
    }
}
#[test]
fn consecutive_default_newlines_remain_and_no_lf_does_not_trim() {
    let text = normalized("\r \r\n\t\r");
    assert_eq!(text.text(), "\n \n\t\n");
    assert_eq!(
        remove_whitespace_around_lf(&text, WhiteSpace::PreLine, limits()).unwrap(),
        [
            LfWhitespaceAction::Keep,
            LfWhitespaceAction::Remove,
            LfWhitespaceAction::Keep,
            LfWhitespaceAction::Remove,
            LfWhitespaceAction::Keep
        ]
    );
    let text = normalized(" \tx \t");
    assert_eq!(
        remove_whitespace_around_lf(&text, WhiteSpace::Normal, limits()).unwrap(),
        vec![LfWhitespaceAction::Keep; 5]
    );
}
#[test]
fn pre_line_pipeline_retains_original_byte_ranges_after_both_collapse_steps() {
    let source = " \tA \t\r\n \tB  ";
    let text = normalized(source);
    let around = remove_whitespace_around_lf(&text, WhiteSpace::PreLine, limits()).unwrap();
    let items: Vec<_> = text
        .text()
        .chars()
        .zip(around)
        .map(|(c, a)| SpaceItem {
            unit: if a == LfWhitespaceAction::Remove {
                SpaceUnit::Removed
            } else {
                SpaceUnit::Scalar(c)
            },
            white_space: WhiteSpace::PreLine,
        })
        .collect();
    let actions = collapse_adjacent_spaces(
        &items,
        SpaceCollapseLimits {
            items: nz(1000),
            retained_bytes: nz(10000),
            work: nz(1000),
        },
    )
    .unwrap();
    let rendered: Vec<_> = items
        .iter()
        .zip(actions)
        .enumerate()
        .filter_map(|(index, (item, action))| {
            if action == SpaceAction::RemovedAdjacentSpace {
                return None;
            }
            match item.unit {
                SpaceUnit::Scalar(c) => Some((
                    index,
                    if action == SpaceAction::TabToSpace {
                        ' '
                    } else {
                        c
                    },
                )),
                _ => None,
            }
        })
        .collect();
    let mut retained = Vec::new();
    for line in rendered.split(|(_, c)| *c == '\n') {
        let edge_items: Vec<_> = line
            .iter()
            .map(|(_, c)| LineEdgeItem {
                kind: LineEdgeKind::from_scalar(*c),
                white_space: WhiteSpace::PreLine,
            })
            .collect();
        let trimmed = trim_line_edges(
            &edge_items,
            LineEdgeLimits {
                items: nz(1000),
                retained_bytes: nz(10000),
                work: nz(3000),
            },
        )
        .unwrap();
        retained.push(
            line.iter()
                .zip(trimmed)
                .filter(|(_, d)| *d == EdgeDisposition::Keep)
                .map(|((index, _), _)| &source[text.scalars()[*index].source.clone()])
                .collect::<String>(),
        );
    }
    assert_eq!(retained, ["A", "B"]);
    let newline = rendered.iter().find(|(_, c)| *c == '\n').unwrap().0;
    assert_eq!(&source[text.scalars()[newline].source.clone()], "\r\n");
}
#[test]
fn empty_and_admission_limits_are_explicit() {
    assert!(
        remove_whitespace_around_lf(&normalized(""), WhiteSpace::Normal, limits())
            .unwrap()
            .is_empty()
    );
    let text = normalized(" \n");
    let mut cap = limits();
    cap.scalars = nz(1);
    assert!(matches!(
        remove_whitespace_around_lf(&text, WhiteSpace::Normal, cap),
        Err(LfWhitespaceError::Scalars)
    ));
    cap = limits();
    cap.retained_bytes = nz(1);
    assert!(matches!(
        remove_whitespace_around_lf(&text, WhiteSpace::Normal, cap),
        Err(LfWhitespaceError::Bytes)
    ));
    cap = limits();
    cap.work = nz(5);
    assert!(matches!(
        remove_whitespace_around_lf(&text, WhiteSpace::Normal, cap),
        Err(LfWhitespaceError::Work)
    ));
}
