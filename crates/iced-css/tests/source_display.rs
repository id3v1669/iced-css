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
    animation::engine::{EngineError, ProgressLimits},
    capture::{
        CaptureLimits, CapturedElement, Construction, adapters,
        source::{ConstructionSourceError, ConstructionState},
    },
    evaluation::source_display::{
        SourceDisplayError, SourceDisplayLimits, evaluate_source_display,
    },
    host::source_publication::{InitialSourceHost, PublicationLimits},
};
use iced_css_core::{
    cascade::rule::{RuleBatchError, RuleBatchLimits, RuleCascadeError, RuleCascadeLimits},
    formatting::display::{Display, Float, Flow, Position},
    identity::{IdentityError, SourceChangeLimits},
    selectors::{MatchError, MatchLimits},
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

#[test]
fn ua_table_defaults_and_original_source_association() {
    let construction = Construction::table(
        1,
        1,
        vec![text().into_parts().1, text().into_parts().1],
        capture_limits(),
    )
    .unwrap();
    let owner = host(construction, None, None);
    let snapshot = owner.snapshot().unwrap();
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let result = evaluate_source_display(snapshot, &bundle, limits()).unwrap();
    assert!(std::ptr::eq(result.snapshot(), snapshot));
    assert!(std::ptr::eq(result.bundle(), &bundle));
    assert_eq!(result.node_count(), snapshot.tokens().len());
    for (index, node) in snapshot.construction().nodes().iter().enumerate() {
        let expected = match node.canonical_type() {
            "text" => Display::Inline,
            "table" => Display::Table,
            "table_header_group" => Display::TableHeaderGroup,
            "table_row_group" => Display::TableRowGroup,
            "table_row" => Display::TableRow,
            "table_header_cell" | "table_cell" => Display::TableCell,
            other => panic!("unexpected table source {other}"),
        };
        assert_eq!(result.resolution(index).unwrap().display, expected);
        let text = result.text_keywords(index).unwrap();
        assert_eq!(text.white_space().keyword().as_str(), "normal");
        assert_eq!(text.text_transform().keyword().as_str(), "none");
        assert!(std::ptr::eq(
            result.token(index).unwrap(),
            snapshot.token(index).unwrap()
        ));
    }
    for index in [result.node_count(), usize::MAX] {
        assert!(result.resolution(index).is_none());
        assert!(result.text_keywords(index).is_none());
        assert!(result.token(index).is_none());
    }
    let leaf = host(text().into_parts().1, None, None);
    assert_eq!(
        evaluate_source_display(leaf.snapshot().unwrap(), &bundle, limits())
            .unwrap()
            .resolution(0)
            .unwrap()
            .display,
        Display::Block
    );
}

#[test]
fn real_user_author_priority_and_disabled_author_controls() {
    let owner = host(row(vec![named("normal"), named("important")]), None, None);
    let ua = catalog();
    let user = graph(
        "#normal{display:none} #important{display:inline!important}",
        ua.next_context(),
    );
    let author = graph(
        "#normal{display:block} #important{display:none!important}",
        user.next_context().unwrap(),
    );
    for enabled in [true, false] {
        let sheets = controls(&[1], enabled, 100);
        let bindings = [
            binding(&user, GraphOrigin::User),
            binding(&author, GraphOrigin::Author(sheet(1))),
        ];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let result = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
        assert_eq!(
            result.resolution(0).unwrap().display,
            if enabled {
                Display::Block
            } else {
                Display::None
            }
        );
        assert_eq!(result.resolution(1).unwrap().display, Display::Inline);
    }
}

#[test]
fn inherit_uses_computed_source_parent_and_root_float_conversions() {
    let owner = host(row(vec![named("inherit"), named("floating")]), None, None);
    let ua = catalog();
    let graph = graph(
        "row{display:inline-table;position:absolute;float:left} #inherit{display:inherit;position:inherit;float:inherit} #floating{display:inline-table;float:right}",
        ua.next_context(),
    );
    let sheets = controls(&[], true, 100);
    let bindings = [binding(&graph, GraphOrigin::User)];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let result = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    for index in [0, 2] {
        let computed = result.resolution(index).unwrap();
        assert_eq!(
            (
                computed.display,
                computed.position,
                computed.float,
                computed.flow
            ),
            (
                Display::Table,
                Position::Absolute,
                Float::None,
                Flow::AbsolutelyPositioned
            )
        );
    }
    let floating = result.resolution(1).unwrap();
    assert_eq!(
        (floating.display, floating.float, floating.flow),
        (Display::Table, Float::Right, Flow::Floating)
    );
}

#[test]
fn suppressed_descendants_are_still_computed() {
    let nested = adapters::container(named("grandchild"), capture_limits())
        .unwrap()
        .erase()
        .style_id("inherit", attrs())
        .unwrap();
    let owner = host(row(vec![named("explicit"), nested]), None, None);
    let ua = catalog();
    let graph = graph(
        "row{display:none} #explicit,#grandchild{display:block} #inherit{display:inherit}",
        ua.next_context(),
    );
    let sheets = controls(&[], true, 100);
    let bindings = [binding(&graph, GraphOrigin::User)];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let result = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    for (index, display) in [Display::Block, Display::Block, Display::None, Display::None]
        .into_iter()
        .enumerate()
    {
        assert_eq!(result.resolution(index).unwrap().display, display);
    }
}

#[test]
fn siblings_language_and_derived_state_use_real_snapshot_view() {
    let first = named("first").style_attribute("lang", "", attrs()).unwrap();
    let second = named("second");
    let mut states = vec![ConstructionState::default(); 3];
    states[1].hovered = true;
    states[1].active = true;
    states[1].focused = true;
    let owner = host(row(vec![first, second]), Some(states), Some("fr-CA"));
    let ua = catalog();
    let graph = graph(
        "text:first-child{display:block} #first:lang(fr){display:none} #first + text:lang(fr):focus{display:inline-table} row:hover:active{position:relative}",
        ua.next_context(),
    );
    let sheets = controls(&[], true, 100);
    let bindings = [binding(&graph, GraphOrigin::User)];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let result = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    assert_eq!(result.resolution(0).unwrap().display, Display::Block);
    assert_eq!(result.resolution(1).unwrap().display, Display::InlineTable);
    assert_eq!(result.resolution(2).unwrap().position, Position::Relative);
    assert!(!result.snapshot().states()[2].hovered);
    assert!(!result.snapshot().states()[2].active);
}

#[test]
fn deep_chain_evaluation_is_iterative_and_preserves_computed_inheritance() {
    let mut captured = text();
    for _ in 0..512 {
        captured = adapters::container(captured, capture_limits())
            .unwrap()
            .erase();
    }
    let owner = host(captured.into_parts().1, None, None);
    let ua = catalog();
    let graph = graph("*{display:inherit}", ua.next_context());
    let sheets = controls(&[], true, 100);
    let bindings = [binding(&graph, GraphOrigin::User)];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let result = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    assert_eq!(result.node_count(), 513);
    for index in 0..513 {
        assert_eq!(result.resolution(index).unwrap().display, Display::Block);
    }
}

#[test]
fn two_host_tokens_remain_exact_owner_tokens() {
    let first = host(row(vec![text()]), None, None);
    let second = host(row(vec![text()]), None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    for (owner, other) in [(&first, &second), (&second, &first)] {
        let result = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
        for index in 0..result.node_count() {
            let token = result.token(index).unwrap();
            assert!(std::ptr::eq(
                token,
                owner.snapshot().unwrap().token(index).unwrap()
            ));
            assert!(owner.engine().instance(token).is_ok());
            assert_eq!(
                other.engine().instance(token),
                Err(EngineError::Identity(IdentityError::WrongHost))
            );
        }
    }
}

#[test]
fn late_match_failure_exposes_no_candidate_and_preserves_source() {
    let mut costly = text();
    for index in 0..100 {
        costly = costly
            .style_attribute(&format!("data-{index}"), "", attrs())
            .unwrap();
    }
    let owner = host(row(vec![costly, text()]), None, None);
    let ua = catalog();
    let graph = graph("[data-absent-long-name]{display:none}", ua.next_context());
    let sheets = controls(&[], true, 100);
    let bindings = [binding(&graph, GraphOrigin::User)];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let tiny = limits();
    let no_bindings = [];
    let ua_only = DisplayRuleBundle::new(&ua, &no_bindings, &sheets, bundle_limits()).unwrap();
    assert!(evaluate_source_display(owner.snapshot().unwrap(), &ua_only, tiny).is_ok());
    let error = evaluate_source_display(owner.snapshot().unwrap(), &bundle, tiny).err();
    assert_eq!(
        error,
        Some(SourceDisplayError::Cascade {
            index: 0,
            error: RuleBatchError::Rule(RuleCascadeError::Match(MatchError::Work))
        })
    );
    let mut enough = limits();
    enough.per_target.per_rule.matching.work = nz(5000);
    let result = evaluate_source_display(owner.snapshot().unwrap(), &bundle, enough).unwrap();
    assert_eq!(result.node_count(), 3);
    for index in 0..3 {
        assert!(
            owner
                .engine()
                .instance(result.token(index).unwrap())
                .is_ok()
        );
    }
    assert_eq!(
        result.snapshot().construction().nodes()[0]
            .metadata()
            .attribute_count(),
        100
    );
}

#[test]
fn exact_aggregate_and_capacity_admission_and_one_less_boundaries() {
    let owner = host(row(vec![text()]), None, None);
    let ua = catalog();
    let graph = graph("text{display:block;float:left}", ua.next_context());
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let snapshot = owner.snapshot().unwrap();
    let base = evaluate_source_display(snapshot, &bundle, limits()).unwrap();
    assert_eq!(
        (
            base.work(),
            base.rule_visits(),
            base.matching_work(),
            base.declaration_offers(),
            base.membership_work()
        ),
        (16, 18, 18_000, 20, 200)
    );
    let mut exact = limits();
    exact.nodes = nz(2);
    exact.retained_bytes = nz(base.retained_bytes());
    exact.work = nz(16);
    exact.rule_visits = nz(18);
    exact.matching_work = nz(18_000);
    exact.declaration_offers = nz(20);
    exact.membership_work = nz(200);
    exact.per_target.rules = nz(9);
    exact.per_target.per_rule.declarations = nz(2);
    assert!(evaluate_source_display(snapshot, &bundle, exact).is_ok());
    let mut cap = exact;
    cap.nodes = nz(1);
    assert!(matches!(
        evaluate_source_display(snapshot, &bundle, cap),
        Err(SourceDisplayError::Nodes)
    ));
    cap = exact;
    cap.retained_bytes = nz(base.retained_bytes() - 1);
    assert!(matches!(
        evaluate_source_display(snapshot, &bundle, cap),
        Err(SourceDisplayError::Bytes)
    ));
    cap = exact;
    cap.work = nz(15);
    assert!(matches!(
        evaluate_source_display(snapshot, &bundle, cap),
        Err(SourceDisplayError::Work)
    ));
    cap = exact;
    cap.rule_visits = nz(17);
    assert!(matches!(
        evaluate_source_display(snapshot, &bundle, cap),
        Err(SourceDisplayError::RuleVisits)
    ));
    cap = exact;
    cap.matching_work = nz(17_999);
    assert!(matches!(
        evaluate_source_display(snapshot, &bundle, cap),
        Err(SourceDisplayError::MatchingWork)
    ));
    cap = exact;
    cap.declaration_offers = nz(19);
    assert!(matches!(
        evaluate_source_display(snapshot, &bundle, cap),
        Err(SourceDisplayError::DeclarationOffers)
    ));
    cap = exact;
    cap.membership_work = nz(199);
    assert!(matches!(
        evaluate_source_display(snapshot, &bundle, cap),
        Err(SourceDisplayError::MembershipWork)
    ));
    cap = exact;
    cap.per_target.rules = nz(8);
    assert!(matches!(
        evaluate_source_display(snapshot, &bundle, cap),
        Err(SourceDisplayError::Rules)
    ));
    cap = exact;
    cap.per_target.per_rule.declarations = nz(1);
    assert!(matches!(
        evaluate_source_display(snapshot, &bundle, cap),
        Err(SourceDisplayError::Declarations { rule: 8 })
    ));
}

#[test]
fn disabled_and_unmatched_declarations_are_still_admitted() {
    let owner = host(text().into_parts().1, None, None);
    let ua = catalog();
    let graph = graph("absent{display:block;float:left}", ua.next_context());
    for enabled in [false, true] {
        let sheets = controls(&[1], enabled, 100);
        let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let mut cap = limits();
        cap.per_target.per_rule.declarations = nz(1);
        assert!(matches!(
            evaluate_source_display(owner.snapshot().unwrap(), &bundle, cap),
            Err(SourceDisplayError::Declarations { rule: 8 })
        ));
    }
}

#[test]
fn source_view_failures_retain_their_named_account() {
    let owner = host(row(vec![text()]), None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let mut cap = limits();
    cap.selectors.entries = nz(1);
    assert!(matches!(
        evaluate_source_display(owner.snapshot().unwrap(), &bundle, cap),
        Err(SourceDisplayError::Source(ConstructionSourceError::Entries))
    ));
    cap = limits();
    cap.selectors.retained_bytes = nz(1);
    assert!(matches!(
        evaluate_source_display(owner.snapshot().unwrap(), &bundle, cap),
        Err(SourceDisplayError::Source(ConstructionSourceError::Bytes))
    ));
    cap = limits();
    cap.selectors.work = nz(1);
    assert!(matches!(
        evaluate_source_display(owner.snapshot().unwrap(), &bundle, cap),
        Err(SourceDisplayError::Source(ConstructionSourceError::Work))
    ));
}

#[test]
fn huge_byte_allowances_are_monotone_and_checked_work_overflow_rejects() {
    let owner = host(row(vec![text()]), None, None);
    let ua = catalog();
    let sheets = controls(&[], true, usize::MAX);
    let bindings = [];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let mut cap = limits();
    cap.retained_bytes = nz(usize::MAX);
    cap.per_target.per_rule.matching.retained_bytes = nz(usize::MAX);
    let result = evaluate_source_display(owner.snapshot().unwrap(), &bundle, cap).unwrap();
    assert_eq!(result.membership_work(), 0);
    cap.per_target.per_rule.matching.work = nz(usize::MAX);
    cap.matching_work = nz(usize::MAX);
    assert!(matches!(
        evaluate_source_display(owner.snapshot().unwrap(), &bundle, cap),
        Err(SourceDisplayError::MatchingWork)
    ));
    let graph = graph("text{display:block}", ua.next_context());
    let sheets = controls(&[1], true, usize::MAX);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let mut large_bundle = bundle_limits();
    large_bundle.membership_work = nz(usize::MAX);
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, large_bundle).unwrap();
    cap = limits();
    cap.membership_work = nz(usize::MAX);
    assert!(matches!(
        evaluate_source_display(owner.snapshot().unwrap(), &bundle, cap),
        Err(SourceDisplayError::MembershipWork)
    ));
}

#[test]
fn text_keywords_follow_origins_importance_specificity_order_and_controls() {
    let owner = host(
        row(vec![named("normal"), named("important"), named("order")]),
        None,
        None,
    );
    let ua = catalog();
    let user = graph(
        "row{white-space:pre-wrap;text-transform:uppercase} #normal{white-space:pre} #important{white-space:nowrap!important;text-transform:lowercase!important}",
        ua.next_context(),
    );
    let author = graph(
        "#normal{white-space:pre-line} #important{white-space:normal!important;text-transform:capitalize!important} #order{white-space:pre} #order{white-space:nowrap} text{white-space:normal}",
        user.next_context().unwrap(),
    );
    for enabled in [true, false] {
        let sheets = controls(&[1], enabled, 100);
        let bindings = [
            binding(&user, GraphOrigin::User),
            binding(&author, GraphOrigin::Author(sheet(1))),
        ];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let result = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
        let expected = if enabled {
            ["pre-line", "nowrap", "nowrap", "pre-wrap"]
        } else {
            ["pre", "nowrap", "pre-wrap", "pre-wrap"]
        };
        for (index, white) in expected.into_iter().enumerate() {
            let pair = result.text_keywords(index).unwrap();
            assert_eq!(pair.white_space().keyword().as_str(), white);
            assert_eq!(
                pair.text_transform().keyword().as_str(),
                if index == 1 { "lowercase" } else { "uppercase" }
            );
        }
    }
}
#[test]
fn every_text_keyword_computes_through_real_stylesheet_graphs() {
    let owner = host(row(vec![text()]), None, None);
    let ua = catalog();
    let sheets = controls(&[], true, 100);
    for white in ["normal", "pre", "nowrap", "pre-wrap", "pre-line"] {
        for transform in ["none", "capitalize", "uppercase", "lowercase"] {
            let graph = graph(
                &format!(
                    "row{{white-space:{white};text-transform:{transform}}} text{{white-space:inherit;text-transform:inherit}}"
                ),
                ua.next_context(),
            );
            let bindings = [binding(&graph, GraphOrigin::User)];
            let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
            let result =
                evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
            for index in 0..2 {
                let pair = result.text_keywords(index).unwrap();
                assert_eq!(
                    pair.white_space().property(),
                    iced_css_core::values::registry::PropertyId::lookup("white-space").unwrap()
                );
                assert_eq!(
                    pair.text_transform().property(),
                    iced_css_core::values::registry::PropertyId::lookup("text-transform").unwrap()
                );
                assert_eq!(pair.white_space().keyword().as_str(), white);
                assert_eq!(pair.text_transform().keyword().as_str(), transform);
            }
        }
    }
}

#[test]
fn suppressed_sources_inherit_keywords_without_mutating_text_or_runs() {
    let spans: Vec<iced::widget::text::Span<'static, (), iced::Font>> = vec![
        iced::widget::text::Span::new(" MiXeD "),
        iced::widget::text::Span::new("\ncase"),
    ];
    let rich = adapters::rich_text_vec::<_, (), iced::Theme, ()>(spans, capture_limits())
        .unwrap()
        .erase()
        .style_id("rich", attrs())
        .unwrap();
    let nested = adapters::container(rich, capture_limits()).unwrap().erase();
    let plain = adapters::text(" Mixed  \ntext ", capture_limits())
        .unwrap()
        .erase();
    let owner = host(row(vec![plain, nested]), None, None);
    let snapshot = owner.snapshot().unwrap();
    let originals: Vec<_> = snapshot
        .construction()
        .nodes()
        .iter()
        .map(|node| (node.text(), node.text_runs()))
        .collect();
    let ua = catalog();
    let graph = graph(
        "row{display:none;white-space:pre-wrap;text-transform:uppercase} container{white-space:inherit;text-transform:inherit} #rich{display:block;white-space:pre-line;text-transform:lowercase}",
        ua.next_context(),
    );
    let sheets = controls(&[], true, 100);
    let bindings = [binding(&graph, GraphOrigin::User)];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let result = evaluate_source_display(snapshot, &bundle, limits()).unwrap();
    let eligibility = iced_css::evaluation::source_eligibility::evaluate_source_eligibility(
        &result,
        iced_css::evaluation::source_eligibility::SourceEligibilityLimits {
            nodes: bound(),
            retained_bytes: nz(20_000_000),
            work: nz(20_000_000),
        },
    )
    .unwrap();
    assert!(eligibility.ordered().is_empty());
    assert_eq!(result.resolution(3).unwrap().display, Display::None);
    assert_eq!(result.resolution(1).unwrap().display, Display::Block);
    for (index, original) in originals.iter().enumerate() {
        let pair = result.text_keywords(index).unwrap();
        assert_eq!(
            pair.white_space().keyword().as_str(),
            if index == 1 { "pre-line" } else { "pre-wrap" }
        );
        assert_eq!(
            pair.text_transform().keyword().as_str(),
            if index == 1 { "lowercase" } else { "uppercase" }
        );
        let node = &result.snapshot().construction().nodes()[index];
        assert_eq!((node.text(), node.text_runs()), *original);
        if let Some(text) = original.0 {
            assert!(std::ptr::eq(node.text().unwrap(), text));
        }
        if let Some(runs) = original.1 {
            assert!(std::ptr::eq(node.text_runs().unwrap(), runs));
        }
        assert!(std::ptr::eq(
            result.token(index).unwrap(),
            snapshot.token(index).unwrap()
        ));
    }
    assert_eq!(originals[0].0, Some(" Mixed  \ntext "));
    assert_eq!(originals[1].0, Some(" MiXeD \ncase"));
    assert_eq!(originals[1].1.unwrap(), [0..7, 7..12]);
}

#[test]
fn root_text_inherit_uses_initials_independently_of_display_transformation() {
    let owner = host(text().into_parts().1, None, None);
    let ua = catalog();
    let graph = graph(
        "text{display:inline;white-space:inherit;text-transform:inherit}",
        ua.next_context(),
    );
    let sheets = controls(&[], true, 100);
    let bindings = [binding(&graph, GraphOrigin::User)];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let result = evaluate_source_display(owner.snapshot().unwrap(), &bundle, limits()).unwrap();
    assert_eq!(result.resolution(0).unwrap().display, Display::Block);
    let pair = result.text_keywords(0).unwrap();
    assert_eq!(pair.white_space().keyword().as_str(), "normal");
    assert_eq!(pair.text_transform().keyword().as_str(), "none");
}
