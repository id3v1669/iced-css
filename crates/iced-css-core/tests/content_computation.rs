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
fn ordinary_elements_compute_normal_without_evaluating_content_items() {
    for input in [
        "content:normal",
        "content:none",
        r#"content:attr(missing) counter(n) url(relative.png)"#,
        "content:inherit",
    ] {
        assert!(matches!(
            select_content(&property(input), ContentTarget::Element),
            Ok(ContentSelection::Normal)
        ));
    }
    assert!(select_content(&property("display:none"), ContentTarget::Element).is_err());
}
#[test]
fn pseudo_selection_distinguishes_suppression_unresolved_values_and_item_computation() {
    for target in [ContentTarget::Before, ContentTarget::After] {
        for input in ["content:none", "content:normal"] {
            assert!(matches!(
                select_content(&property(input), target),
                Ok(ContentSelection::Suppressed)
            ));
        }
        assert_eq!(
            select_content(&property("content:inherit"), target).unwrap_err(),
            ContentComputationError::Unresolved
        );
        let parsed = property(r#"content:"" attr(title) url(relative.png) open-quote"#);
        let ContentSelection::Items(items) = select_content(&parsed, target).unwrap() else {
            panic!()
        };
        assert_eq!(items.len(), 4);
        assert!(matches!(&items[0],ContentItem::Text(text) if text.is_empty()));
        assert!(matches!(&items[2],ContentItem::Uri(uri) if uri=="relative.png"));
    }
}
#[test]
fn attribute_text_is_borrowed_verbatim_from_the_originating_metadata() {
    let mut metadata = iced_css_core::source::Metadata::default();
    metadata.set_attribute("title", "counter(n) \n url(x)");
    metadata.set_attribute("Title", "different");
    let budget = NonZeroUsize::new(100).unwrap();
    let text = resolve_content_attribute("title", &metadata, budget).unwrap();
    assert_eq!(text, "counter(n) \n url(x)");
    assert_eq!(text.as_ptr(), metadata.attribute("title").unwrap().as_ptr());
    assert_eq!(
        resolve_content_attribute("Title", &metadata, budget).unwrap(),
        "different"
    );
    assert_eq!(
        resolve_content_attribute("absent", &metadata, budget).unwrap(),
        ""
    );
    assert_eq!(
        resolve_content_attribute("title", &metadata, NonZeroUsize::MIN),
        Err(ContentComputationError::Work)
    );
}
