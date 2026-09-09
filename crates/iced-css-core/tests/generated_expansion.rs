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
use iced_css_core::values::content_computation::*;
fn property(input: &str) -> ParsedProperty {
    let PropertyResult::Parsed(property) = parse(input).unwrap() else {
        panic!("{input}")
    };
    property
}

use iced_css_core::formatting::{
    counters::{CounterRead, CounterTraversalError, CounterTraversalLimits},
    generated::*,
    quotes::{QuoteError, QuotePairs},
};
use iced_css_core::values::{CounterStyle, Integer};
use std::borrow::Cow;
fn generated_limits() -> GeneratedLimits {
    GeneratedLimits {
        counters: CounterTraversalLimits {
            participants: NonZeroUsize::new(100).unwrap(),
            depth: NonZeroUsize::new(20).unwrap(),
            scopes: NonZeroUsize::new(100).unwrap(),
            results: NonZeroUsize::new(100).unwrap(),
            retained_bytes: NonZeroUsize::new(100000).unwrap(),
            numeric: limits().numeric,
        },
        quote_depth: NonZeroUsize::new(20).unwrap(),
    }
}
fn content_limits() -> ContentLimits {
    ContentLimits {
        items: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn directive(value: &str) -> CounterDirective {
    CounterDirective {
        name: "n".into(),
        value: Integer::parse(value, limits().numeric).unwrap(),
    }
}
fn participant<'a>(
    parent: Option<usize>,
    content: &'a ComputedContent<'a>,
    resets: &'a [CounterDirective],
    increments: &'a [CounterDirective],
    quotes: QuotePairs<'a>,
) -> GeneratedParticipant<'a> {
    GeneratedParticipant {
        parent,
        participates: true,
        content,
        resets,
        increments,
        quotes,
    }
}
#[test]
fn computed_content_expands_in_logical_order_with_counter_scopes_and_shared_quote_depth() {
    let mut origin = iced_css_core::source::Metadata::default();
    origin.set_attribute("title", "Title");
    let before = property(r#"content:open-quote attr(title) url(pic.png) counter(n)"#);
    let inside = property(r#"content:no-open-quote counters(n, ".") close-quote"#);
    let after = property("content:close-quote counter(n)");
    let before = compute_content(
        &before,
        ContentTarget::Before,
        &origin,
        Some("asset:root"),
        content_limits(),
    )
    .unwrap();
    let inside = compute_content(
        &inside,
        ContentTarget::Before,
        &origin,
        None,
        content_limits(),
    )
    .unwrap();
    let after = compute_content(
        &after,
        ContentTarget::After,
        &origin,
        None,
        content_limits(),
    )
    .unwrap();
    let normal = ComputedContent::Normal;
    let pairs = [["[".into(), "]".into()], ["(".into(), ")".into()]];
    let quotes = QuotePairs::new(&pairs);
    let zero = [directive("0")];
    let ten = [directive("10")];
    let one = [directive("1")];
    let nodes = [
        participant(None, &normal, &zero, &[], quotes),
        participant(Some(0), &before, &[], &one, quotes),
        participant(Some(0), &normal, &[], &[], quotes),
        participant(Some(2), &inside, &ten, &one, quotes),
        participant(Some(0), &after, &[], &[], quotes),
    ];
    let expansion = expand_generated(&nodes, generated_limits()).unwrap();
    assert_eq!(expansion.quote_depth, 0);
    let text: Vec<_> = expansion
        .fragments
        .iter()
        .map(|f| match &f.content {
            FragmentContent::Text(text) => text.as_ref(),
            FragmentContent::Image(uri) => uri,
        })
        .collect();
    assert_eq!(
        text,
        ["[", "Title", "asset:pic.png", "1", "1.11", ")", "]", "1"]
    );
    assert_eq!(
        expansion
            .fragments
            .iter()
            .map(|f| (f.participant, f.item))
            .collect::<Vec<_>>(),
        [
            (1, 0),
            (1, 1),
            (1, 2),
            (1, 3),
            (3, 1),
            (3, 2),
            (4, 0),
            (4, 1)
        ]
    );
    assert!(matches!(
        &expansion.fragments[1].content,
        FragmentContent::Text(Cow::Borrowed(_))
    ));
    assert!(matches!(
        &expansion.fragments[3].content,
        FragmentContent::Text(Cow::Owned(_))
    ));
}
#[test]
fn suppressed_pseudos_and_subtrees_have_no_logical_effects() {
    let normal = ComputedContent::Normal;
    let suppressed = ComputedContent::Suppressed;
    let read = ComputedContent::Items(vec![ComputedContentItem::Counter(CounterRead {
        name: "n",
        separator: None,
        style: CounterStyle::Decimal,
    })]);
    let one = [directive("1")];
    let quotes = QuotePairs::none();
    let mut nodes = [
        participant(None, &normal, &[], &[], quotes),
        participant(Some(0), &suppressed, &one, &one, quotes),
        participant(Some(1), &read, &one, &one, quotes),
        participant(Some(0), &read, &[], &one, quotes),
        participant(Some(0), &read, &[], &one, quotes),
    ];
    nodes[3].participates = false;
    let expansion = expand_generated(&nodes, generated_limits()).unwrap();
    assert_eq!(expansion.fragments.len(), 1);
    assert_eq!(expansion.fragments[0].participant, 4);
    assert!(matches!(&expansion.fragments[0].content,FragmentContent::Text(text) if text=="1"));
}
#[test]
fn empty_marks_and_empty_counter_results_emit_text_while_none_marks_do_not() {
    let content = ComputedContent::Items(vec![
        ComputedContentItem::Quote(iced_css_core::formatting::quotes::QuoteOperation::Open),
        ComputedContentItem::Counter(CounterRead {
            name: "n",
            separator: None,
            style: CounterStyle::None,
        }),
    ]);
    let pairs = [[String::new(), String::new()]];
    for (quotes, count) in [(QuotePairs::none(), 1), (QuotePairs::new(&pairs), 2)] {
        let expansion = expand_generated(
            &[participant(None, &content, &[], &[], quotes)],
            generated_limits(),
        )
        .unwrap();
        assert_eq!(expansion.quote_depth, 1);
        assert_eq!(expansion.fragments.len(), count);
        assert!(
            expansion
                .fragments
                .iter()
                .all(|f| matches!(&f.content,FragmentContent::Text(text) if text.is_empty()))
        );
    }
}
#[test]
fn depth_and_fragment_exhaustion_return_no_partial_candidate() {
    let content = ComputedContent::Items(vec![
        ComputedContentItem::Quote(iced_css_core::formatting::quotes::QuoteOperation::Open),
        ComputedContentItem::Quote(iced_css_core::formatting::quotes::QuoteOperation::Open),
    ]);
    let nodes = [participant(None, &content, &[], &[], QuotePairs::none())];
    let mut bounded = generated_limits();
    bounded.quote_depth = NonZeroUsize::MIN;
    assert!(matches!(
        expand_generated(&nodes, bounded),
        Err(GeneratedError::Quote(QuoteError::Depth))
    ));
    let content = ComputedContent::Items(vec![
        ComputedContentItem::Text("first"),
        ComputedContentItem::Text("second"),
    ]);
    let nodes = [participant(None, &content, &[], &[], QuotePairs::none())];
    bounded = generated_limits();
    bounded.counters.results = NonZeroUsize::MIN;
    assert!(matches!(
        expand_generated(&nodes, bounded),
        Err(GeneratedError::Counter(CounterTraversalError::Results))
    ));
    assert_eq!(
        expand_generated(&nodes, generated_limits())
            .unwrap()
            .fragments
            .len(),
        2
    );
}
