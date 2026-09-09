use iced_css_core::{selectors::*, source::Metadata};
use std::num::NonZeroUsize;
struct Node {
    name: &'static str,
    metadata: Metadata,
    parent: Option<usize>,
    previous: Option<usize>,
}
struct Tree(Vec<Node>);
impl SelectorSource for Tree {
    type Node = usize;
    fn facts(&self, node: usize) -> Option<ElementFacts<'_>> {
        let node = self.0.get(node)?;
        Some(ElementFacts {
            type_name: node.name,
            metadata: &node.metadata,
            has_parent: node.parent.is_some(),
            has_previous_element: node.previous.is_some(),
            language: None,
            link: LinkState::NotLink,
            hovered: false,
            active: false,
            focused: false,
        })
    }
    fn parent(&self, node: usize) -> Option<usize> {
        self.0.get(node)?.parent
    }
    fn previous_element(&self, node: usize) -> Option<usize> {
        self.0.get(node)?.previous
    }
}
fn node(name: &'static str, class: &str, parent: Option<usize>, previous: Option<usize>) -> Node {
    let mut metadata = Metadata::default();
    metadata.set_attribute("class", class);
    Node {
        name,
        metadata,
        parent,
        previous,
    }
}
fn selectors(input: &str) -> Vec<Selector> {
    parse_selector_list(
        input,
        SelectorLimits {
            work: NonZeroUsize::new(100000).unwrap(),
            retained_bytes: NonZeroUsize::new(65536).unwrap(),
            entries: NonZeroUsize::new(1000).unwrap(),
        },
    )
    .unwrap()
}
fn limits() -> MatchLimits {
    MatchLimits {
        work: NonZeroUsize::new(100000).unwrap(),
        retained_bytes: NonZeroUsize::new(65536).unwrap(),
    }
}
#[test]
fn descendant_search_backtracks_after_a_nearer_ancestor_fails() {
    let tree = Tree(vec![
        node("row", "", None, None),
        node("container", "a", Some(0), None),
        node("container", "", Some(1), None),
        node("container", "a", Some(2), None),
        node("text", "", Some(3), None),
    ]);
    assert!(matches_selector(&selectors("row > .a text")[0], &tree, 4, None, limits()).unwrap());
    assert!(
        !matches_selector(&selectors("button > .a text")[0], &tree, 4, None, limits()).unwrap()
    );
}
#[test]
fn child_and_adjacent_combinators_follow_source_element_relations() {
    let tree = Tree(vec![
        node("row", "", None, None),
        node("text", "a", Some(0), None),
        node("button", "", Some(0), Some(1)),
        node("text", "b", Some(0), Some(2)),
    ]);
    assert!(
        matches_selector(
            &selectors("row > text.a + button")[0],
            &tree,
            2,
            None,
            limits()
        )
        .unwrap()
    );
    assert!(!matches_selector(&selectors("text.a + text.b")[0], &tree, 3, None, limits()).unwrap());
    assert!(matches_selector(&selectors(":first-child")[0], &tree, 1, None, limits()).unwrap());
    assert!(!matches_selector(&selectors(":first-child")[0], &tree, 0, None, limits()).unwrap());
}
#[test]
fn pseudo_element_targets_do_not_become_source_children() {
    let tree = Tree(vec![node("text", "a", None, None)]);
    let selector = &selectors("text.a:before")[0];
    assert!(matches_selector(selector, &tree, 0, Some(PseudoElement::Before), limits()).unwrap());
    assert!(!matches_selector(selector, &tree, 0, None, limits()).unwrap());
    assert!(!matches_selector(selector, &tree, 0, Some(PseudoElement::After), limits()).unwrap());
}
#[test]
fn selector_groups_use_the_highest_matching_specificity_only() {
    let tree = Tree(vec![node("text", "a", None, None)]);
    let value = matching_specificity(&selectors("text,.a,#missing"), &tree, 0, None, limits())
        .unwrap()
        .unwrap();
    assert_eq!(value.components(), [0, 0, 1, 0]);
}
#[test]
fn missing_nodes_cycles_and_resource_limits_are_explicit_errors() {
    let tree = Tree(vec![node("text", "", Some(0), None)]);
    assert_eq!(
        matches_selector(
            &selectors(".missing text")[0],
            &tree,
            0,
            None,
            MatchLimits {
                work: NonZeroUsize::new(100).unwrap(),
                ..limits()
            }
        ),
        Err(MatchError::Work)
    );
    assert_eq!(
        matches_selector(&selectors("text")[0], &tree, 9, None, limits()),
        Err(MatchError::InvalidSource)
    );
    assert_eq!(
        matches_selector(
            &selectors("text")[0],
            &tree,
            0,
            None,
            MatchLimits {
                retained_bytes: NonZeroUsize::MIN,
                ..limits()
            }
        ),
        Err(MatchError::Bytes)
    );
}

#[test]
fn metadata_lookup_exhaustion_is_a_matching_error() {
    let mut metadata = Metadata::default();
    for name in ["aa", "ab", "ac"] {
        metadata.set_attribute(name, "");
    }
    let tree = Tree(vec![Node {
        name: "text",
        metadata,
        parent: None,
        previous: None,
    }]);
    let selector = selectors("[zz]");
    assert_eq!(
        matching_specificity(
            &selector,
            &tree,
            0,
            None,
            MatchLimits {
                work: NonZeroUsize::new(12).unwrap(),
                ..limits()
            }
        ),
        Err(MatchError::Work)
    );
    assert_eq!(
        matching_specificity(&selector, &tree, 0, None, limits()),
        Ok(None)
    );
}

#[test]
fn ancestor_metadata_lookup_uses_the_visited_nodes_attribute_count() {
    let mut ancestor = node("row", "", None, None);
    for name in ["aa", "ab", "ac", "ad", "ae", "af", "ag", "ah"] {
        ancestor.metadata.set_attribute(name, "");
    }
    let tree = Tree(vec![ancestor, node("text", "", Some(0), None)]);
    assert_eq!(
        matching_specificity(
            &selectors("[zz] > *"),
            &tree,
            1,
            None,
            MatchLimits {
                work: NonZeroUsize::new(24).unwrap(),
                ..limits()
            }
        ),
        Err(MatchError::Work)
    );
    assert_eq!(
        matching_specificity(&selectors("[zz] > *"), &tree, 1, None, limits()),
        Ok(None)
    );
}
