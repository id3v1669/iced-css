use iced_css_core::formatting::{absolute_height::*, block::AutoLength};
fn value(v: f32) -> AutoLength {
    AutoLength::Value(v)
}
fn input() -> AbsoluteHeightInput {
    AbsoluteHeightInput {
        containing_height: 300.0,
        static_top: 20.0,
        top: AutoLength::Auto,
        height: AutoLength::Auto,
        bottom: AutoLength::Auto,
        margin_top: AutoLength::Auto,
        margin_bottom: AutoLength::Auto,
        border_top: 2.0,
        padding_top: 3.0,
        padding_bottom: 5.0,
        border_bottom: 7.0,
        content_height: 80.0,
        min_height: 0.0,
        max_height: None,
    }
}
#[test]
fn all_auto_and_six_partial_auto_rules_use_the_content_height() {
    for (top, height, bottom, expected) in [
        (
            AutoLength::Auto,
            AutoLength::Auto,
            AutoLength::Auto,
            (20.0, 80.0, 183.0),
        ),
        (
            AutoLength::Auto,
            AutoLength::Auto,
            value(70.0),
            (133.0, 80.0, 70.0),
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
            (50.0, 80.0, 153.0),
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
        let result = absolute_height(AbsoluteHeightInput {
            top,
            height,
            bottom,
            ..input()
        })
        .unwrap();
        assert_eq!((result.top, result.height, result.bottom), expected);
        assert_eq!((result.margin_top, result.margin_bottom), (0.0, 0.0));
    }
}
#[test]
fn paired_auto_vertical_margins_share_negative_space() {
    let result = absolute_height(AbsoluteHeightInput {
        top: value(150.0),
        height: value(100.0),
        bottom: value(150.0),
        ..input()
    })
    .unwrap();
    assert_eq!((result.margin_top, result.margin_bottom), (-58.5, -58.5));
    let result = absolute_height(AbsoluteHeightInput {
        top: value(50.0),
        height: value(100.0),
        bottom: value(70.0),
        margin_bottom: value(10.0),
        ..input()
    })
    .unwrap();
    assert_eq!((result.margin_top, result.margin_bottom), (53.0, 10.0));
}
#[test]
fn max_min_reruns_and_overconstraint_recompute_margins_or_bottom() {
    let fixed = AbsoluteHeightInput {
        top: value(50.0),
        height: value(100.0),
        bottom: value(70.0),
        ..input()
    };
    let result = absolute_height(AbsoluteHeightInput {
        min_height: 150.0,
        max_height: Some(80.0),
        ..fixed
    })
    .unwrap();
    assert_eq!(
        (
            result.height,
            result.margin_top,
            result.margin_bottom,
            result.passes
        ),
        (150.0, 6.5, 6.5, 3)
    );
    let result = absolute_height(AbsoluteHeightInput {
        margin_top: value(10.0),
        margin_bottom: value(20.0),
        ..fixed
    })
    .unwrap();
    assert_eq!((result.top, result.bottom), (50.0, 103.0));
    let result = absolute_height(AbsoluteHeightInput {
        top: value(200.0),
        bottom: value(200.0),
        ..input()
    })
    .unwrap();
    assert_eq!(
        (
            result.height,
            result.margin_top,
            result.margin_bottom,
            result.passes
        ),
        (0.0, -58.5, -58.5, 2)
    );
}
#[test]
fn invalid_domains_and_final_overflow_are_explicit() {
    assert_eq!(
        absolute_height(AbsoluteHeightInput {
            content_height: -1.0,
            ..input()
        }),
        Err(AbsoluteHeightError::InvalidInput("content_height"))
    );
    assert_eq!(
        absolute_height(AbsoluteHeightInput {
            top: value(f32::NAN),
            ..input()
        }),
        Err(AbsoluteHeightError::InvalidInput("top"))
    );
    let result = absolute_height(AbsoluteHeightInput {
        containing_height: 0.0,
        top: value(f32::MAX),
        height: value(f32::MAX),
        bottom: AutoLength::Auto,
        ..input()
    });
    assert_eq!(result, Err(AbsoluteHeightError::Unrepresentable));
}
