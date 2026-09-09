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
    values::{Decimal, Integer, computed_color::ComputedColor, registry::PropertyId},
};
fn d(s: &str) -> Decimal {
    Decimal::parse(s, numeric()).unwrap()
}
fn endpoint(css: &str) -> ColorAnimationEndpoint {
    ColorAnimationEndpoint::from_computed(ComputedColor::try_from(&parsed(css)).unwrap(), numeric())
        .unwrap()
}
fn channels(value: &ColorAnimationEndpoint) -> (&[Decimal; 3], f32) {
    let ColorEndpointComponent::Numeric { numerators, alpha } = value.component() else {
        panic!()
    };
    (numerators, alpha)
}
#[test]
fn integer_to_decimal_is_exact_bounded_and_non_mutating() {
    for s in [
        "0".to_owned(),
        "-0".to_owned(),
        "+123".to_owned(),
        "-789".to_owned(),
        "9".repeat(500),
    ] {
        let i = Integer::parse(&s, numeric()).unwrap();
        assert_eq!(Decimal::from_integer(&i, numeric()).unwrap(), d(&s));
        assert_eq!(i, Integer::parse(&s, numeric()).unwrap());
    }
    let i = Integer::parse("12345", numeric()).unwrap();
    let mut small = numeric();
    small.work = NonZeroUsize::MIN;
    assert!(Decimal::from_integer(&i, small).is_err());
    small = numeric();
    small.digits = NonZeroUsize::new(4).unwrap();
    assert!(Decimal::from_integer(&i, small).is_err());
}
#[test]
fn equivalent_css22_rgb_forms_share_exact_channel_units_for_all_properties() {
    for name in [
        "color",
        "background-color",
        "border-top-color",
        "border-right-color",
        "border-bottom-color",
        "border-left-color",
        "outline-color",
    ] {
        for value in ["red", "#ff0000", "rgb(255,0,0)", "rgb(100%,0%,0%)"] {
            let e = endpoint(&format!("{name}:{value}"));
            let (rgb, alpha) = channels(&e);
            assert_eq!(rgb, &[d("25500"), d("0"), d("0")]);
            assert_eq!(alpha, 1.0);
            assert_eq!(e.property(), PropertyId::lookup(name).unwrap());
        }
    }
    let huge = "9".repeat(200);
    let e = endpoint(&format!("color:rgb({huge},-2,3)"));
    assert_eq!(channels(&e).0, &[d(&(huge + "00")), d("-200"), d("300")]);
    let e = endpoint("color:rgb(120.5%,0%,0%)");
    assert_eq!(channels(&e).0[0], d("30727.5"));
}
#[test]
fn typed_alternatives_and_resolved_presentations_remain_explicit() {
    let transparent = endpoint("background-color:transparent");
    assert_eq!(channels(&transparent), (&[d("0"), d("0"), d("0")], 0.0));
    let invert = endpoint("outline-color:invert");
    assert!(matches!(invert.component(), ColorEndpointComponent::Invert));
    let system = parsed("color:ButtonFace");
    assert!(matches!(
        ColorAnimationEndpoint::from_computed(ComputedColor::try_from(&system).unwrap(), numeric()),
        Err(ColorEndpointAdmissionError::UnresolvedSystem(_))
    ));
    let color = ResolvedLegacyColor::new([0.25, -0.5, 2.0], 0.5).unwrap();
    let e = ColorAnimationEndpoint::from_resolved(
        PropertyId::lookup("color").unwrap(),
        color,
        numeric(),
    )
    .unwrap();
    assert_eq!(channels(&e), (&[d("6375"), d("-12750"), d("51000")], 0.5));
    assert!(
        ColorAnimationEndpoint::from_resolved(
            PropertyId::lookup("width").unwrap(),
            color,
            numeric()
        )
        .is_err()
    );
}
#[test]
fn numeric_resource_failures_do_not_force_work_for_invert() {
    let p = parsed("color:red");
    let mut small = numeric();
    small.work = NonZeroUsize::MIN;
    assert!(
        ColorAnimationEndpoint::from_computed(ComputedColor::try_from(&p).unwrap(), small).is_err()
    );
    let p = parsed("outline-color:invert");
    assert!(
        ColorAnimationEndpoint::from_computed(ComputedColor::try_from(&p).unwrap(), small).is_ok()
    );
}
