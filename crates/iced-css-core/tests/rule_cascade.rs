use iced_css_core::{
    cascade::{Origin, Specificity, rule::*, sheets::*, winner_table::*},
    identity::IdAllocator,
    selectors::{LinkState, MatchLimits, SelectorLimits, parse_selector_list},
    source::{Metadata, SourceNode, selector_tree::*},
    syntax::{DeclarationItem, DeclarationLimits, Declarations, RecoveryLimits, StringLimits},
    values::{
        NumericLimits,
        color::ColorLimits,
        property::{ParsedProperty, PropertyResult, parse_property},
        registry::PropertyId,
    },
};
use std::num::{NonZeroU64, NonZeroUsize};
fn work() -> NonZeroUsize {
    NonZeroUsize::new(1000000).unwrap()
}
fn parsed(css: &str) -> ParsedProperty {
    let item = Declarations::new(
        css,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: work(),
                retained_bytes: work(),
                nesting: work(),
            },
            name_bytes: work(),
        },
    )
    .next()
    .unwrap()
    .unwrap();
    let DeclarationItem::Declaration(declaration) = item else {
        panic!()
    };
    let PropertyResult::Parsed(value) = parse_property(
        &declaration,
        ColorLimits {
            syntax: StringLimits {
                work: work(),
                retained_bytes: work(),
            },
            numeric: NumericLimits {
                work: work(),
                digits: work(),
            },
        },
    )
    .unwrap() else {
        panic!()
    };
    value
}
fn record(css: &str, order: u64) -> RuleDeclaration {
    RuleDeclaration {
        parsed: parsed(css),
        order,
        declaration: NonZeroU64::new(order).unwrap(),
    }
}
fn selectors(css: &str) -> Vec<iced_css_core::selectors::Selector> {
    parse_selector_list(
        css,
        SelectorLimits {
            work: work(),
            retained_bytes: work(),
            entries: work(),
        },
    )
    .unwrap()
}
fn sheet() -> SheetId {
    SheetId::new(NonZeroU64::MIN)
}
fn controls() -> AuthorSheets {
    AuthorSheets::new(
        vec![AuthorSheet {
            id: sheet(),
            enabled: true,
            membership: Membership::Persistent,
        }],
        vec![],
        true,
        None,
        SheetLimits {
            entries: work(),
            retained_bytes: work(),
            work: work(),
        },
    )
    .unwrap()
}
fn node() -> SourceNode {
    let mut metadata = Metadata::default();
    metadata.set_classes(&["a"]).unwrap();
    SourceNode {
        instance: IdAllocator::new(NonZeroU64::MIN).allocate().unwrap(),
        parent: None,
        children: vec![],
        metadata,
    }
}
fn element(node: &SourceNode) -> SelectorElement<'_> {
    SelectorElement {
        node,
        type_name: "button",
        link: LinkState::NotLink,
        hovered: false,
        active: false,
        focused: false,
    }
}
fn limits() -> RuleCascadeLimits {
    RuleCascadeLimits {
        matching: MatchLimits {
            work: work(),
            retained_bytes: work(),
        },
        declarations: work(),
    }
}

