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

use iced_css_core::{
    stylesheet::imports::*,
    syntax::imports::{ImportLimits, ResourceId},
};
fn id(value: u64) -> ResourceId {
    ResourceId::new(NonZeroU64::new(value).unwrap())
}
fn graph_limits() -> LoadedGraphLimits {
    LoadedGraphLimits {
        unavailable: UnavailableImportPolicy::Record,
        entries: n(),
        record_bytes: n(),
        work: n(),
        tracking: ImportLimits {
            entries: n(),
            retained_bytes: n(),
            work: n(),
            occurrences: NonZeroU64::new(1000).unwrap(),
        },
    }
}
fn parse(css: &str) -> ParsedStylesheet {
    parse_stylesheet(css, context(), sheet_limits()).unwrap()
}
#[test]
fn repeated_imports_expand_in_order_while_only_active_cycles_are_cut() {
    let root = parse("@import \"a\"; @import \"a\"; @import \"not-loaded\" print; r{color:red}");
    let a = parse("@import \"b\"; a{width:1px}");
    let b = parse("@import \"a\"; b{height:2px}");
    let root_links = [
        ImportBinding {
            entry: 0,
            resolution: ImportResolution::Sheet(1),
        },
        ImportBinding {
            entry: 1,
            resolution: ImportResolution::Sheet(1),
        },
    ];
    let a_links = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::Sheet(2),
    }];
    let b_links = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::Sheet(1),
    }];
    let loaded = [
        LoadedSheet {
            resource: id(1),
            sheet: &root,
            imports: &root_links,
        },
        LoadedSheet {
            resource: id(2),
            sheet: &a,
            imports: &a_links,
        },
        LoadedSheet {
            resource: id(3),
            sheet: &b,
            imports: &b_links,
        },
    ];
    let plan = expand_loaded_imports(&loaded, 0, context(), graph_limits()).unwrap();
    assert_eq!(
        plan.rules
            .iter()
            .map(|r| (
                r.source.get(),
                r.occurrence.get(),
                r.context.first_order,
                r.context.first_declaration.get()
            ))
            .collect::<Vec<_>>(),
        [
            (3, 3, 10, 20),
            (2, 2, 11, 21),
            (3, 5, 12, 22),
            (2, 4, 13, 23),
            (1, 1, 14, 24)
        ]
    );
    assert_eq!(
        plan.cuts.iter().map(|cut| cut.reason).collect::<Vec<_>>(),
        [
            ImportCutReason::Cycle,
            ImportCutReason::Cycle,
            ImportCutReason::MediaExcluded
        ]
    );
    assert_eq!(plan.occurrences, 5);
    assert_eq!(plan.visited_sheets, vec![0, 1, 2]);
    assert_eq!(plan.next_context.unwrap().first_order, 15);
    assert!(std::ptr::eq(plan.rules[0].rule, plan.rules[2].rule));
    assert_eq!(
        plan.offers(iced_css_core::cascade::rule::RuleOrigin::User)
            .count(),
        5
    );
}
#[test]
fn missing_resolution_is_distinct_from_explicit_unavailable_and_bad_bindings() {
    let root = parse("@import \"a\"; r{color:red}");
    let loaded = [LoadedSheet {
        resource: id(1),
        sheet: &root,
        imports: &[],
    }];
    assert!(matches!(
        expand_loaded_imports(&loaded, 0, context(), graph_limits()),
        Err(LoadedGraphError::UnresolvedImport { sheet: 0, entry: 0 })
    ));
    let links = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::Unavailable,
    }];
    let loaded = [LoadedSheet {
        resource: id(1),
        sheet: &root,
        imports: &links,
    }];
    let plan = expand_loaded_imports(&loaded, 0, context(), graph_limits()).unwrap();
    assert_eq!(plan.rules.len(), 1);
    assert_eq!(plan.cuts[0].reason, ImportCutReason::Unavailable);
    let links = [ImportBinding {
        entry: 1,
        resolution: ImportResolution::Unavailable,
    }];
    let loaded = [LoadedSheet {
        resource: id(1),
        sheet: &root,
        imports: &links,
    }];
    assert!(matches!(
        expand_loaded_imports(&loaded, 0, context(), graph_limits()),
        Err(LoadedGraphError::InvalidBinding)
    ));
    let links = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::Unavailable,
    }; 2];
    let loaded = [LoadedSheet {
        resource: id(1),
        sheet: &root,
        imports: &links,
    }];
    assert!(matches!(
        expand_loaded_imports(&loaded, 0, context(), graph_limits()),
        Err(LoadedGraphError::DuplicateBinding)
    ));
}
#[test]
fn graph_limits_and_identity_exhaustion_reject_the_whole_plan() {
    let root = parse("r{color:red;width:1px}");
    let loaded = [LoadedSheet {
        resource: id(1),
        sheet: &root,
        imports: &[],
    }];
    let mut bounded = graph_limits();
    bounded.record_bytes = NonZeroUsize::MIN;
    assert!(expand_loaded_imports(&loaded, 0, context(), bounded).is_err());
    bounded = graph_limits();
    bounded.work = NonZeroUsize::MIN;
    assert!(expand_loaded_imports(&loaded, 0, context(), bounded).is_err());
    let last = RuleContext {
        first_order: u64::MAX,
        first_declaration: NonZeroU64::new(u64::MAX).unwrap(),
    };
    assert!(expand_loaded_imports(&loaded, 0, last, graph_limits()).is_err());
    let root = parse("r{color:red}");
    let loaded = [LoadedSheet {
        resource: id(1),
        sheet: &root,
        imports: &[],
    }];
    let plan = expand_loaded_imports(&loaded, 0, last, graph_limits()).unwrap();
    assert!(plan.next_context.is_none());
    let loaded = [
        LoadedSheet {
            resource: id(1),
            sheet: &root,
            imports: &[],
        },
        LoadedSheet {
            resource: id(1),
            sheet: &root,
            imports: &[],
        },
    ];
    assert!(matches!(
        expand_loaded_imports(&loaded, 0, context(), graph_limits()),
        Err(LoadedGraphError::DuplicateResource)
    ));
}

