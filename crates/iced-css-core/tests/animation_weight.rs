use iced_css_core::{
    animation::{combine::Combination, number::NumberError, weight::combine_font_weight},
    values::font_weight::FontWeight,
};
fn w(n: u16) -> FontWeight {
    FontWeight::new(n).unwrap()
}
#[test]
fn hundred_step_ties_round_up_in_both_directions() {
    for low in (100..900).step_by(100) {
        let a = w(low);
        let b = w(low + 100);
        for (start, end) in [(a, b), (b, a)] {
            assert_eq!(
                combine_font_weight(start, end, Combination::Interpolate(0.5)).unwrap(),
                b
            );
        }
        assert_eq!(
            combine_font_weight(
                a,
                b,
                Combination::Interpolate(f32::from_bits(0.5f32.to_bits() - 1))
            )
            .unwrap(),
            a
        );
        assert_eq!(
            combine_font_weight(
                b,
                a,
                Combination::Interpolate(f32::from_bits(0.5f32.to_bits() + 1))
            )
            .unwrap(),
            a
        );
    }
}
#[test]
fn weight_extrapolation_and_arithmetic_apply_the_final_weight_range() {
    for a in (100..=900).step_by(100) {
        for b in (100..=900).step_by(100) {
            for op in [Combination::Add, Combination::Accumulate] {
                assert_eq!(
                    combine_font_weight(w(a), w(b), op).unwrap(),
                    w((a + b).min(900))
                );
            }
            assert_eq!(
                combine_font_weight(w(a), w(b), Combination::Interpolate(0.0)).unwrap(),
                w(a)
            );
            assert_eq!(
                combine_font_weight(w(a), w(b), Combination::Interpolate(1.0)).unwrap(),
                w(b)
            );
        }
        for p in [-f32::MAX, f32::MAX] {
            assert_eq!(
                combine_font_weight(w(a), w(a), Combination::Interpolate(p)).unwrap(),
                w(a)
            );
        }
    }
    assert_eq!(
        combine_font_weight(w(100), w(900), Combination::Interpolate(-f32::MAX)).unwrap(),
        w(100)
    );
    assert_eq!(
        combine_font_weight(w(100), w(900), Combination::Interpolate(f32::MAX)).unwrap(),
        w(900)
    );
}
#[test]
fn weight_combination_rejects_nonfinite_progress() {
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_font_weight(w(400), w(700), Combination::Interpolate(p)).unwrap_err(),
            NumberError::NonfiniteProgress
        );
    }
}
