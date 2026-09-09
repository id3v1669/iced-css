use iced_css::evaluation::display_rules::{
    BundleError, BundleLimits, DisplayRuleBundle, GraphBinding, GraphOrigin,
};
use iced_css_core::{
    cascade::{
        rule::{RuleContext, RuleOrigin},
        sheets::{AuthorSheet, AuthorSheets, Membership, SheetError, SheetId, SheetLimits},
        ua_display::{UaDisplayCatalog, UaDisplayLimits},
    },
    selectors::SelectorLimits,
    stylesheet::{
        RuleAssemblyLimits, StylesheetIntakeLimits, StylesheetLimits,
        imports::{ImportBinding, ImportResolution, LoadedGraphLimits, UnavailableImportPolicy},
        owned_graph::{OwnedGraphLimits, OwnedLoadedSheet, OwnedStylesheetGraph},
        parse_separate_unicode_stylesheet,
    },
    syntax::{
        RecoveryLimits, StringLimits,
        decode::DecodeLimits,
        encoding_selection::{EncodingMetadata, SelectionLimits},
        imports::{ImportLimits, ResourceId},
        unicode_intake::IntakeLimits,
    },
    values::{NumericLimits, color::ColorLimits},
};
use std::num::{NonZeroU64, NonZeroUsize};

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn bound() -> NonZeroUsize {
    nz(100_000)
}
fn context(order: u64, id: u64) -> RuleContext {
    RuleContext {
        first_order: order,
        first_declaration: NonZeroU64::new(id).unwrap(),
    }
}
fn sheet(value: u64) -> SheetId {
    SheetId::new(NonZeroU64::new(value).unwrap())
}
fn values() -> ColorLimits {
    ColorLimits {
        syntax: StringLimits {
            work: bound(),
            retained_bytes: bound(),
        },
        numeric: NumericLimits {
            work: bound(),
            digits: bound(),
        },
    }
}
fn catalog() -> UaDisplayCatalog {
    UaDisplayCatalog::new(UaDisplayLimits {
        rules: bound(),
        retained_bytes: bound(),
        work: bound(),
        values: values(),
    })
    .unwrap()
}
fn controls(ids: &[u64], enabled: bool, work: usize) -> AuthorSheets {
    AuthorSheets::new(
        ids.iter()
            .map(|&id| AuthorSheet {
                id: sheet(id),
                enabled: true,
                membership: Membership::Persistent,
            })
            .collect(),
        vec![],
        enabled,
        None,
        SheetLimits {
            entries: bound(),
            retained_bytes: bound(),
            work: nz(work),
        },
    )
    .unwrap()
}
fn limits() -> BundleLimits {
    BundleLimits {
        graphs: bound(),
        rules: bound(),
        retained_bytes: bound(),
        work: bound(),
        membership_work: bound(),
    }
}
fn resource(id: u64, css: &str, imports: Vec<ImportBinding>) -> OwnedLoadedSheet {
    OwnedLoadedSheet {
        resource: ResourceId::new(NonZeroU64::new(id).unwrap()),
        candidate: parse_separate_unicode_stylesheet(
            css.as_bytes(),
            EncodingMetadata {
                transport: None,
                linking: None,
                referring: None,
            },
            context(0, 1),
            StylesheetIntakeLimits {
                intake: IntakeLimits {
                    selection: SelectionLimits {
                        registry_work: bound(),
                        prefix_work: bound(),
                    },
                    decode: DecodeLimits {
                        entries: bound(),
                        retained_bytes: bound(),
                        work: bound(),
                    },
                    charset_work: bound(),
                },
                sheet: StylesheetLimits {
                    rules: RuleAssemblyLimits {
                        recovery: RecoveryLimits {
                            work: bound(),
                            retained_bytes: bound(),
                            nesting: bound(),
                        },
                        selectors: SelectorLimits {
                            work: bound(),
                            retained_bytes: bound(),
                            entries: bound(),
                        },
                        values: values(),
                        entries: bound(),
                        record_bytes: bound(),
                        work: bound(),
                        name_bytes: bound(),
                    },
                    statements: bound(),
                    record_bytes: bound(),
                    work: bound(),
                },
            },
        )
        .unwrap(),
        imports,
    }
}
fn loaded(resources: Vec<OwnedLoadedSheet>, start: RuleContext) -> OwnedStylesheetGraph {
    OwnedStylesheetGraph::new(
        resources,
        0,
        start,
        OwnedGraphLimits {
            record_bytes: bound(),
            work: bound(),
            expansion: LoadedGraphLimits {
                unavailable: UnavailableImportPolicy::Record,
                entries: bound(),
                record_bytes: bound(),
                work: bound(),
                tracking: ImportLimits {
                    entries: bound(),
                    retained_bytes: bound(),
                    work: bound(),
                    occurrences: NonZeroU64::new(100).unwrap(),
                },
            },
        },
    )
    .unwrap()
}
fn graph(css: &str, start: RuleContext) -> OwnedStylesheetGraph {
    loaded(vec![resource(1, css, vec![])], start)
}
fn binding(graph: &OwnedStylesheetGraph, origin: GraphOrigin) -> GraphBinding<'_> {
    GraphBinding { graph, origin }
}

