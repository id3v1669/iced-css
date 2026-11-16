use iced_css_core::values::{RangeEvent, font_size::*};
#[test]
fn keyword_table_validates_actual_entries_and_returns_exact_anchors() {
    let table = FontSizeTable::new(1.0).unwrap();
    assert_eq!(
        table.entries().map(f32::to_bits),
        [
            0x3f19999a, 0x3f400000, 0x3f63d70a, 0x3f800000, 0x3f99999a, 0x3fc00000, 0x40000000
        ]
    );
    for i in 0..6 {
        let value = table.larger(table.entries()[i]).unwrap();
        assert_eq!(value.value.to_bits(), table.entries()[i + 1].to_bits());
        assert_eq!(value.event, None);
    }
    for i in 1..7 {
        let value = table.smaller(table.entries()[i]).unwrap();
        assert_eq!(value.value.to_bits(), table.entries()[i - 1].to_bits());
        assert_eq!(value.event, None);
    }
    for medium in [
        0.0,
        -1.0,
        f32::NAN,
        f32::INFINITY,
        f32::MAX,
        f32::from_bits(1),
    ] {
        assert!(FontSizeTable::new(medium).is_err());
    }
}
#[test]
fn relative_keywords_validate_parents_and_classify_completed_range_events() {
    let table = FontSizeTable::new(16.0).unwrap();
    assert_eq!(table.larger(-0.0).unwrap().value.to_bits(), 0);
    assert_eq!(table.smaller(0.0).unwrap().event, None);
    let upper = table.larger(f32::MAX).unwrap();
    assert_eq!(upper.value, f32::MAX);
    assert_eq!(upper.event, Some(RangeEvent::UpperSaturation));
    assert!(table.smaller(f32::from_bits(1)).unwrap().value.is_finite());
    for parent in [-1.0, f32::NAN, f32::INFINITY] {
        assert!(table.larger(parent).is_err());
        assert!(table.smaller(parent).is_err());
    }
}

#[test]
fn typed_font_sizes_apply_context_before_the_selected_range_conversion() {
    use iced_css_core::values::{LengthTarget, NumericLimits, property::SpecifiedValue};
    use std::num::NonZeroUsize;
    let limits = NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    };
    let context = FontSizeContext {
        table: FontSizeTable::new(16.0).unwrap(),
        parent_size: 20.0,
        parent_x_height: 9.0,
        root_size: 16.0,
    };
    for (raw, expected) in [("2em", 40.0), ("2ex", 18.0), ("150%", 30.0), ("1in", 96.0)] {
        let value = SpecifiedValue::Length(LengthTarget::parse(raw, limits).unwrap());
        assert_eq!(
            compute_font_size(&value, context, limits).unwrap().value,
            expected
        );
    }
    let value = SpecifiedValue::Length(
        LengthTarget::parse("10000000000000000000000000000000000000000em", limits).unwrap(),
    );
    let tiny_context = FontSizeContext {
        parent_size: f32::from_bits(1),
        ..context
    };
    assert!(
        compute_font_size(&value, tiny_context, limits)
            .unwrap()
            .value
            < 1.0
    );
    assert!(compute_font_size(&SpecifiedValue::Inherit, context, limits).is_err());
}
