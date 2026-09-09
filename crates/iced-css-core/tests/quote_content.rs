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
use iced_css_core::formatting::quotes::{QuoteDepth, QuoteOperation, QuotePairs};
fn property(input: &str) -> ParsedProperty {
    let PropertyResult::Parsed(property) = parse(input).unwrap() else {
        panic!("{input}")
    };
    property
}
#[test]
fn parsed_quote_tokens_use_each_participants_marks_and_one_logical_depth() {
    let outer = property(r#"quotes:"[" "]" "(" ")""#);
    let suppressed = property("quotes:none");
    let content = property(
        r#"content:OPEN-QUOTE "literal" no-open-quote close-quote no-close-quote close-quote"#,
    );
    let SpecifiedValue::Content(items) = &content.value else {
        panic!()
    };
    let outer = QuotePairs::from_property(&outer).unwrap();
    let suppressed = QuotePairs::from_property(&suppressed).unwrap();
    let mut depth = QuoteDepth::new(NonZeroUsize::new(8).unwrap());
    assert_eq!(
        depth
            .apply(QuoteOperation::from_content(&items[0]).unwrap(), outer)
            .unwrap(),
        Some("[")
    );
    assert_eq!(QuoteOperation::from_content(&items[1]), None);
    assert_eq!(
        depth
            .apply(QuoteOperation::from_content(&items[2]).unwrap(), suppressed)
            .unwrap(),
        None
    );
    assert_eq!(depth.depth(), 2);
    assert_eq!(
        depth
            .apply(QuoteOperation::from_content(&items[3]).unwrap(), outer)
            .unwrap(),
        Some(")")
    );
    assert_eq!(
        depth
            .apply(QuoteOperation::from_content(&items[4]).unwrap(), outer)
            .unwrap(),
        None
    );
    assert_eq!(
        depth
            .apply(QuoteOperation::from_content(&items[5]).unwrap(), outer)
            .unwrap(),
        None
    );
    assert_eq!(depth.depth(), 0);
}
#[test]
fn wrong_properties_and_unresolved_values_are_not_silently_none() {
    for input in ["display:none", "quotes:inherit", "content:none"] {
        assert!(QuotePairs::from_property(&property(input)).is_none());
    }
    let mut quotes = property("quotes:none");
    quotes.value = SpecifiedValue::InitialReference(
        iced_css_core::values::initial::InitialReference::UserAgentQuotes,
    );
    assert!(QuotePairs::from_property(&quotes).is_none());
    quotes.value = SpecifiedValue::Quotes(Vec::new());
    let pairs = QuotePairs::from_property(&quotes).unwrap();
    let mut depth = QuoteDepth::new(NonZeroUsize::MIN);
    assert_eq!(depth.apply(QuoteOperation::Open, pairs).unwrap(), None);
    assert_eq!(depth.depth(), 1);
    let SpecifiedValue::Keyword(keyword) = property("display:none").value else {
        panic!()
    };
    assert_eq!(
        QuoteOperation::from_content(&ContentItem::Quote(keyword)),
        None
    );
}
