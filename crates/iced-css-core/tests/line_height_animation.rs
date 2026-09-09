use iced_css_core::{
    animation::{
        combine::Combination,
        line_height::*,
        number::{Clamp, NumberError},
    },
    syntax::*,
    values::{NumericLimits, color::ColorLimits, line_height::*, property::*},
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

fn computed(css: &str) -> ComputedLineHeight {
    compute_line_height(
        parsed(css).value,
        LineHeightFont {
            size: 20.0,
            x_height: 10.0,
        },
        numeric(),
    )
    .unwrap()
    .value
}
fn used(value: &ComputedLineHeight, size: f32) -> f32 {
    used_line_height(value, size, 25.0, numeric())
        .unwrap()
        .value
}
#[test]
fn exact_unitless_animation_remains_relative_to_the_using_font() {
    let a = computed("line-height:1.1");
    let b = computed("line-height:1.3");
    for (op, at10, at20) in [
        (Combination::Interpolate(0.5), 12.0, 24.0),
        (Combination::Add, 24.0, 48.0),
        (Combination::Accumulate, 24.0, 48.0),
    ] {
        let result = combine_line_height(&a, &b, op, numeric())
            .unwrap()
            .resolve(100.0, numeric())
            .unwrap();
        assert_eq!(used(result.computed(), 10.0), at10);
        assert_eq!(used(result.computed(), 20.0), at20);
        assert_eq!(result.clamped, None);
    }
    let a = computed(&format!("line-height:1{}", "0".repeat(40)));
    let b = computed(&format!("line-height:3{}", "0".repeat(40)));
    let expected = computed(&format!("line-height:2{}", "0".repeat(40)));
    let sample = combine_line_height(&a, &b, Combination::Interpolate(0.5), numeric())
        .unwrap()
        .resolve(1.0, numeric())
        .unwrap();
    assert_eq!(
        used(sample.computed(), f32::from_bits(1)),
        used(&expected, f32::from_bits(1))
    );
    assert!(used(sample.computed(), f32::from_bits(1)) > 0.00001);
}
#[test]
fn computed_percentage_and_length_combine_but_number_and_length_are_discrete() {
    let percent = computed("line-height:150%");
    let length = computed("line-height:50px");
    let sample = combine_line_height(&percent, &length, Combination::Interpolate(0.5), numeric())
        .unwrap()
        .resolve(100.0, numeric())
        .unwrap();
    assert_eq!(used(sample.computed(), 10.0), 40.0);
    assert_eq!(used(sample.computed(), 100.0), 40.0);
    let number = computed("line-height:2");
    for (p, expected) in [(0.25, 20.0), (0.5, 50.0)] {
        let sample = combine_line_height(&number, &length, Combination::Interpolate(p), numeric())
            .unwrap()
            .resolve(100.0, numeric())
            .unwrap();
        assert_eq!(used(sample.computed(), 10.0), expected);
    }
    for op in [Combination::Add, Combination::Accumulate] {
        let sample = combine_line_height(&number, &length, op, numeric())
            .unwrap()
            .resolve(100.0, numeric())
            .unwrap();
        assert_eq!(used(sample.computed(), 10.0), 20.0);
    }
}
#[test]
fn normal_and_final_numeric_ranges_are_preserved() {
    let normal = computed("line-height:normal");
    let sample = combine_line_height(&normal, &normal, Combination::Add, numeric())
        .unwrap()
        .resolve(100.0, numeric())
        .unwrap();
    assert_eq!(used(sample.computed(), 10.0), 25.0);
    for (a, b, maximum, expected, clamp) in [
        (
            "line-height:2px",
            "line-height:10px",
            15.0,
            15.0,
            Clamp::Upper,
        ),
        ("line-height:2", "line-height:1", 100.0, 0.0, Clamp::Lower),
    ] {
        let a = computed(a);
        let b = computed(b);
        let sample = combine_line_height(&a, &b, Combination::Interpolate(3.0), numeric())
            .unwrap()
            .resolve(maximum, numeric())
            .unwrap();
        assert_eq!(used(sample.computed(), 10.0), expected);
        assert_eq!(sample.clamped, Some(clamp));
    }
}
#[test]
fn line_height_animation_validates_progress_ranges_and_decimal_limits() {
    let a = computed("line-height:1.1");
    let b = computed("line-height:1.3");
    for p in [f32::NAN, f32::INFINITY] {
        assert_eq!(
            combine_line_height(&a, &b, Combination::Interpolate(p), numeric()).unwrap_err(),
            LineHeightAnimationError::NonfiniteProgress
        );
    }
    assert_eq!(
        combine_line_height(&a, &b, Combination::Add, numeric())
            .unwrap()
            .resolve(-1.0, numeric())
            .unwrap_err(),
        LineHeightAnimationError::Number(NumberError::InvalidRange)
    );
    let mut tiny = numeric();
    tiny.work = NonZeroUsize::MIN;
    assert!(matches!(
        combine_line_height(&a, &b, Combination::Add, tiny),
        Err(LineHeightAnimationError::Numeric(_))
    ));
}

#[test]
fn a_discretely_selected_absolute_value_obeys_the_final_working_range() {
    let number = computed("line-height:2");
    let absolute = computed("line-height:50px");
    let sample = combine_line_height(&number, &absolute, Combination::Interpolate(0.5), numeric())
        .unwrap()
        .resolve(30.0, numeric())
        .unwrap();
    assert_eq!(used(sample.computed(), 10.0), 30.0);
    assert_eq!(sample.clamped, Some(Clamp::Upper));
}
