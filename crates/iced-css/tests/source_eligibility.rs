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
    formatting::display::{Display, Float, Flow, Position},
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
#[test]
fn branching_preorder_keeps_original_indices_and_exact_owner() {
    let owner = host(row(vec![wrapped("left"), wrapped("right")]), None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    let result = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    assert!(std::ptr::eq(result.display(), &display));
    assert!(std::ptr::eq(
        result.display().snapshot(),
        owner.snapshot().unwrap()
    ));
    assert_eq!(
        result
            .ordered()
            .iter()
            .map(|e| e.source)
            .collect::<Vec<_>>(),
        [4, 1, 0, 3, 2]
    );
    assert_eq!(
        result
            .ordered()
            .iter()
            .map(|e| e.parent)
            .collect::<Vec<_>>(),
        [None, Some(0), Some(1), Some(0), Some(3)]
    );
    for (source, ordinal) in [Some(2), Some(1), Some(4), Some(3), Some(0)]
        .into_iter()
        .enumerate()
    {
        assert_eq!(result.ordinal(source), ordinal);
        assert_eq!(result.disposition(source), Some(Eligibility::Eligible));
        assert!(std::ptr::eq(
            result.token(source).unwrap(),
            display.token(source).unwrap()
        ));
    }
    assert_eq!(result.work(), 36);
}

fn check_limits_and_owners(
    display: &iced_css::evaluation::source_display::SourceDisplay<'_, '_, '_>,
) {
    let before: Vec<_> = (0..display.node_count())
        .map(|i| {
            (
                *display.resolution(i).unwrap(),
                display.token(i).unwrap() as *const _,
            )
        })
        .collect();
    let baseline = evaluate_source_eligibility(display, eligibility_limits()).unwrap();
    let n = baseline.node_count();
    let k = baseline.eligible_count();
    assert_eq!(baseline.work(), 1 + 6 * n + k);
    assert!(baseline.peak_retained_bytes() > baseline.retained_bytes());
    for i in [n, usize::MAX] {
        assert_eq!(baseline.disposition(i), None);
        assert_eq!(baseline.ordinal(i), None);
        assert!(baseline.token(i).is_none());
    }
    for i in [k, usize::MAX] {
        assert!(baseline.entry(i).is_none());
    }
    for (ordinal, entry) in baseline.ordered().iter().enumerate() {
        assert_eq!(baseline.entry(ordinal), Some(entry));
        assert_eq!(baseline.ordinal(entry.source), Some(ordinal));
    }
    let exact = SourceEligibilityLimits {
        nodes: nz(n),
        retained_bytes: nz(baseline.peak_retained_bytes()),
        work: nz(baseline.work()),
    };
    assert!(evaluate_source_eligibility(display, exact).is_ok());
    let mut cap = exact;
    cap.work = nz(baseline.work() - 1);
    assert!(matches!(
        evaluate_source_eligibility(display, cap),
        Err(SourceEligibilityError::Work)
    ));
    cap = exact;
    cap.retained_bytes = nz(baseline.peak_retained_bytes() - 1);
    assert!(matches!(
        evaluate_source_eligibility(display, cap),
        Err(SourceEligibilityError::Bytes)
    ));
    if n > 1 {
        cap = exact;
        cap.nodes = nz(n - 1);
        assert!(matches!(
            evaluate_source_eligibility(display, cap),
            Err(SourceEligibilityError::Nodes)
        ));
    }
    cap = exact;
    cap.work = nz(1);
    assert!(matches!(
        evaluate_source_eligibility(display, cap),
        Err(SourceEligibilityError::Work)
    ));
    cap = exact;
    cap.retained_bytes = NonZeroUsize::MAX;
    assert!(evaluate_source_eligibility(display, cap).is_ok());
    for (i, (resolution, token)) in before.into_iter().enumerate() {
        assert_eq!(*display.resolution(i).unwrap(), resolution);
        assert_eq!(display.token(i).unwrap() as *const _, token);
        assert!(std::ptr::eq(
            baseline.token(i).unwrap(),
            display.token(i).unwrap()
        ));
    }
}

#[test]
fn middle_subtree_exclusion_retains_explicit_computed_descendant() {
    let middle = wrapped("explicit").style_id("middle", attrs()).unwrap();
    let owner = host(
        row(vec![wrapped("left"), middle, wrapped("right")]),
        None,
        None,
    );
    let ua = catalog();
    let graph = graph(
        "#middle{display:none} #explicit{display:block}",
        ua.next_context(),
    );
    let sheets = controls(&[], true, 100);
    let bindings = [binding(&graph, GraphOrigin::User)];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    let result = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    assert_eq!(
        result.ordered(),
        [
            EligibleSource {
                source: 6,
                parent: None
            },
            EligibleSource {
                source: 1,
                parent: Some(0)
            },
            EligibleSource {
                source: 0,
                parent: Some(1)
            },
            EligibleSource {
                source: 5,
                parent: Some(0)
            },
            EligibleSource {
                source: 4,
                parent: Some(3)
            }
        ]
    );
    assert_eq!(result.disposition(3), Some(Eligibility::DisplayNone));
    assert_eq!(
        result.disposition(2),
        Some(Eligibility::Ancestor { index: 3 })
    );
    assert_eq!(result.ordinal(2), None);
    assert_eq!(result.ordinal(3), None);
    assert_eq!(display.resolution(2).unwrap().display, Display::Block);
    assert_eq!(result.work(), 48);
    check_limits_and_owners(&display);
}

#[test]
fn root_none_keeps_all_computed_values_and_first_ancestor_diagnostics() {
    let nested = wrapped("inherit").style_id("nested", attrs()).unwrap();
    let owner = host(
        row(vec![
            named("explicit"),
            named("absolute"),
            named("float"),
            nested,
        ]),
        None,
        None,
    );
    let ua = catalog();
    let graph = graph(
        "row,#nested{display:none} #inherit{display:inherit} #explicit{display:block} #absolute{display:block;position:absolute} #float{display:block;float:left}",
        ua.next_context(),
    );
    let sheets = controls(&[], true, 100);
    let bindings = [binding(&graph, GraphOrigin::User)];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    let result = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    assert!(result.ordered().is_empty());
    assert_eq!(result.eligible_count(), 0);
    assert_eq!(result.work(), 37);
    assert_eq!(result.disposition(5), Some(Eligibility::DisplayNone));
    for i in 0..5 {
        assert_eq!(
            result.disposition(i),
            Some(Eligibility::Ancestor { index: 5 })
        );
        assert_eq!(result.ordinal(i), None);
    }
    assert_eq!(display.resolution(0).unwrap().display, Display::Block);
    assert_eq!(
        display.resolution(1).unwrap().flow,
        Flow::AbsolutelyPositioned
    );
    assert_eq!(display.resolution(2).unwrap().flow, Flow::Floating);
    assert_eq!(display.resolution(3).unwrap().display, Display::None);
    assert_eq!(display.resolution(4).unwrap().display, Display::None);
    check_limits_and_owners(&display);
}

#[test]
fn eligible_flow_and_text_payloads_are_preserved_without_box_claims() {
    let empty = adapters::text("", capture_limits()).unwrap().erase();
    let space = adapters::text(" ", capture_limits()).unwrap().erase();
    let spans: Vec<iced::widget::text::Span<'static, (), iced::Font>> = vec![
        iced::widget::text::Span::new(""),
        iced::widget::text::Span::new(" "),
    ];
    let rich = adapters::rich_text_vec::<_, (), iced::Theme, ()>(spans, capture_limits())
        .unwrap()
        .erase();
    let owner = host(
        row(vec![
            named("float"),
            named("absolute"),
            named("fixed"),
            empty,
            space,
            rich,
        ]),
        None,
        None,
    );
    let ua = catalog();
    let graph = graph(
        "row{display:inline-table} #float{float:right} #absolute{position:absolute} #fixed{position:fixed}",
        ua.next_context(),
    );
    let sheets = controls(&[], true, 100);
    let bindings = [binding(&graph, GraphOrigin::User)];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    let result = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
    assert_eq!(
        result
            .ordered()
            .iter()
            .map(|e| e.source)
            .collect::<Vec<_>>(),
        [6, 0, 1, 2, 3, 4, 5]
    );
    for i in 0..7 {
        assert_eq!(result.disposition(i), Some(Eligibility::Eligible));
    }
    let root = display.resolution(6).unwrap();
    assert_eq!(
        (root.display, root.position, root.float),
        (Display::Table, Position::Static, Float::None)
    );
    assert_eq!(display.resolution(0).unwrap().flow, Flow::Floating);
    assert_eq!(
        display.resolution(1).unwrap().flow,
        Flow::AbsolutelyPositioned
    );
    assert_eq!(display.resolution(2).unwrap().position, Position::Fixed);
    let nodes = result.display().snapshot().construction().nodes();
    assert_eq!(nodes[3].text(), Some(""));
    assert_eq!(nodes[4].text(), Some(" "));
    assert_eq!(nodes[5].text_runs().unwrap(), [0..0, 0..1]);
    let originals = owner.snapshot().unwrap().construction().nodes();
    assert!(std::ptr::eq(
        nodes[5].text_runs().unwrap(),
        originals[5].text_runs().unwrap()
    ));
    assert!(std::ptr::eq(
        nodes[4].text().unwrap(),
        originals[4].text().unwrap()
    ));
    check_limits_and_owners(&display);
}

#[test]
fn deep_chain_and_single_root_have_linear_work_and_actual_parent_ordinals() {
    for depth in [0, 512] {
        let mut captured = text();
        for _ in 0..depth {
            captured = adapters::container(captured, capture_limits())
                .unwrap()
                .erase();
        }
        let owner = host(captured.into_parts().1, None, None);
        let ua = catalog();
        let sheets = controls(&[], true, 100);
        let bindings = [];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let display =
            evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
        let result = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
        assert_eq!(result.eligible_count(), depth + 1);
        for (ordinal, entry) in result.ordered().iter().enumerate() {
            assert_eq!(entry.source, depth - ordinal);
            assert_eq!(entry.parent, ordinal.checked_sub(1));
        }
        assert_eq!(result.work(), 1 + 7 * (depth + 1));
        check_limits_and_owners(&display);
    }
}
