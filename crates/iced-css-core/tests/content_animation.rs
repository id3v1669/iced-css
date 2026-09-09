use iced_css_core::{
    animation::{combine::Combination, content::*},
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

use iced_css_core::source::Metadata;
use iced_css_core::values::content_computation::*;
fn limits() -> ContentLimits {
    ContentLimits {
        items: NonZeroUsize::new(32).unwrap(),
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(100000).unwrap(),
    }
}
#[test]
fn computation_precedes_whole_content_selection() {
    let property = parsed(r#"content:attr(title) url(../image.png) counter(n) open-quote"#);
    let other = parsed(r#"content:"replacement""#);
    let mut origin = Metadata::default();
    origin.set_attribute("title", "original");
    let start = compute_content(
        &property,
        ContentTarget::Before,
        &origin,
        Some("https://example.test/css/theme.css"),
        limits(),
    )
    .unwrap();
    let end = compute_content(&other, ContentTarget::After, &origin, None, limits()).unwrap();
    let ComputedContent::Items(items) = &start else {
        panic!()
    };
    assert!(matches!(items[0], ComputedContentItem::Text("original")));
    assert!(matches!(&items[1],ComputedContentItem::Uri(s) if s=="https://example.test/image.png"));
    assert!(matches!(&items[2],ComputedContentItem::Counter(c) if c.name=="n"));
    assert!(matches!(items[3], ComputedContentItem::Quote(_)));
    for (a, b) in [(&start, &end), (&end, &start), (&start, &start)] {
        for p in [
            -f32::MAX,
            0.0,
            f32::from_bits(0.5_f32.to_bits() - 1),
            0.5,
            1.0,
            f32::MAX,
        ] {
            assert!(std::ptr::eq(
                combine_content(a, b, Combination::Interpolate(p)).unwrap(),
                if p < 0.5 { a } else { b }
            ));
        }
        for op in [Combination::Add, Combination::Accumulate] {
            assert!(std::ptr::eq(combine_content(a, b, op).unwrap(), a));
        }
    }
}
#[test]
fn computed_alternatives_remain_distinct_and_nonfinite_progress_fails() {
    let property = parsed("content:normal");
    let origin = Metadata::default();
    let normal =
        compute_content(&property, ContentTarget::Element, &origin, None, limits()).unwrap();
    let none = compute_content(&property, ContentTarget::Before, &origin, None, limits()).unwrap();
    assert!(matches!(normal, ComputedContent::Normal));
    assert!(matches!(none, ComputedContent::Suppressed));
    for (a, b) in [(&normal, &none), (&none, &normal), (&none, &none)] {
        assert!(std::ptr::eq(
            combine_content(a, b, Combination::Interpolate(0.25)).unwrap(),
            a
        ));
        assert!(std::ptr::eq(
            combine_content(a, b, Combination::Interpolate(0.5)).unwrap(),
            b
        ));
        for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(combine_content(a, b, Combination::Interpolate(p)).is_err());
        }
    }
}
