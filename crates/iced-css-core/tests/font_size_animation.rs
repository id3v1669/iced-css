use iced_css_core::{
    animation::{
        combine::Combination,
        font_size::*,
        number::{Clamp, NumberError},
    },
    syntax::*,
    values::{NumericLimits, RangeEvent, color::ColorLimits, font_size::*, property::*},
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

#[test]
fn computed_percentage_and_keyword_endpoints_combine_as_absolute_font_sizes() {
    let context = FontSizeContext {
        table: FontSizeTable::new(16.0).unwrap(),
        parent_size: 20.0,
        parent_x_height: 8.0,
        root_size: 16.0,
    };
    let a = compute_font_size(&parsed("font-size:50%").value, context, numeric())
        .unwrap()
        .value;
    let b = compute_font_size(&parsed("font-size:medium").value, context, numeric())
        .unwrap()
        .value;
    assert_eq!((a, b), (10.0, 16.0));
    assert_eq!(
        combine_font_size(a, b, Combination::Interpolate(0.5))
            .unwrap()
            .value,
        13.0
    );
    let target = parsed("font-size:2em").value;
    let old = compute_font_size(&target, context, numeric())
        .unwrap()
        .value;
    let new = compute_font_size(
        &target,
        FontSizeContext {
            parent_size: 40.0,
            ..context
        },
        numeric(),
    )
    .unwrap()
    .value;
    assert_eq!(
        combine_font_size(a, old, Combination::Interpolate(0.5))
            .unwrap()
            .value,
        25.0
    );
    assert_eq!(
        combine_font_size(a, new, Combination::Interpolate(0.5))
            .unwrap()
            .value,
        45.0
    );
}
#[test]
fn exact_positive_underflow_is_distinct_from_zero_and_lower_clamping() {
    let tiny = f32::from_bits(1);
    let sample = combine_font_size(0.0, tiny, Combination::Interpolate(0.5)).unwrap();
    assert_eq!(sample.value.to_bits(), 0);
    assert_eq!(sample.event, Some(RangeEvent::PositiveRoundedToZero));
    assert_eq!(sample.clamped, None);
    let sample = combine_font_size(0.0, tiny, Combination::Interpolate(0.0)).unwrap();
    assert_eq!(sample.value.to_bits(), 0);
    assert_eq!(sample.event, None);
    assert_eq!(sample.clamped, None);
    let sample = combine_font_size(0.0, tiny, Combination::Interpolate(-0.5)).unwrap();
    assert_eq!(sample.value.to_bits(), 0);
    assert_eq!(sample.event, None);
    assert_eq!(sample.clamped, Some(Clamp::Lower));
    let sample = combine_font_size(tiny, f32::from_bits(3), Combination::Interpolate(0.5)).unwrap();
    assert_eq!(sample.value.to_bits(), 2);
    assert_eq!(sample.event, None);
    assert_eq!(sample.clamped, None);
}
#[test]
fn final_upper_saturation_is_reported_without_warning_for_an_exact_maximum() {
    for op in [Combination::Add, Combination::Accumulate] {
        let sample = combine_font_size(f32::MAX, f32::MAX, op).unwrap();
        assert_eq!(sample.value, f32::MAX);
        assert_eq!(sample.event, Some(RangeEvent::UpperSaturation));
        assert_eq!(sample.clamped, Some(Clamp::Upper));
    }
    let sample = combine_font_size(f32::MAX, f32::MAX, Combination::Interpolate(f32::MAX)).unwrap();
    assert_eq!(sample.value, f32::MAX);
    assert_eq!(sample.event, None);
    assert_eq!(sample.clamped, None);
    let sample = combine_font_size(10.0, 20.0, Combination::Interpolate(2.0)).unwrap();
    assert_eq!(sample.value, 30.0);
    assert_eq!(sample.event, None);
    for invalid in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_font_size(invalid, 0.0, Combination::Add).unwrap_err(),
            FontSizeAnimationError::InvalidEndpoint
        );
        assert_eq!(
            combine_font_size(0.0, invalid, Combination::Add).unwrap_err(),
            FontSizeAnimationError::InvalidEndpoint
        );
    }
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_font_size(0.0, 0.0, Combination::Interpolate(invalid)).unwrap_err(),
            FontSizeAnimationError::Number(NumberError::NonfiniteProgress)
        );
    }
}
