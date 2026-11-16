use iced_css::capture::{CaptureLimits, CapturedElement, adapters};
use iced_css_core::{
    selectors::{ElementFacts, LinkState, MatchBudget, SimpleSelector, matches_simple},
    source::{AttributeError, AttributeLimits},
};
use std::num::NonZeroUsize;
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100000).unwrap()
}
fn limits() -> CaptureLimits {
    CaptureLimits {
        nodes: n(),
        depth: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn attributes() -> AttributeLimits {
    AttributeLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    }
}

#[test]
fn captured_builders_feed_the_actual_class_and_id_matcher() {
    let text: CapturedElement<'_, (), iced::Theme, ()> = adapters::text("label", limits())
        .unwrap()
        .style_classes(&["old"], attributes())
        .unwrap()
        .style_attribute("class", "new\tselected", attributes())
        .unwrap()
        .style_id("subject", attributes())
        .unwrap()
        .erase();
    let parent = adapters::container(text, limits())
        .unwrap()
        .style_classes(&["outer"], attributes())
        .unwrap();
    let nodes = parent.construction().nodes();
    let facts = ElementFacts {
        type_name: nodes[0].canonical_type(),
        metadata: nodes[0].metadata(),
        has_parent: true,
        has_previous_element: false,
        language: None,
        link: LinkState::NotLink,
        hovered: false,
        active: false,
        focused: false,
        empty: false,
        position: None,
    };
    for (selector, expected) in [
        (SimpleSelector::Class("selected".into()), true),
        (SimpleSelector::Class("old".into()), false),
        (SimpleSelector::Id("subject".into()), true),
    ] {
        assert_eq!(
            matches_simple(&selector, facts, &mut MatchBudget::new(n())).unwrap(),
            expected
        );
    }
    assert_eq!(nodes[1].metadata().attribute("class"), Some("outer"));
    assert!(
        parent.construction().retained_bytes()
            >= nodes.iter().map(|n| n.metadata().retained_bytes()).sum()
    );
}

#[test]
fn class_validation_and_parent_aggregate_capacity_include_metadata() {
    let text = adapters::text::<iced::Theme, ()>("label", limits()).unwrap();
    assert!(matches!(
        text.style_classes(&["invalid class"], attributes()),
        Err(AttributeError::InvalidClassToken)
    ));
    let child: CapturedElement<'_, (), iced::Theme, ()> = adapters::text("", limits())
        .unwrap()
        .style_attribute("large", &"x".repeat(1000), attributes())
        .unwrap()
        .erase();
    let mut small = limits();
    small.retained_bytes = NonZeroUsize::new(1000).unwrap();
    assert!(matches!(
        adapters::container(child, small),
        Err(iced_css::capture::CaptureError::Bytes)
    ));
}
