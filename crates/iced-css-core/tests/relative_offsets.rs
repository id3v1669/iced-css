use iced_css_core::formatting::{
    block::{AutoLength, Direction},
    relative::*,
};
fn input() -> RelativeInput {
    RelativeInput {
        left: AutoLength::Auto,
        right: AutoLength::Auto,
        top: AutoLength::Auto,
        bottom: AutoLength::Auto,
        containing_direction: Direction::Ltr,
    }
}
#[test]
fn relative_auto_pairs_and_containing_direction_choose_used_offsets() {
    let zero = relative_offsets(input()).unwrap();
    assert_eq!(
        (zero.left, zero.right, zero.top, zero.bottom),
        (0.0, 0.0, 0.0, 0.0)
    );
    let specified = RelativeInput {
        left: AutoLength::Value(-10.0),
        right: AutoLength::Value(25.0),
        top: AutoLength::Value(8.0),
        bottom: AutoLength::Value(9.0),
        ..input()
    };
    let ltr = relative_offsets(specified).unwrap();
    assert_eq!(
        (ltr.left, ltr.right, ltr.top, ltr.bottom),
        (-10.0, 10.0, 8.0, -8.0)
    );
    let rtl = relative_offsets(RelativeInput {
        containing_direction: Direction::Rtl,
        ..specified
    })
    .unwrap();
    assert_eq!(
        (rtl.left, rtl.right, rtl.top, rtl.bottom),
        (-25.0, 25.0, 8.0, -8.0)
    );
    let trailing = relative_offsets(RelativeInput {
        right: AutoLength::Value(12.0),
        bottom: AutoLength::Value(-15.0),
        ..input()
    })
    .unwrap();
    assert_eq!(
        (trailing.left, trailing.right, trailing.top, trailing.bottom),
        (-12.0, 12.0, 15.0, -15.0)
    );
}
#[test]
fn finite_extremes_subnormals_and_signed_zero_remain_finite_and_normalized() {
    for value in [
        f32::MAX,
        -f32::MAX,
        f32::from_bits(1),
        -f32::from_bits(1),
        0.0,
        -0.0,
    ] {
        let offsets = relative_offsets(RelativeInput {
            left: AutoLength::Value(value),
            top: AutoLength::Value(value),
            ..input()
        })
        .unwrap();
        for actual in [offsets.left, offsets.right, offsets.top, offsets.bottom] {
            assert!(actual.is_finite());
            if value == 0.0 {
                assert_eq!(actual.to_bits(), 0)
            } else {
                assert_eq!(actual.abs(), value.abs())
            }
        }
    }
}
#[test]
fn nonfinite_resolved_inputs_are_not_silently_ignored_as_overconstraint() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for field in ["left", "right", "top", "bottom"] {
            let mut candidate = RelativeInput {
                left: AutoLength::Value(1.0),
                top: AutoLength::Value(1.0),
                ..input()
            };
            match field {
                "left" => candidate.left = AutoLength::Value(value),
                "right" => candidate.right = AutoLength::Value(value),
                "top" => candidate.top = AutoLength::Value(value),
                "bottom" => candidate.bottom = AutoLength::Value(value),
                _ => unreachable!(),
            }
            assert_eq!(
                relative_offsets(candidate),
                Err(RelativeError::InvalidInput(field))
            );
        }
    }
}
