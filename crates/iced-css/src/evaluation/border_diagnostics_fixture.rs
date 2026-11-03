//! Small private real-graph fixture for collector allocation tests only.
use super::*;
use crate::evaluation::{
    clearance_box_facts::{
        BoxFactMode, ClearanceBoxFactsLimits, evaluate_box_rows_observed, prepare_box_facts,
    },
    display_rules::{BundleLimits, DisplayRuleBundle, GraphBinding, GraphOrigin},
    sibling_box_facts::{SiblingBoxFactsLimits, evaluate_sibling_box_facts},
    sibling_source_projection::{SiblingProjectionLimits, prepare_sibling_source_projection},
    source_display::{SourceDisplayLimits, evaluate_source_display},
};
use iced_css_core::{
    cascade::{
        rule::{RuleBatchLimits, RuleCascadeLimits, RuleContext},
        sheets::{AuthorSheets, SheetLimits},
        ua_display::{UaDisplayCatalog, UaDisplayLimits},
    },
    selectors::{MatchLimits, SelectorLimits},
    stylesheet::{
        RuleAssemblyLimits, StylesheetIntakeLimits, StylesheetLimits,
        imports::{LoadedGraphLimits, UnavailableImportPolicy},
        owned_graph::{OwnedGraphLimits, OwnedLoadedSheet},
        parse_separate_unicode_stylesheet,
    },
    syntax::{
        RecoveryLimits, StringLimits,
        decode::DecodeLimits,
        encoding_selection::{EncodingMetadata, SelectionLimits},
        imports::ImportLimits,
        unicode_intake::IntakeLimits,
    },
    values::{NumericLimits, color::ColorLimits},
};

fn bound() -> NonZeroUsize {
    nz(1_000_000)
}
fn numeric() -> NumericLimits {
    NumericLimits {
        digits: bound(),
        work: bound(),
    }
}
fn colors() -> ColorLimits {
    ColorLimits {
        syntax: StringLimits {
            work: bound(),
            retained_bytes: bound(),
        },
        numeric: numeric(),
    }
}
fn selectors() -> SelectorTreeLimits {
    SelectorTreeLimits {
        entries: bound(),
        retained_bytes: bound(),
        work: bound(),
    }
}
fn per_target() -> RuleBatchLimits {
    RuleBatchLimits {
        rules: bound(),
        per_rule: RuleCascadeLimits {
            declarations: bound(),
            matching: MatchLimits {
                work: nz(1000),
                retained_bytes: bound(),
            },
        },
    }
}
fn facts_limits() -> SiblingBoxFactsLimits {
    SiblingBoxFactsLimits {
        sources: bound(),
        retained_bytes: bound(),
        evaluation: ClearanceBoxFactsLimits {
            work: bound(),
            rule_visits: bound(),
            matching_work: nz(2_000_000_000),
            declaration_offers: bound(),
            membership_work: bound(),
            // Five Sequence rows prepay 22 conversions each, at numeric().work.
            numeric_work: nz(110_000_000),
            selectors: selectors(),
            per_target: per_target(),
            numeric: numeric(),
        },
    }
}

