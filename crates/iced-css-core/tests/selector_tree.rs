use iced_css_core::{
    identity::IdAllocator,
    selectors::{
        LinkState, MatchLimits, SelectorLimits, SelectorSource, matches_selector,
        parse_selector_list,
    },
    source::{Metadata, SourceNode, selector_tree::*},
};
use std::num::{NonZeroU64, NonZeroUsize};
fn limits() -> SelectorTreeLimits {
    SelectorTreeLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(100000).unwrap(),
    }
}
fn nodes() -> Vec<SourceNode> {
    let mut ids = IdAllocator::new(NonZeroU64::new(4).unwrap());
    let ids: Vec<_> = (0..4).map(|_| ids.allocate().unwrap()).collect();
    vec![
        SourceNode {
            instance: ids[0],
            parent: None,
            children: vec![ids[1], ids[2]],
            metadata: Metadata::default(),
        },
        SourceNode {
            instance: ids[1],
            parent: Some(ids[0]),
            children: vec![],
            metadata: Metadata::default(),
        },
        SourceNode {
            instance: ids[2],
            parent: Some(ids[0]),
            children: vec![ids[3]],
            metadata: Metadata::default(),
        },
        SourceNode {
            instance: ids[3],
            parent: Some(ids[2]),
            children: vec![],
            metadata: Metadata::default(),
        },
    ]
}
fn element<'a>(node: &'a SourceNode, name: &'a str) -> SelectorElement<'a> {
    SelectorElement {
        node,
        type_name: name,
        link: LinkState::NotLink,
        hovered: false,
        active: false,
        focused: false,
    }
}
#[test]
fn real_matcher_reads_declared_child_order_resolved_language_and_propagated_states() {
    let mut nodes = nodes();
    nodes[0].metadata.set_attribute("lang", "en-GB");
    nodes[2].metadata.set_attribute("lang", "");
    let elements = [
        element(&nodes[2], "button"),
        SelectorElement {
            hovered: true,
            active: true,
            focused: true,
            ..element(&nodes[3], "text")
        },
        element(&nodes[0], "container"),
        element(&nodes[1], "text"),
    ];
    let tree = SelectorTree::new(&elements, DocumentLanguage::Known("fr"), limits()).unwrap();
    let button = tree.node(0).unwrap();
    let text = tree.node(1).unwrap();
    let root = tree.node(2).unwrap();
    let left = tree.node(3).unwrap();
    assert_eq!(tree.root(), root);
    assert_eq!(tree.previous_element(button), Some(left));
    assert_eq!(tree.parent(text), Some(button));
    assert_eq!(tree.facts(left).unwrap().language, Some("en-GB"));
    assert_eq!(tree.facts(text).unwrap().language, None);
    assert!(tree.facts(root).unwrap().hovered);
    assert!(tree.facts(button).unwrap().active);
    assert!(!tree.facts(root).unwrap().focused);
    assert!(!tree.facts(left).unwrap().hovered);
    let work = limits().work;
    for (selector, subject, expected) in [
        ("container:hover > button:active > text:focus", text, true),
        ("text + button", button, true),
        ("text:first-child:lang(en)", left, true),
        ("text:lang(en)", text, false),
        ("container:focus", root, false),
    ] {
        let selector = parse_selector_list(
            selector,
            SelectorLimits {
                work,
                retained_bytes: work,
                entries: work,
            },
        )
        .unwrap();
        assert_eq!(
            matches_selector(
                &selector[0],
                &tree,
                subject,
                None,
                MatchLimits {
                    work,
                    retained_bytes: work
                }
            )
            .unwrap(),
            expected
        );
    }
    assert!(tree.node(4).is_none());
    assert!(tree.retained_bytes() <= limits().retained_bytes.get());
}
#[test]
fn malformed_membership_and_duplicate_nonempty_css_ids_reject_the_view() {
    for fault in 0..5 {
        let mut nodes = nodes();
        match fault {
            0 => nodes[1].instance = nodes[0].instance,
            1 => {
                let child = nodes[1].instance;
                nodes[0].children.push(child);
            }
            2 => nodes[1].parent = Some(nodes[2].instance),
            3 => {
                nodes[0].metadata.set_id("same");
                nodes[2].metadata.set_id("same");
            }
            4 => nodes[1].parent = None,
            _ => unreachable!(),
        }
        let elements: Vec<_> = nodes
            .iter()
            .map(|node| element(node, "container"))
            .collect();
        assert!(
            SelectorTree::new(&elements, DocumentLanguage::Unknown, limits()).is_err(),
            "{fault}"
        );
    }
    let mut nodes = nodes();
    nodes[0].metadata.set_id("");
    nodes[2].metadata.set_id("");
    let elements: Vec<_> = nodes
        .iter()
        .map(|node| element(node, "container"))
        .collect();
    assert!(SelectorTree::new(&elements, DocumentLanguage::Unknown, limits()).is_ok());
}
#[test]
fn cycles_and_explicit_resource_or_document_language_errors_are_distinct() {
    let mut nodes = nodes();
    nodes[0].children.clear();
    nodes[1].parent = Some(nodes[3].instance);
    nodes[1].children = vec![nodes[2].instance];
    nodes[2].parent = Some(nodes[1].instance);
    nodes[3].children = vec![nodes[1].instance];
    let elements: Vec<_> = nodes
        .iter()
        .map(|node| element(node, "container"))
        .collect();
    assert!(matches!(
        SelectorTree::new(&elements, DocumentLanguage::Unknown, limits()),
        Err(SelectorTreeError::Cycle)
    ));
    let nodes = self::nodes();
    let elements: Vec<_> = nodes
        .iter()
        .map(|node| element(node, "container"))
        .collect();
    assert!(matches!(
        SelectorTree::new(&elements, DocumentLanguage::Known(""), limits()),
        Err(SelectorTreeError::EmptyDocumentLanguage)
    ));
    for kind in 0..3 {
        let mut bounded = limits();
        match kind {
            0 => bounded.entries = NonZeroUsize::MIN,
            1 => bounded.retained_bytes = NonZeroUsize::MIN,
            _ => bounded.work = NonZeroUsize::MIN,
        }
        assert!(matches!(
            SelectorTree::new(&elements, DocumentLanguage::Unknown, bounded),
            Err(SelectorTreeError::Entries | SelectorTreeError::Bytes | SelectorTreeError::Work)
        ));
    }
}

