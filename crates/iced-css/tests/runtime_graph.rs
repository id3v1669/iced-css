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
use iced_css::host::stylesheet::graph::{GraphLoadError, RuntimeStylesheetGraph};
fn owner() -> RuntimeStylesheetGraph {
    RuntimeStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::new(20).unwrap(),
            revisions: NonZeroU64::new(20).unwrap(),
        },
        n(),
    )
}
fn complete_inputs(root: &str, child: &str) -> Vec<OwnedLoadedSheet> {
    vec![
        resource(1, root, vec![link(0, 1)]),
        resource(2, child, vec![]),
    ]
}
fn missing(root: &str) -> Vec<OwnedLoadedSheet> {
    vec![resource(
        1,
        root,
        vec![ImportBinding {
            entry: 0,
            resolution: ImportResolution::Unavailable,
        }],
    )]
}
#[test]
fn failed_import_reload_retains_both_old_root_and_old_import() {
    let mut owner = owner();
    let first = owner.request(0, context(), graph_limits()).unwrap();
    let duplicate = first.clone();
    let completion = first.assemble(Ok::<_, &str>(complete_inputs(
        "@import \"child\"; r{color:red}",
        "a{height:2px}",
    )));
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    let ignored = duplicate.assemble(Ok::<_, &str>(missing("@import \"child\"; r{color:green}")));
    assert!(matches!(
        owner.finish(ignored).unwrap(),
        SheetCompletion::Ignored
    ));
    let revision = owner.active_revision();
    let next = owner.request(0, context(), graph_limits()).unwrap();
    let completion = next.assemble(Ok::<_, &str>(missing("@import \"child\"; r{color:blue}")));
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Assembly(OwnedGraphError::Expansion(
            LoadedGraphError::UnavailableImport { .. }
        )))
    ));
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().resources()[0]
            .candidate
            .source()
            .text(),
        "@import \"child\"; r{color:red}"
    );
    assert_eq!(
        owner.active().unwrap().resources()[1]
            .candidate
            .source()
            .text(),
        "a{height:2px}"
    );
    let next = owner.request(0, context(), graph_limits()).unwrap();
    let done = next.assemble(Ok::<_, &str>(complete_inputs(
        "@import \"child\"; r{color:blue}",
        "a{height:3px}",
    )));
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(
        owner.active().unwrap().resources()[1]
            .candidate
            .source()
            .text(),
        "a{height:3px}"
    );
}
#[test]
fn first_load_can_omit_unavailable_import_and_empty_success_replaces_it() {
    let mut owner = owner();
    let first = owner.request(0, context(), graph_limits()).unwrap();
    let done = first.assemble(Ok::<_, &str>(missing(
        "@import \"child\"; r{width:bogus;color:red}",
    )));
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().rules().count(), 1);
    let next = owner.request(0, context(), graph_limits()).unwrap();
    let done = next.assemble(Ok::<_, &str>(vec![resource(1, "", vec![])]));
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().rules().count(), 0);
}
#[test]
fn explicit_failure_and_foreign_or_older_completions_cannot_publish() {
    let mut owner = owner();
    let older = owner.request(0, context(), graph_limits()).unwrap();
    let newest = owner.request(0, context(), graph_limits()).unwrap();
    let failed = newest.assemble(Err::<Vec<OwnedLoadedSheet>, _>("import read failed"));
    assert!(matches!(
        owner.finish(failed).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition("import read failed"))
    ));
    let done = older.assemble(Ok::<_, &str>(vec![resource(1, "r{color:red}", vec![])]));
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(owner.active().is_none());
    let other = RuntimeStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        n(),
    );
    let mut other = other;
    let foreign = other.request(0, context(), graph_limits()).unwrap();
    let fresh = owner.request(0, context(), graph_limits()).unwrap();
    let done = foreign.assemble(Ok::<_, &str>(vec![resource(1, "r{color:blue}", vec![])]));
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Ignored
    ));
    fn send<T: Send + 'static>(_: &T) {}
    send(&fresh);
    let done = std::thread::spawn(move || {
        fresh.assemble(Ok::<_, &str>(vec![resource(1, "r{color:red}", vec![])]))
    })
    .join()
    .unwrap();
    send(&done);
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Published(_)
    ));
}
