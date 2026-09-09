use iced_css_core::{
    animation::{combine::Combination, family::*},
    syntax::*,
    values::{NumericLimits, color::ColorLimits, property::*},
};
use std::num::NonZeroUsize;
fn numeric() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn parsed(input: &str) -> ParsedProperty {
    let n = |n| NonZeroUsize::new(n).unwrap();
    let DeclarationItem::Declaration(d) = Declarations::new(
        input,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: n(100000),
                nesting: n(20),
                retained_bytes: n(4096),
            },
            name_bytes: n(4096),
        },
    )
    .next()
    .unwrap()
    .unwrap() else {
        panic!()
    };
    let PropertyResult::Parsed(value) = parse_property(
        &d,
        ColorLimits {
            syntax: StringLimits {
                work: n(100000),
                retained_bytes: n(4096),
            },
            numeric: numeric(),
        },
    )
    .unwrap() else {
        panic!()
    };
    value
}

fn families(input: &str) -> Vec<FontFamily> {
    let SpecifiedValue::FontFamily(value) = parsed(input).value else {
        panic!()
    };
    value
}
#[test]
fn ordered_lists_select_whole_borrowed_endpoints() {
    let a = families(r#"font-family: 'serif', Times New Roman, serif"#);
    let b = families("font-family: monospace");
    assert!(matches!(&a[0], FontFamily::Quoted(s) if s == "serif"));
    assert!(matches!(&a[1], FontFamily::Identifiers(s) if s == &["Times", "New", "Roman"]));
    assert!(matches!(&a[2], FontFamily::Generic(k) if k.as_str() == "serif"));
    for (start, end) in [(&a, &b), (&b, &a), (&a, &a)] {
        for p in [
            -f32::MAX,
            0.0,
            f32::from_bits(0.5_f32.to_bits() - 1),
            0.5,
            1.0,
            f32::MAX,
        ] {
            let selected = combine_font_family(start, end, Combination::Interpolate(p)).unwrap();
            let expected = if p < 0.5 { start } else { end };
            assert_eq!(selected, expected);
            assert!(std::ptr::eq(selected, expected.as_slice()));
        }
        for op in [Combination::Add, Combination::Accumulate] {
            let selected = combine_font_family(start, end, op).unwrap();
            assert!(std::ptr::eq(selected, start.as_slice()));
        }
    }
}
#[test]
fn nonfinite_progress_is_rejected_even_for_identical_lists() {
    let a = families("font-family: serif");
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_font_family(&a, &a, Combination::Interpolate(p)),
            Err(iced_css_core::animation::combine::CombinationError::NonfiniteProgress)
        );
    }
}
