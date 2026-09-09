use iced_css_core::{
    animation::{combine::Combination, quotes::*},
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

use iced_css_core::formatting::quotes::{QuoteDepth, QuoteOperation, QuotePairs};
fn marks(value: QuotePairs<'_>) -> Vec<String> {
    let mut depth = QuoteDepth::new(NonZeroUsize::new(8).unwrap());
    let ops = [
        QuoteOperation::Open,
        QuoteOperation::Open,
        QuoteOperation::Open,
        QuoteOperation::Close,
        QuoteOperation::Close,
        QuoteOperation::Close,
    ];
    ops.into_iter()
        .filter_map(|op| depth.apply(op, value).unwrap().map(str::to_owned))
        .collect()
}
#[test]
fn sampled_lists_drive_quote_depth_without_merging_pairs() {
    let a = parsed(r#"quotes: "<" ">" "[" "]""#);
    let b = parsed(r#"quotes: "{" "}""#);
    let a = QuotePairs::from_property(&a).unwrap();
    let b = QuotePairs::from_property(&b).unwrap();
    for (start, end) in [
        (a, b),
        (b, a),
        (a, a),
        (a, QuotePairs::none()),
        (QuotePairs::none(), b),
    ] {
        for p in [
            -f32::MAX,
            0.0,
            f32::from_bits(0.5_f32.to_bits() - 1),
            0.5,
            1.0,
            f32::MAX,
        ] {
            let sample = combine_quotes(start, end, Combination::Interpolate(p)).unwrap();
            assert_eq!(marks(sample), marks(if p < 0.5 { start } else { end }));
        }
        for op in [Combination::Add, Combination::Accumulate] {
            assert_eq!(marks(combine_quotes(start, end, op).unwrap()), marks(start));
        }
    }
    assert_eq!(marks(a), ["<", "[", "[", "]", "]", ">"]);
    assert_eq!(marks(b), ["{", "{", "{", "}", "}", "}"]);
}
#[test]
fn selected_none_suppresses_marks_without_suppressing_depth_operations() {
    let parsed = parsed(r#"quotes: "<" ">""#);
    let visible = QuotePairs::from_property(&parsed).unwrap();
    let none = combine_quotes(visible, QuotePairs::none(), Combination::Interpolate(0.5)).unwrap();
    let mut depth = QuoteDepth::new(NonZeroUsize::new(8).unwrap());
    assert_eq!(depth.apply(QuoteOperation::Open, none).unwrap(), None);
    assert_eq!(depth.depth(), 1);
    assert_eq!(
        depth.apply(QuoteOperation::Close, visible).unwrap(),
        Some(">")
    );
    assert_eq!(depth.depth(), 0);
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(combine_quotes(none, none, Combination::Interpolate(p)).is_err());
    }
}
