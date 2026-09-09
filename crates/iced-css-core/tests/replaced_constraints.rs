use iced_css_core::formatting::replaced_constraints::{
    ConstraintMode, ReplacedConstraintError, ReplacedConstraintInput, constrain_replaced_size,
};
fn input(width: f32, height: f32) -> ReplacedConstraintInput {
    ReplacedConstraintInput {
        width,
        height,
        min_width: 40.0,
        max_width: Some(100.0),
        min_height: 30.0,
        max_height: Some(90.0),
        mode: ConstraintMode::IntrinsicRatio,
    }
}
#[test]
fn every_css_ratio_table_row_has_literal_used_dimensions() {
    for (w, h, want_w, want_h) in [
        (60.0, 60.0, 60.0, 60.0),
        (120.0, 60.0, 100.0, 50.0),
        (20.0, 40.0, 40.0, 80.0),
        (60.0, 120.0, 45.0, 90.0),
        (60.0, 20.0, 90.0, 30.0),
        (200.0, 120.0, 100.0, 60.0),
        (120.0, 180.0, 60.0, 90.0),
        (20.0, 10.0, 60.0, 30.0),
        (10.0, 20.0, 40.0, 80.0),
        (20.0, 120.0, 40.0, 90.0),
        (120.0, 20.0, 100.0, 30.0),
    ] {
        let actual = constrain_replaced_size(input(w, h)).unwrap();
        assert_eq!((actual.width, actual.height), (want_w, want_h), "{w} x {h}");
    }
}
#[test]
fn ratio_scaling_obeys_the_opposite_axis_bound() {
    for (w, h, ww, hh) in [
        (1000.0, 40.0, 100.0, 30.0),
        (10.0, 60.0, 40.0, 90.0),
        (50.0, 1000.0, 40.0, 90.0),
        (90.0, 10.0, 100.0, 30.0),
    ] {
        let actual = constrain_replaced_size(input(w, h)).unwrap();
        assert_eq!((actual.width, actual.height), (ww, hh));
    }
}
#[test]
fn independent_and_zero_dimension_routes_do_not_invent_a_ratio() {
    let actual = constrain_replaced_size(ReplacedConstraintInput {
        mode: ConstraintMode::Independent,
        ..input(120.0, 60.0)
    })
    .unwrap();
    assert_eq!((actual.width, actual.height), (100.0, 60.0));
    for (w, h, ww, hh) in [
        (0.0, 60.0, 40.0, 60.0),
        (60.0, 0.0, 60.0, 30.0),
        (0.0, 0.0, 40.0, 30.0),
    ] {
        let actual = constrain_replaced_size(input(w, h)).unwrap();
        assert_eq!((actual.width, actual.height), (ww, hh));
    }
    let actual = constrain_replaced_size(ReplacedConstraintInput {
        min_width: 120.0,
        max_width: Some(10.0),
        min_height: 70.0,
        max_height: Some(20.0),
        ..input(60.0, 60.0)
    })
    .unwrap();
    assert_eq!((actual.width, actual.height), (120.0, 70.0));
}
#[test]
fn unbounded_axes_extreme_products_and_subnormal_rounding_remain_explicit() {
    let base = ReplacedConstraintInput {
        min_width: 0.0,
        max_width: None,
        min_height: 0.0,
        max_height: None,
        ..input(1.0, 1.0)
    };
    let actual = constrain_replaced_size(ReplacedConstraintInput {
        width: f32::MAX,
        height: f32::MAX,
        max_width: Some(f32::MAX / 2.0),
        ..base
    })
    .unwrap();
    assert_eq!(
        (actual.width, actual.height),
        (f32::MAX / 2.0, f32::MAX / 2.0)
    );
    let tiny = f32::from_bits(1);
    let actual = constrain_replaced_size(ReplacedConstraintInput {
        width: 2.0,
        height: tiny,
        max_width: Some(1.0),
        ..base
    })
    .unwrap();
    assert_eq!(
        (actual.width.to_bits(), actual.height.to_bits()),
        (1.0f32.to_bits(), 0)
    );
    let actual = constrain_replaced_size(ReplacedConstraintInput {
        width: 2.0,
        height: f32::from_bits(3),
        max_width: Some(1.0),
        ..base
    })
    .unwrap();
    assert_eq!(actual.height.to_bits(), 2);
    assert_eq!(
        constrain_replaced_size(ReplacedConstraintInput {
            width: tiny,
            height: f32::MAX,
            min_width: 1.0,
            ..base
        }),
        Err(ReplacedConstraintError::Unrepresentable)
    );
    let actual = constrain_replaced_size(ReplacedConstraintInput {
        width: -0.0,
        height: -0.0,
        ..base
    })
    .unwrap();
    assert_eq!((actual.width.to_bits(), actual.height.to_bits()), (0, 0));
}
#[test]
fn nonfinite_and_negative_dimensions_fail_before_ratio_operations() {
    for invalid in [
        ReplacedConstraintInput {
            width: -1.0,
            ..input(1.0, 1.0)
        },
        ReplacedConstraintInput {
            height: f32::INFINITY,
            ..input(1.0, 1.0)
        },
        ReplacedConstraintInput {
            min_width: f32::NAN,
            ..input(1.0, 1.0)
        },
        ReplacedConstraintInput {
            max_height: Some(-1.0),
            ..input(1.0, 1.0)
        },
    ] {
        assert!(matches!(
            constrain_replaced_size(invalid),
            Err(ReplacedConstraintError::InvalidInput(_))
        ));
    }
}
