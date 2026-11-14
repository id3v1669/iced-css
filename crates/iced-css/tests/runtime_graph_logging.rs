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
    stylesheet::{imports::*, owned_graph::*},
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
use iced_css::host::stylesheet::graph::RuntimeStylesheetGraph;
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
use std::sync::Mutex;
struct Sink(Mutex<Vec<String>>);
static SINK: Sink = Sink(Mutex::new(Vec::new()));
static PANIC_NEXT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
impl log::Log for Sink {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.target() == "iced_css::stylesheet"
    }
    fn log(&self, r: &log::Record<'_>) {
        if self.enabled(r.metadata()) {
            assert!(
                !PANIC_NEXT.swap(false, std::sync::atomic::Ordering::SeqCst),
                "application logger panic probe"
            );
            assert_eq!(r.level(), log::Level::Error);
            self.0.lock().unwrap().push(r.args().to_string());
        }
    }
    fn flush(&self) {}
}
#[test]
fn current_graph_failures_and_recovery_log_without_duplicate_occurrences() {
    log::set_logger(&SINK).unwrap();
    log::set_max_level(log::LevelFilter::Error);
    let mut runtime = owner();
    let first = runtime.request(0, context(), graph_limits()).unwrap();
    runtime
        .finish(first.assemble(Ok::<_, &str>(complete_inputs(
            "@import \"child\"; r{color:red}",
            "a{width:bogus}",
        ))))
        .unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 1);
    // Recovery names the original-source position and the rejected property.
    assert!(
        SINK.0.lock().unwrap()[0]
            .contains("at line 1, column 3 (source bytes 2..13) in property `width`")
    );
    let next = runtime.request(0, context(), graph_limits()).unwrap();
    runtime
        .finish(next.assemble(Ok::<_, &str>(complete_inputs(
            "@import \"child\"; r{color:blue}",
            "a{color:red;width:bogus}",
        ))))
        .unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 1);
    let older = runtime.request(0, context(), graph_limits()).unwrap();
    let latest = runtime.request(0, context(), graph_limits()).unwrap();
    runtime
        .finish(latest.assemble(Ok::<_, &str>(missing("@import \"child\"; r{color:green}"))))
        .unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 2);
    assert!(SINK.0.lock().unwrap()[1].contains("UnavailableImport"));
    runtime
        .finish(older.assemble(Err::<Vec<OwnedLoadedSheet>, _>("stale read failure")))
        .unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 2);
    assert_eq!(
        runtime.active().unwrap().resources()[0]
            .candidate
            .source()
            .text(),
        "@import \"child\"; r{color:blue}"
    );
    let mut first_load = owner();
    let request = first_load.request(0, context(), graph_limits()).unwrap();
    first_load
        .finish(request.assemble(Ok::<_, &str>(vec![
            resource(
                1,
                "@import \"child\"; @import \"child\";",
                vec![link(0, 1), link(1, 1)],
            ),
            resource(
                2,
                "@import \"missing\";",
                vec![ImportBinding {
                    entry: 0,
                    resolution: ImportResolution::Unavailable,
                }],
            ),
        ])))
        .unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 3);
    assert!(SINK.0.lock().unwrap()[2].contains("unavailable"));
    assert_eq!(first_load.active().unwrap().cuts().len(), 2);
    let mut tiny = RuntimeStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        NonZeroUsize::MIN,
    );
    let request = tiny.request(0, context(), graph_limits()).unwrap();
    tiny.finish(request.assemble(Ok::<_, &str>(vec![resource(1, "", vec![])])))
        .unwrap();
    assert!(tiny.active().is_some());
    assert_eq!(SINK.0.lock().unwrap().len(), 4);
    assert!(SINK.0.lock().unwrap()[3].contains("Work"));
    assert!(tiny.request(0, context(), graph_limits()).is_err());
    assert_eq!(SINK.0.lock().unwrap().len(), 5);
    // A cycle observation for this edge must not suppress its later unavailable observation.
    let mut contextual = owner();
    let cycles = [1, 2, 3].map(|value| ImportCycleId::new(NonZeroU64::new(value).unwrap()));
    let inputs = || {
        vec![
            resource(
                10,
                "@import 'a'; @import 'c'; r{color:red}",
                vec![link(0, 1), link(1, 2)],
            ),
            resource(20, "@import 'c'; a{width:1px}", vec![link(0, 2)]),
            resource(
                30,
                "@import 'a'; c{height:2px}",
                vec![ImportBinding {
                    entry: 0,
                    resolution: ImportResolution::UnavailableWithCycle(cycles[1]),
                }],
            ),
        ]
    };
    let request = contextual.request(0, context(), graph_limits()).unwrap();
    contextual
        .finish(request.assemble_with_cycles(Ok::<_, &str>(inputs()), &cycles))
        .unwrap();
    assert_eq!(
        contextual
            .active()
            .unwrap()
            .cuts()
            .iter()
            .map(|cut| cut.reason)
            .collect::<Vec<_>>(),
        [ImportCutReason::Cycle, ImportCutReason::Unavailable]
    );
    assert_eq!(SINK.0.lock().unwrap().len(), 7);
    assert!(SINK.0.lock().unwrap()[5].contains("cyclic"));
    assert!(SINK.0.lock().unwrap()[6].contains("unavailable"));
    let revision = contextual.active_revision();
    let request = contextual.request(0, context(), graph_limits()).unwrap();
    contextual
        .finish(request.assemble_with_cycles(Ok::<_, &str>(inputs()), &cycles))
        .unwrap();
    assert_eq!(contextual.active_revision(), revision);
    assert_eq!(SINK.0.lock().unwrap().len(), 8);
    assert!(SINK.0.lock().unwrap()[7].contains("UnavailableImport"));
    let mut cyclic = owner();
    let cases = [
        ("@import 'self'; r{color:red}", 1, 9),
        ("/* shift */ @import 'self'; r{color:blue}", 1, 9),
        ("@import 'self'; @import 'self'; r{color:blue}", 2, 10),
        (
            "/* shift */ @import 'self'; @import 'self'; r{color:red}",
            2,
            10,
        ),
        ("@import 'alias'; r{color:red}", 1, 11),
        ("@import 'self'; r{color:red}", 1, 12),
    ];
    for (css, count, notices) in cases {
        let request = cyclic.request(0, context(), graph_limits()).unwrap();
        let links = (0..count).map(|entry| link(entry, 0)).collect();
        cyclic
            .finish(request.assemble(Ok::<_, &str>(vec![resource(1, css, links)])))
            .unwrap();
        assert_eq!(SINK.0.lock().unwrap().len(), notices, "{css}");
    }
    let stale = cyclic.request(0, context(), graph_limits()).unwrap();
    let latest = cyclic.request(0, context(), graph_limits()).unwrap();
    cyclic
        .finish(latest.assemble(Err::<Vec<OwnedLoadedSheet>, _>(
            "latest acquisition failure",
        )))
        .unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 13);
    cyclic
        .finish(stale.assemble(Ok::<_, &str>(vec![resource(
            1,
            "@import 'stale';",
            vec![link(0, 0)],
        )])))
        .unwrap();
    assert_eq!(SINK.0.lock().unwrap().len(), 13);
    let before = SINK.0.lock().unwrap().len();
    let mut limited = RuntimeStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        NonZeroUsize::MIN,
    );
    let request = limited.request(0, context(), graph_limits()).unwrap();
    let completion = request.assemble(Ok::<_, &str>(vec![resource(
        1,
        "@import 'self'; @import 'self';",
        vec![link(0, 0), link(1, 0)],
    )]));
    assert!(matches!(
        limited.finish(completion).unwrap(),
        iced_css_core::stylesheet::publication::SheetCompletion::Published(_)
    ));
    assert_eq!(limited.active().unwrap().cuts().len(), 2);
    assert!(
        SINK.0.lock().unwrap()[before..]
            .iter()
            .any(|notice| notice.contains("import diagnostics omitted") && notice.contains("Work"))
    );
    #[cfg(unix)]
    {
        use iced_css::host::stylesheet::file_graph::*;
        use iced_css::resources::{
            file_dependencies::FileDependencyLimits,
            file_imports::FileImportLimits,
            file_path::{FilePathLimits, unix_file_uri},
            file_uri_read::FileUriReadLimits,
            local::ReadBodyLimits,
            stylesheet::StylesheetEncoding,
            worker::FileReader,
        };
        use iced_css_core::{
            resources::{
                body::BodyLimits,
                publication::PublicationStatus,
                triggers::{RequestReason, RuntimeMode},
                uri::UriLimits,
            },
            stylesheet::{
                collector::CollectorLimits, identities::IdentityLimits, located_graph::BaseLimits,
                requests::ImportRequestLimits,
            },
        };
        struct Temp(std::path::PathBuf);
        impl Drop for Temp {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let temp = Temp(
            std::env::temp_dir().join(format!("iced-css-file-log-panic-{}", std::process::id())),
        );
        std::fs::create_dir(&temp.0).unwrap();
        let path = temp.0.join("missing.css");
        let path_limits = FilePathLimits {
            bytes: n(),
            work: n(),
        };
        let uri = unix_file_uri(&path, path_limits).unwrap();
        let mut file = RuntimeFileStylesheetGraph::new(
            RuntimeMode::Manual,
            context(),
            PublicationLimits {
                requests: NonZeroU64::new(10).unwrap(),
                revisions: NonZeroU64::new(3).unwrap(),
            },
            n(),
            IdentityLimits {
                entries: n(),
                bytes: n(),
                work: n(),
            },
        )
        .unwrap();
        let settings = FileGraphLimits {
            file: FileImportLimits {
                file: FileUriReadLimits {
                    uri: UriLimits {
                        bytes: n(),
                        work: n(),
                    },
                    path: path_limits,
                    read: ReadBodyLimits {
                        reads: n(),
                        body: BodyLimits {
                            retained_bytes: n(),
                            work: n(),
                        },
                    },
                },
                intake: intake_limits(),
                dependencies: FileDependencyLimits {
                    entries: n(),
                    retained_bytes: n(),
                    work: n(),
                },
            },
            collector: CollectorLimits {
                entries: n(),
                record_bytes: n(),
                work: n(),
                cycle_bytes: n(),
                requests: ImportRequestLimits {
                    entries: n(),
                    retained_bytes: n(),
                    work: n(),
                },
                tracking: graph_limits().expansion.tracking,
            },
            graph: graph_limits(),
            bases: BaseLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
            },
            metadata_bytes: n(),
        };
        let reader = FileReader::new(NonZeroUsize::MIN);
        let completion = iced::futures::executor::block_on(
            file.request(
                &reader,
                RequestReason::FirstAcquisition,
                FileGraphSource {
                    uri,
                    encoding: StylesheetEncoding::default(),
                },
                settings,
            )
            .unwrap(),
        );
        PANIC_NEXT.store(true, std::sync::atomic::Ordering::SeqCst);
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| file.finish(completion)));
        assert!(result.is_err());
        assert_eq!(file.status(), PublicationStatus::Failed);
        assert_eq!(
            file.candidate_dependencies().unwrap().entries()[0].native_path(),
            path
        );
        assert!(file.active_dependencies().is_none());
        std::fs::write(&path, b"r{color:red; not-a-css-property:value}").unwrap();
        let uri = unix_file_uri(&path, path_limits).unwrap();
        let work = file
            .request(
                &reader,
                RequestReason::ExplicitReload,
                FileGraphSource {
                    uri,
                    encoding: StylesheetEncoding::default(),
                },
                settings,
            )
            .unwrap();
        let completion = iced::futures::executor::block_on(work);
        use iced_css::{
            host::stylesheet::file_watch::FileGraphWatchConfig,
            resources::{watch::WatchConfig, watch_paths::*, watch_sessions::*},
        };
        use iced_css_core::resources::watch_ownership::*;
        let mut paths = WatchPathRegistry::new(WatchPathRegistryLimits {
            directories: n(),
            retained_bytes: n(),
        })
        .unwrap();
        let mut watches = WatchSessionHost::new(WatchOwnershipLimits {
            owners: n(),
            bindings: n(),
            dependencies: n(),
            retained_bytes: n(),
            work: n(),
            revisions: NonZeroU64::new(10).unwrap(),
        })
        .unwrap();
        let prepared = iced::futures::executor::block_on(file.prepare_watch(
            completion,
            &mut paths,
            &watches,
            FileGraphWatchConfig {
                owner: WatchOwnerId::new(NonZeroU64::MIN),
                paths: WatchPathLimits {
                    reports: n(),
                    dependencies: n(),
                    retained_bytes: n(),
                    work: n(),
                },
                mapping: WatchMappingLimits {
                    bindings: n(),
                    input_bytes: n(),
                    work: n(),
                },
                watch: WatchConfig {
                    recovery_interval: std::time::Duration::from_millis(10),
                    directories: n(),
                    retained_path_bytes: n(),
                },
            },
        ))
        .unwrap();
        let control = prepared.candidate_control().unwrap().clone();
        PANIC_NEXT.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| file
                .finish_watched(prepared, &mut watches)
                .is_ok()))
            .is_err()
        );
        assert_eq!(watches.ownership().snapshot().dependencies().len(), 1);
        assert!(!control.is_shutdown_requested());
        assert!(!watches.active().unwrap().control().is_shutdown_requested());
        assert_eq!(file.status(), PublicationStatus::Ready);
        assert_eq!(
            file.active_dependencies().unwrap().entries()[0].native_path(),
            path
        );
        assert!(file.candidate_dependencies().is_none());
        use iced_css_core::stylesheet::publication::SheetCompletion;
        let bad_watch = FileGraphWatchConfig {
            owner: WatchOwnerId::new(NonZeroU64::MIN),
            paths: WatchPathLimits {
                reports: n(),
                dependencies: n(),
                retained_bytes: n(),
                work: NonZeroUsize::MIN,
            },
            mapping: WatchMappingLimits {
                bindings: n(),
                input_bytes: n(),
                work: n(),
            },
            watch: WatchConfig {
                recovery_interval: std::time::Duration::from_secs(30),
                directories: n(),
                retained_path_bytes: n(),
            },
        };
        let prepare_failure = |file: &mut RuntimeFileStylesheetGraph,
                               paths: &mut WatchPathRegistry| {
            let current = iced::futures::executor::block_on(
                file.retry(&reader, RequestReason::ExplicitReload).unwrap(),
            );
            iced::futures::executor::block_on(
                file.prepare_watch(current, paths, &watches, bad_watch),
            )
            .unwrap_err()
        };
        let old_revision = file.active_revision();
        let failure = prepare_failure(&mut file, &mut paths);
        let before = SINK.0.lock().unwrap().len();
        assert!(matches!(
            file.fail_watch(failure).unwrap(),
            SheetCompletion::Failed(_)
        ));
        let logs = SINK.0.lock().unwrap();
        assert_eq!(logs.len(), before + 1);
        assert!(logs[before].contains("Watch(Paths(Work))"));
        drop(logs);
        assert_eq!(file.active_revision(), old_revision);
        std::fs::remove_file(&path).unwrap();
        let failure = prepare_failure(&mut file, &mut paths);
        let before = SINK.0.lock().unwrap().len();
        assert!(matches!(
            file.fail_watch(failure).unwrap(),
            SheetCompletion::Failed(_)
        ));
        let logs = SINK.0.lock().unwrap();
        assert_eq!(logs.len(), before + 2);
        assert!(logs[before].contains("Root("));
        assert!(logs[before + 1].contains("coverage also failed"));
        drop(logs);
        let stale = prepare_failure(&mut file, &mut paths);
        let current = prepare_failure(&mut file, &mut paths);
        let before = SINK.0.lock().unwrap().len();
        assert!(matches!(
            file.fail_watch(stale).unwrap(),
            SheetCompletion::Ignored
        ));
        assert_eq!(SINK.0.lock().unwrap().len(), before);
        PANIC_NEXT.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| file
                .fail_watch(current)
                .is_ok()))
            .is_err()
        );
        assert_eq!(file.status(), PublicationStatus::Failed);
        assert_eq!(file.active_revision(), old_revision);
        assert_eq!(
            file.candidate_dependencies().unwrap().entries()[0].native_path(),
            path
        );
        use iced_css::host::stylesheet::file_coordinator::*;
        let host_config = FileStylesheetHostConfig {
            work: iced_css::resources::watch_work::WatchWorkLimits {
                slots: NonZeroUsize::new(4).unwrap(),
                retained_bytes: n(),
            },
            mode: RuntimeMode::Manual,
            context: context(),
            publication: PublicationLimits {
                requests: NonZeroU64::new(10).unwrap(),
                revisions: NonZeroU64::new(10).unwrap(),
            },
            diagnostic_work: n(),
            identities: IdentityLimits {
                entries: n(),
                bytes: n(),
                work: n(),
            },
            graph: settings,
            paths: WatchPathRegistryLimits {
                directories: n(),
                retained_bytes: n(),
            },
            ownership: WatchOwnershipLimits {
                owners: n(),
                bindings: n(),
                dependencies: n(),
                retained_bytes: n(),
                work: n(),
                revisions: NonZeroU64::new(10).unwrap(),
            },
            watch: FileGraphWatchConfig {
                paths: WatchPathLimits {
                    work: n(),
                    ..bad_watch.paths
                },
                ..bad_watch
            },
        };
        let mut host =
            FileStylesheetHost::new(FileReader::new(NonZeroUsize::MIN), host_config).unwrap();
        let source = || FileGraphSource {
            uri: unix_file_uri(&path, path_limits).unwrap(),
            encoding: StylesheetEncoding::default(),
        };
        let work = host
            .request(RequestReason::FirstAcquisition, source())
            .unwrap();
        let message = iced::futures::executor::block_on(work);
        PANIC_NEXT.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.update(message)))
                .is_err()
        );
        assert!(!host.is_busy());
        assert_eq!(host.status(), PublicationStatus::Failed);
        let mut root_failure = FileStylesheetHost::new(
            FileReader::new(NonZeroUsize::MIN),
            FileStylesheetHostConfig {
                mode: RuntimeMode::Auto,
                watch: FileGraphWatchConfig {
                    paths: WatchPathLimits {
                        work: NonZeroUsize::MIN,
                        ..host_config.watch.paths
                    },
                    ..host_config.watch
                },
                ..host_config
            },
        )
        .unwrap();
        let work = root_failure
            .request(RequestReason::FirstAcquisition, source())
            .unwrap();
        let message = iced::futures::executor::block_on(work);
        PANIC_NEXT.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| root_failure.update(message)))
                .is_err()
        );
        assert!(!root_failure.is_busy());
        assert_eq!(root_failure.pending_work().unwrap(), 0);
        assert_eq!(root_failure.status(), PublicationStatus::Failed);
        assert!(iced::advanced::subscription::into_recipes(root_failure.subscription()).is_empty());
        std::fs::write(&path, b"a{color:blue;not-a-css-property:value}").unwrap();
        let mut host = FileStylesheetHost::new(
            FileReader::new(NonZeroUsize::MIN),
            FileStylesheetHostConfig {
                mode: RuntimeMode::Auto,
                ..host_config
            },
        )
        .unwrap();
        let work = host
            .request(RequestReason::FirstAcquisition, source())
            .unwrap();
        let root = host.update(iced::futures::executor::block_on(work));
        let work = root.work.unwrap();
        let preparation = host.update(iced::futures::executor::block_on(work));
        let message = iced::futures::executor::block_on(preparation.work.unwrap());
        PANIC_NEXT.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.update(message)))
                .is_err()
        );
        assert!(!host.is_busy());
        assert_eq!(host.status(), PublicationStatus::Ready);
        assert_eq!(
            iced::advanced::subscription::into_recipes(host.subscription()).len(),
            1
        );
    }
    {
        use iced::futures::executor::block_on;
        use iced_css::{host::stylesheet::embedded_graph::*, resources::embedded::*};
        use iced_css_core::{
            resources::{
                body::BodyLimits,
                publication::PublicationStatus,
                triggers::{RequestReason, RuntimeMode},
                uri::UriLimits,
            },
            stylesheet::{
                collector::CollectorLimits, identities::IdentityLimits, located_graph::BaseLimits,
                requests::ImportRequestLimits,
            },
        };
        use std::sync::{
            Arc,
            atomic::{AtomicU8, Ordering},
        };
        struct Resolver(Arc<AtomicU8>);
        impl EmbeddedResolver for Resolver {
            type Error = std::io::Error;
            fn resolve(&self, request: EmbeddedRequest<'_>) -> Result<EmbeddedSource, Self::Error> {
                let phase = self.0.load(Ordering::SeqCst);
                if phase == 2 || request.acquisition_uri.ends_with("missing.css") {
                    return Err(std::io::Error::other("missing embedded stylesheet"));
                }
                Ok(EmbeddedSource {
                    bytes: if phase == 0 {
                        b"a{color:red;not-a-css-property:value}".to_vec()
                    } else {
                        b"@import 'missing.css'; a{color:blue}".to_vec()
                    },
                    version: vec![phase],
                    dependency_base: "app:/root.css".into(),
                })
            }
        }
        let uri = UriLimits {
            bytes: n(),
            work: n(),
        };
        let settings = EmbeddedRuntimeLimits {
            graph: graph_limits(),
            acquisition: EmbeddedGraphLimits {
                stylesheet: EmbeddedStylesheetLimits {
                    asset: EmbeddedLimits {
                        uri,
                        metadata_bytes: n(),
                        body: BodyLimits {
                            retained_bytes: n(),
                            work: n(),
                        },
                    },
                    metadata_bytes: n(),
                    intake: intake_limits(),
                },
                collector: CollectorLimits {
                    entries: n(),
                    record_bytes: n(),
                    work: n(),
                    cycle_bytes: n(),
                    requests: ImportRequestLimits {
                        entries: n(),
                        retained_bytes: n(),
                        work: n(),
                    },
                    tracking: graph_limits().expansion.tracking,
                },
                bases: BaseLimits {
                    entries: n(),
                    retained_bytes: n(),
                    work: n(),
                },
                dependencies: EmbeddedDependencyLimits {
                    entries: n(),
                    retained_bytes: n(),
                    work: n(),
                },
            },
        };
        for mode in [RuntimeMode::Manual, RuntimeMode::Auto] {
            let phase = Arc::new(AtomicU8::new(0));
            let reader = EmbeddedReader::new(
                "app".into(),
                Resolver(Arc::clone(&phase)),
                NonZeroUsize::MIN,
                uri,
            )
            .unwrap();
            let mut runtime = RuntimeEmbeddedStylesheetGraph::new(
                reader,
                mode,
                context(),
                PublicationLimits {
                    requests: NonZeroU64::new(10).unwrap(),
                    revisions: NonZeroU64::new(10).unwrap(),
                },
                n(),
                IdentityLimits {
                    entries: n(),
                    bytes: n(),
                    work: n(),
                },
            )
            .unwrap();
            let completion = block_on(
                runtime
                    .request(
                        RequestReason::FirstAcquisition,
                        EmbeddedGraphSource {
                            base: "app:/main.css".into(),
                            reference: "root.css".into(),
                            encoding: Default::default(),
                        },
                        settings,
                    )
                    .unwrap(),
            );
            PANIC_NEXT.store(true, Ordering::SeqCst);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || runtime.finish(completion)
                ))
                .is_err()
            );
            assert_eq!(runtime.status(), PublicationStatus::Ready);
            let revision = runtime.active_revision();
            assert_eq!(
                runtime.active_dependencies().unwrap().entries()[0].version(),
                Some([0].as_slice())
            );
            assert!(runtime.candidate_dependencies().is_none());
            phase.store(1, Ordering::SeqCst);
            let completion = block_on(runtime.retry(RequestReason::EmbeddedNotification).unwrap());
            PANIC_NEXT.store(true, Ordering::SeqCst);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || runtime.finish(completion)
                ))
                .is_err()
            );
            assert_eq!(runtime.status(), PublicationStatus::Failed);
            assert_eq!(runtime.active_revision(), revision);
            assert_eq!(
                runtime.active_dependencies().unwrap().entries()[0].version(),
                Some([0].as_slice())
            );
            assert_eq!(
                runtime.candidate_dependencies().unwrap().entries()[0].version(),
                Some([1].as_slice())
            );
            phase.store(2, Ordering::SeqCst);
            let completion = block_on(runtime.retry(RequestReason::EmbeddedNotification).unwrap());
            PANIC_NEXT.store(true, Ordering::SeqCst);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || runtime.finish(completion)
                ))
                .is_err()
            );
            assert_eq!(runtime.active_revision(), revision);
            assert_eq!(
                runtime.active_dependencies().unwrap().entries()[0].version(),
                Some([0].as_slice())
            );
            assert_eq!(
                runtime.candidate_dependencies().unwrap().entries()[0].version(),
                None
            );

            use iced_css::host::stylesheet::embedded_coordinator::*;
            phase.store(0, Ordering::SeqCst);
            let reader = EmbeddedReader::new(
                "app".into(),
                Resolver(Arc::clone(&phase)),
                NonZeroUsize::MIN,
                uri,
            )
            .unwrap();
            let mut host = EmbeddedStylesheetHost::new(
                reader,
                EmbeddedStylesheetHostConfig {
                    work: iced_css::resources::watch_work::WatchWorkLimits {
                        slots: NonZeroUsize::MIN,
                        retained_bytes: n(),
                    },
                    mode,
                    context: context(),
                    publication: PublicationLimits {
                        requests: NonZeroU64::new(10).unwrap(),
                        revisions: NonZeroU64::new(10).unwrap(),
                    },
                    diagnostic_work: n(),
                    identities: IdentityLimits {
                        entries: n(),
                        bytes: n(),
                        work: n(),
                    },
                    runtime: settings,
                },
            )
            .unwrap();
            let message = block_on(
                host.request(
                    RequestReason::FirstAcquisition,
                    EmbeddedGraphSource {
                        base: "app:/main.css".into(),
                        reference: "root.css".into(),
                        encoding: Default::default(),
                    },
                )
                .unwrap(),
            );
            PANIC_NEXT.store(true, Ordering::SeqCst);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.update(message)))
                    .is_err()
            );
            assert!(!host.is_busy());
            assert_eq!(host.pending_work().unwrap(), 0);
            assert_eq!(
                host.active_dependencies().unwrap().entries()[0].version(),
                Some([0].as_slice())
            );
            phase.store(1, Ordering::SeqCst);
            let notification = host.notification();
            let update = host.update(notification);
            let message = block_on(update.work.unwrap());
            let notification = host.notification();
            assert!(host.update(notification).work.is_none());
            assert!(host.is_busy() && host.is_dirty());
            PANIC_NEXT.store(true, Ordering::SeqCst);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.update(message)))
                    .is_err()
            );
            assert!(!host.is_busy() && host.is_dirty());
            assert_eq!(host.pending_work().unwrap(), 0);
            assert_eq!(
                host.active_dependencies().unwrap().entries()[0].version(),
                Some([0].as_slice())
            );
            assert_eq!(
                host.candidate_dependencies().unwrap().entries()[0].version(),
                Some([1].as_slice())
            );
            phase.store(2, Ordering::SeqCst);
            let notification = host.notification();
            let update = host.update(notification);
            let message = block_on(update.work.unwrap());
            PANIC_NEXT.store(true, Ordering::SeqCst);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.update(message)))
                    .is_err()
            );
            assert!(!host.is_busy());
            assert_eq!(host.pending_work().unwrap(), 0);
            assert_eq!(
                host.active_dependencies().unwrap().entries()[0].version(),
                Some([0].as_slice())
            );
            assert_eq!(
                host.candidate_dependencies().unwrap().entries()[0].version(),
                None
            );
        }
    }
    http_runtime_reports_precede_logger_panics();
    http_message_reports_precede_logger_panics();
}