#[test]
fn ua_prefix_is_mandatory_and_borrows_exact_owners() {
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, limits()).unwrap();
    assert_eq!(bundle.rule_count(), 8);
    assert_eq!(bundle.declaration_count(), 8);
    assert_eq!(bundle.author_rule_count(), 0);
    assert_eq!(bundle.work(), 17);
    assert_eq!(bundle.membership_work(), 0);
    assert!(std::ptr::eq(bundle.catalog(), &ua));
    assert!(std::ptr::eq(bundle.sheets(), &sheets));
    assert!(std::ptr::eq(bundle.bindings(), &bindings));
    for (index, offer) in bundle.offers().iter().enumerate() {
        assert!(matches!(offer.rule.origin, RuleOrigin::UserAgent));
        assert_eq!(offer.context, context(index as u64, index as u64 + 1));
    }
}

#[test]
fn chained_imports_keep_original_contexts_shared_leaves_and_provenance() {
    let ua = catalog();
    let user = graph("text{display:block}", ua.next_context());
    let imported = loaded(
        vec![
            resource(
                11,
                "@import 'child'; @import 'child'; text{display:none}",
                vec![
                    ImportBinding {
                        entry: 0,
                        resolution: ImportResolution::Sheet(1),
                    },
                    ImportBinding {
                        entry: 1,
                        resolution: ImportResolution::Sheet(1),
                    },
                ],
            ),
            resource(12, "text{display:inline;float:left}", vec![]),
        ],
        user.next_context().unwrap(),
    );
    let sheets = controls(&[4], true, 100);
    let bindings = [
        binding(&user, GraphOrigin::User),
        binding(&imported, GraphOrigin::Author(sheet(4))),
    ];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, limits()).unwrap();
    assert_eq!(bundle.rule_count(), 12);
    assert_eq!(bundle.declaration_count(), 14);
    assert_eq!(bundle.author_rule_count(), 3);
    assert_eq!(bundle.work(), 30); // 1 + 2*2 + 1 + 2*12
    assert_eq!(bundle.membership_work(), 100);
    assert_eq!(
        bundle.offers()[8..]
            .iter()
            .map(|o| o.context.first_order)
            .collect::<Vec<_>>(),
        [8, 9, 11, 13]
    );
    assert!(std::ptr::eq(
        bundle.offers()[9].rule.declarations,
        bundle.offers()[10].rule.declarations
    ));
    for (index, id) in [(9, 10), (10, 12)] {
        let original = imported
            .find_declaration(NonZeroU64::new(id).unwrap(), bound())
            .unwrap()
            .unwrap();
        assert_eq!(original.source.get(), 12);
        assert_eq!(original.order, bundle.offers()[index].context.first_order);
        assert!(std::ptr::eq(
            original.parsed,
            &bundle.offers()[index].rule.declarations[0].parsed
        ));
    }
    let first = imported
        .find_declaration(NonZeroU64::new(10).unwrap(), bound())
        .unwrap()
        .unwrap();
    let repeated = imported
        .find_declaration(NonZeroU64::new(12).unwrap(), bound())
        .unwrap()
        .unwrap();
    assert_ne!(first.occurrence, repeated.occurrence);
}

