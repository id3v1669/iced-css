use iced_css_core::values::font_metrics::{FontMetricScaleError, scale_font_units};
#[test]
fn metrics_scale_from_design_units_with_one_final_rounding() {
    for (units, upem, size, want) in [
        (935, 1000, 20.0, 18.7),
        (-265, 1000, 20.0, -5.3),
        (350, 1000, 20.0, 7.0),
        (527, 1000, 0.0, 0.0),
        (16384, 16384, 32.0, 32.0),
    ] {
        let actual = scale_font_units(units, upem, size).unwrap();
        assert_eq!(actual.value, want);
        assert!(!actual.rounded_to_zero);
    }
}
#[test]
fn full_integer_precision_and_ties_are_retained_before_scaling() {
    assert_eq!(
        scale_font_units(16777217, 16, 16.0).unwrap().value,
        16777216.0
    );
    assert_eq!(
        scale_font_units(16777219, 16, 16.0).unwrap().value,
        16777220.0
    );
    assert_eq!(
        scale_font_units(i32::MIN, 1024, 1024.0).unwrap().value,
        -2147483648.0
    );
    // Rounding the integer first would lose the low bits before multiplication.
    assert_eq!(
        scale_font_units(16777217, 16, 24.0).unwrap().value,
        25165826.0
    );
}
#[test]
fn oversized_products_cancel_and_final_overflows_are_errors() {
    for units in [1000, -1000] {
        let actual = scale_font_units(units, 1000, f32::MAX).unwrap();
        assert_eq!(actual.value, if units > 0 { f32::MAX } else { -f32::MAX });
    }
    assert_eq!(
        scale_font_units(32768, 16, f32::MAX),
        Err(FontMetricScaleError::Unrepresentable)
    );
}
#[test]
fn signed_underflow_is_reported_without_negative_zero() {
    for units in [1, -1] {
        let actual = scale_font_units(units, 16, f32::from_bits(1)).unwrap();
        assert_eq!(actual.value.to_bits(), 0);
        assert!(actual.rounded_to_zero);
    }
    let actual = scale_font_units(16, 16, f32::from_bits(1)).unwrap();
    assert_eq!(actual.value.to_bits(), 1);
    assert!(!actual.rounded_to_zero);
    let actual = scale_font_units(i32::MIN, 1000, -0.0).unwrap();
    assert_eq!(actual.value.to_bits(), 0);
    assert!(!actual.rounded_to_zero);
}
#[test]
fn invalid_font_size_and_units_per_em_are_rejected() {
    for upem in [0, 1, 15, 16385, u16::MAX] {
        assert_eq!(
            scale_font_units(1, upem, 16.0),
            Err(FontMetricScaleError::UnitsPerEm)
        );
    }
    for size in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            scale_font_units(1, 1000, size),
            Err(FontMetricScaleError::FontSize)
        );
    }
}
