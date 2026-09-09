use iced_css_core::formatting::margins::*;
use std::num::NonZeroUsize;
fn work() -> NonZeroUsize {
    NonZeroUsize::new(100).unwrap()
}
fn group(values: &[f32]) -> MarginExtrema {
    let mut g = MarginExtrema::default();
    g.extend(values, work()).unwrap();
    g
}
#[test]
fn collapse_combines_extremes_instead_of_summing_members() {
    for (values, expected) in [
        (&[10.0, 20.0, -5.0][..], 15.0),
        (&[-10.0, -20.0][..], -20.0),
        (&[10.0, -20.0][..], -10.0),
        (&[0.0, -0.0][..], 0.0),
        (&[][..], 0.0),
    ] {
        assert_eq!(group(values).collapsed(), expected);
    }
    assert_eq!(group(&[-0.0]).collapsed().to_bits(), 0.0f32.to_bits());
}
#[test]
fn joining_adjoining_groups_preserves_both_extrema() {
    let mut first = group(&[10.0, -8.0]);
    let second = group(&[2.0, -15.0]);
    first.merge(second);
    assert_eq!(first.collapsed(), -5.0);
    assert_eq!(first.positive(), 10.0);
    assert_eq!(first.negative(), -15.0);
    assert_eq!(first, group(&[10.0, -8.0, 2.0, -15.0]));
    for order in [
        [10.0, -8.0, -15.0],
        [-15.0, 10.0, -8.0],
        [-8.0, -15.0, 10.0],
    ] {
        assert_eq!(group(&order), first);
    }
}
#[test]
fn invalid_or_over_budget_extension_is_atomic_and_extreme_finite_values_work() {
    let mut current = group(&[5.0]);
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            current.extend(&[10.0, invalid], work()),
            Err(MarginError::Nonfinite { index: 1 })
        );
        assert_eq!(current, group(&[5.0]));
    }
    assert_eq!(
        current.extend(&[10.0, 20.0], NonZeroUsize::new(1).unwrap()),
        Err(MarginError::Work)
    );
    assert_eq!(current, group(&[5.0]));
    assert_eq!(group(&[f32::MAX, -f32::MAX]).collapsed(), 0.0);
    assert_eq!(group(&[f32::MAX, f32::from_bits(1)]).collapsed(), f32::MAX);
    assert_eq!(group(&[-f32::MAX]).collapsed(), -f32::MAX);
}
