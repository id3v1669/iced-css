use iced_css_core::{
    animation::{combine::Combination, legacy_color::*},
    values::NumericLimits,
};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn color(rgb: [f32; 3], alpha: f32) -> ResolvedLegacyColor {
    ResolvedLegacyColor::new(rgb, alpha).unwrap()
}
#[test]
fn inverse_uses_exact_alpha_before_rounding_or_final_range_enforcement() {
    let red = color([1.0, 0.0, 0.0], 1.0);
    let green = color([0.0, 1.0, 0.0], 0.5);
    let value = combine_legacy_color(red, green, Combination::Add, limits())
        .unwrap()
        .to_finite_unpremultiplied(limits())
        .unwrap();
    assert_eq!(
        value.map(|v| v.value.to_bits()),
        [0x3f2aaaab, 0x3eaaaaab, 0, 0x3fc00000]
    );
    assert!(value.iter().all(|v| !v.rounded_to_zero));
    let zero = combine_legacy_color(red, green, Combination::Interpolate(2.0), limits())
        .unwrap()
        .to_finite_unpremultiplied(limits())
        .unwrap();
    assert_eq!(zero.map(|v| v.value), [-1.0, 1.0, 0.0, 0.0]);
    let negative = combine_legacy_color(red, green, Combination::Interpolate(3.0), limits())
        .unwrap()
        .to_finite_unpremultiplied(limits())
        .unwrap();
    assert_eq!(negative.map(|v| v.value), [4.0, -3.0, 0.0, -0.5]);
    let transparent = color([0.0, 0.0, 1.0], 0.0);
    let half = combine_legacy_color(red, transparent, Combination::Interpolate(0.5), limits())
        .unwrap()
        .to_finite_unpremultiplied(limits())
        .unwrap();
    assert_eq!(half.map(|v| v.value), [1.0, 0.0, 0.0, 0.5]);
}
#[test]
fn a_nonzero_alpha_that_rounds_to_zero_still_unpremultiplies_channels() {
    let start = color([1.0, 0.0, 0.0], f32::from_bits(1));
    let end = color([0.0; 3], 0.0);
    let pair = combine_legacy_color(start, end, Combination::Interpolate(0.5), limits()).unwrap();
    let value = pair.to_finite_unpremultiplied(limits()).unwrap();
    assert_eq!(value.map(|v| v.value), [1.0, 0.0, 0.0, 0.0]);
    assert!(value[3].rounded_to_zero);
    assert!(!value[0].rounded_to_zero);
    let tiny = color([f32::from_bits(1), 0.0, 0.0], 0.5);
    let value = combine_legacy_color(tiny, tiny, Combination::Interpolate(0.5), limits())
        .unwrap()
        .to_finite_unpremultiplied(limits())
        .unwrap();
    assert_eq!(value[0].value.to_bits(), 1);
    assert!(!value[0].rounded_to_zero);
}
#[test]
fn failed_conversion_preserves_the_exact_pair() {
    let pair = combine_legacy_color(
        color([1.0, 0.0, 0.0], 1.0),
        color([0.0, 1.0, 0.0], 0.5),
        Combination::Interpolate(0.5),
        limits(),
    )
    .unwrap();
    let before = pair.to_finite_unpremultiplied(limits()).unwrap();
    let mut small = limits();
    small.work = NonZeroUsize::MIN;
    assert!(pair.to_finite_unpremultiplied(small).is_err());
    assert_eq!(pair.to_finite_unpremultiplied(limits()).unwrap(), before);
}