fn graph() -> OwnedStylesheetGraph {
    let css = "* { border-top-style:solid; border-top-width:1000000000000000000000000000000000000000px; }";
    let candidate = parse_separate_unicode_stylesheet(
        css.as_bytes(),
        EncodingMetadata {
            transport: None,
            linking: None,
            referring: None,
        },
        RuleContext {
            first_order: 0,
            first_declaration: NonZeroU64::MIN,
        },
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
                    values: colors(),
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
    .unwrap();
    OwnedStylesheetGraph::new(
        vec![OwnedLoadedSheet {
            resource: ResourceId::new(NonZeroU64::MIN),
            candidate,
            imports: Vec::new(),
        }],
        0,
        RuleContext {
            first_order: 100,
            first_declaration: NonZeroU64::new(100).unwrap(),
        },
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

pub(super) fn with_graph<T>(
    source: &InitialSourceHost,
    run: impl FnOnce(
        &SiblingSourceProjection<'_>,
        &SourceDisplay<'_, '_, '_>,
        &[BorderDeclarationBinding<'_>],
    ) -> T,
) -> T {
    let snapshot = source.snapshot().unwrap();
    let projection = prepare_sibling_source_projection(
        source,
        snapshot,
        SiblingProjectionLimits {
            sources: bound(),
            retained_bytes: bound(),
            work: bound(),
        },
    )
    .unwrap();
    let graph = graph();
    let ua = UaDisplayCatalog::new(UaDisplayLimits {
        rules: bound(),
        retained_bytes: bound(),
        work: bound(),
        values: colors(),
    })
    .unwrap();
    let sheets = AuthorSheets::new(
        Vec::new(),
        Vec::new(),
        true,
        None,
        SheetLimits {
            entries: bound(),
            retained_bytes: bound(),
            work: bound(),
        },
    )
    .unwrap();
    let bindings = [GraphBinding {
        graph: &graph,
        origin: GraphOrigin::User,
    }];
    let bundle = DisplayRuleBundle::new(
        &ua,
        &bindings,
        &sheets,
        BundleLimits {
            graphs: bound(),
            rules: bound(),
            retained_bytes: bound(),
            work: bound(),
            membership_work: bound(),
        },
    )
    .unwrap();
    let display = evaluate_source_display(
        snapshot,
        &bundle,
        SourceDisplayLimits {
            nodes: bound(),
            retained_bytes: bound(),
            work: bound(),
            rule_visits: bound(),
            matching_work: nz(2_000_000_000),
            declaration_offers: bound(),
            membership_work: bound(),
            selectors: selectors(),
            per_target: per_target(),
        },
    )
    .unwrap();
    let provenance = Arc::new(Provenance {
        logical_name: "collector-allocation.css".into(),
        revision: NonZeroU64::MIN,
    });
    let mut mappings = Vec::new();
    for rule in graph.rules() {
        for ordinal in 0..rule.rule.declarations.len() {
            let id = rule
                .context
                .first_declaration
                .get()
                .checked_add(u64::try_from(ordinal).unwrap())
                .unwrap();
            mappings.push(BorderDeclarationBinding {
                graph: &graph,
                resource: rule.source,
                occurrence: rule.occurrence,
                declaration: NonZeroU64::new(id).unwrap(),
                logical_key: NonZeroU64::new(id).unwrap(),
                provenance: Arc::clone(&provenance),
            });
        }
    }
    assert_eq!(mappings.len(), 2);
    run(&projection, &display, &mappings)
}

pub(super) fn collect<R: Reservation>(
    projection: &SiblingSourceProjection<'_>,
    display: &SourceDisplay<'_, '_, '_>,
    mappings: &[BorderDeclarationBinding<'_>],
    reporter: &BorderReporter<'_>,
    limits: BorderCaptureLimits,
    policy: R,
) -> Result<BorderObservationBatch> {
    let ordinary = evaluate_sibling_box_facts(projection, display, facts_limits()).unwrap();
    let mut prepared = prepare_box_facts(
        projection,
        display,
        facts_limits().evaluation,
        BoxFactMode::Sequence,
        None,
    )
    .unwrap();
    let mut rows = vec![initial_box_fact(); projection.nodes().len()];
    let mut collector = Collector::with_reservation(rows.len(), reporter, limits, policy);
    evaluate_box_rows_observed(
        projection,
        display,
        facts_limits().evaluation,
        &mut prepared,
        &mut rows,
        &mut collector,
    )
    .unwrap();
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(
            Some(row),
            ordinary.row(index),
            "ordinary facts changed at row {index}"
        );
        assert_eq!(row.border_width[0].event, Some(RangeEvent::UpperSaturation));
        assert_eq!(row.border_width[0].pixels.to_bits(), f32::MAX.to_bits());
    }
    collector.finish(projection, display, reporter, mappings)
}