#[test]
fn winning_declaration_identity_recovers_shared_source_and_shorthand_provenance() {
    let root = parse("@import \"a\"; @import \"a\";");
    let child_css = "a{margin:1px;color:red}";
    let child = parse(child_css);
    let links = [
        ImportBinding {
            entry: 0,
            resolution: ImportResolution::Sheet(1),
        },
        ImportBinding {
            entry: 1,
            resolution: ImportResolution::Sheet(1),
        },
    ];
    let loaded = [
        LoadedSheet {
            resource: id(1),
            sheet: &root,
            imports: &links,
        },
        LoadedSheet {
            resource: id(2),
            sheet: &child,
            imports: &[],
        },
    ];
    let plan = expand_loaded_imports(&loaded, 0, context(), graph_limits()).unwrap();
    let first = plan
        .find_declaration(NonZeroU64::new(20).unwrap(), n())
        .unwrap()
        .unwrap();
    let repeated = plan
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
    assert_eq!(repeated.identity.get(), 22);
    assert_eq!(repeated.parsed.property.descriptor().name, "margin");
    assert_eq!(&child_css[repeated.location.value.clone()], "1px");
    assert!(std::ptr::eq(first.parsed, repeated.parsed));
    assert!(
        plan.find_declaration(NonZeroU64::new(24).unwrap(), n())
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        plan.find_declaration(NonZeroU64::new(22).unwrap(), NonZeroUsize::MIN),
        Err(ProvenanceError::Work)
    ));
}

#[test]
fn compile_policy_rejects_unavailable_applicable_imports_after_earlier_success() {
    let root = parse("@import \"a\"; @import \"missing\"; r{color:red}");
    let a = parse("a{width:1px}");
    let links = [
        ImportBinding {
            entry: 0,
            resolution: ImportResolution::Sheet(1),
        },
        ImportBinding {
            entry: 1,
            resolution: ImportResolution::Unavailable,
        },
    ];
    let loaded = [
        LoadedSheet {
            resource: id(1),
            sheet: &root,
            imports: &links,
        },
        LoadedSheet {
            resource: id(2),
            sheet: &a,
            imports: &[],
        },
    ];
    let mut limits = graph_limits();
    assert_eq!(
        expand_loaded_imports(&loaded, 0, context(), limits)
            .unwrap()
            .rules
            .len(),
        2
    );
    limits.unavailable = UnavailableImportPolicy::Fail;
    assert!(matches!(
        expand_loaded_imports(&loaded, 0, context(), limits),
        Err(LoadedGraphError::UnavailableImport { sheet: 0, entry: 1 })
    ));
    let root = parse("@import \"missing\" print; r{color:red}");
    let loaded = [LoadedSheet {
        resource: id(1),
        sheet: &root,
        imports: &[],
    }];
    assert_eq!(
        expand_loaded_imports(&loaded, 0, context(), limits)
            .unwrap()
            .rules
            .len(),
        1
    );
}

