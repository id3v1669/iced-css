use iced_css_core::{
    selectors::SelectorLimits,
    stylesheet::*,
    syntax::{RecoveryLimits, StringLimits},
    values::{NumericLimits, color::ColorLimits},
};
use std::num::{NonZeroU64, NonZeroUsize};
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100000).unwrap()
}
fn limits() -> RuleAssemblyLimits {
    RuleAssemblyLimits {
        recovery: RecoveryLimits {
            work: n(),
            retained_bytes: n(),
            nesting: n(),
        },
        selectors: SelectorLimits {
            work: n(),
            retained_bytes: n(),
            entries: n(),
        },
        values: ColorLimits {
            syntax: StringLimits {
                work: n(),
                retained_bytes: n(),
            },
            numeric: NumericLimits {
                work: n(),
                digits: n(),
            },
        },
        entries: n(),
        record_bytes: n(),
        work: n(),
        name_bytes: n(),
    }
}
fn context() -> RuleContext {
    RuleContext {
        first_order: 10,
        first_declaration: NonZeroU64::new(20).unwrap(),
    }
}

fn sheet_limits() -> StylesheetLimits {
    StylesheetLimits {
        rules: limits(),
        statements: n(),
        record_bytes: n(),
        work: n(),
    }
}

use iced_css_core::syntax::{
    decode::DecodeLimits, encoding_selection::SelectionLimits, unicode_intake::IntakeLimits,
};
fn intake_limits() -> StylesheetIntakeLimits {
    StylesheetIntakeLimits {
        intake: IntakeLimits {
            selection: SelectionLimits {
                registry_work: n(),
                prefix_work: n(),
            },
            decode: DecodeLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
            },
            charset_work: n(),
        },
        sheet: sheet_limits(),
    }
}

