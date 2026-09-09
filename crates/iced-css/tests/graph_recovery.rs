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
use iced_css::host::stylesheet::graph_recovery::{GraphRecoveryError, report_graph_recovery};
use std::sync::Mutex;
struct Sink(Mutex<Vec<String>>);
static SINK: Sink = Sink(Mutex::new(Vec::new()));
impl log::Log for Sink {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.target() == "iced_css::stylesheet"
    }
    fn log(&self, r: &log::Record<'_>) {
        if self.enabled(r.metadata()) {
            assert_eq!(r.level(), log::Level::Error);
            self.0.lock().unwrap().push(r.args().to_string());
        }
    }
    fn flush(&self) {}
}
fn graph(root: &str, child: &str) -> OwnedStylesheetGraph {
    let links = if root.starts_with("@import") {
        vec![link(0, 1), link(1, 1)]
    } else {
        vec![]
    };
    OwnedStylesheetGraph::new(
        vec![
            resource(1, root, links),
            resource(2, child, vec![]),
            resource(3, "unused{height:bogus}", vec![]),
        ],
        0,
        context(),
        graph_limits(),
    )
    .unwrap()
}
#[test]
fn accepted_graph_recovery_is_scoped_to_active_resources_and_prior_causes() {
    log::set_logger(&SINK).unwrap();
    log::set_max_level(log::LevelFilter::Error);
    let root = "@import \"child\"; @import \"child\"; r{color:red}";
    let mut owner = CandidatePublication::<OwnedStylesheetGraph>::new(PublicationLimits {
        requests: NonZeroU64::new(10).unwrap(),
        revisions: NonZeroU64::new(10).unwrap(),
    });
    let first = owner.request().unwrap();
    owner
        .finish(first, Ok::<_, &str>(graph(root, "a{width:bogus}")))
        .unwrap();
    report_graph_recovery(owner.active().unwrap(), None, n()).unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 1);
    let second = owner.request().unwrap();
    let (_, previous) = owner
        .finish_with_previous(
            second,
            Ok::<_, &str>(graph(root, "a{color:red;width:bogus}")),
        )
        .unwrap();
    report_graph_recovery(owner.active().unwrap(), previous.as_ref(), n()).unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 1);
    let third = owner.request().unwrap();
    let shuffled = OwnedStylesheetGraph::new(
        vec![
            resource(2, "a{width:bogus}", vec![]),
            resource(3, "unused{height:bogus}", vec![]),
            resource(1, root, vec![link(0, 0), link(1, 0)]),
        ],
        2,
        context(),
        graph_limits(),
    )
    .unwrap();
    let (_, previous) = owner
        .finish_with_previous(third, Ok::<_, &str>(shuffled))
        .unwrap();
    report_graph_recovery(owner.active().unwrap(), previous.as_ref(), n()).unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 1);
    let third = owner.request().unwrap();
    let (_, previous) = owner
        .finish_with_previous(
            third,
            Ok::<_, &str>(graph("r{color:blue}", "a{width:bogus}")),
        )
        .unwrap();
    report_graph_recovery(owner.active().unwrap(), previous.as_ref(), n()).unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 1);
    let fourth = owner.request().unwrap();
    let (_, previous) = owner
        .finish_with_previous(fourth, Ok::<_, &str>(graph(root, "a{width:bogus}")))
        .unwrap();
    report_graph_recovery(owner.active().unwrap(), previous.as_ref(), n()).unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 2);
    assert!(SINK.0.lock().unwrap()[0].contains("resource"));
    assert!(SINK.0.lock().unwrap()[0].contains("InvalidValue"));
    assert!(matches!(
        report_graph_recovery(owner.active().unwrap(), None, NonZeroUsize::MIN),
        Err(GraphRecoveryError::Work)
    ));
    assert_eq!(owner.active().unwrap().rules().count(), 1);
    assert_eq!(SINK.0.lock().unwrap().len(), 2);
    let healthy = build();
    report_graph_recovery(&healthy, None, n()).unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 2);
    let empty_import = OwnedStylesheetGraph::new(
        vec![
            resource(1, "@import \"empty\";", vec![link(0, 1)]),
            resource(2, "", vec![]),
        ],
        0,
        context(),
        graph_limits(),
    )
    .unwrap();
    assert!(matches!(
        report_graph_recovery(&empty_import, None, NonZeroUsize::new(4).unwrap()),
        Err(GraphRecoveryError::Work)
    ));
    report_graph_recovery(&empty_import, None, n()).unwrap();
}
