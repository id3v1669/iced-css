use iced_css_core::{selectors::*, source::Metadata};
use std::num::NonZeroUsize;
fn facts(metadata: &Metadata) -> ElementFacts<'_> {
    ElementFacts {
        type_name: "text",
        metadata,
        has_parent: true,
        has_previous_element: false,
        language: Some("en-GB"),
        link: LinkState::NotLink,
        hovered: false,
        active: false,
        focused: false,
    }
}
fn matches(selector: SimpleSelector, element: ElementFacts<'_>) -> bool {
    matches_simple(
        &selector,
        element,
        &mut MatchBudget::new(NonZeroUsize::new(10000).unwrap()),
    )
    .unwrap()
}
fn attr(name: &str, operator: AttributeOperator, value: &str) -> SimpleSelector {
    SimpleSelector::Attribute {
        name: name.into(),
        operator,
        value: value.into(),
    }
}
#[test]
fn type_id_and_class_use_exact_source_metadata() {
    let mut metadata = Metadata::default();
    metadata.set_id("Main");
    metadata.set_attribute("class", "a\tb\nC");
    let element = facts(&metadata);
    assert!(matches(SimpleSelector::Universal, element));
    assert!(matches(SimpleSelector::Type("text".into()), element));
    assert!(!matches(SimpleSelector::Type("Text".into()), element));
    assert!(matches(SimpleSelector::Id("Main".into()), element));
    assert!(!matches(SimpleSelector::Id("main".into()), element));
    assert!(matches(SimpleSelector::Class("b".into()), element));
    assert!(!matches(SimpleSelector::Class("c".into()), element));
    assert!(!matches(SimpleSelector::Class("".into()), element));
}
#[test]
fn attribute_presence_empty_values_and_operators_are_distinct() {
    use AttributeOperator::*;
    let mut metadata = Metadata::default();
    metadata.set_attribute("empty", "");
    metadata.set_attribute("words", "one\ttwo three");
    metadata.set_attribute("lang", "en-GB");
    let element = facts(&metadata);
    assert!(matches(attr("empty", Present, ""), element));
    assert!(matches(attr("empty", Equal, ""), element));
    assert!(!matches(attr("missing", Present, ""), element));
    assert!(matches(attr("words", Includes, "two"), element));
    assert!(!matches(attr("words", Includes, "two three"), element));
    assert!(!matches(attr("words", Includes, ""), element));
    assert!(matches(attr("lang", DashMatch, "en"), element));
    assert!(!matches(attr("lang", DashMatch, "e"), element));
    assert!(!matches(attr("lang", DashMatch, "EN"), element));
}
#[test]
fn language_uses_resolved_source_language_and_its_own_case_rule() {
    let metadata = Metadata::default();
    let element = facts(&metadata);
    assert!(matches(SimpleSelector::Lang("EN".into()), element));
    assert!(!matches(
        attr("lang", AttributeOperator::Present, ""),
        element
    ));
    assert!(!matches(SimpleSelector::Lang("e".into()), element));
    assert!(!matches(SimpleSelector::Lang("".into()), element));
    assert!(!matches(
        SimpleSelector::Lang("en".into()),
        ElementFacts {
            language: None,
            ..element
        }
    ));
}
#[test]
fn first_child_requires_a_source_parent_and_no_preceding_element() {
    let metadata = Metadata::default();
    let element = facts(&metadata);
    assert!(matches(SimpleSelector::FirstChild, element));
    assert!(!matches(
        SimpleSelector::FirstChild,
        ElementFacts {
            has_parent: false,
            ..element
        }
    ));
    assert!(!matches(
        SimpleSelector::FirstChild,
        ElementFacts {
            has_previous_element: true,
            ..element
        }
    ));
}
#[test]
fn link_and_interaction_predicates_use_explicit_facts() {
    let metadata = Metadata::default();
    let element = facts(&metadata);
    assert!(!matches(SimpleSelector::Link, element));
    let visited = ElementFacts {
        link: LinkState::Visited,
        hovered: true,
        active: true,
        focused: true,
        ..element
    };
    assert!(matches(SimpleSelector::Visited, visited));
    assert!(!matches(SimpleSelector::Link, visited));
    for selector in [
        SimpleSelector::Hover,
        SimpleSelector::Active,
        SimpleSelector::Focus,
    ] {
        assert!(matches(selector, visited));
    }
    let unvisited = ElementFacts {
        link: LinkState::Unvisited,
        ..element
    };
    assert!(matches(SimpleSelector::Link, unvisited));
    assert!(!matches(SimpleSelector::Visited, unvisited));
}
#[test]
fn comparison_budget_exhaustion_is_not_a_nonmatch() {
    let mut metadata = Metadata::default();
    metadata.set_attribute("class", &"x".repeat(100));
    assert!(
        matches_simple(
            &SimpleSelector::Class("x".into()),
            facts(&metadata),
            &mut MatchBudget::new(NonZeroUsize::MIN)
        )
        .is_err()
    );
}

