use S::{Italic as I, Normal as N, Oblique as O};
use iced_css_core::{
    animation::{combine::CombinationError, style::FontStyleKeyword as S},
    syntax::*,
    text::font_style_matching::AvailableFontStyles,
    values::{NumericLimits, color::ColorLimits, property::*},
};
use std::num::NonZeroUsize;

const ROWS: [(&[S], [Option<S>; 3]); 8] = [
    (&[], [None, None, None]),
    (&[N], [Some(N), None, None]),
    (&[I], [None, Some(I), None]),
    (&[O], [None, Some(O), Some(O)]),
    (&[N, I], [Some(N), Some(I), None]),
    (&[N, O], [Some(N), Some(O), Some(O)]),
    (&[I, O], [None, Some(I), Some(O)]),
    (&[N, I, O], [Some(N), Some(I), Some(O)]),
];

fn parsed(input: &str) -> ParsedProperty {
    let n = |n| NonZeroUsize::new(n).unwrap();
    let DeclarationItem::Declaration(d) = Declarations::new(
        input,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: n(100000),
                nesting: n(20),
                retained_bytes: n(4096),
            },
            name_bytes: n(4096),
        },
    )
    .next()
    .unwrap()
    .unwrap() else {
        panic!("expected declaration")
    };
    let PropertyResult::Parsed(value) = parse_property(
        &d,
        ColorLimits {
            syntax: StringLimits {
                work: n(100000),
                retained_bytes: n(4096),
            },
            numeric: NumericLimits {
                digits: n(1000),
                work: n(1000000),
            },
        },
    )
    .unwrap() else {
        panic!("expected parsed property")
    };
    value
}

#[test]
fn parsed_keywords_select_all_twenty_four_literal_cases() {
    let requests = [
        S::try_from(&parsed("font-style:normal")).unwrap(),
        S::try_from(&parsed("font-style:italic")).unwrap(),
        S::try_from(&parsed("font-style:oblique")).unwrap(),
    ];
    assert_eq!(requests, [N, I, O]);

    for (labels, expected) in ROWS {
        let mut available = AvailableFontStyles::default();
        for label in labels {
            available.insert(*label);
        }
        let mut reversed = AvailableFontStyles::default();
        for label in labels.iter().rev() {
            reversed.insert(*label);
            reversed.insert(*label);
        }
        assert_eq!(available, reversed);
        for (index, request) in requests.into_iter().enumerate() {
            assert_eq!(available.contains(request), labels.contains(&request));
            assert_eq!(available.select(request), expected[index]);
            assert_eq!(reversed.select(request), expected[index]);
        }
    }
}

#[test]
fn unresolved_requests_do_not_become_matching_defaults() {
    for css in ["font-style:inherit", "font-variant:normal"] {
        assert_eq!(
            S::try_from(&parsed(css)),
            Err(CombinationError::UnresolvedFontStyle)
        );
    }
    let SpecifiedValue::Expanded(leaves) = parsed("font:menu").value else {
        panic!("expected expanded system font")
    };
    let style = leaves
        .iter()
        .find(|leaf| leaf.property.descriptor().name == "font-style")
        .unwrap();
    assert!(matches!(&style.value, SpecifiedValue::SystemFont(_)));
    assert_eq!(
        S::try_from(style),
        Err(CombinationError::UnresolvedFontStyle)
    );
}

#[test]
fn independent_insertion_sequences_preserve_membership_and_selection() {
    // Literal bindings of the independent oracle, including its repeated cases.
    let sequences: [(&[S], usize); 64] = [
        (&[], 0),
        (&[N], 1),
        (&[N, N], 1),
        (&[N, N], 1),
        (&[I], 2),
        (&[I, I], 2),
        (&[I, I], 2),
        (&[O], 3),
        (&[O, O], 3),
        (&[O, O], 3),
        (&[N, I], 4),
        (&[N, I, N, I], 4),
        (&[N, N, I], 4),
        (&[N, I, I], 4),
        (&[I, N], 4),
        (&[I, N, I, N], 4),
        (&[I, I, N], 4),
        (&[I, N, N], 4),
        (&[N, O], 5),
        (&[N, O, N, O], 5),
        (&[N, N, O], 5),
        (&[N, O, O], 5),
        (&[O, N], 5),
        (&[O, N, O, N], 5),
        (&[O, O, N], 5),
        (&[O, N, N], 5),
        (&[I, O], 6),
        (&[I, O, I, O], 6),
        (&[I, I, O], 6),
        (&[I, O, O], 6),
        (&[O, I], 6),
        (&[O, I, O, I], 6),
        (&[O, O, I], 6),
        (&[O, I, I], 6),
        (&[N, I, O], 7),
        (&[N, I, O, N, I, O], 7),
        (&[N, N, I, O], 7),
        (&[N, I, I, O], 7),
        (&[N, I, O, O], 7),
        (&[N, O, I], 7),
        (&[N, O, I, N, O, I], 7),
        (&[N, N, O, I], 7),
        (&[N, O, O, I], 7),
        (&[N, O, I, I], 7),
        (&[I, N, O], 7),
        (&[I, N, O, I, N, O], 7),
        (&[I, I, N, O], 7),
        (&[I, N, N, O], 7),
        (&[I, N, O, O], 7),
        (&[I, O, N], 7),
        (&[I, O, N, I, O, N], 7),
        (&[I, I, O, N], 7),
        (&[I, O, O, N], 7),
        (&[I, O, N, N], 7),
        (&[O, N, I], 7),
        (&[O, N, I, O, N, I], 7),
        (&[O, O, N, I], 7),
        (&[O, N, N, I], 7),
        (&[O, N, I, I], 7),
        (&[O, I, N], 7),
        (&[O, I, N, O, I, N], 7),
        (&[O, O, I, N], 7),
        (&[O, I, I, N], 7),
        (&[O, I, N, N], 7),
    ];
    let mut decisions = 0;
    for (insertions, row) in sequences {
        let (labels, expected) = ROWS[row];
        let mut available = AvailableFontStyles::default();
        for &label in insertions {
            available.insert(label);
        }
        for (index, request) in [N, I, O].into_iter().enumerate() {
            assert_eq!(available.contains(request), labels.contains(&request));
            assert_eq!(available.select(request), expected[index]);
            decisions += 1;
        }
    }
    assert_eq!(decisions, 192);
}
