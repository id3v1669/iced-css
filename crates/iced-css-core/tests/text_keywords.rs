use iced_css_core::{
    cascade::{winner_table::*, *},
    syntax::*,
    values::{
        NumericLimits, color::ColorLimits, keyword::ComputedKeywordError, property::*,
        registry::PropertyId, text_keywords::*,
    },
};
use std::num::{NonZeroU64, NonZeroUsize};
fn parsed(css: &str) -> ParsedProperty {
    let n = NonZeroUsize::new(100000).unwrap();
    let DeclarationItem::Declaration(d) = Declarations::new(
        css,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: n,
                retained_bytes: n,
                nesting: n,
            },
            name_bytes: n,
        },
    )
    .next()
    .unwrap()
    .unwrap() else {
        panic!()
    };
    let PropertyResult::Parsed(p) = parse_property(
        &d,
        ColorLimits {
            syntax: StringLimits {
                work: n,
                retained_bytes: n,
            },
            numeric: NumericLimits { work: n, digits: n },
        },
    )
    .unwrap() else {
        panic!()
    };
    p
}
fn table(values: &[ParsedProperty]) -> PropertyWinners<'_> {
    let mut table = PropertyWinners::new();
    for (index, value) in values.iter().enumerate() {
        table
            .offer(
                value,
                TableAdmission {
                    origin: Origin::Author,
                    specificity: Specificity::ZERO,
                    order: index as u64,
                    declaration: NonZeroU64::new(index as u64 + 1).unwrap(),
                },
            )
            .unwrap();
    }
    table
}
fn check(value: ComputedTextKeywords, white: &str, transform: &str) {
    for (actual, name, expected) in [
        (value.white_space(), "white-space", white),
        (value.text_transform(), "text-transform", transform),
    ] {
        assert_eq!(actual.property(), PropertyId::lookup(name).unwrap());
        assert_eq!(actual.keyword().as_str(), expected);
    }
}
#[test]
fn defaults_inheritance_root_inherit_and_child_overrides() {
    check(
        compute_text_keywords(&table(&[]), None).unwrap(),
        "normal",
        "none",
    );
    let declarations = [
        parsed("white-space:pre-wrap"),
        parsed("text-transform:uppercase"),
    ];
    let parent = compute_text_keywords(&table(&declarations), None).unwrap();
    check(
        compute_text_keywords(&table(&[]), Some(&parent)).unwrap(),
        "pre-wrap",
        "uppercase",
    );
    let inherit = [
        parsed("white-space:inherit"),
        parsed("text-transform:inherit"),
    ];
    check(
        compute_text_keywords(&table(&inherit), Some(&parent)).unwrap(),
        "pre-wrap",
        "uppercase",
    );
    check(
        compute_text_keywords(&table(&inherit), None).unwrap(),
        "normal",
        "none",
    );
    let overrides = [
        parsed("white-space:nowrap"),
        parsed("text-transform:lowercase"),
    ];
    check(
        compute_text_keywords(&table(&overrides), Some(&parent)).unwrap(),
        "nowrap",
        "lowercase",
    );
}
#[test]
fn every_finite_keyword_keeps_its_property_identity() {
    for white in ["normal", "pre", "nowrap", "pre-wrap", "pre-line"] {
        for transform in ["none", "capitalize", "uppercase", "lowercase"] {
            let declarations = [
                parsed(&format!("white-space:{white}")),
                parsed(&format!("text-transform:{transform}")),
            ];
            check(
                compute_text_keywords(&table(&declarations), None).unwrap(),
                white,
                transform,
            );
        }
    }
}
#[test]
fn winner_values_reject_invalid_tagged_keywords_and_non_keywords() {
    for name in ["white-space", "text-transform"] {
        for (css, expected) in [
            ("display:block", ComputedKeywordError::InvalidKeyword),
            ("width:1px", ComputedKeywordError::Unresolved),
        ] {
            let mut declaration = parsed(css);
            declaration.property = PropertyId::lookup(name).unwrap();
            let property = declaration.property;
            assert_eq!(
                compute_text_keywords(&table(&[declaration]), None),
                Err(ComputedTextKeywordsError::Value {
                    property,
                    error: expected
                })
            );
        }
    }
}