#[path = "fixtures_support/http_graph.rs"]
mod http_fixture;
fn http_runtime_reports_precede_logger_panics() {
    use iced::futures::executor::block_on;
    use iced_css::{host::stylesheet::http_graph::*, resources::worker::FileReader};
    use iced_css_core::{
        resources::{publication::PublicationStatus, triggers::*},
        stylesheet::{identities::IdentityLimits, located_graph::BaseLimits},
    };
    use std::sync::atomic::Ordering;
    for mode in [RuntimeMode::Manual, RuntimeMode::Auto] {
        let server = http_fixture::Server::new();
        server.state.lock().unwrap().warning = true;
        let acquisition = http_fixture::settings(
            intake_limits(),
            graph_limits().expansion.tracking,
            BaseLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
            },
        );
        let mut runtime = RuntimeHttpStylesheetGraph::new(
            FileReader::new(NonZeroUsize::MIN),
            http_fixture::transport(),
            mode,
            context(),
            PublicationLimits {
                requests: NonZeroU64::new(20).unwrap(),
                revisions: NonZeroU64::new(20).unwrap(),
            },
            n(),
            IdentityLimits {
                entries: n(),
                bytes: n(),
                work: n(),
            },
        )
        .unwrap();
        let limits = HttpRuntimeLimits {
            acquisition,
            graph: graph_limits(),
        };
        let completion = block_on(
            runtime
                .request(
                    RequestReason::FirstAcquisition,
                    server.source("/start"),
                    limits,
                )
                .unwrap(),
        );
        PANIC_NEXT.store(true, Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.finish(completion)))
                .is_err()
        );
        assert_eq!(runtime.status(), PublicationStatus::Ready);
        let revision = runtime.active_revision();
        let body = runtime.active_dependencies().unwrap().entries()[0]
            .representation()
            .unwrap()
            .content_decoded_body()
            .as_slice()
            .to_vec();
        assert!(runtime.candidate_dependencies().is_none());
        {
            let mut state = server.state.lock().unwrap();
            state.warning = false;
            state.blue = true;
            state.missing = true;
        }
        let completion = block_on(runtime.retry(RequestReason::ExplicitReload).unwrap());
        PANIC_NEXT.store(true, Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.finish(completion)))
                .is_err()
        );
        assert_eq!(runtime.active_revision(), revision);
        assert_eq!(
            runtime.active_dependencies().unwrap().entries()[0]
                .representation()
                .unwrap()
                .content_decoded_body()
                .as_slice(),
            body
        );
        assert!(
            runtime
                .candidate_dependencies()
                .unwrap()
                .entries()
                .iter()
                .any(|entry| entry.error().is_some())
        );
        let completion = block_on(
            runtime
                .request(
                    RequestReason::ExplicitReload,
                    server.source("/missing.css"),
                    limits,
                )
                .unwrap(),
        );
        PANIC_NEXT.store(true, Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.finish(completion)))
                .is_err()
        );
        assert_eq!(runtime.active_revision(), revision);
        assert_eq!(
            runtime.active_dependencies().unwrap().entries()[0]
                .representation()
                .unwrap()
                .content_decoded_body()
                .as_slice(),
            body
        );
        assert_eq!(runtime.candidate_dependencies().unwrap().entries().len(), 1);
        assert!(
            runtime.candidate_dependencies().unwrap().entries()[0]
                .error()
                .is_some()
        );
        assert_eq!(
            runtime.configuration().unwrap().source().uri,
            server.source("/missing.css").uri
        );
        assert_eq!(server.count("/missing.css"), 1);
    }
}

