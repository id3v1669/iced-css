use iced_css_core::{
    animation::{box_length::*, combine::Combination, number::Clamp},
    formatting::block::*,
    syntax::*,
    values::{
        NumericLimits, box_length::*, color::ColorLimits, geometry::LengthContext, property::*,
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
const CASES: &[(&str, bool, Option<&str>)] = &[
    ("margin-top", true, Some("auto")),
    ("margin-right", true, Some("auto")),
    ("margin-bottom", true, Some("auto")),
    ("margin-left", true, Some("auto")),
    ("padding-top", false, None),
    ("padding-right", false, None),
    ("padding-bottom", false, None),
    ("padding-left", false, None),
    ("width", false, Some("auto")),
    ("height", false, Some("auto")),
    ("min-width", false, None),
    ("min-height", false, None),
    ("max-width", false, Some("none")),
    ("max-height", false, Some("none")),
    ("top", true, Some("auto")),
    ("right", true, Some("auto")),
    ("bottom", true, Some("auto")),
    ("left", true, Some("auto")),
    ("text-indent", true, None),
];

fn computed(css: &str) -> ComputedBoxLength {
    compute_box_length(parsed(css), LengthContext::default(), numeric())
        .unwrap()
        .value
}
#[test]
fn all_nineteen_properties_keep_identity_and_apply_their_signed_ranges() {
    for &(name, signed, _) in CASES {
        let a = computed(&format!("{name}:10px"));
        let b = computed(&format!("{name}:50%"));
        assert_eq!(a.property().descriptor().name, name);
        assert_eq!(a.allows_negative(), signed);
        assert!(matches!(b.component(), BoxLengthComponent::Percentage(_)));
        let value = combine_box_length(&a, &b, Combination::Interpolate(0.5), numeric())
            .unwrap()
            .resolve(100.0, 1000.0, numeric())
            .unwrap();
        assert_eq!(value.property, a.property());
        assert_eq!(value.value, BoxLengthSampleValue::Pixels(30.0));
        assert_eq!(value.clamped, None);
        for op in [Combination::Add, Combination::Accumulate] {
            assert_eq!(
                combine_box_length(&a, &b, op, numeric())
                    .unwrap()
                    .resolve(100.0, 1000.0, numeric())
                    .unwrap()
                    .value,
                BoxLengthSampleValue::Pixels(60.0)
            );
        }
        let value = combine_box_length(&a, &b, Combination::Interpolate(-1.0), numeric())
            .unwrap()
            .resolve(100.0, 1000.0, numeric())
            .unwrap();
        assert_eq!(
            value.value,
            BoxLengthSampleValue::Pixels(if signed { -30.0 } else { 0.0 })
        );
        assert_eq!(
            value.clamped,
            if signed { None } else { Some(Clamp::Lower) }
        );
    }
}
#[test]
fn keyword_alternatives_are_whole_values_and_do_not_become_numeric_endpoints() {
    for &(name, _, keyword) in CASES {
        let Some(keyword) = keyword else { continue };
        let a = computed(&format!("{name}:10px"));
        let b = computed(&format!("{name}:{keyword}"));
        let want = if keyword == "auto" {
            BoxLengthSampleValue::Auto
        } else {
            BoxLengthSampleValue::None
        };
        for (op, expected) in [
            (
                Combination::Interpolate(0.25),
                BoxLengthSampleValue::Pixels(10.0),
            ),
            (Combination::Interpolate(0.5), want),
            (Combination::Add, BoxLengthSampleValue::Pixels(10.0)),
            (Combination::Accumulate, BoxLengthSampleValue::Pixels(10.0)),
        ] {
            assert_eq!(
                combine_box_length(&a, &b, op, numeric())
                    .unwrap()
                    .resolve(100.0, 1000.0, numeric())
                    .unwrap()
                    .value,
                expected
            );
        }
        assert_eq!(
            combine_box_length(&b, &a, Combination::Add, numeric())
                .unwrap()
                .resolve(100.0, 1000.0, numeric())
                .unwrap()
                .value,
            want
        );
        let mut tiny = numeric();
        tiny.work = NonZeroUsize::MIN;
        assert_eq!(
            combine_box_length(&a, &b, Combination::Interpolate(0.5), tiny)
                .unwrap()
                .resolve(0.0, 1000.0, tiny)
                .unwrap()
                .value,
            want
        );
        assert_eq!(
            combine_box_length(&a, &b, Combination::Add, numeric())
                .unwrap()
                .resolve(100.0, 5.0, numeric())
                .unwrap()
                .clamped,
            Some(Clamp::Upper)
        );
    }
}
#[test]
fn sampled_margin_and_width_feed_the_existing_block_equation() {
    let a = computed("margin-left:10px");
    let b = computed("margin-left:50%");
    let margin = combine_box_length(&a, &b, Combination::Interpolate(0.5), numeric()).unwrap();
    let width = CombinedBoxLength::from_computed(&computed("width:100px"), numeric()).unwrap();
    for (basis, left, right) in [(200.0, 55.0, 45.0), (300.0, 80.0, 120.0)] {
        let BoxLengthSampleValue::Pixels(margin) =
            margin.resolve(basis, 1000.0, numeric()).unwrap().value
        else {
            panic!()
        };
        let BoxLengthSampleValue::Pixels(width) =
            width.resolve(basis, 1000.0, numeric()).unwrap().value
        else {
            panic!()
        };
        let result = solve_block_width(BlockWidthInput {
            containing_width: basis,
            direction: Direction::Ltr,
            width: AutoLength::Value(width),
            margin_left: AutoLength::Value(margin),
            margin_right: AutoLength::Auto,
            border_left: 0.0,
            border_right: 0.0,
            padding_left: 0.0,
            padding_right: 0.0,
            min_width: 0.0,
            max_width: None,
        })
        .unwrap();
        assert_eq!(
            (result.width, result.margin_left, result.margin_right),
            (100.0, left, right)
        );
    }
}
#[test]
fn invalid_values_and_property_mismatches_are_rejected_before_representation_loss() {
    let negative = parsed(&format!("margin-left:-0.{}1px", "0".repeat(60))).value;
    for &(name, signed, keyword) in CASES {
        let mut p = parsed(&format!("{name}:1px"));
        p.value = negative.clone();
        let result = compute_box_length(p, LengthContext::default(), numeric());
        if signed {
            assert!(result.unwrap().rounded_to_zero);
        } else {
            assert_eq!(result.unwrap_err(), BoxLengthError::InvalidValue);
        }
        if keyword != Some("auto") {
            let mut p = parsed(&format!("{name}:1px"));
            p.value = parsed("width:auto").value;
            assert_eq!(
                compute_box_length(p, LengthContext::default(), numeric()).unwrap_err(),
                BoxLengthError::InvalidValue
            );
        }
        assert_eq!(
            compute_box_length(
                parsed(&format!("{name}:inherit")),
                LengthContext::default(),
                numeric()
            )
            .unwrap_err(),
            BoxLengthError::Unresolved
        );
    }
    assert_eq!(
        compute_box_length(parsed("font-size:1px"), LengthContext::default(), numeric())
            .unwrap_err(),
        BoxLengthError::UnsupportedProperty
    );
    let a = computed("margin-left:auto");
    let b = computed("margin-right:auto");
    assert!(matches!(
        combine_box_length(&a, &b, Combination::Add, numeric()),
        Err(BoxLengthAnimationError::PropertyMismatch)
    ));
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(combine_box_length(&a, &a, Combination::Interpolate(p), numeric()).is_err());
    }
    let a = CombinedBoxLength::from_computed(&a, numeric()).unwrap();
    for bad in [-1.0, f32::NAN, f32::INFINITY] {
        assert!(a.resolve(bad, 100.0, numeric()).is_err());
        assert!(a.resolve(100.0, bad, numeric()).is_err());
    }
}
