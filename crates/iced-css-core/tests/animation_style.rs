use iced_css_core::{
    animation::{
        combine::{Combination, CombinationError},
        number::Clamp,
        style::*,
    },
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

#[test]
fn base_style_keywords_require_a_resolved_font_style_property() {
    for (css, expected) in [
        ("font-style:normal", FontStyleKeyword::Normal),
        ("font-style:italic", FontStyleKeyword::Italic),
        ("font-style:oblique", FontStyleKeyword::Oblique),
    ] {
        assert_eq!(FontStyleKeyword::try_from(&parsed(css)).unwrap(), expected);
    }
    for css in ["font-style:inherit", "font-variant:normal"] {
        assert_eq!(
            FontStyleKeyword::try_from(&parsed(css)).unwrap_err(),
            CombinationError::UnresolvedFontStyle
        );
    }
}
#[test]
fn every_keyword_pair_has_the_selected_midpoint_and_addition_behavior() {
    use FontStyleKeyword::{Italic as I, Normal as N, Oblique as O};
    use FontStyleValue::{Italic, ObliqueDegrees as A};
    for (a, b, mid, added) in [
        (N, N, A(0.0), A(0.0)),
        (N, O, A(7.0), A(14.0)),
        (O, N, A(7.0), A(14.0)),
        (O, O, A(14.0), A(28.0)),
        (I, I, Italic, Italic),
        (I, N, A(0.0), Italic),
        (I, O, A(14.0), Italic),
        (N, I, Italic, A(0.0)),
        (O, I, Italic, A(14.0)),
    ] {
        let result = combine_font_style(a, b, Combination::Interpolate(0.5))
            .unwrap()
            .resolve();
        assert_eq!(result.value, mid);
        assert_eq!(result.clamped, None);
        for op in [Combination::Add, Combination::Accumulate] {
            let result = combine_font_style(a, b, op).unwrap().resolve();
            assert_eq!(result.value, added);
            assert_eq!(result.clamped, None);
        }
    }
}
#[test]
fn style_angles_extrapolate_then_clamp_and_discrete_pairs_keep_the_midpoint() {
    use FontStyleKeyword::{Italic as I, Normal as N, Oblique as O};
    for (p, expected, clamped) in [
        (-1.0, -14.0, None),
        (2.0, 28.0, None),
        (-f32::MAX, -90.0, Some(Clamp::Lower)),
        (f32::MAX, 90.0, Some(Clamp::Upper)),
    ] {
        let result = combine_font_style(N, O, Combination::Interpolate(p))
            .unwrap()
            .resolve();
        assert_eq!(result.value, FontStyleValue::ObliqueDegrees(expected));
        assert_eq!(result.clamped, clamped);
    }
    let below = f32::from_bits(0.5f32.to_bits() - 1);
    assert_eq!(
        combine_font_style(I, N, Combination::Interpolate(below))
            .unwrap()
            .resolve()
            .value,
        FontStyleValue::Italic
    );
    assert_eq!(
        combine_font_style(N, I, Combination::Interpolate(below))
            .unwrap()
            .resolve()
            .value,
        FontStyleValue::ObliqueDegrees(0.0)
    );
    for a in [N, I, O] {
        for b in [N, I, O] {
            for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert_eq!(
                    combine_font_style(a, b, Combination::Interpolate(p)).unwrap_err(),
                    CombinationError::NonfiniteProgress
                );
            }
        }
    }
}

#[test]
fn interruption_preserves_the_presented_angle_instead_of_a_keyword() {
    let captured = combine_font_style(
        FontStyleKeyword::Normal,
        FontStyleKeyword::Oblique,
        Combination::Interpolate(0.5),
    )
    .unwrap()
    .resolve();
    assert_eq!(captured.value, FontStyleValue::ObliqueDegrees(7.0));
    let start = FontStyleEndpoint::try_from(captured.value).unwrap();
    let end = FontStyleEndpoint::from(FontStyleKeyword::Normal);
    for (p, want) in [(0.0, 7.0), (0.5, 3.5), (1.0, 0.0), (2.0, -7.0)] {
        let sample = combine_font_style_endpoints(start, end, Combination::Interpolate(p))
            .unwrap()
            .resolve();
        assert_eq!(sample.value, FontStyleValue::ObliqueDegrees(want));
        assert_eq!(sample.clamped, None);
    }
    let low = FontStyleEndpoint::try_from(FontStyleValue::ObliqueDegrees(-90.0)).unwrap();
    let high = FontStyleEndpoint::try_from(FontStyleValue::ObliqueDegrees(90.0)).unwrap();
    assert_eq!(
        combine_font_style_endpoints(low, high, Combination::Interpolate(0.5))
            .unwrap()
            .resolve()
            .value,
        FontStyleValue::ObliqueDegrees(0.0)
    );
    let sample = combine_font_style_endpoints(low, high, Combination::Interpolate(f32::MAX))
        .unwrap()
        .resolve();
    assert_eq!(sample.value, FontStyleValue::ObliqueDegrees(90.0));
    assert_eq!(sample.clamped, Some(Clamp::Upper));
}
#[test]
fn presented_angle_pairs_keep_italic_fallback_and_final_range_events() {
    let angle = FontStyleEndpoint::try_from(FontStyleValue::ObliqueDegrees(-7.0)).unwrap();
    let italic = FontStyleEndpoint::from(FontStyleKeyword::Italic);
    for (a, b, before, after) in [
        (
            angle,
            italic,
            FontStyleValue::ObliqueDegrees(-7.0),
            FontStyleValue::Italic,
        ),
        (
            italic,
            angle,
            FontStyleValue::Italic,
            FontStyleValue::ObliqueDegrees(-7.0),
        ),
    ] {
        for (p, want) in [(0.25, before), (0.5, after)] {
            assert_eq!(
                combine_font_style_endpoints(a, b, Combination::Interpolate(p))
                    .unwrap()
                    .resolve()
                    .value,
                want
            );
        }
        for op in [Combination::Add, Combination::Accumulate] {
            assert_eq!(
                combine_font_style_endpoints(a, b, op)
                    .unwrap()
                    .resolve()
                    .value,
                before
            );
        }
    }
    let large = FontStyleEndpoint::try_from(FontStyleValue::ObliqueDegrees(70.0)).unwrap();
    for op in [Combination::Add, Combination::Accumulate] {
        let sample = combine_font_style_endpoints(large, large, op)
            .unwrap()
            .resolve();
        assert_eq!(sample.value, FontStyleValue::ObliqueDegrees(90.0));
        assert_eq!(sample.clamped, Some(Clamp::Upper));
    }
}
#[test]
fn presented_style_admission_rejects_invalid_angles_and_nonfinite_progress() {
    for value in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        90.00001,
        -90.00001,
    ] {
        assert!(FontStyleEndpoint::try_from(FontStyleValue::ObliqueDegrees(value)).is_err());
    }
    let zero = FontStyleEndpoint::try_from(FontStyleValue::ObliqueDegrees(-0.0)).unwrap();
    let italic = FontStyleEndpoint::from(FontStyleKeyword::Italic);
    let sample = combine_font_style_endpoints(zero, italic, Combination::Interpolate(0.25))
        .unwrap()
        .resolve();
    let FontStyleValue::ObliqueDegrees(angle) = sample.value else {
        panic!()
    };
    assert_eq!(angle.to_bits(), 0);
    for a in [zero, italic] {
        for b in [zero, italic] {
            for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert_eq!(
                    combine_font_style_endpoints(a, b, Combination::Interpolate(p)).unwrap_err(),
                    CombinationError::NonfiniteProgress
                );
            }
        }
    }
}
