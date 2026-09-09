use iced_css_core::{
    animation::{combine::Combination, text_align::*},
    cascade::{winner_table::*, *},
    syntax::*,
    values::{
        NumericLimits, color::ColorLimits, keyword::ComputedKeyword, property::*, text_align::*,
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

fn direction(css: &str) -> ComputedKeyword {
    ComputedKeyword::try_from(&parsed(css)).unwrap()
}
#[test]
fn nameless_initial_survives_inheritance_and_uses_local_direction() {
    let empty = PropertyWinners::new();
    let root = compute_text_align(&empty, None).unwrap();
    assert_eq!(root, ComputedTextAlign::DirectionalInitial);
    let child = compute_text_align(&empty, Some(root)).unwrap();
    assert_eq!(child, root);
    assert_eq!(
        root.resolve(Some(direction("direction:ltr"))).unwrap(),
        UsedTextAlign::Left
    );
    assert_eq!(
        child.resolve(Some(direction("direction:rtl"))).unwrap(),
        UsedTextAlign::Right
    );
    let values = [parsed("text-align:inherit")];
    assert_eq!(
        compute_text_align(&table(&values), Some(root)).unwrap(),
        root
    );
    assert_eq!(compute_text_align(&table(&values), None).unwrap(), root);
    let values = [parsed("text-align:left")];
    let explicit = compute_text_align(&table(&values), None).unwrap();
    let inherited = compute_text_align(&empty, Some(explicit)).unwrap();
    assert_eq!(
        inherited.resolve(Some(direction("direction:rtl"))).unwrap(),
        UsedTextAlign::Left
    );
    let mut initial = parsed("text-align:right");
    initial.value = SpecifiedValue::Initial;
    assert_eq!(
        compute_text_align(&table(&[initial]), Some(explicit)).unwrap(),
        root
    );
}
#[test]
fn five_computed_alternatives_select_discretely_without_used_value_collapse() {
    let values = [
        ComputedTextAlign::DirectionalInitial,
        ComputedTextAlign::Left,
        ComputedTextAlign::Right,
        ComputedTextAlign::Center,
        ComputedTextAlign::Justify,
    ];
    for a in values {
        for b in values {
            for p in [
                -f32::MAX,
                0.0,
                f32::from_bits(0.5_f32.to_bits() - 1),
                0.5,
                1.0,
                f32::MAX,
            ] {
                assert_eq!(
                    combine_text_align(a, b, Combination::Interpolate(p)).unwrap(),
                    if p < 0.5 { a } else { b }
                );
            }
            for op in [Combination::Add, Combination::Accumulate] {
                assert_eq!(combine_text_align(a, b, op).unwrap(), a);
            }
            for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert!(combine_text_align(a, b, Combination::Interpolate(p)).is_err());
            }
        }
    }
    let start = ComputedTextAlign::DirectionalInitial;
    let end = ComputedTextAlign::Left;
    let sample = combine_text_align(start, end, Combination::Interpolate(0.25)).unwrap();
    assert_eq!(
        sample.resolve(Some(direction("direction:rtl"))).unwrap(),
        UsedTextAlign::Right
    );
}
#[test]
fn admission_and_direction_requirements_are_explicit() {
    assert!(ComputedTextAlign::try_from(&parsed("text-align:inherit")).is_err());
    assert!(ComputedTextAlign::try_from(&parsed("float:left")).is_err());
    let mut p = parsed("text-align:left");
    p.value = SpecifiedValue::InitialReference(
        iced_css_core::values::initial::InitialReference::DirectionalTextAlign,
    );
    assert_eq!(
        ComputedTextAlign::try_from(&p).unwrap(),
        ComputedTextAlign::DirectionalInitial
    );
    let initial = ComputedTextAlign::DirectionalInitial;
    assert!(initial.resolve(None).is_err());
    assert!(
        initial
            .resolve(Some(direction("overflow:visible")))
            .is_err()
    );
    for (css, expected) in [
        ("left", UsedTextAlign::Left),
        ("right", UsedTextAlign::Right),
        ("center", UsedTextAlign::Center),
        ("justify", UsedTextAlign::Justify),
    ] {
        let v = ComputedTextAlign::try_from(&parsed(&format!("text-align:{css}"))).unwrap();
        assert_eq!(v.resolve(None).unwrap(), expected);
    }
}