fn http_message_reports_precede_logger_panics() {
    use iced::futures::executor::block_on;
    use iced_css::{
        host::stylesheet::{http_coordinator::*, http_graph::HttpRuntimeLimits},
        resources::{watch_work::WatchWorkLimits, worker::FileReader},
    };
    use iced_css_core::{
        resources::{publication::PublicationStatus, triggers::*},
        stylesheet::{identities::IdentityLimits, located_graph::BaseLimits},
    };
    use std::sync::atomic::Ordering;
    for mode in [RuntimeMode::Manual, RuntimeMode::Auto] {
        let server = http_fixture::Server::new();
        server.state.lock().unwrap().warning = true;
        let acquisition = http_fixture::settings(
            intake_limits(),
            graph_limits().expansion.tracking,
            BaseLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
            },
        );
        let config = HttpStylesheetHostConfig {
            mode,
            context: context(),
            publication: PublicationLimits {
                requests: NonZeroU64::new(20).unwrap(),
                revisions: NonZeroU64::new(20).unwrap(),
            },
            diagnostic_work: n(),
            identities: IdentityLimits {
                entries: n(),
                bytes: n(),
                work: n(),
            },
            runtime: HttpRuntimeLimits {
                acquisition,
                graph: graph_limits(),
            },
            work: WatchWorkLimits {
                slots: NonZeroUsize::MIN,
                retained_bytes: n(),
            },
        };
        let mut host = HttpStylesheetHost::new(
            FileReader::new(NonZeroUsize::MIN),
            http_fixture::transport(),
            config,
        )
        .unwrap();
        let message = block_on(
            host.request(RequestReason::FirstAcquisition, server.source("/start"))
                .unwrap(),
        );
        let reload = host.reload_message();
        assert!(host.update(reload).work.is_none());
        PANIC_NEXT.store(true, Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.update(message)))
                .is_err()
        );
        assert_eq!(host.status(), PublicationStatus::Ready);
        assert!(!host.is_busy() && host.is_dirty());
        assert_eq!(host.pending_work().unwrap(), 0);
        let revision = host.active_revision();
        let body = host.active_dependencies().unwrap().entries()[0]
            .representation()
            .unwrap()
            .content_decoded_body()
            .as_slice()
            .to_vec();
        assert!(host.candidate_dependencies().is_none());
        {
            let mut state = server.state.lock().unwrap();
            state.warning = false;
            state.blue = true;
            state.missing = true;
        }
        let reload = host.reload_message();
        let message = block_on(host.update(reload).work.unwrap());
        let reload = host.reload_message();
        assert!(host.update(reload).work.is_none());
        PANIC_NEXT.store(true, Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.update(message)))
                .is_err()
        );
        assert!(!host.is_busy() && host.is_dirty());
        assert_eq!(host.pending_work().unwrap(), 0);
        assert_eq!(host.active_revision(), revision);
        assert_eq!(
            host.active_dependencies().unwrap().entries()[0]
                .representation()
                .unwrap()
                .content_decoded_body()
                .as_slice(),
            body
        );
        assert!(
            host.candidate_dependencies()
                .unwrap()
                .entries()
                .iter()
                .any(|entry| entry.error().is_some())
        );
        let message = block_on(
            host.request(RequestReason::ExplicitReload, server.source("/missing.css"))
                .unwrap(),
        );
        PANIC_NEXT.store(true, Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.update(message)))
                .is_err()
        );
        assert!(!host.is_busy() && !host.is_dirty());
        assert_eq!(host.pending_work().unwrap(), 0);
        assert_eq!(host.active_revision(), revision);
        assert_eq!(
            host.active_dependencies().unwrap().entries()[0]
                .representation()
                .unwrap()
                .content_decoded_body()
                .as_slice(),
            body
        );
        assert_eq!(host.candidate_dependencies().unwrap().entries().len(), 1);
        assert!(
            host.candidate_dependencies().unwrap().entries()[0]
                .error()
                .is_some()
        );
    }
}
