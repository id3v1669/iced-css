use iced_css_core::values::{
    Decimal, LengthTarget, NumericLimits, RangeEvent, line_height::*, property::SpecifiedValue,
};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn font() -> LineHeightFont {
    LineHeightFont {
        size: 20.0,
        x_height: 9.0,
    }
}
fn number(raw: &str) -> SpecifiedValue {
    SpecifiedValue::Number(Decimal::parse(raw, limits()).unwrap())
}
#[test]
fn inherited_number_uses_child_font_but_percentage_inherits_absolute_length() {
    let factor = compute_line_height(number("1.5"), font(), limits()).unwrap();
    let percent = compute_line_height(
        SpecifiedValue::Length(LengthTarget::parse("150%", limits()).unwrap()),
        font(),
        limits(),
    )
    .unwrap();
    assert_eq!(
        used_line_height(&factor.value, 20.0, f32::NAN, limits())
            .unwrap()
            .value,
        30.0
    );
    assert_eq!(
        used_line_height(&factor.value, 10.0, f32::NAN, limits())
            .unwrap()
            .value,
        15.0
    );
    assert_eq!(
        used_line_height(&percent.value, 10.0, f32::NAN, limits())
            .unwrap()
            .value,
        30.0
    );
}
#[test]
fn relative_and_absolute_lengths_resolve_at_the_computed_boundary() {
    for (raw, expected) in [("2em", 40.0), ("2ex", 18.0), ("1in", 96.0), ("0", 0.0)] {
        let computed = compute_line_height(
            SpecifiedValue::Length(LengthTarget::parse(raw, limits()).unwrap()),
            font(),
            limits(),
        )
        .unwrap();
        assert_eq!(computed.event, None);
        assert_eq!(
            used_line_height(&computed.value, f32::NAN, f32::NAN, limits())
                .unwrap()
                .value,
            expected
        );
    }
    assert!(compute_line_height(number("-1"), font(), limits()).is_err());
    assert!(
        compute_line_height(SpecifiedValue::Length(LengthTarget::Auto), font(), limits()).is_err()
    );
    assert!(compute_line_height(SpecifiedValue::Inherit, font(), limits()).is_err());
}
#[test]
fn normal_stays_unresolved_until_using_elements_font_metrics_are_supplied() {
    use iced_css_core::syntax::{
        DeclarationItem, DeclarationLimits, Declarations, RecoveryLimits, StringLimits,
    };
    use iced_css_core::values::{
        color::ColorLimits,
        property::{PropertyResult, parse_property},
    };
    let work = limits().work;
    let declaration = Declarations::new(
        "line-height: normal",
        DeclarationLimits {
            recovery: RecoveryLimits {
                work,
                nesting: work,
                retained_bytes: work,
            },
            name_bytes: work,
        },
    )
    .next()
    .unwrap()
    .unwrap();
    let DeclarationItem::Declaration(declaration) = declaration else {
        panic!()
    };
    let PropertyResult::Parsed(value) = parse_property(
        &declaration,
        ColorLimits {
            syntax: StringLimits {
                retained_bytes: work,
                work,
            },
            numeric: limits(),
        },
    )
    .unwrap() else {
        panic!()
    };
    let computed = compute_line_height(value.value, font(), limits()).unwrap();
    assert_eq!(
        used_line_height(&computed.value, f32::NAN, 24.0, limits())
            .unwrap()
            .value,
        24.0
    );
    assert_eq!(
        used_line_height(&computed.value, f32::NAN, 12.0, limits())
            .unwrap()
            .value,
        12.0
    );
    assert_eq!(
        used_line_height(&computed.value, f32::NAN, -0.0, limits())
            .unwrap()
            .value
            .to_bits(),
        0
    );
    for height in [-1.0, f32::NAN, f32::INFINITY] {
        assert!(used_line_height(&computed.value, 20.0, height, limits()).is_err());
    }
}
#[test]
fn range_events_belong_to_the_stage_that_performs_conversion() {
    let huge = "999999999999999999999999999999999999999999999999999";
    let computed = compute_line_height(
        SpecifiedValue::Length(LengthTarget::parse(&format!("{huge}px"), limits()).unwrap()),
        font(),
        limits(),
    )
    .unwrap();
    assert_eq!(computed.event, Some(RangeEvent::UpperSaturation));
    let used = used_line_height(&computed.value, 20.0, 24.0, limits()).unwrap();
    assert_eq!(used.value, f32::MAX);
    assert_eq!(used.event, None);
    let factor = compute_line_height(number(huge), font(), limits()).unwrap();
    assert_eq!(factor.event, None);
    assert_eq!(
        used_line_height(&factor.value, 20.0, 24.0, limits())
            .unwrap()
            .event,
        Some(RangeEvent::UpperSaturation)
    );
    let tiny = compute_line_height(
        number("0.000000000000000000000000000000000000000000000000001"),
        font(),
        limits(),
    )
    .unwrap();
    assert_eq!(
        used_line_height(&tiny.value, 1.0, 1.0, limits())
            .unwrap()
            .event,
        Some(RangeEvent::PositiveRoundedToZero)
    );
}
