use iced_css_core::formatting::bfc_height::*;
fn auto(
    top: f32,
    normal: f32,
    margin: f32,
    float: f32,
) -> Result<BfcHeightPlacement, BfcHeightError> {
    solve_bfc_height(BfcHeightInput {
        content_top: top,
        mode: BfcHeightMode::Auto {
            last_in_flow_border_bottom: normal,
            last_in_flow_margin_bottom: margin,
            float_bottom: float,
        },
    })
}
fn explicit(top: f32, height: f32) -> Result<BfcHeightPlacement, BfcHeightError> {
    solve_bfc_height(BfcHeightInput {
        content_top: top,
        mode: BfcHeightMode::Explicit(height),
    })
}
#[test]
fn normal_margin_and_float_maximum_determine_auto_height() {
    for (normal, margin, float, height, bottom, extended) in [
        (19., 0., 18., 8., 19., false),
        (19., 3., 18., 11., 22., false),
        (17., 0., 36., 25., 36., true),
        (17., 30., 36., 36., 47., false),
        (19., 0., 19., 8., 19., false),
    ] {
        let got = auto(11., normal, margin, float).unwrap();
        assert_eq!(got.used_height, height);
        assert_eq!(got.content_bottom, bottom);
        assert_eq!(
            got.auto_extent,
            Some(AutoHeightExtent {
                normal_margin_bottom: normal + margin,
                selected_bottom: bottom,
                float_extended: extended
            })
        );
    }
}
#[test]
fn explicit_has_no_extent_inputs_and_preserves_height() {
    for height in [0., 2., f32::from_bits(1), f32::MAX] {
        let got = explicit(0., height).unwrap();
        assert_eq!(got.used_height.to_bits(), height.to_bits());
        assert_eq!(got.content_bottom.to_bits(), height.to_bits());
        assert_eq!(got.auto_extent, None);
    }
    assert_eq!(explicit(11., 2.).unwrap().content_bottom, 13.);
    assert_eq!(explicit(11., 0.).unwrap().content_bottom, 11.);
}
#[test]
fn validation_precedes_order_and_arithmetic() {
    use BfcHeightError::InvalidInput as Invalid;
    assert_eq!(explicit(f32::NAN, -1.), Err(Invalid("content_top")));
    assert_eq!(
        auto(f32::INFINITY, f32::NAN, -1., f32::NAN),
        Err(Invalid("content_top"))
    );
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.] {
        assert_eq!(explicit(0., bad), Err(Invalid("height")));
        assert_eq!(
            auto(0., 1., bad, f32::NAN),
            Err(Invalid("last_in_flow_margin_bottom"))
        );
    }
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            auto(0., bad, -1., f32::NAN),
            Err(Invalid("last_in_flow_border_bottom"))
        );
        assert_eq!(
            auto(0., f32::MAX, f32::MAX, bad),
            Err(Invalid("float_bottom"))
        );
        assert_eq!(auto(0., -1., 0., bad), Err(Invalid("float_bottom")));
    }
    assert_eq!(
        auto(0., -1., 0., -1.),
        Err(Invalid("normal_bottom_before_top"))
    );
    assert_eq!(
        auto(0., 1., 0., -1.),
        Err(Invalid("float_bottom_before_top"))
    );
}
#[test]
fn publication_failures_name_the_observable_stage() {
    use BfcHeightError::Unrepresentable as U;
    assert_eq!(
        auto(0., f32::MAX, f32::MAX, 0.),
        Err(U {
            stage: BfcHeightStage::NormalMarginBottom
        })
    );
    assert_eq!(
        auto(-f32::MAX, f32::MAX, 0., 0.),
        Err(U {
            stage: BfcHeightStage::UsedHeight
        })
    );
    assert_eq!(
        explicit(f32::MAX, f32::MAX),
        Err(U {
            stage: BfcHeightStage::ContentBottom
        })
    );
    assert_eq!(auto(0., f32::MAX, 1., 0.).unwrap().content_bottom, f32::MAX);
    assert_eq!(explicit(f32::MAX, 1.).unwrap().content_bottom, f32::MAX);
}
#[test]
fn auto_retains_selected_bottom_without_recomposing_rounded_height() {
    let got = auto(-16777216., 1., 0., 0.).unwrap();
    assert_eq!(got.used_height.to_bits(), 16777216f32.to_bits());
    assert_eq!(got.content_bottom.to_bits(), 1f32.to_bits());
    assert_eq!(
        got.auto_extent.unwrap().selected_bottom.to_bits(),
        1f32.to_bits()
    );
    assert_eq!(
        explicit(-16777216., 16777216.)
            .unwrap()
            .content_bottom
            .to_bits(),
        0
    );
}
#[test]
fn signed_coordinates_subnormals_cancellation_and_zero_normalization() {
    let tiny = f32::from_bits(1);
    for (top, normal, margin, float, height, bottom) in [
        (-10., -6., 2., -5., 6., -4.),
        (-tiny, 0., tiny, 0., tiny + tiny, tiny),
        (-0., -0., -0., -0., 0., 0.),
        (10., 10., 0., 10., 0., 10.),
    ] {
        let got = auto(top, normal, margin, float).unwrap();
        assert_eq!(got.used_height.to_bits(), height.to_bits());
        assert_eq!(got.content_bottom.to_bits(), bottom.to_bits());
        if bottom == 0. {
            assert_eq!(got.auto_extent.unwrap().normal_margin_bottom.to_bits(), 0);
        }
    }
    let got = explicit(-0., -0.).unwrap();
    assert_eq!(got.used_height.to_bits(), 0);
    assert_eq!(got.content_bottom.to_bits(), 0);
}
