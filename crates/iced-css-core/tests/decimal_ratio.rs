use iced_css_core::values::{ConversionError, Decimal, NumericLimits};
use std::num::NonZeroUsize;
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn d(s: &str) -> Decimal {
    Decimal::parse(s, limits()).unwrap()
}
#[test]
fn exact_quotients_round_once_with_signed_ties_even() {
    for (a, b, bits) in [
        ("1", "3", 0x3eaaaaab),
        ("2", "3", 0x3f2aaaab),
        ("1", "-3", 0xbeaaaaab),
        ("-1", "-3", 0x3eaaaaab),
        ("16777217", "16777216", 0x3f800000),
        ("16777219", "16777216", 0x3f800002),
        ("-16777217", "16777216", 0xbf800000),
        ("-16777219", "16777216", 0xbf800002),
    ] {
        let value = d(a).ratio_finite_f32(&d(b), limits()).unwrap();
        assert_eq!(value.value.to_bits(), bits);
        assert!(!value.rounded_to_zero);
    }
    let zero = d("-0").ratio_finite_f32(&d("-3"), limits()).unwrap();
    assert_eq!(zero.value.to_bits(), 0);
    assert!(!zero.rounded_to_zero);
}
#[test]
fn wide_and_tiny_operands_do_not_convert_before_division() {
    let wide = d(&"9".repeat(500));
    assert_eq!(wide.ratio_finite_f32(&wide, limits()).unwrap().value, 1.0);
    let tiny = d(&format!("0.{}1", "0".repeat(450)));
    assert_eq!(tiny.ratio_finite_f32(&tiny, limits()).unwrap().value, 1.0);
    let two149 = d("713623846352979940529142984724747568191373312");
    let two150 = d("1427247692705959881058285969449495136382746624");
    assert_eq!(
        d("1")
            .ratio_finite_f32(&two149, limits())
            .unwrap()
            .value
            .to_bits(),
        1
    );
    for a in ["1", "-1"] {
        let value = d(a).ratio_finite_f32(&two150, limits()).unwrap();
        assert_eq!(value.value.to_bits(), 0);
        assert!(value.rounded_to_zero);
    }
    let half = d(
        "0.000000000000000000000000000000000000000000000700649232162408535461864791644958065640130970938257885878534141944895541342930300743319094181060791015625",
    );
    assert_eq!(
        half.ratio_finite_f32(&d("0.5"), limits())
            .unwrap()
            .value
            .to_bits(),
        1
    );
}
#[test]
fn zero_denominator_range_and_resource_errors_are_explicit() {
    assert_eq!(
        d("1").ratio_finite_f32(&d("0"), limits()).unwrap_err(),
        ConversionError::ZeroDenominator
    );
    assert_eq!(
        d("0").ratio_finite_f32(&d("-0"), limits()).unwrap_err(),
        ConversionError::ZeroDenominator
    );
    assert_eq!(
        d("1")
            .ratio_finite_f32(&d("0.0000000000000000000000000000000000000001"), limits())
            .unwrap_err(),
        ConversionError::Unrepresentable
    );
    let a = d("12345");
    let b = d("67");
    let mut small = limits();
    small.work = NonZeroUsize::MIN;
    assert!(a.ratio_finite_f32(&b, small).is_err());
    small = limits();
    small.digits = NonZeroUsize::new(4).unwrap();
    assert!(a.ratio_finite_f32(&b, small).is_err());
    assert_eq!(a, d("12345"));
    assert_eq!(b, d("67"));
}
