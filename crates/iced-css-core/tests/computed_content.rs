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

#[test]
fn computation_preserves_item_order_resolves_uri_and_defers_logical_operations() {
    let parsed = property(
        r#"content:"literal" attr(title) url(../image.png#part) counter(n, upper-roman) open-quote attr(absent)"#,
    );
    let mut origin = iced_css_core::source::Metadata::default();
    origin.set_attribute("title", "counter(other)");
    let computed = compute_content(
        &parsed,
        ContentTarget::Before,
        &origin,
        Some("https://example.test/css/main.css"),
        content_limits(),
    )
    .unwrap();
    let ComputedContent::Items(items) = computed else {
        panic!()
    };
    assert_eq!(items.len(), 6);
    assert!(matches!(items[0], ComputedContentItem::Text("literal")));
    assert!(matches!(
        items[1],
        ComputedContentItem::Text("counter(other)")
    ));
    assert!(
        matches!(&items[2],ComputedContentItem::Uri(uri) if uri=="https://example.test/image.png#part")
    );
    assert!(
        matches!(items[3],ComputedContentItem::Counter(read) if read.name=="n" && read.style==iced_css_core::values::CounterStyle::UpperRoman)
    );
    assert!(matches!(
        items[4],
        ComputedContentItem::Quote(iced_css_core::formatting::quotes::QuoteOperation::Open)
    ));
    assert!(matches!(items[5], ComputedContentItem::Text("")));
    let ComputedContentItem::Text(value) = items[1] else {
        panic!()
    };
    assert_eq!(value.as_ptr(), origin.attribute("title").unwrap().as_ptr());
}
fn content_limits() -> ContentLimits {
    ContentLimits {
        items: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
#[test]
fn ordinary_or_suppressed_content_needs_no_base_and_empty_strings_still_generate_items() {
    let origin = iced_css_core::source::Metadata::default();
    assert!(matches!(
        compute_content(
            &property("content:url(relative)"),
            ContentTarget::Element,
            &origin,
            None,
            content_limits()
        ),
        Ok(ComputedContent::Normal)
    ));
    assert!(matches!(
        compute_content(
            &property("content:normal"),
            ContentTarget::After,
            &origin,
            None,
            content_limits()
        ),
        Ok(ComputedContent::Suppressed)
    ));
    let parsed = property(r#"content:"""#);
    let result = compute_content(
        &parsed,
        ContentTarget::After,
        &origin,
        None,
        content_limits(),
    )
    .unwrap();
    let ComputedContent::Items(items) = result else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], ComputedContentItem::Text("")));
    assert!(matches!(
        compute_content(
            &property("content:url(relative)"),
            ContentTarget::Before,
            &origin,
            None,
            content_limits()
        ),
        Err(ContentComputationError::MissingBase)
    ));
}
#[test]
fn item_memory_and_shared_uri_work_limits_reject_the_whole_candidate() {
    let origin = iced_css_core::source::Metadata::default();
    let parsed = property("content:url(a) url(b)");
    let original = parsed.clone();
    let mut bounded = content_limits();
    bounded.items = NonZeroUsize::MIN;
    assert!(matches!(
        compute_content(
            &parsed,
            ContentTarget::Before,
            &origin,
            Some("asset:root"),
            bounded
        ),
        Err(ContentComputationError::Items)
    ));
    bounded = content_limits();
    bounded.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        compute_content(
            &parsed,
            ContentTarget::Before,
            &origin,
            Some("asset:root"),
            bounded
        ),
        Err(ContentComputationError::Bytes)
    ));
    bounded = content_limits();
    bounded.work = NonZeroUsize::new(700).unwrap();
    assert!(
        compute_content(
            &property("content:url(a)"),
            ContentTarget::Before,
            &origin,
            Some("asset:root"),
            bounded
        )
        .is_ok()
    );
    assert!(
        compute_content(
            &parsed,
            ContentTarget::Before,
            &origin,
            Some("asset:root"),
            bounded
        )
        .is_err()
    );
    assert_eq!(parsed, original);
    assert!(matches!(
        compute_content(
            &property("content:url(é)"),
            ContentTarget::Before,
            &origin,
            Some("asset:root"),
            content_limits()
        ),
        Err(ContentComputationError::Uri(
            iced_css_core::resources::uri::UriError::NonAscii
        ))
    ));
}
