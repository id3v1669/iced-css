use iced_css_core::{
    animation::{combine::Combination, legacy_color::*},
    values::{Decimal, NumericLimits},
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
fn expected(values: [&str; 4]) -> [Decimal; 4] {
    values.map(|v| Decimal::parse(v, limits()).unwrap())
}
#[test]
fn premultiplied_pair_retains_alpha_weighting_and_unclamped_results() {
    let red = color([1.0, 0.0, 0.0], 1.0);
    let blue = color([0.0, 0.0, 1.0], 0.0);
    assert_eq!(
        combine_legacy_color(red, blue, Combination::Interpolate(0.5), limits())
            .unwrap()
            .components(),
        &expected(["0.5", "0", "0", "0.5"])
    );
    let green = color([0.0, 1.0, 0.0], 0.5);
    assert_eq!(
        combine_legacy_color(red, green, Combination::Interpolate(2.0), limits())
            .unwrap()
            .components(),
        &expected(["-1", "1", "0", "0"])
    );
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(
            combine_legacy_color(red, green, op, limits())
                .unwrap()
                .components(),
            &expected(["1", "0.5", "0", "1.5"])
        );
    }
    let extended = color([-2.0, 3.0, 0.0], 1.0);
    assert_eq!(
        combine_legacy_color(extended, extended, Combination::Interpolate(0.5), limits())
            .unwrap()
            .components(),
        &expected(["-2", "3", "0", "1"])
    );
}
#[test]
fn subnormal_premultiplication_and_extreme_progress_do_not_round_early() {
    let tiny = color([f32::from_bits(1), 0.0, 0.0], 0.5);
    let e = expected([
        "0.000000000000000000000000000000000000000000000700649232162408535461864791644958065640130970938257885878534141944895541342930300743319094181060791015625",
        "0",
        "0",
        "0.5",
    ]);
    for p in [0.0, 0.5, 1.0, f32::MAX, -f32::MAX, f32::from_bits(1)] {
        assert_eq!(
            combine_legacy_color(tiny, tiny, Combination::Interpolate(p), limits())
                .unwrap()
                .components(),
            &e
        );
    }
    let huge = color([f32::MAX, -f32::MAX, 0.0], 1.0);
    let e = expected([
        "340282346638528859811704183484516925440",
        "-340282346638528859811704183484516925440",
        "0",
        "1",
    ]);
    assert_eq!(
        combine_legacy_color(huge, huge, Combination::Interpolate(f32::MAX), limits())
            .unwrap()
            .components(),
        &e
    );
}
#[test]
fn endpoint_and_resource_errors_are_explicit() {
    for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(ResolvedLegacyColor::new([v, 0.0, 0.0], 1.0).is_err());
    }
    for a in [-1.0, 1.1, f32::NAN, f32::INFINITY] {
        assert!(ResolvedLegacyColor::new([0.0; 3], a).is_err());
    }
    let a = color([1.0, 0.0, 0.0], 1.0);
    let b = color([0.0, 0.0, 1.0], 0.0);
    let mut small = limits();
    small.work = NonZeroUsize::MIN;
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_legacy_color(a, b, Combination::Interpolate(p), small).unwrap_err(),
            ColorCombinationError::NonfiniteProgress
        );
    }
    assert!(combine_legacy_color(a, b, Combination::Interpolate(0.5), small).is_err());
    small = limits();
    small.digits = NonZeroUsize::MIN;
    assert!(
        combine_legacy_color(
            color([f32::from_bits(1), 0.0, 0.0], 1.0),
            b,
            Combination::Add,
            small
        )
        .is_err()
    );
    assert_eq!(a.rgb(), [1.0, 0.0, 0.0]);
    assert_eq!(a.alpha(), 1.0);
}
