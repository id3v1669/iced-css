use iced_css_core::formatting::{
    clearance::{
        ClearSide, ClearanceFloatError, ClearanceState, lowest_clearance_float_bottom,
        solve_clearance,
    },
    floats::{FloatSide, PlacedFloat},
};
use std::num::NonZeroUsize;

fn budget(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn float(side: FloatSide, bottom: f32) -> PlacedFloat {
    PlacedFloat {
        side,
        left: 0.0,
        right: 1.0,
        top: bottom,
        bottom,
    }
}

#[test]
fn sides_select_the_lowest_matching_outer_bottom() {
    let floats = [
        float(FloatSide::Left, -5.0),
        float(FloatSide::Left, 8.0),
        float(FloatSide::Right, 12.0),
    ];
    for (side, expected) in [
        (ClearSide::None, None),
        (ClearSide::Left, Some(8.0)),
        (ClearSide::Right, Some(12.0)),
        (ClearSide::Both, Some(12.0)),
    ] {
        assert_eq!(
            lowest_clearance_float_bottom(&floats, side, budget(3)),
            Ok(expected)
        );
    }
}

#[test]
fn empty_and_nonmatching_slices_have_no_constraint() {
    for side in [
        ClearSide::None,
        ClearSide::Left,
        ClearSide::Right,
        ClearSide::Both,
    ] {
        assert_eq!(
            lowest_clearance_float_bottom(&[], side, budget(1)),
            Ok(None)
        );
    }
    assert_eq!(
        lowest_clearance_float_bottom(&[float(FloatSide::Left, 0.0)], ClearSide::Right, budget(1)),
        Ok(None)
    );
    assert_eq!(
        lowest_clearance_float_bottom(&[float(FloatSide::Right, 0.0)], ClearSide::Left, budget(1)),
        Ok(None)
    );
}

#[test]
fn all_negative_duplicate_and_extreme_bottoms_are_preserved() {
    let floats = [
        float(FloatSide::Left, -5.0),
        float(FloatSide::Right, -2.0),
        float(FloatSide::Left, -2.0),
    ];
    assert_eq!(
        lowest_clearance_float_bottom(&floats, ClearSide::Both, budget(3)),
        Ok(Some(-2.0))
    );
    for value in [-f32::MAX, f32::MAX, f32::from_bits(1)] {
        assert_eq!(
            lowest_clearance_float_bottom(
                &[float(FloatSide::Left, value)],
                ClearSide::Both,
                budget(1)
            ),
            Ok(Some(value))
        );
    }
}

#[test]
fn real_zero_is_present_and_has_canonical_bits() {
    for bottoms in [[-0.0, 0.0], [0.0, -0.0], [-0.0, -0.0]] {
        let floats = bottoms.map(|b| float(FloatSide::Left, b));
        let actual = lowest_clearance_float_bottom(&floats, ClearSide::Left, budget(2))
            .unwrap()
            .unwrap();
        assert_eq!(actual.to_bits(), 0.0_f32.to_bits());
    }
}

#[test]
fn degenerate_floats_are_not_filtered_by_occupancy() {
    let floats = [
        PlacedFloat {
            top: 1.0,
            ..float(FloatSide::Left, 8.0)
        },
        PlacedFloat {
            left: 2.0,
            right: 2.0,
            ..float(FloatSide::Left, 20.0)
        },
    ];
    assert_eq!(
        lowest_clearance_float_bottom(&floats, ClearSide::Left, budget(2)),
        Ok(Some(20.0))
    );
    // Isolate zero width with positive height, then zero height with positive width.
    for (left, right, top) in [(2.0, 2.0, 10.0), (2.0, 3.0, 20.0)] {
        let mut variant = floats;
        variant[1] = PlacedFloat {
            left,
            right,
            top,
            ..float(FloatSide::Left, 20.0)
        };
        assert_eq!(
            lowest_clearance_float_bottom(&variant, ClearSide::Left, budget(2)),
            Ok(Some(20.0))
        );
    }
}

#[test]
fn unused_geometry_is_not_certified_or_inspected() {
    let floats = [PlacedFloat {
        left: f32::NAN,
        right: f32::INFINITY,
        top: f32::NEG_INFINITY,
        ..float(FloatSide::Right, 6.0)
    }];
    assert_eq!(
        lowest_clearance_float_bottom(&floats, ClearSide::Both, budget(1)),
        Ok(Some(6.0))
    );
}

#[test]
fn every_inspected_bottom_must_be_finite_even_on_the_other_side() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for index in 0..3 {
            let mut floats = [float(FloatSide::Right, 1.0); 3];
            floats[index].bottom = bad;
            assert_eq!(
                lowest_clearance_float_bottom(&floats, ClearSide::Left, budget(3)),
                Err(ClearanceFloatError::InvalidBottom { index })
            );
        }
    }
    let floats = [
        float(FloatSide::Left, f32::NAN),
        float(FloatSide::Right, f32::INFINITY),
    ];
    assert_eq!(
        lowest_clearance_float_bottom(&floats, ClearSide::Both, budget(2)),
        Err(ClearanceFloatError::InvalidBottom { index: 0 })
    );
}

#[test]
fn budget_is_preflighted_over_all_records_before_validation() {
    let floats = [float(FloatSide::Right, 1.0); 3];
    assert_eq!(
        lowest_clearance_float_bottom(&floats, ClearSide::Left, budget(2)),
        Err(ClearanceFloatError::Work)
    );
    assert_eq!(
        lowest_clearance_float_bottom(&floats, ClearSide::Left, budget(3)),
        Ok(None)
    );
    let invalid = [float(FloatSide::Left, f32::NAN); 3];
    assert_eq!(
        lowest_clearance_float_bottom(&invalid, ClearSide::Both, budget(2)),
        Err(ClearanceFloatError::Work)
    );
    assert_eq!(
        lowest_clearance_float_bottom(&invalid, ClearSide::None, budget(1)),
        Ok(None)
    );
}

#[test]
fn scalar_reduction_is_permutation_invariant() {
    let base = [
        float(FloatSide::Left, -5.0),
        float(FloatSide::Right, 12.0),
        float(FloatSide::Left, 8.0),
    ];
    // Arithmetic invariance only: callers still establish semantic source eligibility.
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let floats = order.map(|i| base[i]);
        for (side, expected) in [
            (ClearSide::Left, 8.0),
            (ClearSide::Right, 12.0),
            (ClearSide::Both, 12.0),
        ] {
            assert_eq!(
                lowest_clearance_float_bottom(&floats, side, budget(3)),
                Ok(Some(expected))
            );
        }
    }
}

#[test]
fn independently_supplied_contexts_and_explicit_clearance_compose() {
    // Membership is provided by the caller, not inferred from PlacedFloat geometry.
    let first = [float(FloatSide::Left, 6.0), float(FloatSide::Right, 100.0)];
    let second = [float(FloatSide::Left, 20.0)];
    let bottom = lowest_clearance_float_bottom(&first, ClearSide::Left, budget(2))
        .unwrap()
        .unwrap();
    assert_eq!(
        lowest_clearance_float_bottom(&second, ClearSide::Left, budget(1)),
        Ok(Some(20.0))
    );
    let placement = solve_clearance(
        4.0,
        ClearanceState::Required {
            zero_clearance_border_top: 7.0,
            lowest_float_bottom: bottom,
        },
    )
    .unwrap();
    assert_eq!(placement.border_top, 6.0);
    assert_eq!(placement.clearance, Some(-1.0));
}
