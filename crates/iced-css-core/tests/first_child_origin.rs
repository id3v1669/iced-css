use iced_css_core::formatting::{block::AutoLength, first_child_origin::*};
fn input(top: f32, margin: AutoLength, border: f32, padding: f32) -> FirstChildOriginInput {
    FirstChildOriginInput {
        containing_top: top,
        margin_top: margin,
        border_top: border,
        padding_top: padding,
    }
}
#[test]
fn auto_signed_and_zero_margins_preserve_resolution() {
    for (margin, used, resolution, border, content) in [
        (
            AutoLength::Auto,
            0.,
            TopMarginResolution::AutoZero,
            10.,
            11.,
        ),
        (
            AutoLength::Value(0.),
            0.,
            TopMarginResolution::Absolute,
            10.,
            11.,
        ),
        (
            AutoLength::Value(4.),
            4.,
            TopMarginResolution::Absolute,
            14.,
            15.,
        ),
        (
            AutoLength::Value(-4.),
            -4.,
            TopMarginResolution::Absolute,
            6.,
            7.,
        ),
    ] {
        assert_eq!(
            place_first_child_origin(input(10., margin, 0., 1.)).unwrap(),
            FirstChildOriginPlacement {
                used_margin_top: used,
                margin_resolution: resolution,
                border_top: border,
                content_top: content
            }
        );
    }
    let p = place_first_child_origin(input(-0., AutoLength::Value(-0.), -0., -0.)).unwrap();
    assert_eq!(
        (
            p.used_margin_top.to_bits(),
            p.border_top.to_bits(),
            p.content_top.to_bits()
        ),
        (0, 0, 0)
    );
    assert_eq!(p.margin_resolution, TopMarginResolution::Absolute);
    assert_eq!(
        place_first_child_origin(input(-5., AutoLength::Auto, 0., 1.))
            .unwrap()
            .content_top,
        -4.
    );
}
#[test]
fn exact_stages_publish_separately_and_allow_completed_cancellation() {
    for (top, margin, border, padding, want_border, want_content) in [
        (16777216., 1., 0., 1., 16777216., 16777216.),
        (f32::MAX, -f32::MAX, 0., 1., 0., 1.),
        (-f32::MAX, 0., f32::MAX, f32::MAX, -f32::MAX, f32::MAX),
        (
            f32::MAX,
            f32::from_bits(1),
            0.,
            f32::from_bits(1),
            f32::MAX,
            f32::MAX,
        ),
    ] {
        let p = place_first_child_origin(input(top, AutoLength::Value(margin), border, padding))
            .unwrap();
        assert_eq!(
            (p.border_top.to_bits(), p.content_top.to_bits()),
            (want_border.to_bits(), want_content.to_bits())
        );
    }
    for (i, stage) in [
        (
            input(-f32::MAX, AutoLength::Value(-f32::MAX), f32::MAX, f32::MAX),
            OriginCoordinateStage::BorderTop,
        ),
        (
            input(f32::MAX, AutoLength::Auto, f32::MAX, 0.),
            OriginCoordinateStage::ContentTop,
        ),
    ] {
        assert_eq!(
            place_first_child_origin(i),
            Err(FirstChildOriginError::Unrepresentable { stage })
        );
    }
}
#[test]
fn validation_is_ordered_and_precedes_arithmetic() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for (i, field) in [
            (
                input(bad, AutoLength::Value(bad), bad, bad),
                "containing_top",
            ),
            (input(0., AutoLength::Value(bad), bad, bad), "margin_top"),
            (input(0., AutoLength::Auto, bad, bad), "border_top"),
            (input(0., AutoLength::Auto, 0., bad), "padding_top"),
        ] {
            assert_eq!(
                place_first_child_origin(i),
                Err(FirstChildOriginError::InvalidInput(field))
            );
        }
    }
    for (border, padding, field) in [(-1., -1., "border_top"), (0., -1., "padding_top")] {
        assert_eq!(
            place_first_child_origin(input(
                f32::MAX,
                AutoLength::Value(f32::MAX),
                border,
                padding
            )),
            Err(FirstChildOriginError::InvalidInput(field))
        );
    }
}