#[test]
fn late_metadata_lookup_failure_preserves_the_batch_winners() {
    let mut source_node = node();
    source_node.metadata = Metadata::default();
    for name in ["aa", "ab", "ac"] {
        source_node.metadata.set_attribute(name, "");
    }
    let nodes = [element(&source_node)];
    let tree = SelectorTree::new(
        &nodes,
        DocumentLanguage::Unknown,
        SelectorTreeLimits {
            entries: work(),
            retained_bytes: work(),
            work: work(),
        },
    )
    .unwrap();
    let universal = selectors("*");
    let missing = selectors("[zz]");
    let block = [record("display:block", 1)];
    let hidden = [record("display:none", 2)];
    let offers = [
        RuleOffer {
            selectors: &universal,
            declarations: &block,
            origin: RuleOrigin::User,
        },
        RuleOffer {
            selectors: &missing,
            declarations: &hidden,
            origin: RuleOrigin::User,
        },
    ];
    let mut table = PropertyWinners::new();
    let result = cascade_rules(
        &mut table,
        offers,
        RuleTarget {
            source: &tree,
            node: tree.root(),
            pseudo: None,
        },
        &controls(),
        RuleBatchLimits {
            rules: work(),
            per_rule: RuleCascadeLimits {
                matching: MatchLimits {
                    work: NonZeroUsize::new(12).unwrap(),
                    ..limits().matching
                },
                ..limits()
            },
        },
    );
    assert_eq!(
        result,
        Err(RuleBatchError::Rule(RuleCascadeError::Match(
            iced_css_core::selectors::MatchError::Work,
        )))
    );
    assert!(
        table
            .candidate(PropertyId::lookup("display").unwrap())
            .unwrap()
            .is_none()
    );
}
#[test]
fn only_matching_group_members_contribute_specificity_to_property_winners() {
    let source_node = node();
    let nodes = [element(&source_node)];
    let tree = SelectorTree::new(
        &nodes,
        DocumentLanguage::Unknown,
        SelectorTreeLimits {
            entries: work(),
            retained_bytes: work(),
            work: work(),
        },
    )
    .unwrap();
    let first = selectors(".a,#unmatched");
    let second = selectors("button.a");
    let red = [record("color:red", 1)];
    let blue = [record("color:blue", 2)];
    let mut table = PropertyWinners::new();
    let controls = controls();
    for (selectors, declarations) in [(&first, &red), (&second, &blue)] {
        cascade_rule(
            &mut table,
            RuleOffer {
                selectors,
                declarations,
                origin: RuleOrigin::Author(sheet()),
            },
            RuleTarget {
                source: &tree,
                node: tree.root(),
                pseudo: None,
            },
            &controls,
            limits(),
        )
        .unwrap();
    }
    assert!(std::ptr::eq(
        table
            .candidate(PropertyId::lookup("color").unwrap())
            .unwrap()
            .unwrap()
            .value,
        &blue[0].parsed
    ));
}
#[test]
fn late_declaration_conflicts_abort_the_entire_rule_transaction() {
    let source_node = node();
    let nodes = [element(&source_node)];
    let tree = SelectorTree::new(
        &nodes,
        DocumentLanguage::Unknown,
        SelectorTreeLimits {
            entries: work(),
            retained_bytes: work(),
            work: work(),
        },
    )
    .unwrap();
    let existing = parsed("width:9px");
    let declarations = [record("color:red", 1), record("width:1px", 2)];
    let selectors = selectors("*");
    let mut table = PropertyWinners::new();
    table
        .offer(
            &existing,
            TableAdmission {
                origin: Origin::Author,
                specificity: Specificity::ZERO,
                order: 2,
                declaration: NonZeroU64::new(2).unwrap(),
            },
        )
        .unwrap();
    assert!(matches!(
        cascade_rule(
            &mut table,
            RuleOffer {
                selectors: &selectors,
                declarations: &declarations,
                origin: RuleOrigin::Author(sheet())
            },
            RuleTarget {
                source: &tree,
                node: tree.root(),
                pseudo: None
            },
            &controls(),
            limits()
        ),
        Err(RuleCascadeError::Table(TableError::ConflictingOrder))
    ));
    assert!(
        table
            .candidate(PropertyId::lookup("color").unwrap())
            .unwrap()
            .is_none()
    );
    assert!(std::ptr::eq(
        table.candidate(existing.property).unwrap().unwrap().value,
        &existing
    ));
}
#[test]
fn excluded_and_nonmatching_rules_skip_declaration_application() {
    let source_node = node();
    let nodes = [element(&source_node)];
    let tree = SelectorTree::new(
        &nodes,
        DocumentLanguage::Unknown,
        SelectorTreeLimits {
            entries: work(),
            retained_bytes: work(),
            work: work(),
        },
    )
    .unwrap();
    let mut controls = controls();
    controls.set_author_enabled(false).unwrap();
    let declarations = [record("color:red", 1), record("width:1px", 2)];
    let matched = selectors("*");
    let unmatched = selectors(".missing");
    let mut table = PropertyWinners::new();
    let tiny = RuleCascadeLimits {
        matching: MatchLimits {
            work: NonZeroUsize::MIN,
            retained_bytes: work(),
        },
        declarations: NonZeroUsize::MIN,
    };
    assert_eq!(
        cascade_rule(
            &mut table,
            RuleOffer {
                selectors: &matched,
                declarations: &declarations,
                origin: RuleOrigin::Author(sheet())
            },
            RuleTarget {
                source: &tree,
                node: tree.root(),
                pseudo: None
            },
            &controls,
            tiny
        )
        .unwrap(),
        RuleCascadeResult::Excluded
    );
    let bounded = RuleCascadeLimits {
        declarations: NonZeroUsize::MIN,
        ..limits()
    };
    assert_eq!(
        cascade_rule(
            &mut table,
            RuleOffer {
                selectors: &unmatched,
                declarations: &declarations,
                origin: RuleOrigin::User
            },
            RuleTarget {
                source: &tree,
                node: tree.root(),
                pseudo: None
            },
            &controls,
            bounded
        )
        .unwrap(),
        RuleCascadeResult::NotMatched
    );
    assert!(matches!(
        cascade_rule(
            &mut table,
            RuleOffer {
                selectors: &matched,
                declarations: &declarations,
                origin: RuleOrigin::User
            },
            RuleTarget {
                source: &tree,
                node: tree.root(),
                pseudo: None
            },
            &controls,
            bounded
        ),
        Err(RuleCascadeError::Declarations)
    ));
    assert!(
        table
            .candidate(declarations[0].parsed.property)
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_late_rule_failure_rolls_back_all_earlier_rules_in_the_batch() {
    let source_node = node();
    let nodes = [element(&source_node)];
    let tree = SelectorTree::new(
        &nodes,
        DocumentLanguage::Unknown,
        SelectorTreeLimits {
            entries: work(),
            retained_bytes: work(),
            work: work(),
        },
    )
    .unwrap();
    let selectors = selectors("*");
    let first = [record("color:red", 1)];
    let conflict = [record("color:blue", 1)];
    let offers = || {
        [
            RuleOffer {
                selectors: &selectors,
                declarations: &first,
                origin: RuleOrigin::User,
            },
            RuleOffer {
                selectors: &selectors,
                declarations: &conflict,
                origin: RuleOrigin::User,
            },
        ]
    };
    let mut table = PropertyWinners::new();
    let controls = controls();
    assert!(matches!(
        cascade_rules(
            &mut table,
            offers(),
            RuleTarget {
                source: &tree,
                node: tree.root(),
                pseudo: None
            },
            &controls,
            RuleBatchLimits {
                rules: work(),
                per_rule: limits()
            }
        ),
        Err(RuleBatchError::Rule(RuleCascadeError::Table(
            TableError::ConflictingOrder
        )))
    ));
    assert!(
        table
            .candidate(PropertyId::lookup("color").unwrap())
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        cascade_rules(
            &mut table,
            offers(),
            RuleTarget {
                source: &tree,
                node: tree.root(),
                pseudo: None
            },
            &controls,
            RuleBatchLimits {
                rules: NonZeroUsize::MIN,
                per_rule: limits()
            }
        ),
        Err(RuleBatchError::Rules)
    ));
    assert!(
        table
            .candidate(PropertyId::lookup("color").unwrap())
            .unwrap()
            .is_none()
    );
}
#[test]
fn batch_counts_excluded_unmatched_and_applied_rules_and_commits_winners() {
    let source_node = node();
    let nodes = [element(&source_node)];
    let tree = SelectorTree::new(
        &nodes,
        DocumentLanguage::Unknown,
        SelectorTreeLimits {
            entries: work(),
            retained_bytes: work(),
            work: work(),
        },
    )
    .unwrap();
    let matched = selectors("*");
    let unmatched = selectors(".missing");
    let declarations = [record("margin:1px", 1)];
    let mut controls = controls();
    controls.set_author_enabled(false).unwrap();
    let mut table = PropertyWinners::new();
    let result = cascade_rules(
        &mut table,
        [
            RuleOffer {
                selectors: &matched,
                declarations: &declarations,
                origin: RuleOrigin::Author(sheet()),
            },
            RuleOffer {
                selectors: &unmatched,
                declarations: &declarations,
                origin: RuleOrigin::User,
            },
            RuleOffer {
                selectors: &matched,
                declarations: &declarations,
                origin: RuleOrigin::User,
            },
        ],
        RuleTarget {
            source: &tree,
            node: tree.root(),
            pseudo: None,
        },
        &controls,
        RuleBatchLimits {
            rules: work(),
            per_rule: limits(),
        },
    )
    .unwrap();
    assert_eq!(
        result,
        RuleBatchResult {
            excluded: 1,
            not_matched: 1,
            applied: 1,
            declarations: 1,
            slot_replacements: 4
        }
    );
    assert!(
        table
            .candidate(PropertyId::lookup("margin-left").unwrap())
            .unwrap()
            .is_some()
    );
}

#[test]
fn repeated_parsed_rules_receive_distinct_checked_occurrence_positions() {
    let source_node = node();
    let nodes = [element(&source_node)];
    let tree = SelectorTree::new(
        &nodes,
        DocumentLanguage::Unknown,
        SelectorTreeLimits {
            entries: work(),
            retained_bytes: work(),
            work: work(),
        },
    )
    .unwrap();
    let selectors = selectors("*");
    let declarations = [record("color:red", 99)];
    let offer = RuleOffer {
        selectors: &selectors,
        declarations: &declarations,
        origin: RuleOrigin::User,
    };
    let mut table = PropertyWinners::new();
    let result = cascade_positioned_rules(
        &mut table,
        [
            PositionedRuleOffer {
                rule: offer,
                context: RuleContext {
                    first_order: 1,
                    first_declaration: NonZeroU64::MIN,
                },
            },
            PositionedRuleOffer {
                rule: offer,
                context: RuleContext {
                    first_order: 2,
                    first_declaration: NonZeroU64::new(2).unwrap(),
                },
            },
        ],
        RuleTarget {
            source: &tree,
            node: tree.root(),
            pseudo: None,
        },
        &controls(),
        RuleBatchLimits {
            rules: work(),
            per_rule: limits(),
        },
    )
    .unwrap();
    assert_eq!(result.applied, 2);
    let winner = table
        .candidate(declarations[0].parsed.property)
        .unwrap()
        .unwrap();
    assert_eq!(winner.key.order, 2);
    assert_eq!(winner.declaration.get(), 2);
    assert!(std::ptr::eq(winner.value, &declarations[0].parsed));
    assert_eq!(declarations[0].order, 99);
}
#[test]
fn occurrence_identity_exhaustion_does_not_apply_a_partial_rule() {
    let source_node = node();
    let nodes = [element(&source_node)];
    let tree = SelectorTree::new(
        &nodes,
        DocumentLanguage::Unknown,
        SelectorTreeLimits {
            entries: work(),
            retained_bytes: work(),
            work: work(),
        },
    )
    .unwrap();
    let selectors = selectors("*");
    let declarations = [record("color:red", 1), record("width:1px", 2)];
    let mut table = PropertyWinners::new();
    let positioned = PositionedRuleOffer {
        rule: RuleOffer {
            selectors: &selectors,
            declarations: &declarations,
            origin: RuleOrigin::User,
        },
        context: RuleContext {
            first_order: u64::MAX,
            first_declaration: NonZeroU64::new(u64::MAX).unwrap(),
        },
    };
    assert!(matches!(
        cascade_positioned_rules(
            &mut table,
            [positioned],
            RuleTarget {
                source: &tree,
                node: tree.root(),
                pseudo: None
            },
            &controls(),
            RuleBatchLimits {
                rules: work(),
                per_rule: limits()
            }
        ),
        Err(RuleBatchError::Rule(RuleCascadeError::Identity))
    ));
    assert!(
        table
            .candidate(declarations[0].parsed.property)
            .unwrap()
            .is_none()
    );
}
