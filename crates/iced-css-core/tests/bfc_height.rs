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

/// MMX: a tentative Auto placement with every `auto_extent` field.
fn mmx_tentative(used: f32, bottom: f32) -> BfcHeightPlacement {
    BfcHeightPlacement {
        used_height: used,
        content_bottom: bottom,
        auto_extent: Some(AutoHeightExtent {
            normal_margin_bottom: bottom,
            selected_bottom: bottom,
            float_extended: false,
        }),
    }
}

/// `placement` as (used height bits, content bottom bits, extent).
fn mmx_bits(placement: BfcHeightPlacement) -> (u32, u32, Option<AutoHeightExtent>) {
    (
        placement.used_height.to_bits(),
        placement.content_bottom.to_bits(),
        placement.auto_extent,
    )
}

/// CB1 (M19, M22 witness): the exact span 16777236 - 11.5 = 16777224.5 is above
/// `max_height` 16777224, so the explicit route reruns: used 16777224 (`4b800004`),
/// bottom round(16777235.5) = 16777236 (`4b80000a`), no extent. The rounded tentative
/// used height equals the limit, so comparing it would keep the tentative.
#[test]
fn mmx_cb1_exact_span_above_max_reruns() {
    let tentative = mmx_tentative(16777224.0, 16777236.0);
    let got = constrain_bfc_height(11.5, tentative, 0.0, Some(16777224.0)).unwrap();
    assert_eq!(mmx_bits(got), (0x4b800004, 0x4b80000a, None));
    assert_eq!(tentative.used_height.to_bits(), 0x4b800004);
}

/// CB2 (M19 witness): the exact span 16777236 - 12.5 = 16777223.5 is below
/// `min_height` 16777224, so the explicit route reruns: used `4b800004`, bottom
/// round(16777236.5) = `4b80000a`, no extent.
#[test]
fn mmx_cb2_exact_span_below_min_reruns() {
    let tentative = mmx_tentative(16777224.0, 16777236.0);
    let got = constrain_bfc_height(12.5, tentative, 16777224.0, None).unwrap();
    assert_eq!(mmx_bits(got), (0x4b800004, 0x4b80000a, None));
}

/// CB3 (M20 witness): the max rerun first (20 > 10 gives 10), then the min rerun
/// (10 < 30 gives 30): used 30, bottom 30; min first would give 10.
#[test]
fn mmx_cb3_max_then_min() {
    let got = constrain_bfc_height(0.0, mmx_tentative(20.0, 20.0), 30.0, Some(10.0)).unwrap();
    assert_eq!(
        mmx_bits(got),
        (30.0_f32.to_bits(), 30.0_f32.to_bits(), None)
    );
}

/// CB4 (M21 witness): limits equal to the span apply no rerun: the tentative is
/// returned bit for bit, extent included.
#[test]
fn mmx_cb4_equal_limits_keep_the_tentative() {
    let tentative = mmx_tentative(11.0, 22.0);
    let got = constrain_bfc_height(11.0, tentative, 11.0, Some(11.0)).unwrap();
    assert_eq!(mmx_bits(got), mmx_bits(tentative));
}

/// CB5 (M23 witness): validation precedes arithmetic, in the order content top, min,
/// max, tentative bottom.
#[test]
fn mmx_cb5_validation_order() {
    let tentative = mmx_tentative(11.0, 22.0);
    for (top, tentative, min, max, field) in [
        (f32::NAN, tentative, -1.0, Some(-1.0), "content_top"),
        (f32::INFINITY, tentative, 0.0, None, "content_top"),
        (11.0, tentative, -1.0, Some(f32::NAN), "min_height"),
        (11.0, tentative, f32::NAN, None, "min_height"),
        (11.0, tentative, f32::INFINITY, None, "min_height"),
        (11.0, tentative, 0.0, Some(-1.0), "max_height"),
        (11.0, tentative, 0.0, Some(f32::NAN), "max_height"),
        (11.0, tentative, 0.0, Some(f32::INFINITY), "max_height"),
        (
            11.0,
            mmx_tentative(11.0, 10.0),
            0.0,
            Some(-1.0),
            "max_height",
        ),
        (
            11.0,
            mmx_tentative(11.0, 10.0),
            0.0,
            None,
            "tentative_content_bottom",
        ),
        (
            11.0,
            mmx_tentative(11.0, f32::INFINITY),
            0.0,
            None,
            "tentative_content_bottom",
        ),
    ] {
        assert_eq!(
            constrain_bfc_height(top, tentative, min, max),
            Err(BfcHeightError::InvalidInput(field)),
            "{top} {tentative:?} {min} {max:?}"
        );
    }
}

/// K2-C (MMX K-2, ML-10): a tentative Auto span of +0 at content top 2^103 is below
/// `min_height` `f32::MAX`, so the explicit rerun publishes the content bottom 2^103 +
/// `f32::MAX` = 2^128 - 2^103, the midpoint of `f32::MAX` and 2^128, whose tie goes to
/// the even 2^128 (the mantissa of `f32::MAX` is odd): `Unrepresentable { stage:
/// ContentBottom }`.
#[test]
fn pct_k2c_min_height_rerun_content_bottom_overflows() {
    let top = f32::from_bits(0x7300_0000); // 2^103
    assert_eq!(top, 2.0_f32.powi(103));
    let tentative = mmx_tentative(0.0, top);
    assert_eq!(
        constrain_bfc_height(top, tentative, f32::MAX, None),
        Err(BfcHeightError::Unrepresentable {
            stage: BfcHeightStage::ContentBottom,
        })
    );
}
