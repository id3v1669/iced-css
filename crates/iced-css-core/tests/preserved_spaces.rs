use iced_css_core::text::{line_edges::WhiteSpace, newlines::*, preserved_spaces::*};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn normalized(text: &str) -> NormalizedText<'_> {
    normalize_default_newlines(
        text,
        NewlineLimits {
            input_bytes: nz(1000),
            retained_bytes: nz(10000),
            work: nz(2000),
        },
    )
    .unwrap()
}
fn limits() -> PreservedSpaceLimits {
    PreservedSpaceLimits {
        scalars: nz(100),
        retained_bytes: nz(1000),
        work: nz(200),
    }
}
fn space(end: bool) -> PreservedSpace {
    PreservedSpace::NonBreaking {
        sequence_end_opportunity: end,
    }
}
#[test]
fn all_modes_preserve_the_defined_space_distinctions() {
    let text = normalized("a  b ");
    for mode in [WhiteSpace::Normal, WhiteSpace::Nowrap, WhiteSpace::PreLine] {
        assert_eq!(
            preserved_space_sequences(&text, mode, limits()).unwrap(),
            vec![PreservedSpace::Ordinary; 5]
        );
    }
    assert_eq!(
        preserved_space_sequences(&text, WhiteSpace::Pre, limits()).unwrap(),
        [
            PreservedSpace::Ordinary,
            space(false),
            space(false),
            PreservedSpace::Ordinary,
            space(false)
        ]
    );
    assert_eq!(
        preserved_space_sequences(&text, WhiteSpace::PreWrap, limits()).unwrap(),
        [
            PreservedSpace::Ordinary,
            space(false),
            space(true),
            PreservedSpace::Ordinary,
            space(true)
        ]
    );
    assert_eq!(text.text(), "a  b ");
}
#[test]
fn controls_join_sequences_while_tabs_lf_and_nbsp_end_them() {
    let text = normalized(" \u{202b} \u{202c}");
    assert_eq!(
        preserved_space_sequences(&text, WhiteSpace::PreWrap, limits()).unwrap(),
        [
            space(false),
            PreservedSpace::Ordinary,
            space(true),
            PreservedSpace::Ordinary
        ]
    );
    for delimiter in ['\t', '\n', '\u{a0}', '\u{200b}', 'x'] {
        let source = format!(" {delimiter} ");
        let text = normalized(&source);
        assert_eq!(
            preserved_space_sequences(&text, WhiteSpace::PreWrap, limits()).unwrap(),
            [space(true), PreservedSpace::Ordinary, space(true)]
        );
    }
}
#[test]
fn separate_inline_scopes_keep_separate_sequence_end_opportunities() {
    let left = normalized("a ");
    let right = normalized(" b");
    assert_eq!(
        preserved_space_sequences(&left, WhiteSpace::PreWrap, limits()).unwrap(),
        [PreservedSpace::Ordinary, space(true)]
    );
    assert_eq!(
        preserved_space_sequences(&right, WhiteSpace::PreWrap, limits()).unwrap(),
        [space(true), PreservedSpace::Ordinary]
    );
    assert!(
        preserved_space_sequences(&normalized(""), WhiteSpace::PreWrap, limits())
            .unwrap()
            .is_empty()
    );
}
#[test]
fn admission_limits_precede_result_publication() {
    let text = normalized("  ");
    let mut cap = limits();
    cap.scalars = nz(1);
    assert!(matches!(
        preserved_space_sequences(&text, WhiteSpace::PreWrap, cap),
        Err(PreservedSpaceError::Scalars)
    ));
    cap = limits();
    cap.retained_bytes = nz(1);
    assert!(matches!(
        preserved_space_sequences(&text, WhiteSpace::PreWrap, cap),
        Err(PreservedSpaceError::Bytes)
    ));
    cap = limits();
    cap.work = nz(3);
    assert!(matches!(
        preserved_space_sequences(&text, WhiteSpace::PreWrap, cap),
        Err(PreservedSpaceError::Work)
    ));
}
