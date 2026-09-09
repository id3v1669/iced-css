use iced_css_core::{
    cascade::{winner_table::*, *},
    syntax::*,
    values::{
        NumericLimits, bidi::*, color::ColorLimits, keyword::ComputedKeywordError, property::*,
        registry::PropertyId,
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

fn check(value: ComputedBidiStyle, direction: ComputedDirection, unicode: ComputedUnicodeBidi) {
    assert_eq!(value.direction(), direction);
    assert_eq!(value.unicode_bidi(), unicode);
}
#[test]
fn defaults_actual_parent_explicit_inherit_and_root_inherit() {
    use ComputedDirection::{LeftToRight, RightToLeft};
    use ComputedUnicodeBidi::{BidiOverride, Embed, Normal};
    check(
        compute_bidi_style(&table(&[]), None).unwrap(),
        LeftToRight,
        Normal,
    );
    let declarations = [
        parsed("direction:rtl"),
        parsed("unicode-bidi:bidi-override"),
    ];
    let parent = compute_bidi_style(&table(&declarations), None).unwrap();
    check(
        compute_bidi_style(&table(&[]), Some(&parent)).unwrap(),
        RightToLeft,
        Normal,
    );
    let inherited = [parsed("direction:inherit"), parsed("unicode-bidi:inherit")];
    check(
        compute_bidi_style(&table(&inherited), Some(&parent)).unwrap(),
        RightToLeft,
        BidiOverride,
    );
    check(
        compute_bidi_style(&table(&inherited), None).unwrap(),
        LeftToRight,
        Normal,
    );
    let overrides = [parsed("direction:ltr"), parsed("unicode-bidi:embed")];
    check(
        compute_bidi_style(&table(&overrides), Some(&parent)).unwrap(),
        LeftToRight,
        Embed,
    );
    let other = compute_bidi_style(&table(&overrides), None).unwrap();
    check(
        compute_bidi_style(&table(&inherited), Some(&other)).unwrap(),
        LeftToRight,
        Embed,
    );
}
#[test]
fn all_six_pairs_and_css_spellings() {
    for (direction, expected_direction) in [
        ("ltr", ComputedDirection::LeftToRight),
        ("rtl", ComputedDirection::RightToLeft),
    ] {
        for (unicode, expected_unicode) in [
            ("normal", ComputedUnicodeBidi::Normal),
            ("embed", ComputedUnicodeBidi::Embed),
            ("bidi-override", ComputedUnicodeBidi::BidiOverride),
        ] {
            let declarations = [
                parsed(&format!("direction:{direction}")),
                parsed(&format!("unicode-bidi:{unicode}")),
            ];
            let value = compute_bidi_style(&table(&declarations), None).unwrap();
            check(value, expected_direction, expected_unicode);
            assert_eq!(value.direction().as_str(), direction);
            assert_eq!(value.unicode_bidi().as_str(), unicode);
        }
    }
}
#[test]
fn invalid_tagged_values_report_the_fixed_property() {
    for name in ["direction", "unicode-bidi"] {
        for (css, expected) in [
            ("display:block", ComputedKeywordError::InvalidKeyword),
            ("width:1px", ComputedKeywordError::Unresolved),
        ] {
            let mut declaration = parsed(css);
            declaration.property = PropertyId::lookup(name).unwrap();
            let property = declaration.property;
            assert_eq!(
                compute_bidi_style(&table(&[declaration]), None),
                Err(ComputedBidiError::Value {
                    property,
                    error: expected
                })
            );
        }
    }
}
#[test]
fn internal_initial_is_not_parent_inheritance() {
    let declarations = [
        parsed("direction:rtl"),
        parsed("unicode-bidi:bidi-override"),
    ];
    let parent = compute_bidi_style(&table(&declarations), None).unwrap();
    let mut initial = [parsed("direction:rtl"), parsed("unicode-bidi:embed")];
    for declaration in &mut initial {
        declaration.value = SpecifiedValue::Initial;
    }
    check(
        compute_bidi_style(&table(&initial), Some(&parent)).unwrap(),
        ComputedDirection::LeftToRight,
        ComputedUnicodeBidi::Normal,
    );
}
