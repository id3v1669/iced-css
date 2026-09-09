use iced_css_core::values::{
    ConversionError, Decimal, LengthTarget, NumericError, NumericLimits, RangeEvent,
};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(1_000_000).unwrap(),
    }
}
fn decimal(s: &str) -> Decimal {
    Decimal::parse(s, limits()).unwrap()
}
#[test]
fn exact_binary_oracles_cover_rounding_ties_subnormals_and_maximum() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../fixtures/f32-conversion.json")).unwrap();
    for case in cases {
        let input = decimal(case["decimal"].as_str().unwrap());
        let result = input.scaled_nonnegative_f32(1.0, limits()).unwrap();
        assert_eq!(
            result.value.to_bits(),
            case["bits"].as_u64().unwrap() as u32,
            "{}",
            case["id"]
        );
        let event = match case["event"].as_str() {
            None => None,
            Some("zero") => Some(RangeEvent::PositiveRoundedToZero),
            Some("upper") => Some(RangeEvent::UpperSaturation),
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(result.event, event, "{}", case["id"]);
    }
}
#[test]
fn scaling_precedes_conversion_even_when_the_coefficient_exceeds_f32() {
    let coefficient = decimal("713623846352979940529142984724747568191373312"); // 2^149
    let result = coefficient
        .scaled_nonnegative_f32(f32::from_bits(1), limits())
        .unwrap();
    assert_eq!(result.value.to_bits(), 1.0f32.to_bits());
    assert_eq!(result.event, None);
    let percent = decimal("71362384635297994052914298472474756819137331200");
    assert_eq!(
        percent
            .percentage_nonnegative_f32(f32::from_bits(1), limits())
            .unwrap(),
        result
    );
    assert_eq!(
        decimal("50")
            .percentage_nonnegative_f32(18.0, limits())
            .unwrap()
            .value,
        9.0
    );
}
#[test]
fn invalid_negative_cannot_round_or_scale_into_validity() {
    assert!(decimal("0").scaled_nonnegative_f32(1.0, limits()).is_ok());
    for context in [0.0, 1.0, f32::from_bits(1)] {
        assert_eq!(
            decimal("-.000000000000000000000000000000000000000000000000001")
                .scaled_nonnegative_f32(context, limits()),
            Err(ConversionError::InvalidNegative)
        );
    }
    assert_eq!(
        decimal("-0")
            .scaled_nonnegative_f32(1.0, limits())
            .unwrap()
            .value
            .to_bits(),
        0
    );
    for context in [-1.0, f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
        assert_eq!(
            decimal("0").scaled_nonnegative_f32(context, limits()),
            Err(ConversionError::InvalidContext)
        );
    }
}
#[test]
fn absolute_units_convert_after_exact_unit_resolution() {
    for text in ["1in", "2.54cm", "25.4mm", "72pt", "6pc", "96px"] {
        let LengthTarget::Absolute(length) = LengthTarget::parse(text, limits()).unwrap() else {
            panic!()
        };
        let result = length.to_nonnegative_f32(limits()).unwrap();
        assert_eq!(result.value, 96.0);
        assert_eq!(result.event, None);
    }
    let LengthTarget::Absolute(length) = LengthTarget::parse("-1px", limits()).unwrap() else {
        panic!()
    };
    assert_eq!(
        length.to_nonnegative_f32(limits()),
        Err(ConversionError::InvalidNegative)
    );
}
#[test]
fn conversion_budget_failure_does_not_publish_an_approximation() {
    let value = decimal("1.25");
    let tiny = NumericLimits {
        work: NonZeroUsize::new(1).unwrap(),
        ..limits()
    };
    assert_eq!(
        value.scaled_nonnegative_f32(1.0, tiny),
        Err(ConversionError::Numeric(NumericError::WorkLimit))
    );
    assert_eq!(
        value.scaled_nonnegative_f32(1.0, limits()).unwrap().value,
        1.25
    );
}
