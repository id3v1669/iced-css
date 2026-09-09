use iced_css_core::{
    cascade::{
        Level, Specificity, rule::*, sheets::*, ua_display::*, winner_table::PropertyWinners,
    },
    identity::IdAllocator,
    selectors::{LinkState, MatchLimits, PseudoElement, SelectorLimits, parse_selector_list},
    source::{Metadata, SourceNode, selector_tree::*},
    syntax::{DeclarationItem, DeclarationLimits, Declarations, RecoveryLimits, StringLimits},
    values::{NumericLimits, color::ColorLimits, property::*, registry::PropertyId},
};
use std::num::{NonZeroU64, NonZeroUsize};

fn n(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn ample() -> NonZeroUsize {
    n(1_000_000)
}
fn values() -> ColorLimits {
    ColorLimits {
        syntax: StringLimits {
            work: ample(),
            retained_bytes: ample(),
        },
        numeric: NumericLimits {
            work: ample(),
            digits: ample(),
        },
    }
}
fn limits() -> UaDisplayLimits {
    UaDisplayLimits {
        rules: n(8),
        work: n(808),
        retained_bytes: ample(),
        values: values(),
    }
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
            entries: ample(),
            retained_bytes: ample(),
            work: ample(),
        },
    )
    .unwrap()
}
fn sheet() -> SheetId {
    SheetId::new(NonZeroU64::MIN)
}
fn source_node() -> SourceNode {
    SourceNode {
        instance: IdAllocator::new(NonZeroU64::MIN).allocate().unwrap(),
        parent: None,
        children: vec![],
        metadata: Metadata::default(),
    }
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
fn tree<'a>(elements: &'a [SelectorElement<'a>]) -> SelectorTree<'a> {
    SelectorTree::new(
        elements,
        DocumentLanguage::Unknown,
        SelectorTreeLimits {
            entries: ample(),
            retained_bytes: ample(),
            work: ample(),
        },
    )
    .unwrap()
}
fn batch_limits() -> RuleBatchLimits {
    RuleBatchLimits {
        rules: n(8),
        per_rule: RuleCascadeLimits {
            declarations: n(1),
            matching: MatchLimits {
                work: ample(),
                retained_bytes: ample(),
            },
        },
    }
}
fn display() -> PropertyId {
    PropertyId::lookup("display").unwrap()
}

// Literal approved policy fixture, independent of the catalog's grouping/storage.
const FAMILIES: &[(&str, &str)] = &[
    (
        "container row column grid stack keyed_column scrollable pin mouse_area sensor themer float tooltip hover opaque responsive lazy component markdown iced pane_grid pane_content pane_title",
        "block",
    ),
    ("text rich_text", "inline"),
    (
        "button checkbox radio toggler text_input text_editor slider vertical_slider pick_list combo_box progress_bar space rule image image_viewer svg canvas qr_code shader pane_controls",
        "inline-block",
    ),
    ("table", "table"),
    ("table_header_group", "table-header-group"),
    ("table_row_group", "table-row-group"),
    ("table_row", "table-row"),
    ("table_cell table_header_cell", "table-cell"),
];

#[test]
fn every_approved_family_matches_one_normal_ua_display_rule() {
    let catalog = UaDisplayCatalog::new(limits()).unwrap();
    let controls = controls();
    let mut names = std::collections::BTreeSet::new();
    for &(family, expected) in FAMILIES {
        for name in family.split_whitespace() {
            assert!(names.insert(name));
            let node = source_node();
            let elements = [element(&node, name)];
            let source = tree(&elements);
            let mut winners = PropertyWinners::new();
            let result = cascade_positioned_rules(
                &mut winners,
                catalog.offers(),
                RuleTarget {
                    source: &source,
                    node: source.root(),
                    pseudo: None,
                },
                &controls,
                batch_limits(),
            )
            .unwrap();
            assert_eq!(
                (result.applied, result.not_matched, result.declarations),
                (1, 7, 1),
                "{name}"
            );
            let winner = winners.candidate(display()).unwrap().unwrap();
            let SpecifiedValue::Keyword(keyword) = winner.value.value else {
                panic!("{name}")
            };
            assert_eq!(keyword.as_str(), expected, "{name}");
            assert_eq!(winner.key.level, Level::UserAgent);
            assert_eq!(winner.key.specificity, Specificity::selector(0, 0, 1));
            assert!(!winner.value.important);
        }
    }
    assert_eq!(names.len(), 51);
    assert_eq!(
        catalog
            .offers()
            .map(|r| r.rule.selectors.len())
            .sum::<usize>(),
        51
    );
}

#[test]
fn unknown_case_changed_and_pseudo_targets_receive_no_catalog_default() {
    let catalog = UaDisplayCatalog::new(limits()).unwrap();
    for (name, pseudo) in [
        ("custom_widget", None),
        ("Button", None),
        ("button", Some(PseudoElement::Before)),
    ] {
        let node = source_node();
        let elements = [element(&node, name)];
        let source = tree(&elements);
        let mut winners = PropertyWinners::new();
        let result = cascade_positioned_rules(
            &mut winners,
            catalog.offers(),
            RuleTarget {
                source: &source,
                node: source.root(),
                pseudo,
            },
            &controls(),
            batch_limits(),
        )
        .unwrap();
        assert_eq!(result.not_matched, 8);
        assert!(winners.candidate(display()).unwrap().is_none());
    }
}

fn parsed(css: &str) -> ParsedProperty {
    let DeclarationItem::Declaration(d) = Declarations::new(
        css,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: ample(),
                retained_bytes: ample(),
                nesting: ample(),
            },
            name_bytes: ample(),
        },
    )
    .next()
    .unwrap()
    .unwrap() else {
        panic!()
    };
    let PropertyResult::Parsed(p) = parse_property(&d, values()).unwrap() else {
        panic!()
    };
    p
}
#[test]
fn continued_user_and_author_rules_override_ua_and_disabled_author_preserves_it() {
    let catalog = UaDisplayCatalog::new(limits()).unwrap();
    let selectors = parse_selector_list(
        "*",
        SelectorLimits {
            work: ample(),
            retained_bytes: ample(),
            entries: ample(),
        },
    )
    .unwrap();
    let declarations = [RuleDeclaration {
        parsed: parsed("display:none"),
        order: 8,
        declaration: NonZeroU64::new(9).unwrap(),
    }];
    let node = source_node();
    let elements = [element(&node, "button")];
    let source = tree(&elements);
    for (origin, enabled, expected) in [
        (RuleOrigin::User, true, "none"),
        (RuleOrigin::Author(sheet()), true, "none"),
        (RuleOrigin::Author(sheet()), false, "inline-block"),
        (RuleOrigin::User, false, "none"),
    ] {
        let mut controls = controls();
        controls.set_author_enabled(enabled).unwrap();
        let mut winners = PropertyWinners::new();
        let offers = catalog.offers().chain([PositionedRuleOffer {
            rule: RuleOffer {
                selectors: &selectors,
                declarations: &declarations,
                origin,
            },
            context: catalog.next_context(),
        }]);
        cascade_positioned_rules(
            &mut winners,
            offers,
            RuleTarget {
                source: &source,
                node: source.root(),
                pseudo: None,
            },
            &controls,
            RuleBatchLimits {
                rules: n(9),
                ..batch_limits()
            },
        )
        .unwrap();
        let winner = winners.candidate(display()).unwrap().unwrap();
        let SpecifiedValue::Keyword(keyword) = winner.value.value else {
            panic!()
        };
        assert_eq!(keyword.as_str(), expected);
        if expected == "none" {
            assert_eq!((winner.key.order, winner.declaration.get()), (8, 9));
        }
    }
}

