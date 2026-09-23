use iced_css_core::formatting::{block::*, floats::Band};
use std::error::Error;

fn input(width: f32) -> BlockWidthInput {
    BlockWidthInput {
        containing_width: width,
        direction: Direction::Ltr,
        width: AutoLength::Value(width),
        margin_left: AutoLength::Value(0.),
        margin_right: AutoLength::Value(0.),
        border_left: 0.,
        border_right: 0.,
        padding_left: 0.,
        padding_right: 0.,
        min_width: 0.,
        max_width: None,
    }
}
#[test]
fn actual_width_equations_translate_to_content_edges() {
    let mut i = input(40.);
    i.containing_width = 100.;
    i.padding_left = 5.;
    i.padding_right = 5.;
    i.margin_left = AutoLength::Auto;
    i.margin_right = AutoLength::Auto;
    let p = place_block_horizontal(10., i).unwrap();
    assert_eq!(
        p.used,
        BlockWidthResult {
            width: 40.,
            margin_left: 25.,
            margin_right: 25.,
            passes: 1
        }
    );
    assert_eq!(
        p.content,
        Band {
            left: 40.,
            right: 80.
        }
    );
    i.width = AutoLength::Auto;
    i.margin_left = AutoLength::Value(10.);
    i.margin_right = AutoLength::Value(20.);
    i.padding_right = 7.;
    let p = place_block_horizontal(10., i).unwrap();
    assert_eq!(p.used.width, 58.);
    assert_eq!(
        p.content,
        Band {
            left: 25.,
            right: 83.
        }
    );
    i.width = AutoLength::Value(40.);
    i.padding_left = 0.;
    i.padding_right = 0.;
    for (direction, margins, band) in [
        (
            Direction::Ltr,
            (10., 50.),
            Band {
                left: 20.,
                right: 60.,
            },
        ),
        (
            Direction::Rtl,
            (40., 20.),
            Band {
                left: 50.,
                right: 90.,
            },
        ),
    ] {
        i.direction = direction;
        let p = place_block_horizontal(10., i).unwrap();
        assert_eq!((p.used.margin_left, p.used.margin_right), margins);
        assert_eq!(p.content, band);
    }
}
#[test]
fn published_left_is_the_right_stage_origin_and_width_is_retained() {
    for (width, right, collapsed) in [(1., 0x4b800000, true), (3., 0x4b800002, false)] {
        let p = place_block_horizontal(16777216., input(width)).unwrap();
        assert_eq!(p.used.width, width);
        assert_eq!(p.content.left.to_bits(), 0x4b800000);
        assert_eq!(p.content.right.to_bits(), right);
        assert_eq!(p.collapsed_positive_width(), collapsed);
    }
    let mut i = input(1.);
    i.containing_width = 2.;
    i.margin_left = AutoLength::Value(1.);
    let p = place_block_horizontal(16777216., i).unwrap();
    assert_eq!(
        p.content,
        Band {
            left: 16777216.,
            right: 16777216.
        }
    );
}
#[test]
fn exact_stage_cancels_overflowing_prefix_and_preserves_conversion_boundary() {
    let mut i = input(0.);
    i.margin_left = AutoLength::Value(-f32::MAX);
    i.padding_left = f32::MAX;
    let p = place_block_horizontal(-f32::MAX, i).unwrap();
    assert_eq!(p.used.margin_right, 0.);
    assert_eq!(
        p.content,
        Band {
            left: -f32::MAX,
            right: -f32::MAX
        }
    );
    let p = place_block_horizontal(f32::MAX, input(f32::from_bits(1))).unwrap();
    assert_eq!(
        p.content,
        Band {
            left: f32::MAX,
            right: f32::MAX
        }
    );
    assert!(p.collapsed_positive_width());
    for origin in [-0., 0., -23., 23., f32::MAX, -f32::MAX] {
        let p = place_block_horizontal(origin, input(0.)).unwrap();
        let expected = if origin == 0. {
            0_u32
        } else {
            origin.to_bits()
        };
        assert_eq!(p.content.left.to_bits(), expected);
        assert_eq!(p.content.right.to_bits(), expected);
        assert!(!p.collapsed_positive_width());
        assert!(p.content.left.is_finite());
        assert!(p.content.right >= p.content.left);
    }
}
#[test]
fn errors_preserve_stages_causes_and_validation_order() {
    use BlockHorizontalError as E;
    let mut i = input(0.);
    i.containing_width = -1.;
    for origin in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(place_block_horizontal(origin, i), Err(E::InvalidOrigin));
    }
    let error = place_block_horizontal(0., i).unwrap_err();
    assert_eq!(
        error,
        E::Width(BlockWidthError::InvalidInput("containing_width"))
    );
    assert_eq!(
        error.source().unwrap().downcast_ref::<BlockWidthError>(),
        Some(&BlockWidthError::InvalidInput("containing_width"))
    );
    i = input(0.);
    i.margin_left = AutoLength::Value(f32::MAX);
    assert_eq!(
        place_block_horizontal(f32::MAX, i),
        Err(E::Unrepresentable {
            stage: BlockCoordinateStage::ContentLeft
        })
    );
    assert_eq!(
        place_block_horizontal(f32::MAX, input(f32::MAX)),
        Err(E::Unrepresentable {
            stage: BlockCoordinateStage::ContentRight
        })
    );
    assert!(E::InvalidOrigin.source().is_none());
    assert!(
        E::Unrepresentable {
            stage: BlockCoordinateStage::ContentRight
        }
        .source()
        .is_none()
    );
}