#[test]
fn empty_graphs_remain_dependencies_and_charge_both_binding_traversals() {
    let ua = catalog();
    let empty = graph("", context(0, 1));
    let sheets = controls(&[1], false, 100);
    let bindings = [
        binding(&empty, GraphOrigin::User),
        binding(&empty, GraphOrigin::Author(sheet(1))),
    ];
    let mut exact = limits();
    exact.work = nz(22);
    exact.membership_work = nz(100);
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, exact).unwrap();
    assert_eq!(bundle.rule_count(), 8);
    assert_eq!(bundle.work(), 22); // 1 + 4 + 1 + 16
    assert_eq!(bundle.bindings().len(), 2);
    assert!(std::ptr::eq(bundle.bindings()[1].graph, &empty));
    exact.work = nz(21);
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &bindings, &sheets, exact),
        Err(BundleError::Work)
    ));
    exact.work = nz(22);
    exact.membership_work = nz(99);
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &bindings, &sheets, exact),
        Err(BundleError::MembershipWork)
    ));
}

#[test]
fn root_checks_include_empty_and_disabled_graphs() {
    let ua = catalog();
    let empty = graph("", ua.next_context());
    let sheets = controls(&[1], false, 100);
    let unknown = [binding(&empty, GraphOrigin::Author(sheet(2)))];
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &unknown, &sheets, limits()),
        Err(BundleError::Membership {
            binding: 0,
            error: SheetError::UnknownSheet
        })
    ));
    let duplicate = [
        binding(&empty, GraphOrigin::Author(sheet(1))),
        binding(&empty, GraphOrigin::Author(sheet(1))),
    ];
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &duplicate, &sheets, limits()),
        Err(BundleError::DuplicateAuthor { binding: 1, .. })
    ));
    let reversed = [
        binding(&empty, GraphOrigin::Author(sheet(1))),
        binding(&empty, GraphOrigin::User),
    ];
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &reversed, &sheets, limits()),
        Err(BundleError::OriginOrder { binding: 1 })
    ));
    let too_little = controls(&[1], false, 1);
    let one = [binding(&empty, GraphOrigin::Author(sheet(1)))];
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &one, &too_little, limits()),
        Err(BundleError::Membership {
            error: SheetError::Work,
            ..
        })
    ));
}

#[test]
fn overlap_in_either_position_is_rejected_across_origins_even_when_disabled() {
    let ua = catalog();
    let sheets = controls(&[1], false, 100);
    let first = graph("text{display:block;float:left}", context(8, 9));
    for start in [
        context(9, 11),
        context(10, 10),
        context(7, 20),
        context(20, 8),
    ] {
        let second = graph("text{display:none}", start);
        let bindings = [
            binding(&first, GraphOrigin::User),
            binding(&second, GraphOrigin::Author(sheet(1))),
        ];
        assert!(matches!(
            DisplayRuleBundle::new(&ua, &bindings, &sheets, limits()),
            Err(BundleError::Order {
                binding: Some(1),
                ..
            })
        ));
    }
    for start in [context(7, 9), context(8, 8)] {
        let collision = graph("text{display:none}", start);
        let bindings = [binding(&collision, GraphOrigin::User)];
        assert!(matches!(
            DisplayRuleBundle::new(&ua, &bindings, &sheets, limits()),
            Err(BundleError::Order {
                binding: Some(0),
                ..
            })
        ));
    }
}

