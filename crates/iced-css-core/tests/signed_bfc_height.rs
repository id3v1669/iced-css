use iced_css_core::formatting::bfc_height::*;

fn signed(p: f32, t: f32, b: f32, f: f32) -> Result<BfcHeightPlacement, SignedBfcHeightError> {
    solve_signed_bfc_height(SignedBfcHeightInput {
        content_top: p,
        normal_margin_top: t,
        normal_margin_bottom: b,
        float_bottom: f,
    })
}

#[test]
fn signed_span_rebases_and_extends_strictly() {
    for (t, b, f, h, bottom, extended) in [
        (10., 25., 20., 15., 25., false),
        (2., 25., 20., 23., 33., false),
        (2., 25., 40., 30., 40., true),
        (20., 15., 10., 0., 10., false),
        (20., 15., 14., 4., 14., true),
        (2., 25., 33., 23., 33., false),
    ] {
        assert_eq!(
            signed(10., t, b, f).unwrap(),
            BfcHeightPlacement {
                used_height: h,
                content_bottom: bottom,
                auto_extent: Some(AutoHeightExtent {
                    normal_margin_bottom: b,
                    selected_bottom: bottom,
                    float_extended: extended,
                }),
            }
        );
    }
}

#[test]
fn signed_publication_stages_and_negative_overflow_floor() {
    for (p, t, b, f, stage) in [
        (
            0.,
            -f32::MAX,
            f32::MAX,
            0.,
            SignedBfcHeightStage::NormalHeight,
        ),
        (
            f32::MAX,
            0.,
            f32::MAX,
            f32::MAX,
            SignedBfcHeightStage::NormalContentBottom,
        ),
        (
            -f32::MAX,
            0.,
            0.,
            f32::MAX,
            SignedBfcHeightStage::ExtendedHeight,
        ),
    ] {
        assert_eq!(
            signed(p, t, b, f),
            Err(SignedBfcHeightError::Unrepresentable { stage })
        );
    }
    let floor = signed(0., f32::MAX, -f32::MAX, -0.).unwrap();
    assert_eq!(floor.used_height.to_bits(), 0);
    assert_eq!(floor.content_bottom.to_bits(), 0);
    assert!(!floor.auto_extent.unwrap().float_extended);
    let endpoint = signed(-16777216., 0., 0., 1.).unwrap();
    assert_eq!(endpoint.used_height.to_bits(), 16777216f32.to_bits());
    assert_eq!(endpoint.content_bottom.to_bits(), 1f32.to_bits());
}

#[test]
fn signed_scalar_validation_is_ordered_before_arithmetic() {
    let names = [
        "content_top",
        "normal_margin_top",
        "normal_margin_bottom",
        "float_bottom",
    ];
    for first in 0..4 {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut values = [0., 0., 0., 0.];
            values[first..].fill(bad);
            assert_eq!(
                signed(values[0], values[1], values[2], values[3]),
                Err(SignedBfcHeightError::InvalidInput(names[first]))
            );
        }
    }
    assert_eq!(
        signed(1., -f32::MAX, f32::MAX, 0.),
        Err(SignedBfcHeightError::InvalidInput(
            "float_bottom_before_top"
        ))
    );
}

#[test]
fn signed_margin_edges_preserve_inversion_ties_and_error_order() {
    let input = SignedNormalMarginInput {
        border_top: 7.,
        border_bottom: 8.,
        margin_top: -4.,
        margin_bottom: -4.,
    };
    assert_eq!(
        publish_signed_normal_margin_edges(input).unwrap(),
        SignedNormalMarginEdges {
            top: 11.,
            bottom: 4.
        }
    );
    let tie = publish_signed_normal_margin_edges(SignedNormalMarginInput {
        border_top: 16777216.,
        border_bottom: 16777216.,
        margin_top: -1.,
        margin_bottom: 3.,
    })
    .unwrap();
    assert_eq!((tie.top, tie.bottom), (16777216., 16777220.));
    let overflow = SignedNormalMarginInput {
        border_top: f32::MAX,
        border_bottom: f32::MAX,
        margin_top: -f32::MAX,
        margin_bottom: f32::MAX,
    };
    assert_eq!(
        publish_signed_normal_margin_edges(overflow),
        Err(SignedNormalMarginError::Unrepresentable {
            edge: SignedNormalMarginEdge::Top
        })
    );
    assert_eq!(
        publish_signed_normal_margin_edges(SignedNormalMarginInput {
            margin_top: 0.,
            ..overflow
        }),
        Err(SignedNormalMarginError::Unrepresentable {
            edge: SignedNormalMarginEdge::Bottom
        })
    );
    assert_eq!(
        publish_signed_normal_margin_edges(SignedNormalMarginInput {
            margin_bottom: f32::NAN,
            ..overflow
        }),
        Err(SignedNormalMarginError::InvalidInput("margin_bottom"))
    );
    let zero = publish_signed_normal_margin_edges(SignedNormalMarginInput {
        border_top: -0.,
        border_bottom: -0.,
        margin_top: 0.,
        margin_bottom: -0.,
    })
    .unwrap();
    assert_eq!((zero.top.to_bits(), zero.bottom.to_bits()), (0, 0));
}

#[test]
fn signed_margin_all_nonfinite_fields_precede_order_and_arithmetic() {
    let names = ["border_top", "border_bottom", "margin_top", "margin_bottom"];
    for first in 0..4 {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut values = [f32::MAX, -f32::MAX, -f32::MAX, f32::MAX];
            values[first..].fill(bad);
            assert_eq!(
                publish_signed_normal_margin_edges(SignedNormalMarginInput {
                    border_top: values[0],
                    border_bottom: values[1],
                    margin_top: values[2],
                    margin_bottom: values[3],
                }),
                Err(SignedNormalMarginError::InvalidInput(names[first]))
            );
        }
    }
    assert_eq!(
        publish_signed_normal_margin_edges(SignedNormalMarginInput {
            border_top: f32::MAX,
            border_bottom: -f32::MAX,
            margin_top: -f32::MAX,
            margin_bottom: f32::MAX,
        }),
        Err(SignedNormalMarginError::InvalidInput(
            "border_bottom_before_top"
        ))
    );
}
