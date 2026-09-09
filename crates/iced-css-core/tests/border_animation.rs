use iced_css_core::{
    animation::{
        border::*,
        combine::Combination,
        number::{Clamp, NumberError},
    },
    formatting::table_borders::BorderStyle,
};
const STYLES: [BorderStyle; 10] = [
    BorderStyle::None,
    BorderStyle::Hidden,
    BorderStyle::Inset,
    BorderStyle::Groove,
    BorderStyle::Outset,
    BorderStyle::Ridge,
    BorderStyle::Dotted,
    BorderStyle::Dashed,
    BorderStyle::Solid,
    BorderStyle::Double,
];
#[test]
fn all_style_pairs_follow_discrete_semantics() {
    for a in STYLES {
        for b in STYLES {
            for (op, expected) in [
                (Combination::Interpolate(-f32::MAX), a),
                (
                    Combination::Interpolate(f32::from_bits(0.5_f32.to_bits() - 1)),
                    a,
                ),
                (Combination::Interpolate(0.5), b),
                (Combination::Interpolate(f32::MAX), b),
                (Combination::Add, a),
                (Combination::Accumulate, a),
            ] {
                assert_eq!(combine_border_style(a, b, op).unwrap(), expected);
            }
            for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert!(combine_border_style(a, b, Combination::Interpolate(p)).is_err());
            }
        }
    }
}
#[test]
fn presentation_style_suppresses_width_without_mutating_the_pair_result() {
    let combined = combine_border_width(10.0, 30.0, Combination::Interpolate(0.5)).unwrap();
    for style in STYLES {
        let sample = combined.resolve(style, 100.0).unwrap();
        let suppressed = matches!(style, BorderStyle::None | BorderStyle::Hidden);
        assert_eq!(sample.value, if suppressed { 0.0 } else { 20.0 });
        assert_eq!(sample.suppressed_by_style, suppressed);
        assert_eq!(sample.clamped, None);
    }
    let extrapolated = combine_border_width(10.0, 30.0, Combination::Interpolate(-1.0)).unwrap();
    let hidden = extrapolated.resolve(BorderStyle::Hidden, 100.0).unwrap();
    assert_eq!(hidden.value, 0.0);
    assert!(hidden.suppressed_by_style);
    assert_eq!(hidden.clamped, None);
    let visible = extrapolated.resolve(BorderStyle::Solid, 100.0).unwrap();
    assert_eq!(visible.value, 0.0);
    assert!(!visible.suppressed_by_style);
    assert_eq!(visible.clamped, Some(Clamp::Lower));
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(
            combine_border_width(10.0, 30.0, op)
                .unwrap()
                .resolve(BorderStyle::Solid, 25.0)
                .unwrap()
                .clamped,
            Some(Clamp::Upper)
        );
    }
    assert_eq!(
        combine_border_width(f32::MAX, f32::MAX, Combination::Interpolate(f32::MAX))
            .unwrap()
            .resolve(BorderStyle::Solid, f32::MAX)
            .unwrap()
            .value,
        f32::MAX
    );
}
#[test]
fn invalid_width_inputs_cannot_be_hidden_by_style_suppression() {
    for v in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_border_width(v, 10.0, Combination::Interpolate(0.0)).unwrap_err(),
            BorderAnimationError::InvalidEndpoint
        );
        assert_eq!(
            combine_border_width(10.0, v, Combination::Interpolate(0.0)).unwrap_err(),
            BorderAnimationError::InvalidEndpoint
        );
    }
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_border_width(1.0, 2.0, Combination::Interpolate(p)).unwrap_err(),
            BorderAnimationError::Number(NumberError::NonfiniteProgress)
        );
    }
    let width = combine_border_width(-0.0, 0.0, Combination::Add).unwrap();
    for max in [-1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            width.resolve(BorderStyle::Hidden, max).unwrap_err(),
            NumberError::InvalidRange
        );
    }
    assert_eq!(
        width
            .resolve(BorderStyle::Solid, 0.0)
            .unwrap()
            .value
            .to_bits(),
        0
    );
}
