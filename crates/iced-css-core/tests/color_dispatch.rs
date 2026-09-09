use iced_css_core::{
    animation::color::*,
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

use iced_css_core::{
    animation::legacy_color::ResolvedLegacyColor,
    values::{Decimal, computed_color::ComputedColor, registry::PropertyId},
};
fn d(s: &str) -> Decimal {
    Decimal::parse(s, numeric()).unwrap()
}
fn endpoint(css: &str) -> ColorAnimationEndpoint {
    ColorAnimationEndpoint::from_computed(ComputedColor::try_from(&parsed(css)).unwrap(), numeric())
        .unwrap()
}
use iced_css_core::animation::combine::Combination;
fn numeric_sample(pair: &CombinedColor<'_>) -> [iced_css_core::values::FiniteF32; 4] {
    let ColorFiniteValue::Numeric(v) = pair.to_finite_unpremultiplied(numeric()).unwrap().value
    else {
        panic!()
    };
    v
}
#[test]
fn all_color_properties_combine_numeric_forms_before_conversion() {
    for name in [
        "color",
        "background-color",
        "border-top-color",
        "border-right-color",
        "border-bottom-color",
        "border-left-color",
        "outline-color",
    ] {
        let a = endpoint(&format!("{name}:rgb(255,0,0)"));
        let b = endpoint(&format!("{name}:rgb(0%,0%,100%)"));
        let pair = combine_color(&a, &b, Combination::Interpolate(0.5), numeric()).unwrap();
        let ColorPairComponent::Numeric(components) = pair.component() else {
            panic!()
        };
        assert_eq!(components, &[d("12750"), d("0"), d("12750"), d("1")]);
        assert_eq!(numeric_sample(&pair).map(|v| v.value), [0.5, 0.0, 0.5, 1.0]);
        assert_eq!(pair.property(), a.property());
        assert_eq!(
            pair.to_finite_unpremultiplied(numeric()).unwrap().property,
            a.property()
        );
    }
    let huge = "9".repeat(200);
    let a = endpoint(&format!("color:rgb({huge},0,0)"));
    let b = endpoint(&format!("color:rgb(-{huge},0,0)"));
    assert_eq!(
        numeric_sample(&combine_color(&a, &b, Combination::Interpolate(0.5), numeric()).unwrap())
            .map(|v| v.value),
        [0.0, 0.0, 0.0, 1.0]
    );
}
#[test]
fn premultiplied_dispatch_preserves_exact_zero_alpha_and_addition() {
    let id = PropertyId::lookup("color").unwrap();
    let a = ColorAnimationEndpoint::from_resolved(
        id,
        ResolvedLegacyColor::new([1.0, 0.0, 0.0], 1.0).unwrap(),
        numeric(),
    )
    .unwrap();
    let b = ColorAnimationEndpoint::from_resolved(
        id,
        ResolvedLegacyColor::new([0.0, 1.0, 0.0], 0.5).unwrap(),
        numeric(),
    )
    .unwrap();
    assert_eq!(
        numeric_sample(&combine_color(&a, &b, Combination::Interpolate(2.0), numeric()).unwrap())
            .map(|v| v.value),
        [-1.0, 1.0, 0.0, 0.0]
    );
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(
            numeric_sample(&combine_color(&a, &b, op, numeric()).unwrap())
                .map(|v| v.value.to_bits()),
            [0x3f2aaaab, 0x3eaaaaab, 0, 0x3fc00000]
        );
    }
}
#[test]
fn invert_fallback_borrows_whole_endpoints_without_numeric_work() {
    let a = endpoint("outline-color:invert");
    let b = endpoint(&format!("outline-color:rgb({},0,0)", "9".repeat(200)));
    let mut tiny = numeric();
    tiny.work = NonZeroUsize::MIN;
    for (start, end) in [(&a, &b), (&b, &a), (&a, &a)] {
        for p in [-f32::MAX, 0.25, 0.5, f32::MAX] {
            let pair = combine_color(start, end, Combination::Interpolate(p), tiny).unwrap();
            let ColorPairComponent::Discrete(selected) = pair.component() else {
                panic!()
            };
            assert!(std::ptr::eq(selected, if p < 0.5 { start } else { end }));
        }
        for op in [Combination::Add, Combination::Accumulate] {
            let pair = combine_color(start, end, op, tiny).unwrap();
            let ColorPairComponent::Discrete(selected) = pair.component() else {
                panic!()
            };
            assert!(std::ptr::eq(selected, start));
        }
    }
    let pair = combine_color(&a, &b, Combination::Interpolate(0.25), tiny).unwrap();
    assert!(matches!(
        pair.to_finite_unpremultiplied(tiny).unwrap().value,
        ColorFiniteValue::Invert
    ));
    let blue = ColorAnimationEndpoint::from_resolved(
        a.property(),
        ResolvedLegacyColor::new([0.0, 0.0, 1.0], 0.0).unwrap(),
        numeric(),
    )
    .unwrap();
    assert_eq!(
        numeric_sample(&combine_color(&blue, &a, Combination::Interpolate(0.25), tiny).unwrap())
            .map(|v| v.value),
        [0.0, 0.0, 1.0, 0.0]
    );
}
#[test]
fn mismatched_properties_and_nonfinite_progress_fail_before_combination() {
    let a = endpoint("color:red");
    let b = endpoint("background-color:red");
    assert_eq!(
        combine_color(&a, &b, Combination::Add, numeric()).unwrap_err(),
        ColorPairError::PropertyMismatch
    );
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_color(&a, &a, Combination::Interpolate(p), numeric()).unwrap_err(),
            ColorPairError::NonfiniteProgress
        );
        let invert = endpoint("outline-color:invert");
        assert_eq!(
            combine_color(&invert, &invert, Combination::Interpolate(p), numeric()).unwrap_err(),
            ColorPairError::NonfiniteProgress
        );
    }
    let mut tiny = numeric();
    tiny.work = NonZeroUsize::MIN;
    assert!(combine_color(&a, &a, Combination::Add, tiny).is_err());
}
