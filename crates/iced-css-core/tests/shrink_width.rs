use iced_css_core::formatting::{
    block::AutoLength,
    shrink::{ShrinkWidthError, ShrinkWidthInput, solve_shrink_width},
};
fn input() -> ShrinkWidthInput {
    ShrinkWidthInput {
        containing_width: 100.0,
        width: AutoLength::Auto,
        margin_left: AutoLength::Value(5.0),
        margin_right: AutoLength::Auto,
        border_left: 3.0,
        border_right: 4.0,
        padding_left: 6.0,
        padding_right: 7.0,
        scrollbar_width: 5.0,
        preferred_min_width: 40.0,
        preferred_width: 90.0,
        min_width: 0.0,
        max_width: None,
    }
}
#[test]
fn auto_width_accounts_for_edges_scrollbars_and_intrinsic_bounds() {
    let original = input();
    for (containing, expected) in [(100.0, 70.0), (200.0, 90.0), (10.0, 40.0)] {
        let actual = solve_shrink_width(ShrinkWidthInput {
            containing_width: containing,
            ..original
        })
        .unwrap();
        assert_eq!(
            (
                actual.width,
                actual.margin_left,
                actual.margin_right,
                actual.passes
            ),
            (expected, 5.0, 0.0, 1)
        );
    }
    assert_eq!(original.width, AutoLength::Auto);
    assert_eq!(original.margin_right, AutoLength::Auto);
}
#[test]
fn fixed_and_negative_margins_are_not_balanced_like_normal_blocks() {
    let fixed = solve_shrink_width(ShrinkWidthInput {
        width: AutoLength::Value(200.0),
        ..input()
    })
    .unwrap();
    assert_eq!(
        (fixed.width, fixed.margin_left, fixed.margin_right),
        (200.0, 5.0, 0.0)
    );
    let negative = solve_shrink_width(ShrinkWidthInput {
        margin_left: AutoLength::Value(-20.0),
        ..input()
    })
    .unwrap();
    assert_eq!(
        (negative.width, negative.margin_left, negative.margin_right),
        (90.0, -20.0, 0.0)
    );
}
#[test]
fn max_then_min_reruns_override_the_tentative_shrink_size() {
    let actual = solve_shrink_width(ShrinkWidthInput {
        min_width: 60.0,
        max_width: Some(50.0),
        ..input()
    })
    .unwrap();
    assert_eq!((actual.width, actual.passes), (60.0, 3));
    let actual = solve_shrink_width(ShrinkWidthInput {
        min_width: 100.0,
        ..input()
    })
    .unwrap();
    assert_eq!((actual.width, actual.passes), (100.0, 2));
    let actual = solve_shrink_width(ShrinkWidthInput {
        max_width: Some(0.0),
        ..input()
    })
    .unwrap();
    assert_eq!((actual.width, actual.passes), (0.0, 2));
}
#[test]
fn exact_available_width_survives_large_cancellation_and_unrepresentable_intermediates() {
    let base = ShrinkWidthInput {
        containing_width: f32::MAX,
        margin_left: AutoLength::Value(f32::MAX),
        margin_right: AutoLength::Value(-10.0),
        border_left: 1.0,
        border_right: 0.0,
        padding_left: 0.0,
        padding_right: 0.0,
        scrollbar_width: 0.0,
        preferred_min_width: 0.0,
        preferred_width: 100.0,
        ..input()
    };
    assert_eq!(solve_shrink_width(base).unwrap().width, 9.0);
    assert_eq!(
        solve_shrink_width(ShrinkWidthInput {
            margin_left: AutoLength::Value(-f32::MAX),
            ..base
        })
        .unwrap()
        .width,
        100.0
    );
}
#[test]
fn invalid_intrinsic_and_nonfinite_geometry_is_rejected() {
    for bad in [
        ShrinkWidthInput {
            preferred_min_width: 100.0,
            ..input()
        },
        ShrinkWidthInput {
            scrollbar_width: -1.0,
            ..input()
        },
        ShrinkWidthInput {
            preferred_width: f32::NAN,
            ..input()
        },
        ShrinkWidthInput {
            margin_right: AutoLength::Value(f32::INFINITY),
            ..input()
        },
        ShrinkWidthInput {
            width: AutoLength::Value(-1.0),
            ..input()
        },
        ShrinkWidthInput {
            max_width: Some(-1.0),
            ..input()
        },
    ] {
        assert!(matches!(
            solve_shrink_width(bad),
            Err(ShrinkWidthError::InvalidInput(_))
        ));
    }
}

#[test]
fn used_zero_margins_have_canonical_positive_zero_identity() {
    let actual = solve_shrink_width(ShrinkWidthInput {
        width: AutoLength::Value(-0.0),
        margin_left: AutoLength::Value(-0.0),
        margin_right: AutoLength::Value(-0.0),
        ..input()
    })
    .unwrap();
    assert_eq!(actual.width.to_bits(), 0);
    assert_eq!(actual.margin_left.to_bits(), 0);
    assert_eq!(actual.margin_right.to_bits(), 0);
}