fn lookup_boundary(selector: &SimpleSelector, metadata: &Metadata, total: usize, expected: bool) {
    let mut exact = MatchBudget::new(NonZeroUsize::new(total).unwrap());
    assert_eq!(
        matches_simple(selector, facts(metadata), &mut exact).unwrap(),
        expected
    );
    assert_eq!(exact.remaining(), 0);
    let mut short = MatchBudget::new(NonZeroUsize::new(total - 1).unwrap());
    assert!(matches_simple(selector, facts(metadata), &mut short).is_err());
}

#[test]
fn metadata_lookup_misses_reserve_every_attribute_name() {
    let mut metadata = Metadata::default();
    for name in ["aa", "zx", "a-much-longer-unrelated-name"] {
        metadata.set_attribute(name, "");
    }
    lookup_boundary(&SimpleSelector::Id("x".into()), &metadata, 15, false);
    lookup_boundary(&SimpleSelector::Class("x".into()), &metadata, 24, false);
    lookup_boundary(
        &attr("zz", AttributeOperator::Present, ""),
        &metadata,
        16,
        false,
    );
    lookup_boundary(
        &attr("", AttributeOperator::Present, ""),
        &metadata,
        8,
        false,
    );
    lookup_boundary(
        &attr("é", AttributeOperator::Present, ""),
        &metadata,
        16,
        false,
    );
}

#[test]
fn metadata_lookup_hits_reserve_the_full_scan_independent_of_position() {
    for (name, selector, total) in [
        ("id", SimpleSelector::Id("x".into()), 17),
        ("class", SimpleSelector::Class("x".into()), 26),
        ("zz", attr("zz", AttributeOperator::Present, ""), 16),
    ] {
        for first in [true, false] {
            let mut metadata = Metadata::default();
            if first {
                metadata.set_attribute(name, "x");
            }
            metadata.set_attribute("aa", "");
            metadata.set_attribute("long-unrelated-name", "");
            if !first {
                metadata.set_attribute(name, "x");
            }
            lookup_boundary(&selector, &metadata, total, true);
        }
    }
}

#[test]
fn empty_metadata_adds_no_lookup_work_and_presence_ignores_value_bytes() {
    let mut metadata = Metadata::default();
    lookup_boundary(&SimpleSelector::Id("x".into()), &metadata, 3, false);
    lookup_boundary(&SimpleSelector::Class("x".into()), &metadata, 3, false);
    lookup_boundary(
        &attr("zz", AttributeOperator::Present, ""),
        &metadata,
        4,
        false,
    );
    metadata.set_attribute("aa", "");
    metadata.set_attribute("ab", "");
    metadata.set_attribute("zz", "");
    lookup_boundary(
        &attr("zz", AttributeOperator::Present, ""),
        &metadata,
        16,
        true,
    );
    metadata.set_attribute("zz", &"v".repeat(1000));
    lookup_boundary(
        &attr("zz", AttributeOperator::Present, ""),
        &metadata,
        16,
        true,
    );
}
