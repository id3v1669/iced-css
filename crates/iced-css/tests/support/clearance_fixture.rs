#![allow(dead_code, unused_imports)]
pub use iced_css::evaluation::display_rules::{
    BundleLimits, DisplayRuleBundle, GraphBinding, GraphOrigin,
};
pub use iced_css_core::{
    cascade::{
        rule::RuleContext,
        sheets::{AuthorSheet, AuthorSheets, Membership, SheetId, SheetLimits},
        ua_display::{UaDisplayCatalog, UaDisplayLimits},
    },
    selectors::SelectorLimits,
    stylesheet::{
        RuleAssemblyLimits, StylesheetIntakeLimits, StylesheetLimits,
        imports::{ImportBinding, LoadedGraphLimits, UnavailableImportPolicy},
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
pub use std::num::{NonZeroU64, NonZeroUsize};

pub fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
pub fn bound() -> NonZeroUsize {
    nz(100_000)
}
pub fn context(order: u64, id: u64) -> RuleContext {
    RuleContext {
        first_order: order,
        first_declaration: NonZeroU64::new(id).unwrap(),
    }
}
pub fn sheet(value: u64) -> SheetId {
    SheetId::new(NonZeroU64::new(value).unwrap())
}
pub fn values() -> ColorLimits {
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
pub fn catalog() -> UaDisplayCatalog {
    UaDisplayCatalog::new(UaDisplayLimits {
        rules: bound(),
        retained_bytes: bound(),
        work: bound(),
        values: values(),
    })
    .unwrap()
}
pub fn controls(ids: &[u64], enabled: bool, work: usize) -> AuthorSheets {
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
pub fn bundle_limits() -> BundleLimits {
    BundleLimits {
        graphs: bound(),
        rules: bound(),
        retained_bytes: bound(),
        work: bound(),
        membership_work: bound(),
    }
}
pub fn resource(id: u64, css: &str, imports: Vec<ImportBinding>) -> OwnedLoadedSheet {
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
pub fn loaded(resources: Vec<OwnedLoadedSheet>, start: RuleContext) -> OwnedStylesheetGraph {
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
pub fn graph(css: &str, start: RuleContext) -> OwnedStylesheetGraph {
    loaded(vec![resource(1, css, vec![])], start)
}
pub fn binding(graph: &OwnedStylesheetGraph, origin: GraphOrigin) -> GraphBinding<'_> {
    GraphBinding { graph, origin }
}

pub use iced_css::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement, Construction, adapters, source::ConstructionState},
    evaluation::source_display::{SourceDisplayLimits, evaluate_source_display},
    host::source_publication::{InitialSourceHost, PublicationLimits},
};
pub use iced_css_core::{
    cascade::rule::{RuleBatchLimits, RuleCascadeLimits},
    identity::SourceChangeLimits,
    selectors::MatchLimits,
    source::{AttributeLimits, selector_tree::SelectorTreeLimits},
};
pub fn capture_limits() -> CaptureLimits {
    CaptureLimits {
        nodes: nz(2000),
        depth: nz(2000),
        retained_bytes: nz(20_000_000),
        work: nz(20_000_000),
    }
}
pub fn attrs() -> AttributeLimits {
    AttributeLimits {
        entries: bound(),
        retained_bytes: bound(),
        work: bound(),
    }
}
pub fn selector_limits() -> SelectorTreeLimits {
    SelectorTreeLimits {
        entries: bound(),
        retained_bytes: nz(20_000_000),
        work: nz(20_000_000),
    }
}
pub fn host(
    construction: Construction,
    explicit: Option<Vec<ConstructionState>>,
    language: Option<&str>,
) -> InitialSourceHost {
    let count = construction.nodes().len();
    let mut host = InitialSourceHost::new(
        NonZeroU64::new(10000).unwrap(),
        ProgressLimits {
            addresses: NonZeroU64::new(10000).unwrap(),
            bindings: bound(),
            tracks: bound(),
            retained_bytes: bound(),
            work: bound(),
        },
    );
    host.prepare(
        construction,
        explicit.unwrap_or_else(|| vec![ConstructionState::default(); count]),
        language.map(str::to_owned),
        PublicationLimits {
            nodes: bound(),
            snapshot_bytes: nz(20_000_000),
            work: nz(20_000_000),
            selectors: selector_limits(),
            identities: SourceChangeLimits {
                entries: bound(),
                retained_bytes: nz(20_000_000),
                work: nz(20_000_000),
            },
        },
    )
    .unwrap()
    .commit();
    host
}
pub fn display_limits() -> SourceDisplayLimits {
    SourceDisplayLimits {
        nodes: bound(),
        retained_bytes: nz(20_000_000),
        work: nz(20_000_000),
        rule_visits: nz(20_000_000),
        matching_work: nz(2_000_000_000),
        declaration_offers: nz(20_000_000),
        membership_work: nz(20_000_000),
        selectors: selector_limits(),
        per_target: RuleBatchLimits {
            rules: bound(),
            per_rule: RuleCascadeLimits {
                declarations: bound(),
                matching: MatchLimits {
                    work: nz(1000),
                    retained_bytes: bound(),
                },
            },
        },
    }
}

pub use iced_css::evaluation::{clearance_box_facts::*, clearance_source_projection::*};
pub use iced_css_core::formatting::{clearance::ClearSide, table_borders::BorderStyle};
pub fn captured() -> InitialSourceHost {
    captured_shape(false)
}
pub fn captured_shape(nested: bool) -> InitialSourceHost {
    type Element = CapturedElement<'static, (), iced::Theme, ()>;
    let children = ["b1", "f", "b2"].map(|id| {
        adapters::row_from_vec(Vec::<Element>::new(), capture_limits())
            .unwrap()
            .style_id(id, attrs())
            .unwrap()
            .erase()
    });
    let inner = adapters::column_from_vec(Vec::from(children), capture_limits())
        .unwrap()
        .style_id("p", attrs())
        .unwrap()
        .erase();
    let element = if nested {
        adapters::column_from_vec(vec![inner], capture_limits())
            .unwrap()
            .style_id("o", attrs())
            .unwrap()
            .erase()
    } else {
        inner
    };
    let (_, construction) = element.into_parts();
    host(construction, None, None)
}
pub fn fact_limits() -> ClearanceBoxFactsLimits {
    let d = display_limits();
    ClearanceBoxFactsLimits {
        work: bound(),
        rule_visits: bound(),
        matching_work: nz(2_000_000_000),
        declaration_offers: bound(),
        membership_work: nz(20_000_000),
        numeric_work: nz(20_000_000),
        selectors: d.selectors,
        per_target: d.per_target,
        numeric: values().numeric,
    }
}
pub fn fixture(
    css: &str,
    check: impl FnOnce(
        &ClearanceSourceProjection<'_>,
        &iced_css::evaluation::source_display::SourceDisplay<'_, '_, '_>,
    ),
) {
    let host = captured();
    let snapshot = host.snapshot().unwrap();
    let projection = prepare_clearance_source_projection(
        &host,
        snapshot,
        ClearanceProjectionLimits {
            work: nz(27),
            retained_bytes: bound(),
        },
    )
    .unwrap();
    let ua = catalog();
    let graph = graph(css, context(100, 100));
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(snapshot, &bundle, display_limits()).unwrap();
    check(&projection, &display);
}