#[test]
fn separate_cycle_identity_cuts_alias_contexts_but_preserves_later_occurrences() {
    let root = parse("@import 'a'; @import 'other-context'; r{color:red}");
    let first = parse("@import 'other-context'; a{width:1px}");
    let second = parse("b{height:2px}");
    let root_links = [
        ImportBinding {
            entry: 0,
            resolution: ImportResolution::Sheet(1),
        },
        ImportBinding {
            entry: 1,
            resolution: ImportResolution::Sheet(2),
        },
    ];
    let child_links = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::Sheet(2),
    }];
    let loaded = [
        LoadedSheet {
            resource: id(1),
            sheet: &root,
            imports: &root_links,
        },
        LoadedSheet {
            resource: id(2),
            sheet: &first,
            imports: &child_links,
        },
        LoadedSheet {
            resource: id(3),
            sheet: &second,
            imports: &[],
        },
    ];
    let cycles = [
        ImportCycleId::new(NonZeroU64::new(10).unwrap()),
        ImportCycleId::new(NonZeroU64::new(20).unwrap()),
        ImportCycleId::new(NonZeroU64::new(20).unwrap()),
    ];
    let plan =
        expand_loaded_imports_with_cycles(&loaded, &cycles, 0, context(), graph_limits()).unwrap();
    assert_eq!(
        plan.rules
            .iter()
            .map(|r| (
                r.source.get(),
                r.occurrence.get(),
                r.context.first_order,
                r.context.first_declaration.get()
            ))
            .collect::<Vec<_>>(),
        [(2, 2, 10, 20), (3, 3, 11, 21), (1, 1, 12, 22)]
    );
    assert_eq!(plan.cuts.len(), 1);
    assert_eq!(plan.cuts[0].reason, ImportCutReason::Cycle);
    assert_eq!(plan.cuts[0].source, id(2));
    assert_eq!(plan.cuts[0].target, Some(id(3)));
    assert_eq!(plan.visited_sheets, [0, 1, 2]);
    assert_eq!(plan.occurrences, 3);
    let mut bounded = graph_limits();
    bounded.work = NonZeroUsize::MIN;
    assert!(matches!(
        expand_loaded_imports_with_cycles(&loaded, &cycles, 0, context(), bounded),
        Err(LoadedGraphError::Budget(RuleAssemblyError::Work))
    ));
    let legacy = expand_loaded_imports(&loaded, 0, context(), graph_limits()).unwrap();
    assert_eq!(legacy.rules.len(), 4);
    assert!(legacy.cuts.is_empty());
    let extra = [cycles[0], cycles[1], cycles[2], cycles[0]];
    for wrong in [&cycles[..0], &cycles[..2], &extra[..]] {
        assert!(matches!(
            expand_loaded_imports_with_cycles(&loaded, wrong, 0, context(), graph_limits()),
            Err(LoadedGraphError::CycleIdentities)
        ));
    }
}

#[test]
fn deferred_cycle_can_cut_without_a_loaded_target_but_requires_explicit_identity() {
    let root = parse("@import 'self'; r{color:red}");
    let key = ImportCycleId::new(NonZeroU64::new(50).unwrap());
    let links = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::DeferredCycle(key),
    }];
    let loaded = [LoadedSheet {
        resource: id(1),
        sheet: &root,
        imports: &links,
    }];
    for unavailable in [
        UnavailableImportPolicy::Record,
        UnavailableImportPolicy::Fail,
    ] {
        let mut limits = graph_limits();
        limits.unavailable = unavailable;
        let plan =
            expand_loaded_imports_with_cycles(&loaded, &[key], 0, context(), limits).unwrap();
        assert_eq!(plan.rules.len(), 1);
        assert_eq!(plan.occurrences, 1);
        assert_eq!(plan.visited_sheets, [0]);
        assert_eq!(plan.cuts.len(), 1);
        assert_eq!(plan.cuts[0].reason, ImportCutReason::Cycle);
        assert_eq!(plan.cuts[0].target, None);
        assert!(matches!(
            expand_loaded_imports_with_cycles(
                &loaded,
                &[ImportCycleId::new(NonZeroU64::new(51).unwrap())],
                0,
                context(),
                limits
            ),
            Err(LoadedGraphError::UnresolvedImport { sheet: 0, entry: 0 })
        ));
    }
    assert!(matches!(
        expand_loaded_imports(&loaded, 0, context(), graph_limits()),
        Err(LoadedGraphError::CycleIdentities)
    ));
    let inactive = parse("@import 'self' print; r{color:red}");
    let loaded = [LoadedSheet {
        resource: id(1),
        sheet: &inactive,
        imports: &links,
    }];
    let plan = expand_loaded_imports_with_cycles(
        &loaded,
        &[ImportCycleId::new(NonZeroU64::new(51).unwrap())],
        0,
        context(),
        graph_limits(),
    )
    .unwrap();
    assert_eq!(plan.cuts[0].reason, ImportCutReason::MediaExcluded);
}

