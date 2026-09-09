use iced_css_core::{
    animation::{
        combine::Combination,
        number::{Clamp, NumberError},
        text_spacing::*,
    },
    syntax::*,
    values::{
        LengthTarget, NumericLimits,
        color::ColorLimits,
        geometry::{LengthBasis, LengthContext, LengthEvaluationError},
        property::*,
        text_spacing::*,
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

fn computed(css: &str) -> ComputedTextSpacing {
    compute_text_spacing(&parsed(css), LengthContext::default(), numeric())
        .unwrap()
        .value
}
#[test]
fn normal_has_different_computed_meanings_for_letter_and_word_spacing() {
    assert_eq!(
        computed("letter-spacing:normal").value(),
        TextSpacingValue::Normal
    );
    assert_eq!(
        computed("word-spacing:normal").value(),
        TextSpacingValue::Pixels(0.0)
    );
    assert_ne!(
        computed("letter-spacing:normal"),
        computed("letter-spacing:0")
    );
    assert_eq!(computed("word-spacing:normal"), computed("word-spacing:0"));
    for (property, kind) in [
        ("letter-spacing", TextSpacingProperty::Letter),
        ("word-spacing", TextSpacingProperty::Word),
    ] {
        let result = compute_text_spacing(
            &parsed(&format!("{property}:-2ex")),
            LengthContext {
                font_size: Some(16.0),
                x_height: Some(7.0),
                percentage_basis: None,
            },
            numeric(),
        )
        .unwrap();
        assert_eq!(result.value.property(), kind);
        assert_eq!(result.value.value(), TextSpacingValue::Pixels(-14.0));
        assert!(!result.rounded_to_zero);
    }
}
#[test]
fn text_spacing_rejects_unresolved_invalid_and_missing_context_inputs() {
    for css in ["letter-spacing:inherit", "word-spacing:inherit", "width:0"] {
        assert_eq!(
            compute_text_spacing(&parsed(css), LengthContext::default(), numeric()).unwrap_err(),
            TextSpacingError::Unresolved
        );
    }
    assert_eq!(
        compute_text_spacing(
            &parsed("word-spacing:1em"),
            LengthContext::default(),
            numeric()
        )
        .unwrap_err(),
        TextSpacingError::Length(LengthEvaluationError::MissingContext(LengthBasis::FontSize))
    );
    for raw in ["auto", "10%"] {
        let mut p = parsed("letter-spacing:0");
        p.value = SpecifiedValue::Length(LengthTarget::parse(raw, numeric()).unwrap());
        assert_eq!(
            compute_text_spacing(&p, LengthContext::default(), numeric()).unwrap_err(),
            TextSpacingError::InvalidValue
        );
    }
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            ComputedTextSpacing::from_pixels(TextSpacingProperty::Letter, bad).unwrap_err(),
            TextSpacingError::InvalidComputed
        );
    }
}
#[test]
fn normal_letter_spacing_is_discrete_while_normal_word_spacing_is_numeric() {
    let l = computed("letter-spacing:normal");
    let ll = computed("letter-spacing:10px");
    let w = computed("word-spacing:normal");
    let wl = computed("word-spacing:10px");
    for (p, letter, word) in [
        (0.25, TextSpacingValue::Normal, 2.5),
        (0.5, TextSpacingValue::Pixels(10.0), 5.0),
    ] {
        assert_eq!(
            combine_text_spacing(l, ll, Combination::Interpolate(p))
                .unwrap()
                .resolve(-100.0, 100.0)
                .unwrap()
                .value,
            letter
        );
        assert_eq!(
            combine_text_spacing(w, wl, Combination::Interpolate(p))
                .unwrap()
                .resolve(-100.0, 100.0)
                .unwrap()
                .value,
            TextSpacingValue::Pixels(word)
        );
    }
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(
            combine_text_spacing(l, ll, op)
                .unwrap()
                .resolve(-100.0, 100.0)
                .unwrap()
                .value,
            TextSpacingValue::Normal
        );
        assert_eq!(
            combine_text_spacing(w, wl, op)
                .unwrap()
                .resolve(-100.0, 100.0)
                .unwrap()
                .value,
            TextSpacingValue::Pixels(10.0)
        );
    }
    assert_eq!(
        combine_text_spacing(l, w, Combination::Add).unwrap_err(),
        TextSpacingAnimationError::PropertyMismatch
    );
}
#[test]
fn signed_spacing_extrapolates_and_applies_only_explicit_final_limits() {
    let a = computed("letter-spacing:-10px");
    let b = computed("letter-spacing:10px");
    let sample = combine_text_spacing(a, b, Combination::Interpolate(-2.0))
        .unwrap()
        .resolve(-30.0, 30.0)
        .unwrap();
    assert_eq!(sample.value, TextSpacingValue::Pixels(-30.0));
    assert_eq!(sample.clamped, Some(Clamp::Lower));
    assert_eq!(sample.property, TextSpacingProperty::Letter);
    for p in [f32::NAN, f32::INFINITY] {
        assert_eq!(
            combine_text_spacing(a, b, Combination::Interpolate(p)).unwrap_err(),
            TextSpacingAnimationError::Number(NumberError::NonfiniteProgress)
        );
    }
    assert_eq!(
        combine_text_spacing(a, b, Combination::Add)
            .unwrap()
            .resolve(1.0, 0.0)
            .unwrap_err(),
        NumberError::InvalidRange
    );
}

#[test]
fn signed_underflow_and_reverse_keyword_combinations_preserve_their_distinctions() {
    for property in ["letter-spacing", "word-spacing"] {
        let css = format!("{property}:-0.{}1px", "0".repeat(60));
        let result =
            compute_text_spacing(&parsed(&css), LengthContext::default(), numeric()).unwrap();
        assert!(result.rounded_to_zero);
        let TextSpacingValue::Pixels(value) = result.value.value() else {
            panic!("computed length")
        };
        assert_eq!(value.to_bits(), 0);
    }
    let length = computed("letter-spacing:10px");
    let normal = computed("letter-spacing:normal");
    for operation in [
        Combination::Add,
        Combination::Accumulate,
        Combination::Interpolate(0.25),
    ] {
        assert_eq!(
            combine_text_spacing(length, normal, operation)
                .unwrap()
                .resolve(-100.0, 100.0)
                .unwrap()
                .value,
            TextSpacingValue::Pixels(10.0)
        );
    }
    assert_eq!(
        combine_text_spacing(length, normal, Combination::Interpolate(0.5))
            .unwrap()
            .resolve(-100.0, 100.0)
            .unwrap()
            .value,
        TextSpacingValue::Normal
    );
}
