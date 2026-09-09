//! Exercise the existing source publication and UA cascade with resource leaves.
use iced_css::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement, adapters, source::ConstructionState},
    evaluation::{
        display_rules::{BundleLimits, DisplayRuleBundle},
        source_display::{SourceDisplayLimits, evaluate_source_display},
    },
    host::source_publication::{InitialSourceHost, PublicationLimits},
};
use iced_css_core::{
    cascade::{
        rule::{RuleBatchLimits, RuleCascadeLimits},
        sheets::{AuthorSheets, SheetLimits},
        ua_display::{UaDisplayCatalog, UaDisplayLimits},
    },
    formatting::display::Display,
    identity::SourceChangeLimits,
    selectors::{
        MatchLimits, SelectorLimits, SelectorSource, matching_specificity, parse_selector_list,
    },
    source::{AttributeLimits, selector_tree::SelectorTreeLimits},
    syntax::StringLimits,
    values::{NumericLimits, color::ColorLimits},
};
use std::num::{NonZeroU64, NonZeroUsize};
fn n() -> NonZeroUsize {
    NonZeroUsize::new(1000000).unwrap()
}
pub fn exercise(
    child: CapturedElement<'_, (), iced::Theme, super::resource_renderer::ResourceRenderer>,
    kind: &str,
) {
    let capture = CaptureLimits {
        nodes: n(),
        depth: n(),
        retained_bytes: n(),
        work: n(),
    };
    let attrs = AttributeLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    };
    let child = child
        .source_key(b"resource", capture)
        .unwrap()
        .style_id("art", attrs)
        .unwrap()
        .style_classes(&["asset"], attrs)
        .unwrap()
        .style_attribute("data-origin", "authored", attrs)
        .unwrap()
        .map(|()| ());
    let construction = adapters::row_from_vec(vec![child], capture)
        .unwrap()
        .into_parts()
        .1;
    assert_eq!(construction.nodes().len(), 2);
    assert_eq!(construction.nodes()[0].canonical_type(), kind);
    assert_eq!(construction.nodes()[0].source_key(), Some(&b"resource"[..]));
    assert_eq!(construction.nodes()[0].parent(), Some(1));
    assert_eq!(construction.nodes()[0].role_position(), Some(0));
    let selectors = SelectorTreeLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    };
    let mut host = InitialSourceHost::new(
        NonZeroU64::new(100000).unwrap(),
        ProgressLimits {
            addresses: NonZeroU64::new(100000).unwrap(),
            bindings: n(),
            tracks: n(),
            retained_bytes: n(),
            work: n(),
        },
    );
    host.prepare(
        construction,
        vec![ConstructionState::default(); 2],
        None,
        PublicationLimits {
            nodes: n(),
            snapshot_bytes: n(),
            work: n(),
            selectors,
            identities: SourceChangeLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
            },
        },
    )
    .unwrap()
    .commit();
    let snapshot = host.snapshot().unwrap();
    let source = snapshot.selector_source(selectors).unwrap();
    assert_eq!(source.parent(0), Some(1));
    let parsed = parse_selector_list(
        &format!("row > {kind}#art.asset[data-origin=authored]:first-child"),
        SelectorLimits {
            entries: n(),
            retained_bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    assert!(
        matching_specificity(
            &parsed,
            &source,
            0,
            None,
            MatchLimits {
                work: n(),
                retained_bytes: n()
            }
        )
        .unwrap()
        .is_some()
    );
    let ua = UaDisplayCatalog::new(UaDisplayLimits {
        rules: n(),
        retained_bytes: n(),
        work: n(),
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
    })
    .unwrap();
    let sheets = AuthorSheets::new(
        vec![],
        vec![],
        true,
        None,
        SheetLimits {
            entries: n(),
            retained_bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    let bundle = DisplayRuleBundle::new(
        &ua,
        &[],
        &sheets,
        BundleLimits {
            graphs: n(),
            rules: n(),
            retained_bytes: n(),
            work: n(),
            membership_work: n(),
        },
    )
    .unwrap();
    let display = evaluate_source_display(
        snapshot,
        &bundle,
        SourceDisplayLimits {
            nodes: n(),
            retained_bytes: n(),
            work: n(),
            rule_visits: n(),
            matching_work: n(),
            declaration_offers: n(),
            membership_work: n(),
            selectors,
            per_target: RuleBatchLimits {
                rules: n(),
                per_rule: RuleCascadeLimits {
                    declarations: n(),
                    matching: MatchLimits {
                        work: NonZeroUsize::new(1000).unwrap(),
                        retained_bytes: n(),
                    },
                },
            },
        },
    )
    .unwrap();
    assert_eq!(display.resolution(0).unwrap().display, Display::InlineBlock);
    assert!(std::ptr::eq(
        display.token(0).unwrap(),
        snapshot.token(0).unwrap()
    ));
    assert_eq!(
        snapshot.construction().nodes()[0].source_key(),
        Some(&b"resource"[..])
    );
}

pub fn distinct_sources(
    children: Vec<CapturedElement<'_, (), iced::Theme, super::resource_renderer::ResourceRenderer>>,
) {
    let capture = CaptureLimits {
        nodes: n(),
        depth: n(),
        retained_bytes: n(),
        work: n(),
    };
    let construction = adapters::row_from_vec(children, capture)
        .unwrap()
        .into_parts()
        .1;
    assert_eq!(construction.nodes().len(), 3);
    assert!(construction.nodes()[0].source_key().is_none());
    assert!(construction.nodes()[1].source_key().is_none());
    let selectors = SelectorTreeLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    };
    let mut host = InitialSourceHost::new(
        NonZeroU64::new(100000).unwrap(),
        ProgressLimits {
            addresses: NonZeroU64::new(100000).unwrap(),
            bindings: n(),
            tracks: n(),
            retained_bytes: n(),
            work: n(),
        },
    );
    host.prepare(
        construction,
        vec![ConstructionState::default(); 3],
        None,
        PublicationLimits {
            nodes: n(),
            snapshot_bytes: n(),
            work: n(),
            selectors,
            identities: SourceChangeLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
            },
        },
    )
    .unwrap()
    .commit();
    let snapshot = host.snapshot().unwrap();
    assert_ne!(
        host.engine().instance(snapshot.token(0).unwrap()).unwrap(),
        host.engine().instance(snapshot.token(1).unwrap()).unwrap()
    );
}
