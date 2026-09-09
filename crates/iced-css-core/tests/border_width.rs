use iced_css_core::{
    syntax::*,
    values::{
        LengthTarget, NumericLimits, RangeEvent, border_width::*, color::ColorLimits, property::*,
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
fn context() -> BorderFontContext {
    BorderFontContext {
        size: 20.0,
        x_height: 9.0,
    }
}
#[test]
fn border_keywords_and_lengths_use_selected_constants_and_element_metrics() {
    let style = value("border-top-style:solid");
    for (raw, expected) in [
        ("thin", 1.0),
        ("medium", 3.0),
        ("thick", 5.0),
        ("1in", 96.0),
        ("2em", 40.0),
        ("2ex", 18.0),
    ] {
        let width = value(&format!("border-top-width:{raw}"));
        assert_eq!(
            compute_border_width(&width, &style, context(), numeric())
                .unwrap()
                .value,
            expected
        );
    }
}
#[test]
fn none_hidden_skip_conversion_but_do_not_rescue_negative_inputs() {
    let width = value("border-top-width:10000000000000000000000000000000000000000000px");
    let tiny = NumericLimits {
        digits: NonZeroUsize::new(1).unwrap(),
        work: NonZeroUsize::new(1).unwrap(),
    };
    for style in ["none", "hidden"] {
        let style = value(&format!("border-top-style:{style}"));
        let actual = compute_border_width(&width, &style, context(), tiny).unwrap();
        assert_eq!(actual.value, 0.0);
        assert_eq!(actual.event, None);
        let invalid = SpecifiedValue::Length(LengthTarget::parse("-1px", numeric()).unwrap());
        assert!(compute_border_width(&invalid, &style, context(), numeric()).is_err());
    }
    let style = value("border-top-style:solid");
    assert!(compute_border_width(&width, &style, context(), tiny).is_err());
    assert_eq!(
        compute_border_width(&width, &style, context(), numeric())
            .unwrap()
            .event,
        Some(RangeEvent::UpperSaturation)
    );
}