use iced_css_core::{
    resources::publication::PublicationLimits,
    stylesheet::{imports::*, owned_graph::*, publication::*},
    syntax::{
        encoding_selection::EncodingMetadata,
        imports::{ImportLimits, ResourceId},
    },
};
fn id(value: u64) -> ResourceId {
    ResourceId::new(NonZeroU64::new(value).unwrap())
}
fn graph_limits() -> OwnedGraphLimits {
    OwnedGraphLimits {
        record_bytes: n(),
        work: n(),
        expansion: LoadedGraphLimits {
            unavailable: UnavailableImportPolicy::Record,
            entries: n(),
            record_bytes: n(),
            work: n(),
            tracking: ImportLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
                occurrences: NonZeroU64::new(100).unwrap(),
            },
        },
    }
}
fn resource(value: u64, css: &str, imports: Vec<ImportBinding>) -> OwnedLoadedSheet {
    OwnedLoadedSheet {
        resource: id(value),
        candidate: parse_separate_unicode_stylesheet(
            css.as_bytes(),
            EncodingMetadata {
                transport: None,
                linking: None,
                referring: None,
            },
            context(),
            intake_limits(),
        )
        .unwrap(),
        imports,
    }
}
fn link(entry: usize, sheet: usize) -> ImportBinding {
    ImportBinding {
        entry,
        resolution: ImportResolution::Sheet(sheet),
    }
}
fn build() -> OwnedStylesheetGraph {
    OwnedStylesheetGraph::new(
        vec![
            resource(
                1,
                "@import \"child\"; @import \"child\"; r{color:red}",
                vec![link(0, 1), link(1, 1)],
            ),
            resource(
                2,
                "@import \"root\"; @media screen{a{width:1px} b{height:2px}}",
                vec![link(0, 0)],
            ),
        ],
        0,
        context(),
        graph_limits(),
    )
    .unwrap()
}
#[test]
fn owned_graph_preserves_occurrences_media_locations_and_backing_sources() {
    let graph = build();
    let rules: Vec<_> = graph.rules().collect();
    assert_eq!(
        rules.iter().map(|r| r.source).collect::<Vec<_>>(),
        vec![id(2), id(2), id(2), id(2), id(1)]
    );
    assert_eq!(
        rules
            .iter()
            .map(|r| r.context.first_order)
            .collect::<Vec<_>>(),
        vec![10, 11, 12, 13, 14]
    );
    assert_ne!(rules[0].occurrence, rules[2].occurrence);
    assert!(std::ptr::eq(rules[0].rule, rules[2].rule));
    assert!(!std::ptr::eq(rules[0].rule, rules[1].rule));
    assert_eq!(graph.cuts().len(), 2);
    assert!(
        graph
            .cuts()
            .iter()
            .all(|c| c.reason == ImportCutReason::Cycle)
    );
    assert_eq!(graph.occurrences(), 3);
    assert_eq!(graph.root(), id(1));
    let offers: Vec<_> = graph
        .offers(iced_css_core::cascade::rule::RuleOrigin::User)
        .collect();
    assert_eq!(offers.len(), 5);
    assert!(offers.iter().all(|offer| matches!(
        offer.rule.origin,
        iced_css_core::cascade::rule::RuleOrigin::User
    )));
    assert_eq!(offers[4].context.first_order, 14);
    assert_eq!(graph.next_context().unwrap().first_order, 15);
    assert_eq!(
        graph.resources()[0].candidate.source().text(),
        "@import \"child\"; @import \"child\"; r{color:red}"
    );
    fn transport<T: Send + 'static>(_: &T) {}
    transport(&graph);
    let moved = std::thread::spawn(move || graph).join().unwrap();
    assert_eq!(moved.rules().count(), 5);
}
#[test]
fn invalid_unresolved_and_over_budget_graphs_return_no_candidate() {
    let make = || vec![resource(1, "@import \"child\"; r{color:red}", vec![])];
    assert!(matches!(
        OwnedStylesheetGraph::new(make(), 0, context(), graph_limits()),
        Err(OwnedGraphError::Expansion(
            LoadedGraphError::UnresolvedImport { .. }
        ))
    ));
    let mut inputs = make();
    inputs[0].imports.push(link(0, 9));
    assert!(matches!(
        OwnedStylesheetGraph::new(inputs, 0, context(), graph_limits()),
        Err(OwnedGraphError::Expansion(LoadedGraphError::InvalidBinding))
    ));
    let mut tiny = graph_limits();
    tiny.record_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        OwnedStylesheetGraph::new(make(), 0, context(), tiny),
        Err(OwnedGraphError::Budget(RuleAssemblyError::Bytes))
    ));
    let mut tiny = graph_limits();
    tiny.work = NonZeroUsize::MIN;
    assert!(matches!(
        OwnedStylesheetGraph::new(
            vec![resource(1, "r{color:red}", vec![])],
            0,
            context(),
            tiny
        ),
        Err(OwnedGraphError::Budget(RuleAssemblyError::Work))
    ));
    assert!(matches!(
        OwnedStylesheetGraph::new(vec![], 0, context(), graph_limits()),
        Err(OwnedGraphError::Expansion(LoadedGraphError::Root))
    ));
}
#[test]
fn unavailable_policy_and_media_exclusion_use_existing_expansion_contract() {
    let make = || {
        vec![resource(
            1,
            "@import \"print\" print; @import \"missing\"; r{height:2px}",
            vec![ImportBinding {
                entry: 1,
                resolution: ImportResolution::Unavailable,
            }],
        )]
    };
    let graph = OwnedStylesheetGraph::new(make(), 0, context(), graph_limits()).unwrap();
    assert_eq!(graph.rules().count(), 1);
    assert_eq!(
        graph.cuts().iter().map(|c| c.reason).collect::<Vec<_>>(),
        vec![ImportCutReason::MediaExcluded, ImportCutReason::Unavailable]
    );
    let mut strict = graph_limits();
    strict.expansion.unavailable = UnavailableImportPolicy::Fail;
    assert!(matches!(
        OwnedStylesheetGraph::new(make(), 0, context(), strict),
        Err(OwnedGraphError::Expansion(
            LoadedGraphError::UnavailableImport { .. }
        ))
    ));
}
#[test]
fn publication_retains_the_whole_owned_graph_on_failed_replacement() {
    let mut owner = CandidatePublication::<OwnedStylesheetGraph>::new(PublicationLimits {
        requests: NonZeroU64::new(10).unwrap(),
        revisions: NonZeroU64::new(10).unwrap(),
    });
    let first = owner.request().unwrap();
    assert!(matches!(
        owner.finish(first, Ok::<_, &str>(build())).unwrap(),
        SheetCompletion::Published(_)
    ));
    let revision = owner.active_revision();
    let next = owner.request().unwrap();
    assert!(matches!(
        owner
            .finish(next, Err("missing replacement import"))
            .unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(owner.active().unwrap().rules().count(), 5);
    let next = owner.request().unwrap();
    let empty =
        OwnedStylesheetGraph::new(vec![resource(1, "", vec![])], 0, context(), graph_limits())
            .unwrap();
    assert!(matches!(
        owner.finish(next, Ok::<_, &str>(empty)).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().rules().count(), 0);
    assert_ne!(owner.active_revision(), revision);
}

#[test]
fn active_resources_include_empty_imports_and_exclude_unreachable_input() {
    let graph = OwnedStylesheetGraph::new(
        vec![
            resource(1, "", vec![]),
            resource(2, "unused{width:bogus}", vec![]),
            resource(
                3,
                "@import \"empty\"; @import \"empty\"; @import \"unused\" print; r{color:red}",
                vec![link(0, 0), link(1, 0), link(2, 1)],
            ),
        ],
        2,
        context(),
        graph_limits(),
    )
    .unwrap();
    assert_eq!(graph.root(), id(3));
    assert_eq!(graph.resources().len(), 3);
    assert_eq!(
        graph
            .active_resources()
            .map(|r| r.resource)
            .collect::<Vec<_>>(),
        vec![id(3), id(1)]
    );
    assert_eq!(graph.occurrences(), 3);
    assert_eq!(graph.rules().count(), 1);
    assert_eq!(graph.cuts().len(), 1);
    assert_eq!(graph.cuts()[0].reason, ImportCutReason::MediaExcluded);
}

#[test]
fn owned_declaration_provenance_preserves_shorthand_and_import_occurrence() {
    let child = "a{margin:1px;color:red}";
    let graph = OwnedStylesheetGraph::new(
        vec![
            resource(
                1,
                "@import \"child\"; @import \"child\";",
                vec![link(0, 1), link(1, 1)],
            ),
            resource(2, child, vec![]),
        ],
        0,
        context(),
        graph_limits(),
    )
    .unwrap();
    let first = graph
        .find_declaration(NonZeroU64::new(20).unwrap(), n())
        .unwrap()
        .unwrap();
    let repeated = graph
        .find_declaration(NonZeroU64::new(22).unwrap(), n())
        .unwrap()
        .unwrap();
    assert_eq!(
        (first.source.get(), first.occurrence.get(), first.order),
        (2, 2, 10)
    );
    assert_eq!(
        (
            repeated.source.get(),
            repeated.occurrence.get(),
            repeated.order
        ),
        (2, 3, 12)
    );
    assert_eq!(repeated.parsed.property.descriptor().name, "margin");
    assert_eq!(&child[repeated.location.value.clone()], "1px");
    assert!(std::ptr::eq(first.parsed, repeated.parsed));
    assert!(
        graph
            .find_declaration(NonZeroU64::new(24).unwrap(), n())
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        graph.find_declaration(NonZeroU64::new(22).unwrap(), NonZeroUsize::MIN),
        Err(ProvenanceError::Work)
    ));
}

#[test]
fn owned_graph_keeps_cycle_cuts_separate_from_source_provenance() {
    let make = || {
        vec![
            resource(1, "@import 'same-uri'; r{color:red}", vec![link(0, 1)]),
            resource(2, "a{width:1px}", vec![]),
        ]
    };
    let key = ImportCycleId::new(NonZeroU64::new(50).unwrap());
    let graph = {
        let cycles = vec![key, key];
        OwnedStylesheetGraph::new_with_cycles(make(), &cycles, 0, context(), graph_limits())
            .unwrap()
    };
    let moved = std::thread::spawn(move || graph).join().unwrap();
    assert_eq!(moved.rules().count(), 1);
    assert_eq!(moved.rules().next().unwrap().source, id(1));
    assert_eq!(
        moved
            .active_resources()
            .map(|r| r.resource)
            .collect::<Vec<_>>(),
        [id(1)]
    );
    assert_eq!(moved.resources().len(), 2);
    assert_eq!(moved.cuts()[0].source, id(1));
    assert_eq!(moved.cuts()[0].target, Some(id(2)));
    assert_eq!(moved.cuts()[0].reason, ImportCutReason::Cycle);
    assert!(matches!(
        OwnedStylesheetGraph::new_with_cycles(make(), &[key], 0, context(), graph_limits()),
        Err(OwnedGraphError::Expansion(
            LoadedGraphError::CycleIdentities
        ))
    ));
}

#[test]
fn owned_cycle_metadata_admits_actual_capacity_before_worker_transfer() {
    let key = ImportCycleId::new(NonZeroU64::new(5).unwrap());
    let mut over_reserved = Vec::with_capacity(100);
    over_reserved.push(key);
    assert!(matches!(
        ImportCycleTable::new(over_reserved, NonZeroUsize::new(16).unwrap()),
        Err(RuleAssemblyError::Bytes)
    ));
    assert!(matches!(
        ImportCycleTable::new(vec![key], NonZeroUsize::MIN),
        Err(RuleAssemblyError::Bytes)
    ));
    assert!(matches!(
        ImportCycleTable::new(vec![], NonZeroUsize::new(usize::MAX).unwrap()),
        Err(RuleAssemblyError::Bytes)
    ));
    let table = ImportCycleTable::new(vec![key, key], n()).unwrap();
    let moved = std::thread::spawn(move || table).join().unwrap();
    assert_eq!(moved.as_slice(), [key, key]);
}