#[test]
fn policy_provenance_and_next_context_are_nonoverlapping() {
    let catalog = UaDisplayCatalog::new(limits()).unwrap();
    assert_eq!(catalog.rule_count(), 8);
    for (index, offer) in catalog.offers().enumerate() {
        assert_eq!(offer.rule.declarations.len(), 1);
        assert_eq!(offer.context.first_order, index as u64);
        assert_eq!(offer.context.first_declaration.get(), index as u64 + 1);
        assert_eq!(offer.rule.declarations[0].order, offer.context.first_order);
        assert_eq!(
            offer.rule.declarations[0].declaration,
            offer.context.first_declaration
        );
    }
    assert_eq!(
        catalog.next_context(),
        RuleContext {
            first_order: 8,
            first_declaration: NonZeroU64::new(9).unwrap()
        }
    );
}

#[test]
fn exact_rule_work_and_actual_retained_byte_allowances_admit_and_one_less_rejects() {
    let catalog = UaDisplayCatalog::new(limits()).unwrap();
    let exact_bytes = catalog.retained_bytes();
    assert!(exact_bytes > 1);
    assert!(
        UaDisplayCatalog::new(UaDisplayLimits {
            retained_bytes: n(exact_bytes),
            ..limits()
        })
        .is_ok()
    );
    assert!(matches!(
        UaDisplayCatalog::new(UaDisplayLimits {
            retained_bytes: n(exact_bytes - 1),
            ..limits()
        }),
        Err(UaDisplayError::Bytes)
    ));
    assert!(matches!(
        UaDisplayCatalog::new(UaDisplayLimits {
            rules: n(7),
            ..limits()
        }),
        Err(UaDisplayError::Rules)
    ));
    assert!(matches!(
        UaDisplayCatalog::new(UaDisplayLimits {
            work: n(807),
            ..limits()
        }),
        Err(UaDisplayError::Work)
    ));
    assert!(
        UaDisplayCatalog::new(UaDisplayLimits {
            rules: n(usize::MAX),
            work: n(usize::MAX),
            retained_bytes: n(usize::MAX),
            ..limits()
        })
        .is_ok()
    );
    let tiny = ColorLimits {
        syntax: StringLimits {
            work: n(1),
            ..values().syntax
        },
        ..values()
    };
    assert!(
        UaDisplayCatalog::new(UaDisplayLimits {
            values: tiny,
            ..limits()
        })
        .is_err()
    );
}

#[test]
fn increasing_parser_allowances_does_not_reject_the_same_small_catalog() {
    let wide = n(usize::MAX);
    let values = ColorLimits {
        syntax: StringLimits {
            work: wide,
            retained_bytes: wide,
        },
        numeric: NumericLimits {
            work: wide,
            digits: wide,
        },
    };
    UaDisplayCatalog::new(UaDisplayLimits { values, ..limits() }).unwrap();
}

#[test]
fn keyword_parser_byte_exhaustion_is_propagated() {
    let small = ColorLimits {
        syntax: StringLimits {
            retained_bytes: n(7),
            ..values().syntax
        },
        ..values()
    };
    assert!(matches!(
        UaDisplayCatalog::new(UaDisplayLimits {
            values: small,
            ..limits()
        }),
        Err(UaDisplayError::Property(_))
    ));
}
