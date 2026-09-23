use iced_css_core::formatting::floats::{Band, FloatError, FloatRequest, FloatSide, place_float};
use std::num::NonZeroUsize;

#[test]
fn finite_inputs_cannot_publish_infinite_float_bottom() {
    let result = place_float(
        &[],
        Band {
            left: 0.0,
            right: 10.0,
        },
        FloatRequest {
            side: FloatSide::Left,
            width: 1.0,
            height: f32::MAX,
            containing_top: 0.0,
            hypothetical_top: f32::MAX,
        },
        NonZeroUsize::new(1).unwrap(),
    );
    assert_eq!(result, Err(FloatError::Unrepresentable));
}

#[test]
fn oversized_floats_cannot_publish_infinite_horizontal_edges() {
    for (side, containing) in [
        (
            FloatSide::Left,
            Band {
                left: f32::MAX / 2.0,
                right: f32::MAX,
            },
        ),
        (
            FloatSide::Right,
            Band {
                left: -f32::MAX,
                right: -f32::MAX / 2.0,
            },
        ),
    ] {
        let result = place_float(
            &[],
            containing,
            FloatRequest {
                side,
                width: f32::MAX,
                height: 1.0,
                containing_top: 0.0,
                hypothetical_top: 0.0,
            },
            NonZeroUsize::new(1).unwrap(),
        );
        assert_eq!(result, Err(FloatError::Unrepresentable));
    }
}

#[test]
fn oversized_return_path_also_checks_vertical_edges() {
    let result = place_float(
        &[],
        Band {
            left: 0.0,
            right: 0.0,
        },
        FloatRequest {
            side: FloatSide::Left,
            width: 1.0,
            height: f32::MAX,
            containing_top: 0.0,
            hypothetical_top: f32::MAX,
        },
        NonZeroUsize::new(1).unwrap(),
    );
    assert_eq!(result, Err(FloatError::Unrepresentable));
}

#[test]
fn finite_container_overflow_and_extreme_finite_edges_still_succeed() {
    for side in [FloatSide::Left, FloatSide::Right] {
        let result = place_float(
            &[],
            Band {
                left: 0.0,
                right: 10.0,
            },
            FloatRequest {
                side,
                width: 20.0,
                height: 1.0,
                containing_top: 0.0,
                hypothetical_top: 0.0,
            },
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        assert!(result.overflows);
        assert_eq!(
            (result.placed.left, result.placed.right),
            match side {
                FloatSide::Left => (0.0, 20.0),
                FloatSide::Right => (-10.0, 10.0),
            }
        );
    }
    let result = place_float(
        &[],
        Band {
            left: f32::MAX,
            right: f32::MAX,
        },
        FloatRequest {
            side: FloatSide::Left,
            width: 0.0,
            height: 0.0,
            containing_top: f32::MAX,
            hypothetical_top: f32::MAX,
        },
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    assert_eq!(result.placed.left, f32::MAX);
    assert_eq!(result.placed.right, f32::MAX);
    assert_eq!(result.placed.top, f32::MAX);
    assert_eq!(result.placed.bottom, f32::MAX);
    assert!(!result.overflows);
}