#[test]
fn indexed_membership_agrees_with_existing_dfs_validator_for_all_four_node_parent_maps() {
    let mut accepted = 0;
    for mut encoded in 0..625 {
        let mut nodes = nodes();
        for index in 0..4 {
            let parent = encoded % 5;
            encoded /= 5;
            nodes[index].parent = (parent != 0).then(|| nodes[parent - 1].instance);
            nodes[index].children.clear();
        }
        for child in 0..4 {
            if let Some(parent) = nodes[child].parent {
                let parent = nodes
                    .iter()
                    .position(|node| node.instance == parent)
                    .unwrap();
                let id = nodes[child].instance;
                nodes[parent].children.push(id);
            }
        }
        let expected = iced_css_core::source::validate_tree(&nodes).is_ok();
        // Reverse the storage order to separate it from child declaration order.
        let elements: Vec<_> = nodes
            .iter()
            .rev()
            .map(|node| element(node, "container"))
            .collect();
        let indexed = SelectorTree::new(&elements, DocumentLanguage::Known("de"), limits());
        assert_eq!(indexed.is_ok(), expected);
        if let Ok(tree) = indexed {
            accepted += 1;
            for position in 0..4 {
                let node = tree.node(position).unwrap();
                assert_eq!(tree.facts(node).unwrap().language, Some("de"));
                let original = elements[position].node;
                assert_eq!(
                    tree.parent(node).map(|parent| elements
                        .iter()
                        .enumerate()
                        .find(|(i, _)| tree.node(*i) == Some(parent))
                        .unwrap()
                        .1
                        .node
                        .instance),
                    original.parent
                );
            }
        }
    }
    // Four choices of root times sixteen labelled trees on four vertices.
    assert_eq!(accepted, 64);
}

#[test]
fn structural_faults_are_reported_by_kind() {
    let build = |nodes: &[SourceNode]| {
        let elements: Vec<_> = nodes.iter().map(|node| element(node, "x")).collect();
        SelectorTree::new(&elements, DocumentLanguage::Unknown, limits()).err()
    };
    assert_eq!(build(&nodes()), None);
    let mut duplicate = nodes();
    duplicate[3].instance = duplicate[1].instance;
    assert_eq!(
        build(&duplicate),
        Some(SelectorTreeError::DuplicateInstance)
    );
    let mut css_ids = nodes();
    css_ids[1].metadata.set_attribute("id", "a");
    css_ids[3].metadata.set_attribute("id", "a");
    assert_eq!(build(&css_ids), Some(SelectorTreeError::DuplicateCssId));
    css_ids[3].metadata.set_attribute("id", "b");
    assert_eq!(build(&css_ids), None);
    let mut child = nodes();
    let repeated = child[1].instance;
    child[0].children.push(repeated);
    assert_eq!(build(&child), Some(SelectorTreeError::DuplicateChild));
    let mut missing = nodes();
    missing[0].children.pop();
    assert_eq!(build(&missing), Some(SelectorTreeError::Nonreciprocal));
    let mut foreign = nodes();
    let adopted = foreign[1].instance;
    foreign[2].children.push(adopted);
    assert_eq!(build(&foreign), Some(SelectorTreeError::Nonreciprocal));
    // The last node is still listed as a child but absent from the view.
    assert_eq!(
        build(&nodes()[..3]),
        Some(SelectorTreeError::UnknownInstance)
    );
}

#[test]
fn wide_trees_build_within_near_linear_work() {
    const COUNT: usize = 4000;
    let mut allocator = IdAllocator::new(NonZeroU64::new(COUNT as u64).unwrap());
    let ids: Vec<_> = (0..COUNT).map(|_| allocator.allocate().unwrap()).collect();
    let mut nodes: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let mut metadata = Metadata::default();
            metadata.set_attribute("id", &format!("e{index}"));
            SourceNode {
                instance: *id,
                parent: (index > 0).then_some(ids[0]),
                children: vec![],
                metadata,
            }
        })
        .collect();
    nodes[0].children = ids[1..].to_vec();
    let elements: Vec<_> = nodes.iter().map(|node| element(node, "x")).collect();
    let bounded = SelectorTreeLimits {
        entries: NonZeroUsize::new(2 * COUNT).unwrap(),
        retained_bytes: NonZeroUsize::new(usize::MAX).unwrap(),
        // About 1_000_000 units; pairwise duplicate and child-list scans
        // need over COUNT^2 / 2 = 8_000_000 for the instances alone.
        work: NonZeroUsize::new(250 * COUNT).unwrap(),
    };
    let tree = SelectorTree::new(&elements, DocumentLanguage::Unknown, bounded).unwrap();
    assert_eq!(tree.root(), tree.node(0).unwrap());
}
