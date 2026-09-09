use iced_css_core::formatting::{
    absolute::*,
    block::{AutoLength, Direction},
};
fn value(v: f32) -> AutoLength {
    AutoLength::Value(v)
}
fn input() -> AbsoluteWidthInput {
    AbsoluteWidthInput {
        containing_width: 300.0,
        containing_direction: Direction::Ltr,
        static_direction: Direction::Ltr,
        static_left: 20.0,
        static_right: 30.0,
        left: AutoLength::Auto,
        width: AutoLength::Auto,
        right: AutoLength::Auto,
        margin_left: AutoLength::Auto,
        margin_right: AutoLength::Auto,
        border_left: 2.0,
        padding_left: 3.0,
        padding_right: 5.0,
        border_right: 7.0,
        preferred_min_width: 40.0,
        preferred_width: 120.0,
        min_width: 0.0,
        max_width: None,
    }
}
#[test]
fn all_auto_uses_static_position_direction_and_shrink_to_fit() {
    let ltr = absolute_width(input()).unwrap();
    assert_eq!(
        (
            ltr.left,
            ltr.width,
            ltr.right,
            ltr.margin_left,
            ltr.margin_right
        ),
        (20.0, 120.0, 143.0, 0.0, 0.0)
    );
    let rtl = absolute_width(AbsoluteWidthInput {
        static_direction: Direction::Rtl,
        ..input()
    })
    .unwrap();
    assert_eq!((rtl.left, rtl.width, rtl.right), (133.0, 120.0, 30.0));
    let small = absolute_width(AbsoluteWidthInput {
        containing_width: 60.0,
        ..input()
    })
    .unwrap();
    assert_eq!((small.left, small.width, small.right), (20.0, 40.0, -17.0));
}
#[test]
fn all_six_partial_auto_rules_solve_the_constraint() {
    for (left, width, right, expected) in [
        (
            AutoLength::Auto,
            AutoLength::Auto,
            value(70.0),
            (93.0, 120.0, 70.0),
        ),
        (
            AutoLength::Auto,
            value(100.0),
            AutoLength::Auto,
            (20.0, 100.0, 163.0),
        ),
        (
            value(50.0),
            AutoLength::Auto,
            AutoLength::Auto,
            (50.0, 120.0, 113.0),
        ),
        (
            AutoLength::Auto,
            value(100.0),
            value(70.0),
            (113.0, 100.0, 70.0),
        ),
        (
            value(50.0),
            AutoLength::Auto,
            value(70.0),
            (50.0, 163.0, 70.0),
        ),
        (
            value(50.0),
            value(100.0),
            AutoLength::Auto,
            (50.0, 100.0, 133.0),
        ),
    ] {
        let result = absolute_width(AbsoluteWidthInput {
            left,
            width,
            right,
            ..input()
        })
        .unwrap();
        assert_eq!((result.left, result.width, result.right), expected);
    }
}
#[test]
fn auto_margin_centering_has_directional_negative_space_exception() {
    let centered = absolute_width(AbsoluteWidthInput {
        left: value(50.0),
        width: value(100.0),
        right: value(70.0),
        ..input()
    })
    .unwrap();
    assert_eq!((centered.margin_left, centered.margin_right), (31.5, 31.5));
    for (direction, expected) in [
        (Direction::Ltr, (0.0, -117.0)),
        (Direction::Rtl, (-117.0, 0.0)),
    ] {
        let result = absolute_width(AbsoluteWidthInput {
            left: value(150.0),
            width: value(100.0),
            right: value(150.0),
            containing_direction: direction,
            ..input()
        })
        .unwrap();
        assert_eq!((result.margin_left, result.margin_right), expected);
    }
}
#[test]
fn min_max_reruns_and_directional_overconstraint_recompute_other_values() {
    let fixed = AbsoluteWidthInput {
        left: value(50.0),
        width: value(100.0),
        right: value(70.0),
        ..input()
    };
    let result = absolute_width(AbsoluteWidthInput {
        min_width: 150.0,
        max_width: Some(80.0),
        ..fixed
    })
    .unwrap();
    assert_eq!(
        (
            result.width,
            result.margin_left,
            result.margin_right,
            result.passes
        ),
        (150.0, 6.5, 6.5, 3)
    );
    for (direction, expected) in [
        (Direction::Ltr, (50.0, 103.0)),
        (Direction::Rtl, (83.0, 70.0)),
    ] {
        let result = absolute_width(AbsoluteWidthInput {
            margin_left: value(10.0),
            margin_right: value(20.0),
            containing_direction: direction,
            ..fixed
        })
        .unwrap();
        assert_eq!((result.left, result.right), expected);
    }
    let result = absolute_width(AbsoluteWidthInput {
        left: value(200.0),
        right: value(200.0),
        ..input()
    })
    .unwrap();
    assert_eq!(
        (
            result.width,
            result.margin_left,
            result.margin_right,
            result.passes
        ),
        (0.0, 0.0, -117.0, 2)
    );
}
#[test]
fn finite_cancellation_and_final_range_failures_are_distinct() {
    let base = AbsoluteWidthInput {
        containing_width: 0.0,
        left: value(f32::MAX),
        width: value(0.0),
        right: value(f32::MAX),
        margin_left: value(-f32::MAX),
        margin_right: value(-f32::MAX),
        border_left: 0.0,
        padding_left: 0.0,
        padding_right: 0.0,
        border_right: 0.0,
        ..input()
    };
    let result = absolute_width(base).unwrap();
    assert_eq!(
        (result.left, result.right, result.width),
        (f32::MAX, f32::MAX, 0.0)
    );
    assert_eq!(
        absolute_width(AbsoluteWidthInput {
            margin_left: value(f32::MAX),
            margin_right: value(0.0),
            ..base
        }),
        Err(AbsoluteWidthError::Unrepresentable)
    );
    assert_eq!(
        absolute_width(AbsoluteWidthInput {
            preferred_min_width: 121.0,
            ..input()
        }),
        Err(AbsoluteWidthError::InvalidInput("intrinsic_width_order"))
    );
}

#[test]
fn one_auto_margin_and_distinct_static_direction_follow_their_own_rules() {
    let fixed = AbsoluteWidthInput {
        left: value(50.0),
        width: value(100.0),
        right: value(70.0),
        ..input()
    };
    let left = absolute_width(AbsoluteWidthInput {
        margin_right: value(10.0),
        ..fixed
    })
    .unwrap();
    assert_eq!((left.margin_left, left.margin_right), (53.0, 10.0));
    let right = absolute_width(AbsoluteWidthInput {
        margin_left: value(20.0),
        ..fixed
    })
    .unwrap();
    assert_eq!((right.margin_left, right.margin_right), (20.0, 43.0));
    let static_rtl = absolute_width(AbsoluteWidthInput {
        width: value(100.0),
        static_direction: Direction::Rtl,
        containing_direction: Direction::Ltr,
        ..input()
    })
    .unwrap();
    assert_eq!(
        (static_rtl.left, static_rtl.width, static_rtl.right),
        (153.0, 100.0, 30.0)
    );
}