#[test]
fn a_deferred_edge_cannot_hide_a_later_noncyclic_import_occurrence() {
    let root = parse("@import 'a'; @import 'c'; r{color:red}");
    let a = parse("@import 'c'; a{width:1px}");
    let c = parse("@import 'a'; c{height:2px}");
    let cycles = [1, 2, 3].map(|value| ImportCycleId::new(NonZeroU64::new(value).unwrap()));
    let root_links = [
        ImportBinding {
            entry: 0,
            resolution: ImportResolution::Sheet(1),
        },
        ImportBinding {
            entry: 1,
            resolution: ImportResolution::Sheet(2),
        },
    ];
    let a_links = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::Sheet(2),
    }];
    let deferred = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::DeferredCycle(cycles[1]),
    }];
    let mut loaded = [
        LoadedSheet {
            resource: id(10),
            sheet: &root,
            imports: &root_links,
        },
        LoadedSheet {
            resource: id(20),
            sheet: &a,
            imports: &a_links,
        },
        LoadedSheet {
            resource: id(30),
            sheet: &c,
            imports: &deferred,
        },
    ];
    assert!(matches!(
        expand_loaded_imports_with_cycles(&loaded, &cycles, 0, context(), graph_limits()),
        Err(LoadedGraphError::UnresolvedImport { sheet: 2, entry: 0 })
    ));
    let resolved = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::Sheet(1),
    }];
    loaded[2].imports = &resolved;
    let plan =
        expand_loaded_imports_with_cycles(&loaded, &cycles, 0, context(), graph_limits()).unwrap();
    assert_eq!(
        plan.rules
            .iter()
            .map(|r| r.source.get())
            .collect::<Vec<_>>(),
        [30, 20, 20, 30, 10]
    );
    assert_eq!(plan.occurrences, 5);
    assert_eq!(plan.cuts.len(), 2);
    assert!(
        plan.cuts
            .iter()
            .all(|cut| cut.reason == ImportCutReason::Cycle)
    );
}

#[test]
fn unavailable_targets_remain_cyclic_only_in_their_active_occurrences() {
    let root = parse("@import 'a'; @import 'c'; r{color:red}");
    let a = parse("@import 'c'; a{width:1px}");
    let c = parse("@import 'a'; c{height:2px}");
    let cycles = [1, 2, 3].map(|value| ImportCycleId::new(NonZeroU64::new(value).unwrap()));
    let root_links = [
        ImportBinding {
            entry: 0,
            resolution: ImportResolution::Sheet(1),
        },
        ImportBinding {
            entry: 1,
            resolution: ImportResolution::Sheet(2),
        },
    ];
    let a_links = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::Sheet(2),
    }];
    let unavailable = [ImportBinding {
        entry: 0,
        resolution: ImportResolution::UnavailableWithCycle(cycles[1]),
    }];
    let loaded = [
        LoadedSheet {
            resource: id(10),
            sheet: &root,
            imports: &root_links,
        },
        LoadedSheet {
            resource: id(20),
            sheet: &a,
            imports: &a_links,
        },
        LoadedSheet {
            resource: id(30),
            sheet: &c,
            imports: &unavailable,
        },
    ];
    let mut limits = graph_limits();
    limits.tracking.occurrences = NonZeroU64::new(4).unwrap();
    let plan = expand_loaded_imports_with_cycles(&loaded, &cycles, 0, context(), limits).unwrap();
    assert_eq!(
        plan.rules
            .iter()
            .map(|r| r.source.get())
            .collect::<Vec<_>>(),
        [30, 20, 30, 10]
    );
    assert_eq!(plan.occurrences, 4);
    assert_eq!(
        plan.cuts.iter().map(|cut| cut.reason).collect::<Vec<_>>(),
        [ImportCutReason::Cycle, ImportCutReason::Unavailable]
    );
    limits.unavailable = UnavailableImportPolicy::Fail;
    assert!(matches!(
        expand_loaded_imports_with_cycles(&loaded, &cycles, 0, context(), limits),
        Err(LoadedGraphError::UnavailableImport { sheet: 2, entry: 0 })
    ));
    assert!(matches!(
        expand_loaded_imports(&loaded, 0, context(), limits),
        Err(LoadedGraphError::CycleIdentities)
    ));
}
