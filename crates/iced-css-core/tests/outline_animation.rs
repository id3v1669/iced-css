use iced_css_core::{
    animation::{combine::Combination, outline::*},
    syntax::*,
    values::{NumericLimits, color::ColorLimits, property::*},
};
use std::num::NonZeroUsize;
fn numeric() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
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
        panic!()
    };
    let PropertyResult::Parsed(value) = parse_property(
        &d,
        ColorLimits {
            syntax: StringLimits {
                work: n(100000),
                retained_bytes: n(4096),
            },
            numeric: numeric(),
        },
    )
    .unwrap() else {
        panic!()
    };
    value
}

use iced_css_core::animation::number::Clamp;
use iced_css_core::values::{border_width::BorderFontContext, outline::*};
fn style(name: &str) -> OutlineStyle {
    OutlineStyle::try_from(&parsed(&format!("outline-style:{name}"))).unwrap()
}
#[test]
fn all_outline_styles_select_without_admitting_hidden() {
    let names = [
        "none", "inset", "groove", "outset", "ridge", "dotted", "dashed", "solid", "double",
    ];
    for a in names {
        for b in names {
            let a = style(a);
            let b = style(b);
            for p in [
                -f32::MAX,
                0.0,
                f32::from_bits(0.5_f32.to_bits() - 1),
                0.5,
                1.0,
                f32::MAX,
            ] {
                assert_eq!(
                    combine_outline_style(a, b, Combination::Interpolate(p)).unwrap(),
                    if p < 0.5 { a } else { b }
                );
            }
            for op in [Combination::Add, Combination::Accumulate] {
                assert_eq!(combine_outline_style(a, b, op).unwrap(), a);
            }
            for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert!(combine_outline_style(a, b, Combination::Interpolate(p)).is_err());
            }
        }
    }
    assert!(OutlineStyle::try_from(&parsed("border-top-style:solid")).is_err());
    assert!(OutlineStyle::try_from(&parsed("outline-style:inherit")).is_err());
    let mut forged = parsed("outline-style:solid");
    forged.value = parsed("border-top-style:hidden").value;
    assert!(OutlineStyle::try_from(&forged).is_err());
}
#[test]
fn outline_width_computation_and_sampling_keep_suppression_separate() {
    let solid = parsed("outline-style:solid");
    let none = parsed("outline-style:none");
    let context = BorderFontContext {
        size: 10.0,
        x_height: 4.0,
    };
    for (css, expected) in [
        ("thin", 1.0),
        ("medium", 3.0),
        ("thick", 5.0),
        ("2em", 20.0),
        ("2ex", 8.0),
        ("7px", 7.0),
    ] {
        let width = parsed(&format!("outline-width:{css}"));
        assert_eq!(
            compute_outline_width(&width, &solid, context, numeric())
                .unwrap()
                .value,
            expected
        );
        assert_eq!(
            compute_outline_width(&width, &none, context, numeric())
                .unwrap()
                .value,
            0.0
        );
    }
    let width = parsed("outline-width:2em");
    let a = compute_outline_width(&width, &solid, context, numeric())
        .unwrap()
        .value;
    let b = compute_outline_width(
        &width,
        &solid,
        BorderFontContext {
            size: 20.0,
            x_height: 8.0,
        },
        numeric(),
    )
    .unwrap()
    .value;
    let pair = combine_outline_width(a, b, Combination::Interpolate(0.5)).unwrap();
    let hidden = pair.resolve(style("none"), 25.0).unwrap();
    assert_eq!(hidden.value, 0.0);
    assert!(hidden.suppressed_by_style);
    assert_eq!(hidden.clamped, None);
    let visible = pair.resolve(style("solid"), 25.0).unwrap();
    assert_eq!(visible.value, 25.0);
    assert!(!visible.suppressed_by_style);
    assert_eq!(visible.clamped, Some(Clamp::Upper));
    assert_eq!(pair.resolve(style("solid"), 100.0).unwrap().value, 30.0);
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(
            combine_outline_width(a, b, op)
                .unwrap()
                .resolve(style("solid"), 100.0)
                .unwrap()
                .value,
            60.0
        );
    }
    assert_eq!(
        combine_outline_width(a, b, Combination::Interpolate(-2.0))
            .unwrap()
            .resolve(style("solid"), 100.0)
            .unwrap()
            .clamped,
        Some(Clamp::Lower)
    );
}
#[test]
fn wrong_properties_and_invalid_numeric_inputs_cannot_be_hidden() {
    let context = BorderFontContext {
        size: 10.0,
        x_height: 4.0,
    };
    let width = parsed("outline-width:2px");
    let none = parsed("outline-style:none");
    assert!(
        compute_outline_width(&parsed("border-top-width:2px"), &none, context, numeric()).is_err()
    );
    assert!(
        compute_outline_width(&width, &parsed("border-top-style:none"), context, numeric())
            .is_err()
    );
    let mut hidden = parsed("outline-style:solid");
    hidden.value = parsed("border-top-style:hidden").value;
    assert!(compute_outline_width(&width, &hidden, context, numeric()).is_err());
    assert!(
        compute_outline_width(&parsed("outline-width:inherit"), &none, context, numeric()).is_err()
    );
    for v in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(combine_outline_width(v, 2.0, Combination::Add).is_err());
        assert!(combine_outline_width(2.0, v, Combination::Add).is_err());
    }
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(combine_outline_width(1.0, 2.0, Combination::Interpolate(p)).is_err());
    }
    let pair = combine_outline_width(1.0, 2.0, Combination::Add).unwrap();
    for max in [-1.0, f32::NAN, f32::INFINITY] {
        assert!(pair.resolve(style("none"), max).is_err());
    }
}
