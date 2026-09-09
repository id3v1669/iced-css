use iced_css::evaluation::display_rules::{
    BundleLimits, DisplayRuleBundle, GraphBinding, GraphOrigin,
};
use iced_css_core::{
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
fn bundle_limits() -> BundleLimits {
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

use iced_css::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement, Construction, adapters, source::ConstructionState},
    evaluation::source_display::{SourceDisplayLimits, evaluate_source_display},
    host::source_publication::{InitialSourceHost, PublicationLimits},
};
use iced_css_core::{
    cascade::rule::{RuleBatchLimits, RuleCascadeLimits},
    formatting::display::{Display, Flow},
    identity::SourceChangeLimits,
    selectors::MatchLimits,
    source::{AttributeLimits, selector_tree::SelectorTreeLimits},
};
fn capture_limits() -> CaptureLimits {
    CaptureLimits {
        nodes: nz(2000),
        depth: nz(2000),
        retained_bytes: nz(20_000_000),
        work: nz(20_000_000),
    }
}
fn attrs() -> AttributeLimits {
    AttributeLimits {
        entries: bound(),
        retained_bytes: bound(),
        work: bound(),
    }
}
fn text() -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::text("text", capture_limits()).unwrap().erase()
}
fn named(name: &str) -> CapturedElement<'static, (), iced::Theme, ()> {
    text().style_id(name, attrs()).unwrap()
}
fn row(children: Vec<CapturedElement<'static, (), iced::Theme, ()>>) -> Construction {
    adapters::row_from_vec(children, capture_limits())
        .unwrap()
        .into_parts()
        .1
}
fn selector_limits() -> SelectorTreeLimits {
    SelectorTreeLimits {
        entries: bound(),
        retained_bytes: nz(20_000_000),
        work: nz(20_000_000),
    }
}
fn host(
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
fn limits() -> SourceDisplayLimits {
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

use iced_css::evaluation::source_eligibility::*;
fn eligibility_limits() -> SourceEligibilityLimits {
    SourceEligibilityLimits {
        nodes: bound(),
        retained_bytes: nz(20_000_000),
        work: nz(20_000_000),
    }
}
fn wrapped(name: &str) -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::container(named(name), capture_limits())
        .unwrap()
        .erase()
}
