use iced_css_core::{
    animation::{
        background_position::*,
        combine::Combination,
        length_percentage::{LengthPercentage, MixedValueError},
    },
    syntax::*,
    values::{
        Decimal, LengthTarget, NumericLimits,
        background_position::*,
        color::ColorLimits,
        geometry::{LengthBasis, LengthContext, LengthEvaluationError},
        property::*,
    },
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

fn computed(css: &str) -> ComputedBackgroundPosition {
    compute_background_position(parsed(css), LengthContext::default(), numeric())
        .unwrap()
        .value
}
#[test]
fn computed_axes_preserve_percentages_and_resolve_relative_lengths() {
    let p = computed("background-position:top right");
    assert_eq!(
        p.components(),
        [
            PositionComponent::Percentage(&Decimal::parse("100", numeric()).unwrap()),
            PositionComponent::Percentage(&Decimal::zero())
        ]
    );
    let p = computed("background-position:0px 0%");
    assert!(
        matches!(p.components(),[PositionComponent::Pixels(0.0),PositionComponent::Percentage(p)] if p.is_zero())
    );
    let p = compute_background_position(
        parsed("background-position:1em -2ex"),
        LengthContext {
            font_size: Some(16.0),
            x_height: Some(7.0),
            percentage_basis: None,
        },
        numeric(),
    )
    .unwrap();
    assert_eq!(
        p.value.components(),
        [
            PositionComponent::Pixels(16.0),
            PositionComponent::Pixels(-14.0)
        ]
    );
    assert_eq!(p.rounded_to_zero, [false; 2]);
    let p = computed("background-position:10px");
    assert!(
        matches!(p.components(),[PositionComponent::Pixels(10.0),PositionComponent::Percentage(v)] if *v==Decimal::parse("50",numeric()).unwrap())
    );
}
#[test]
fn keywords_and_mixed_animation_resolve_against_current_area_and_image_sizes() {
    for (css, expected) in [
        ("background-position:right bottom", [80.0, -20.0]),
        ("background-position:center", [40.0, -10.0]),
    ] {
        let p = computed(css);
        let arithmetic = BackgroundPositionValue::from_computed(&p, numeric()).unwrap();
        assert_eq!(
            arithmetic
                .resolve(
                    [100.0, 80.0],
                    [20.0, 100.0],
                    [-1000.0; 2],
                    [1000.0; 2],
                    numeric()
                )
                .unwrap()
                .offset,
            expected
        );
    }
    let a = computed("background-position:10px 20%");
    let b = computed("background-position:50% -10px");
    let value =
        combine_background_position(&a, &b, Combination::Interpolate(0.5), numeric()).unwrap();
    assert_eq!(
        value
            .resolve(
                [100.0; 2],
                [20.0, 40.0],
                [-1000.0; 2],
                [1000.0; 2],
                numeric()
            )
            .unwrap()
            .offset,
        [25.0, 1.0]
    );
    assert_eq!(
        value
            .resolve(
                [100.0; 2],
                [40.0, 60.0],
                [-1000.0; 2],
                [1000.0; 2],
                numeric()
            )
            .unwrap()
            .offset,
        [20.0, -1.0]
    );
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(
            combine_background_position(&a, &b, op, numeric())
                .unwrap()
                .resolve(
                    [100.0; 2],
                    [20.0, 40.0],
                    [-1000.0; 2],
                    [1000.0; 2],
                    numeric()
                )
                .unwrap()
                .offset,
            [50.0, 2.0]
        );
    }
}
#[test]
fn percentage_basis_difference_is_not_rounded_before_cancellation() {
    let a = computed("background-position:-100000000px 0px");
    let b = computed("background-position:100% 0%");
    let value = combine_background_position(&a, &b, Combination::Add, numeric()).unwrap();
    let sample = value
        .resolve(
            [100000000.0, 1.0],
            [1.0, 1.0],
            [-10.0; 2],
            [10.0; 2],
            numeric(),
        )
        .unwrap();
    assert_eq!(sample.offset, [-1.0, 0.0]);
    assert_eq!(sample.clamped, [None; 2]);
    assert_eq!(sample.rounded_to_zero, [false; 2]);
    let percentage = LengthPercentage::from_percentage(Decimal::parse("100", numeric()).unwrap());
    assert_eq!(
        percentage
            .resolve_difference_basis(f32::MAX, -f32::MAX, -1.0, 1.0, numeric())
            .unwrap()
            .value,
        1.0
    );
}
#[test]
fn unresolved_values_sizes_and_arithmetic_errors_stay_explicit() {
    for css in ["background-position:inherit", "border-spacing:1px"] {
        assert_eq!(
            compute_background_position(parsed(css), LengthContext::default(), numeric())
                .unwrap_err(),
            BackgroundPositionError::Unresolved
        );
    }
    assert_eq!(
        compute_background_position(
            parsed("background-position:1em 0"),
            LengthContext::default(),
            numeric()
        )
        .unwrap_err(),
        BackgroundPositionError::Length(LengthEvaluationError::MissingContext(
            LengthBasis::FontSize
        ))
    );
    let mut p = parsed("background-position:center");
    p.value = SpecifiedValue::LengthPair(LengthTarget::Auto, LengthTarget::Auto);
    assert_eq!(
        compute_background_position(p, LengthContext::default(), numeric()).unwrap_err(),
        BackgroundPositionError::InvalidValue
    );
    let p = computed("background-position:center");
    let value = BackgroundPositionValue::from_computed(&p, numeric()).unwrap();
    for invalid in [-1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            value
                .resolve([invalid, 1.0], [1.0; 2], [-1.0; 2], [1.0; 2], numeric())
                .unwrap_err(),
            BackgroundPositionAnimationError::InvalidSize
        );
        assert_eq!(
            value
                .resolve([1.0; 2], [1.0, invalid], [-1.0; 2], [1.0; 2], numeric())
                .unwrap_err(),
            BackgroundPositionAnimationError::InvalidSize
        );
    }
    for progress in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(matches!(
            combine_background_position(&p, &p, Combination::Interpolate(progress), numeric()),
            Err(BackgroundPositionAnimationError::Mixed(
                MixedValueError::NonfiniteProgress
            ))
        ));
    }
    assert!(
        value
            .resolve([1.0; 2], [1.0; 2], [0.0, 2.0], [1.0; 2], numeric())
            .is_err()
    );
    let percentage = LengthPercentage::from_percentage(Decimal::parse("1", numeric()).unwrap());
    assert_eq!(
        percentage
            .resolve_difference_basis(1.0, f32::NAN, 0.0, 1.0, numeric())
            .unwrap_err(),
        MixedValueError::InvalidBasis
    );
}
