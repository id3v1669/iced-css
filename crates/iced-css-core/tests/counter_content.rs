use iced_css_core::{
    syntax::*,
    values::{NumericLimits, color::*, property::*},
};
use std::num::NonZeroUsize;
fn limits() -> ColorLimits {
    ColorLimits {
        syntax: StringLimits {
            retained_bytes: NonZeroUsize::new(4096).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
        numeric: NumericLimits {
            digits: NonZeroUsize::new(100).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
    }
}
fn parse(input: &str) -> Result<PropertyResult, PropertyError> {
    parse_with_limits(input, limits())
}
fn parse_with_limits(input: &str, l: ColorLimits) -> Result<PropertyResult, PropertyError> {
    let DeclarationItem::Declaration(d) = Declarations::new(
        input,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: l.syntax.work,
                nesting: NonZeroUsize::new(100).unwrap(),
                retained_bytes: l.syntax.retained_bytes,
            },
            name_bytes: l.syntax.retained_bytes,
        },
    )
    .next()
    .unwrap()
    .unwrap() else {
        panic!("{input:?}")
    };
    parse_property(&d, l)
}
use iced_css_core::formatting::counters::*;
fn property(input: &str) -> ParsedProperty {
    let PropertyResult::Parsed(property) = parse(input).unwrap() else {
        panic!("{input}")
    };
    property
}

#[test]
fn parsed_directives_and_counter_functions_produce_exact_text() {
    let reset = property("counter-reset:chapter 2 chapter 0");
    let increment = property("counter-increment:chapter chapter 2");
    let content = property(
        r#"content:counter(chapter, upper-roman) counters(chapter, ".") counter(missing) "literal""#,
    );
    let SpecifiedValue::Counters(resets) = &reset.value else {
        panic!()
    };
    let SpecifiedValue::Counters(increments) = &increment.value else {
        panic!()
    };
    let SpecifiedValue::Content(items) = &content.value else {
        panic!()
    };
    assert!(CounterRead::from_content(&items[3]).is_none());
    let reads: Vec<_> = items.iter().filter_map(CounterRead::from_content).collect();
    let root = CounterParticipant {
        parent: None,
        participates: true,
        resets,
        increments,
        reads: &reads,
    };
    let limits = CounterTraversalLimits {
        participants: NonZeroUsize::new(10).unwrap(),
        depth: NonZeroUsize::new(10).unwrap(),
        scopes: NonZeroUsize::new(10).unwrap(),
        results: NonZeroUsize::new(10).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        numeric: limits().numeric,
    };
    let texts = evaluate_counters(&[root], limits).unwrap();
    assert_eq!(
        texts.iter().map(|v| v.text.as_str()).collect::<Vec<_>>(),
        ["III", "3", "0"]
    );
}
