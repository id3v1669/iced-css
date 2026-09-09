use iced_css_core::{
    animation::{combine::Combination, decoration::*},
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

fn value(mask: u8) -> TextDecoration {
    let names = ["underline", "overline", "line-through", "blink"];
    let text = if mask == 0 {
        "none".to_owned()
    } else {
        names
            .into_iter()
            .enumerate()
            .filter_map(|(i, n)| (mask & (1 << i) != 0).then_some(n))
            .collect::<Vec<_>>()
            .join(" ")
    };
    TextDecoration::try_from(&parsed(&format!("text-decoration:{text}"))).unwrap()
}
#[test]
fn all_decoration_sets_select_whole_endpoints_without_union() {
    for a in 0_u8..16 {
        for b in 0_u8..16 {
            let start = value(a);
            let end = value(b);
            for (op, want) in [
                (Combination::Interpolate(-f32::MAX), start),
                (
                    Combination::Interpolate(f32::from_bits(0.5_f32.to_bits() - 1)),
                    start,
                ),
                (Combination::Interpolate(0.5), end),
                (Combination::Interpolate(f32::MAX), end),
                (Combination::Add, start),
                (Combination::Accumulate, start),
            ] {
                assert_eq!(
                    combine_text_decoration(start, end, op).unwrap(),
                    want,
                    "{a}/{b}, {op:?}"
                );
            }
            for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert!(combine_text_decoration(start, end, Combination::Interpolate(p)).is_err());
            }
        }
    }
}
#[test]
fn resolved_conversion_preserves_keyword_sets_and_rejects_other_properties() {
    let canonical = value(5);
    assert_eq!(
        TextDecoration::try_from(&parsed("text-decoration:line-through underline")).unwrap(),
        canonical
    );
    assert!(canonical.underline());
    assert!(canonical.line_through());
    assert!(!canonical.overline());
    assert!(!canonical.blink());
    assert!(value(0).is_none());
    assert!(value(8).blink());
    for css in [
        "text-decoration:inherit",
        "font-style:normal",
        "display:none",
    ] {
        assert_eq!(
            TextDecoration::try_from(&parsed(css)),
            Err(TextDecorationError::Unresolved)
        );
    }
    let mut forged = parsed("text-decoration:none");
    for val in [
        SpecifiedValue::Initial,
        SpecifiedValue::Inherit,
        parsed("width:1px").value,
    ] {
        forged.value = val;
        assert_eq!(
            TextDecoration::try_from(&forged),
            Err(TextDecorationError::Unresolved)
        );
    }
}
