use iced_css_core::{
    syntax::*,
    values::{
        Integer, NumericLimits,
        color::ColorLimits,
        font_weight::{AvailableFontWeights, FontWeight, FontWeightError, compute_font_weight},
        property::*,
    },
};
use std::num::NonZeroUsize;
fn numeric() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn value(input: &str) -> SpecifiedValue {
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
    value.value
}

fn weight(value: u16) -> FontWeight {
    FontWeight::new(value).unwrap()
}
#[test]
fn numeric_and_absolute_keywords_compute_after_property_parsing() {
    for n in (100..=900).step_by(100) {
        assert_eq!(
            compute_font_weight(&value(&format!("font-weight:{n}")), FontWeight::NORMAL).unwrap(),
            weight(n)
        );
    }
    assert_eq!(
        compute_font_weight(&value("font-weight:normal"), weight(900)).unwrap(),
        weight(400)
    );
    assert_eq!(
        compute_font_weight(&value("font-weight:bold"), weight(100)).unwrap(),
        weight(700)
    );
}
#[test]
fn both_relative_tables_use_parent_computed_weight() {
    for (parent, bolder, lighter) in [
        (100, 400, 100),
        (200, 400, 100),
        (300, 400, 100),
        (400, 700, 100),
        (500, 700, 100),
        (600, 900, 400),
        (700, 900, 400),
        (800, 900, 700),
        (900, 900, 700),
    ] {
        assert_eq!(
            compute_font_weight(&value("font-weight:bolder"), weight(parent)).unwrap(),
            weight(bolder)
        );
        assert_eq!(
            compute_font_weight(&value("font-weight:lighter"), weight(parent)).unwrap(),
            weight(lighter)
        );
    }
}
#[test]
fn all_available_subsets_follow_literal_css_search_orders() {
    let orders = [
        [100, 200, 300, 400, 500, 600, 700, 800, 900],
        [200, 100, 300, 400, 500, 600, 700, 800, 900],
        [300, 200, 100, 400, 500, 600, 700, 800, 900],
        [400, 500, 300, 200, 100, 600, 700, 800, 900],
        [500, 400, 300, 200, 100, 600, 700, 800, 900],
        [600, 700, 800, 900, 500, 400, 300, 200, 100],
        [700, 800, 900, 600, 500, 400, 300, 200, 100],
        [800, 900, 700, 600, 500, 400, 300, 200, 100],
        [900, 800, 700, 600, 500, 400, 300, 200, 100],
    ];
    for mask in 0u16..512 {
        let mut available = AvailableFontWeights::default();
        for bit in 0..9 {
            if mask & (1 << bit) != 0 {
                available.insert(weight((bit + 1) * 100));
            }
        }
        for (i, order) in orders.iter().enumerate() {
            let expected = order
                .iter()
                .copied()
                .find(|n| mask & (1 << (n / 100 - 1)) != 0);
            let actual = available
                .select(weight((i as u16 + 1) * 100))
                .map(FontWeight::number);
            assert_eq!(actual, expected, "mask={mask} desired={}", (i + 1) * 100);
        }
    }
}
#[test]
fn available_face_fallback_does_not_rewrite_inherited_computed_weight() {
    let parent = compute_font_weight(&value("font-weight:bolder"), FontWeight::NORMAL).unwrap();
    let mut available = AvailableFontWeights::default();
    available.insert(weight(500));
    assert_eq!(available.select(parent), Some(weight(500)));
    assert_eq!(parent, weight(700));
    assert_eq!(
        compute_font_weight(&value("font-weight:bolder"), parent).unwrap(),
        weight(900)
    );
}
#[test]
fn invalid_values_are_rejected_without_accepting_unresolved_requests() {
    for n in [0, 99, 101, 450, 901, u16::MAX] {
        assert_eq!(FontWeight::new(n), Err(FontWeightError));
    }
    for raw in ["0", "-100", "450", "1000000000000000000000000000000"] {
        let v = SpecifiedValue::Integer(Integer::parse(raw, numeric()).unwrap());
        assert_eq!(
            compute_font_weight(&v, FontWeight::NORMAL),
            Err(FontWeightError)
        );
    }
    for v in [
        SpecifiedValue::Inherit,
        SpecifiedValue::Initial,
        value("font-style:italic"),
    ] {
        assert_eq!(
            compute_font_weight(&v, FontWeight::NORMAL),
            Err(FontWeightError)
        );
    }
}
