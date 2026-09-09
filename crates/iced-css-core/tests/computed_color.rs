use iced_css_core::{
    cascade::{winner_table::*, *},
    syntax::*,
    values::{
        NumericLimits, color::Color, color::ColorLimits, computed_color::*, property::*,
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

fn id(name: &str) -> PropertyId {
    PropertyId::lookup(name).unwrap()
}
#[test]
fn defaults_preserve_property_identity_and_required_foreground_context() {
    let empty = PropertyWinners::new();
    let ua = Color::Srgb([12, 34, 56]);
    let root = compute_color(
        &empty,
        id("color"),
        ColorComputationContext {
            user_agent_color: Some(&ua),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(std::ptr::eq(root.value(), &ua));
    for name in [
        "border-top-color",
        "border-right-color",
        "border-bottom-color",
        "border-left-color",
    ] {
        let border = compute_color(
            &empty,
            id(name),
            ColorComputationContext {
                current_color: Some(root),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(border.property(), id(name));
        assert!(std::ptr::eq(border.value(), root.value()));
    }
    let bg = compute_color(&empty, id("background-color"), Default::default()).unwrap();
    assert!(matches!(bg.value(), Color::Transparent));
    let outline = compute_color(&empty, id("outline-color"), Default::default()).unwrap();
    assert!(matches!(outline.value(), Color::Invert));
    let child = compute_color(
        &empty,
        id("color"),
        ColorComputationContext {
            parent: Some(root),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(std::ptr::eq(child.value(), root.value()));
}
#[test]
fn border_inheritance_keeps_parent_computed_color_instead_of_child_default() {
    let red = parsed("color:red");
    let blue = parsed("color:blue");
    let red = ComputedColor::try_from(&red).unwrap();
    let blue = ComputedColor::try_from(&blue).unwrap();
    let empty = PropertyWinners::new();
    for name in [
        "border-top-color",
        "border-right-color",
        "border-bottom-color",
        "border-left-color",
    ] {
        let parent = compute_color(
            &empty,
            id(name),
            ColorComputationContext {
                current_color: Some(red),
                ..Default::default()
            },
        )
        .unwrap();
        let context = ColorComputationContext {
            parent: Some(parent),
            current_color: Some(blue),
            ..Default::default()
        };
        let inherited = [parsed(&format!("{name}:inherit"))];
        assert!(std::ptr::eq(
            compute_color(&table(&inherited), id(name), context)
                .unwrap()
                .value(),
            red.value()
        ));
        assert!(std::ptr::eq(
            compute_color(&empty, id(name), context).unwrap().value(),
            blue.value()
        ));
    }
}
#[test]
fn exact_and_system_colors_are_borrowed_without_normalization() {
    for name in [
        "color",
        "background-color",
        "border-top-color",
        "border-right-color",
        "border-bottom-color",
        "border-left-color",
        "outline-color",
    ] {
        for css in ["rgb(9999,-2,3)", "rgb(110%,0%,0%)", "ButtonFace"] {
            let p = parsed(&format!("{name}:{css}"));
            let value = ComputedColor::try_from(&p).unwrap();
            let SpecifiedValue::Color(original) = &p.value else {
                panic!()
            };
            assert!(std::ptr::eq(value.value(), original));
            assert_eq!(value.property(), id(name));
        }
    }
}
#[test]
fn missing_context_and_forged_alternatives_are_rejected() {
    let empty = PropertyWinners::new();
    assert!(compute_color(&empty, id("color"), Default::default()).is_err());
    assert!(compute_color(&empty, id("border-top-color"), Default::default()).is_err());
    assert!(compute_color(&empty, id("width"), Default::default()).is_err());
    assert!(ComputedColor::try_from(&parsed("color:inherit")).is_err());
    for (name, color) in [
        ("color", Color::Transparent),
        ("outline-color", Color::Transparent),
        ("background-color", Color::Invert),
    ] {
        let mut forged = parsed(&format!("{name}:red"));
        forged.value = SpecifiedValue::Color(color);
        assert!(ComputedColor::try_from(&forged).is_err());
    }
    let p = parsed("background-color:red");
    let wrong = ComputedColor::try_from(&p).unwrap();
    assert!(
        compute_color(
            &empty,
            id("border-top-color"),
            ColorComputationContext {
                current_color: Some(wrong),
                ..Default::default()
            }
        )
        .is_err()
    );
    let inherited = [parsed("color:inherit")];
    assert!(
        compute_color(
            &table(&inherited),
            id("color"),
            ColorComputationContext {
                parent: Some(wrong),
                ..Default::default()
            }
        )
        .is_err()
    );
    let mut recipe = parsed("background-color:red");
    recipe.value = SpecifiedValue::InitialReference(
        iced_css_core::values::initial::InitialReference::CurrentColor,
    );
    assert!(
        compute_color(
            &table(&[recipe]),
            id("background-color"),
            Default::default()
        )
        .is_err()
    );
}