#[test]
fn gaps_and_exact_numeric_exhaustion_are_preserved_without_rebasing() {
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    for start in [
        context(u64::MAX, 20),
        context(20, u64::MAX),
        context(u64::MAX, u64::MAX),
    ] {
        let last = graph("text{display:none}", start);
        assert!(last.next_context().is_none());
        let empty = graph("", context(0, 1));
        let bindings = [
            binding(&last, GraphOrigin::User),
            binding(&empty, GraphOrigin::User),
        ];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, limits()).unwrap();
        assert_eq!(bundle.offers()[8].context, start);
        let reset = graph("text{display:block}", context(30, 30));
        let rejected = [
            binding(&last, GraphOrigin::User),
            binding(&reset, GraphOrigin::User),
        ];
        assert!(matches!(
            DisplayRuleBundle::new(&ua, &rejected, &sheets, limits()),
            Err(BundleError::Order { .. })
        ));
    }
    let gap = graph("text{display:none;float:left}", context(100, 200));
    let bindings = [binding(&gap, GraphOrigin::User)];
    assert_eq!(
        DisplayRuleBundle::new(&ua, &bindings, &sheets, limits())
            .unwrap()
            .offers()[8]
            .context,
        context(100, 200)
    );
}

#[test]
fn exact_counts_and_actual_retained_bytes_bound_the_candidate() {
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let empty = graph("", ua.next_context());
    let bindings = [
        binding(&empty, GraphOrigin::User),
        binding(&empty, GraphOrigin::User),
    ];
    let baseline = DisplayRuleBundle::new(&ua, &bindings, &sheets, limits()).unwrap();
    let mut exact = limits();
    exact.graphs = nz(2);
    exact.rules = nz(8);
    exact.work = nz(21);
    exact.retained_bytes = nz(baseline.retained_bytes());
    assert!(DisplayRuleBundle::new(&ua, &bindings, &sheets, exact).is_ok());
    let mut tiny = exact;
    tiny.graphs = nz(1);
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &bindings, &sheets, tiny),
        Err(BundleError::Graphs)
    ));
    tiny = exact;
    tiny.rules = nz(7);
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &bindings, &sheets, tiny),
        Err(BundleError::Rules)
    ));
    tiny = exact;
    tiny.retained_bytes = nz(exact.retained_bytes.get() - 1);
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &bindings, &sheets, tiny),
        Err(BundleError::Bytes)
    ));
    exact.retained_bytes = nz(usize::MAX);
    assert!(DisplayRuleBundle::new(&ua, &bindings, &sheets, exact).is_ok());
}

#[test]
fn membership_reservation_uses_actual_controls_and_checks_overflow() {
    let ua = catalog();
    let empty = graph("", ua.next_context());
    let sheets = controls(&[1, 2], true, usize::MAX);
    assert_eq!(sheets.work_limit().get(), usize::MAX);
    let bindings = [
        binding(&empty, GraphOrigin::Author(sheet(1))),
        binding(&empty, GraphOrigin::Author(sheet(2))),
    ];
    let mut maximum = limits();
    maximum.membership_work = nz(usize::MAX);
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &bindings, &sheets, maximum),
        Err(BundleError::MembershipWork)
    ));
}

#[test]
fn every_prior_binding_and_each_author_membership_allowance_are_reserved() {
    let ua = catalog();
    let empty = graph("", ua.next_context());
    let sheets = controls(&[1, 2], true, 7);
    let bindings = [
        binding(&empty, GraphOrigin::User),
        binding(&empty, GraphOrigin::User),
        binding(&empty, GraphOrigin::Author(sheet(1))),
        binding(&empty, GraphOrigin::Author(sheet(2))),
    ];
    let mut exact = limits();
    exact.work = nz(30); // admission 1 + graph visits 8 + prior scans 2+3 + UA visits 16
    exact.membership_work = nz(14);
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, exact).unwrap();
    assert_eq!(bundle.work(), 30);
    assert_eq!(bundle.membership_work(), 14);
    exact.work = nz(29);
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &bindings, &sheets, exact),
        Err(BundleError::Work)
    ));
    exact.work = nz(30);
    exact.membership_work = nz(13);
    assert!(matches!(
        DisplayRuleBundle::new(&ua, &bindings, &sheets, exact),
        Err(BundleError::MembershipWork)
    ));
}
