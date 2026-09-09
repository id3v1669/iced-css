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
use iced_css::resources::{
    graph::GraphWorkerError,
    local::{ReadBodyLimits, read_file_blocking},
    worker::FileReader,
};
use iced_css_core::resources::body::BodyLimits;
use std::path::PathBuf;
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "iced-css-graph-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn load(path: PathBuf) -> Result<Vec<OwnedLoadedSheet>, String> {
    let mut output = Vec::new();
    for (index, name) in ["root.css", "child.css"].iter().enumerate() {
        let body = read_file_blocking(
            &path.join(name),
            ReadBodyLimits {
                reads: n(),
                body: BodyLimits {
                    retained_bytes: n(),
                    work: n(),
                },
            },
        )
        .map_err(|e| e.to_string())?;
        let candidate = parse_separate_unicode_stylesheet(
            body.as_slice(),
            EncodingMetadata {
                transport: None,
                linking: None,
                referring: None,
            },
            context(),
            intake_limits(),
        )
        .map_err(|e| e.to_string())?;
        output.push(OwnedLoadedSheet {
            resource: id(index as u64 + 1),
            candidate,
            imports: if index == 0 { vec![link(0, 1)] } else { vec![] },
        });
    }
    Ok(output)
}
#[test]
fn actual_import_read_failure_keeps_old_root_until_complete_retry() {
    let temp = Temp::new();
    let root = temp.0.join("root.css");
    let child = temp.0.join("child.css");
    std::fs::write(&root, b"@import \"child.css\"; r{color:red}").unwrap();
    std::fs::write(&child, b"a{height:2px}").unwrap();
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut owner = owner();
    let first = owner.request(0, context(), graph_limits()).unwrap();
    let path = temp.0.clone();
    let update_thread = std::thread::current().id();
    let work = reader.load_stylesheet_graph(first, move || {
        assert_ne!(std::thread::current().id(), update_thread);
        load(path)
    });
    let done = iced::futures::executor::block_on(work);
    assert!(owner.active().is_none());
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Published(_)
    ));
    let revision = owner.active_revision();
    std::fs::write(&root, b"@import \"child.css\"; r{color:blue}").unwrap();
    std::fs::remove_file(&child).unwrap();
    let next = owner.request(0, context(), graph_limits()).unwrap();
    let path = temp.0.clone();
    let done =
        iced::futures::executor::block_on(reader.load_stylesheet_graph(next, move || load(path)));
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition(GraphWorkerError::Acquisition(
            _
        )))
    ));
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().resources()[0]
            .candidate
            .source()
            .text(),
        "@import \"child.css\"; r{color:red}"
    );
    assert_eq!(
        owner.active().unwrap().resources()[1]
            .candidate
            .source()
            .text(),
        "a{height:2px}"
    );
    std::fs::write(&child, b"a{height:3px}").unwrap();
    let next = owner.request(0, context(), graph_limits()).unwrap();
    let path = temp.0.clone();
    let done =
        iced::futures::executor::block_on(reader.load_stylesheet_graph(next, move || load(path)));
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(
        owner.active().unwrap().resources()[0]
            .candidate
            .source()
            .text(),
        "@import \"child.css\"; r{color:blue}"
    );
    assert_eq!(reader.in_flight(), 0);
}
#[test]
fn admission_failure_does_not_call_loader_and_supersedes_unread_success() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut owner = owner();
    let old = owner.request(0, context(), graph_limits()).unwrap();
    let older = reader.load_stylesheet_graph(old, || {
        Ok::<_, &str>(vec![resource(1, "r{color:red}", vec![])])
    });
    let latest = owner.request(0, context(), graph_limits()).unwrap();
    let called = Arc::new(AtomicBool::new(false));
    let check = called.clone();
    let rejected = reader.load_stylesheet_graph(latest, move || {
        check.store(true, Ordering::SeqCst);
        Ok::<_, &str>(vec![])
    });
    let done = iced::futures::executor::block_on(rejected);
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition(GraphWorkerError::Submit(_)))
    ));
    assert!(!called.load(Ordering::SeqCst));
    let done = iced::futures::executor::block_on(older);
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(owner.active().is_none());
    assert_eq!(reader.in_flight(), 0);
}

#[test]
fn worker_failure_returns_the_original_request_completion() {
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut owner = owner();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let done = iced::futures::executor::block_on(
        reader.load_stylesheet_graph(request, || -> Result<Vec<OwnedLoadedSheet>, &str> {
            panic!("controlled resource worker failure")
        }),
    );
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition(GraphWorkerError::Worker(
            iced_css::resources::worker::WorkerFailure::Panicked
        )))
    ));
    assert!(owner.active().is_none());
    assert_eq!(reader.in_flight(), 0);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let done = iced::futures::executor::block_on(
        reader.load_stylesheet_graph(request, || Ok::<_, &str>(vec![resource(1, "", vec![])])),
    );
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Published(_)
    ));
}
