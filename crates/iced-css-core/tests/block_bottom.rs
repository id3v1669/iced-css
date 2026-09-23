use iced_css_core::formatting::{bfc_height::*, block::AutoLength, block_bottom::*};
fn input(c: f32, p: f32, b: f32, m: f32) -> BlockBottomInput {
    BlockBottomInput {
        content_bottom: c,
        padding_bottom: p,
        border_bottom: b,
        margin_bottom: AutoLength::Value(m),
    }
}
fn edges(i: BlockBottomInput, expected: [f32; 4]) {
    let p = place_block_bottom(i).unwrap();
    assert_eq!(
        [
            p.padding_bottom,
            p.border_bottom,
            p.margin_bottom,
            p.used_margin_bottom
        ]
        .map(f32::to_bits),
        expected.map(f32::to_bits)
    );
}
#[test]
fn signed_edges_and_auto_category() {
    for m in [0., 4., -4., -20.] {
        edges(input(19., 1., 0., m), [20., 20., 20. + m, m]);
    }
    edges(input(19., 2., 3., -4.), [21., 24., 20., -4.]);
    let mut i = input(19., 1., 0., -0.);
    assert_eq!(
        place_block_bottom(i).unwrap().margin_resolution,
        BottomMarginResolution::Absolute
    );
    edges(i, [20., 20., 20., 0.]);
    i.margin_bottom = AutoLength::Auto;
    assert_eq!(
        place_block_bottom(i).unwrap().margin_resolution,
        BottomMarginResolution::AutoZero
    );
    edges(i, [20., 20., 20., 0.]);
}
#[test]
fn zero_subnormal_and_independent_ties_even_publications() {
    edges(input(-0., -0., -0., -0.), [0.; 4]);
    let s = f32::from_bits(1);
    edges(input(-s, s, s, -s), [0., s, 0., -s]);
    let t = 16777216.;
    edges(input(t, 1., 1., 1.), [t, t, t, 1.]);
    edges(input(t + 2., 1., 1., 1.), [t + 4., t + 4., t + 4., 1.]);
    edges(
        input(f32::MAX, 1., 1., -f32::MAX),
        [f32::MAX, f32::MAX, 0., -f32::MAX],
    );
}
#[test]
fn authoritative_height_endpoint_is_not_recomposed() {
    let h = solve_bfc_height(BfcHeightInput {
        content_top: -16777216.,
        mode: BfcHeightMode::Auto {
            last_in_flow_border_bottom: 1.,
            last_in_flow_margin_bottom: 0.,
            float_bottom: 0.,
        },
    })
    .unwrap();
    assert_eq!(h.used_height, 16777216.);
    assert_eq!(-16777216. + h.used_height, 0.);
    edges(input(h.content_bottom, 1., 0., 0.), [2., 2., 2., 0.]);
}
#[test]
fn overflow_reports_first_published_stage() {
    use BlockBottomStage::*;
    for (i, stage) in [
        (input(f32::MAX, f32::MAX, 0., -f32::MAX), PaddingBottom),
        (input(f32::MAX, 0., f32::MAX, -f32::MAX), BorderBottom),
        (input(f32::MAX, 0., 0., f32::MAX), MarginBottom),
        (input(-f32::MAX, 0., 0., -f32::MAX), MarginBottom),
    ] {
        assert_eq!(
            place_block_bottom(i),
            Err(BlockBottomError::Unrepresentable { stage })
        );
    }
}
#[test]
fn all_domains_are_validated_before_arithmetic_in_field_order() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for (i, name) in [
            (input(bad, bad, bad, bad), "content_bottom"),
            (input(0., bad, bad, bad), "padding_bottom"),
            (input(f32::MAX, f32::MAX, bad, bad), "border_bottom"),
            (input(f32::MAX, f32::MAX, 0., bad), "margin_bottom"),
        ] {
            assert_eq!(
                place_block_bottom(i),
                Err(BlockBottomError::InvalidInput(name))
            );
        }
    }
    for (i, name) in [
        (input(0., -1., -1., 0.), "padding_bottom"),
        (input(f32::MAX, f32::MAX, -1., 0.), "border_bottom"),
    ] {
        assert_eq!(
            place_block_bottom(i),
            Err(BlockBottomError::InvalidInput(name))
        );
    }
    use std::error::Error;
    assert!(
        BlockBottomError::InvalidInput("padding_bottom")
            .source()
            .is_none()
    );
}
