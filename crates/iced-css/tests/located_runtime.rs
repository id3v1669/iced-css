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
use iced_css_core::{resources::uri::UriError, stylesheet::located_graph::*};
fn meta_limits() -> BaseLimits {
    BaseLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn bases() -> Vec<StylesheetBase> {
    vec![
        StylesheetBase {
            resource: id(2),
            effective_base: "https://cdn.example/assets/child.css?v=2".into(),
        },
        StylesheetBase {
            resource: id(1),
            effective_base: "https://cdn.example/assets/themes/root.css?v=1".into(),
        },
    ]
}
use iced_css::{
    host::stylesheet::graph::{GraphLoadError, LocatedGraphInputs, RuntimeLocatedStylesheetGraph},
    resources::worker::FileReader,
};
fn inputs(root: &str, child: &str) -> LocatedGraphInputs {
    LocatedGraphInputs {
        resources: vec![
            resource(1, root, vec![link(0, 1)]),
            resource(2, child, vec![]),
        ],
        bases: bases(),
    }
}
#[test]
fn runtime_worker_publishes_source_and_bases_together_and_rejects_invalid_replacement() {
    let mut owner = RuntimeLocatedStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
        n(),
    );
    let reader = FileReader::new(NonZeroUsize::MIN);
    let first = owner.request(0, context(), graph_limits()).unwrap();
    let work = reader.load_located_stylesheet_graph(first, meta_limits(), || {
        Ok::<_, &str>(inputs(
            "@import \"../child.css\"; r{color:red}",
            "a{margin:1px}",
        ))
    });
    let done = iced::futures::executor::block_on(work);
    assert!(owner.active().is_none());
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Published(_)
    ));
    let revision = owner.active_revision();
    let next = owner.request(0, context(), graph_limits()).unwrap();
    let work = reader.load_located_stylesheet_graph(next, meta_limits(), || {
        let mut input = inputs("@import \"../child.css\"; r{color:blue}", "a{margin:2px}");
        input.bases[0].effective_base = "relative.css".into();
        Ok::<_, &str>(input)
    });
    let done = iced::futures::executor::block_on(work);
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Bases(LocatedGraphError::Uri {
            error: UriError::RelativeBase,
            ..
        }))
    ));
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().graph().resources()[0]
            .candidate
            .source()
            .text(),
        "@import \"../child.css\"; r{color:red}"
    );
    assert_eq!(
        owner.active().unwrap().effective_base(id(2), n()).unwrap(),
        Some("https://cdn.example/assets/child.css?v=2")
    );
    let next = owner.request(0, context(), graph_limits()).unwrap();
    let mut input = inputs("@import \"../child.css\"; r{color:blue}", "a{margin:2px}");
    input.bases[0].effective_base = "https://new.example/styles/child.css".into();
    let work = reader.load_located_stylesheet_graph(next, meta_limits(), || Ok::<_, &str>(input));
    let done = iced::futures::executor::block_on(work);
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(
        owner
            .active()
            .unwrap()
            .find_declaration(NonZeroU64::new(20).unwrap(), n(), n())
            .unwrap()
            .unwrap()
            .effective_base,
        "https://new.example/styles/child.css"
    );
}
#[test]
fn located_worker_admission_and_import_failure_preserve_owner_policy() {
    let mut owner = RuntimeLocatedStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
        n(),
    );
    let reader = FileReader::new(NonZeroUsize::MIN);
    let old = owner.request(0, context(), graph_limits()).unwrap();
    let older = reader.load_located_stylesheet_graph(old, meta_limits(), || {
        Ok::<_, &str>(inputs("@import \"child\"; r{color:red}", "a{margin:1px}"))
    });
    let next = owner.request(0, context(), graph_limits()).unwrap();
    let latest = reader.load_located_stylesheet_graph(
        next,
        meta_limits(),
        || -> Result<LocatedGraphInputs, &str> { panic!("rejected loader must not run") },
    );
    let done = iced::futures::executor::block_on(latest);
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Failed(_)
    ));
    let done = iced::futures::executor::block_on(older);
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(owner.active().is_none());
    assert_eq!(reader.in_flight(), 0);
    let next = owner.request(0, context(), graph_limits()).unwrap();
    let done = next.assemble_located(
        Ok::<_, &str>(inputs("@import \"child\"; r{color:red}", "a{margin:1px}")),
        meta_limits(),
    );
    owner.finish(done).unwrap();
    let revision = owner.active_revision();
    let next = owner.request(0, context(), graph_limits()).unwrap();
    let done = next.assemble_located(
        Err::<LocatedGraphInputs, _>("failed import reload"),
        meta_limits(),
    );
    assert!(matches!(
        owner.finish(done).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition("failed import reload"))
    ));
    assert_eq!(owner.active_revision(), revision);
}

#[test]
fn request_bound_cycle_contexts_publish_and_fail_atomically() {
    let mut runtime = RuntimeLocatedStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
        n(),
    );
    let make = || LocatedGraphInputs {
        resources: vec![
            resource(1, "@import 'root.css'; r{color:red}", vec![link(0, 1)]),
            resource(2, "a{width:1px}", vec![]),
        ],
        bases: vec![
            StylesheetBase {
                resource: id(1),
                effective_base: "https://host/root.css".into(),
            },
            StylesheetBase {
                resource: id(2),
                effective_base: "https://host/root.css".into(),
            },
        ],
    };
    let key = ImportCycleId::new(NonZeroU64::new(50).unwrap());
    let request = runtime.request(0, context(), graph_limits()).unwrap();
    let completion = request.assemble_located_with_cycles(
        Ok::<_, std::io::Error>(make()),
        &[key, key],
        meta_limits(),
    );
    assert!(matches!(
        runtime.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(runtime.active().unwrap().graph().rules().count(), 1);
    assert_eq!(
        runtime.active().unwrap().graph().cuts()[0].target,
        Some(id(2))
    );
    let revision = runtime.active_revision();
    let request = runtime.request(0, context(), graph_limits()).unwrap();
    let completion = request.assemble_located_with_cycles(
        Ok::<_, std::io::Error>(make()),
        &[key],
        meta_limits(),
    );
    assert!(matches!(
        runtime.finish(completion).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Assembly(OwnedGraphError::Expansion(
            LoadedGraphError::CycleIdentities
        )))
    ));
    assert_eq!(runtime.active_revision(), revision);
    assert_eq!(
        runtime
            .active()
            .unwrap()
            .effective_base(id(1), n())
            .unwrap(),
        Some("https://host/root.css")
    );
    assert_eq!(runtime.active().unwrap().graph().rules().count(), 1);
}

#[test]
fn worker_cycle_metadata_preserves_publication_failure_and_admission_rules() {
    use iced_css::host::stylesheet::graph::CycleLocatedGraphInputs;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let mut owner = RuntimeLocatedStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
        n(),
    );
    let reader = FileReader::new(NonZeroUsize::MIN);
    let make = |count| {
        let key = ImportCycleId::new(NonZeroU64::new(50).unwrap());
        let mut graph = inputs("@import 'root.css'; r{color:red}", "a{width:1px}");
        for base in &mut graph.bases {
            base.effective_base = "https://host/root.css".into();
        }
        CycleLocatedGraphInputs {
            graph,
            cycles: ImportCycleTable::new(vec![key; count], n()).unwrap(),
        }
    };
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let work =
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), move || {
            Ok::<_, std::io::Error>(make(2))
        });
    let completion = iced::futures::executor::block_on(work);
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().graph().rules().count(), 1);
    let revision = owner.active_revision();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let work =
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), move || {
            Ok::<_, std::io::Error>(make(1))
        });
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(work))
            .unwrap(),
        SheetCompletion::Failed(GraphLoadError::Assembly(OwnedGraphError::Expansion(
            LoadedGraphError::CycleIdentities
        )))
    ));
    assert_eq!(owner.active_revision(), revision);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let older =
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), move || {
            Ok::<_, std::io::Error>(make(2))
        });
    let called = Arc::new(AtomicBool::new(false));
    let seen = called.clone();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let rejected =
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), move || {
            seen.store(true, Ordering::SeqCst);
            Ok::<_, std::io::Error>(make(2))
        });
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(rejected))
            .unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert!(!called.load(Ordering::SeqCst));
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(older))
            .unwrap(),
        SheetCompletion::Ignored
    ));
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(reader.in_flight(), 0);
}

#[test]
fn worker_can_publish_an_unfetched_cycle_but_cannot_omit_a_noncyclic_target() {
    use iced_css::host::stylesheet::graph::CycleLocatedGraphInputs;
    let mut owner = RuntimeLocatedStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
        n(),
    );
    let reader = FileReader::new(NonZeroUsize::MIN);
    let make = |target| {
        let key = ImportCycleId::new(NonZeroU64::new(50).unwrap());
        CycleLocatedGraphInputs {
            graph: LocatedGraphInputs {
                resources: vec![resource(
                    1,
                    "@import 'self'; r{color:red}",
                    vec![ImportBinding {
                        entry: 0,
                        resolution: ImportResolution::DeferredCycle(ImportCycleId::new(
                            NonZeroU64::new(target).unwrap(),
                        )),
                    }],
                )],
                bases: vec![StylesheetBase {
                    resource: id(1),
                    effective_base: "https://host/self".into(),
                }],
            },
            cycles: ImportCycleTable::new(vec![key], n()).unwrap(),
        }
    };
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let work =
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), move || {
            Ok::<_, std::io::Error>(make(50))
        });
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(work))
            .unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().graph().resources().len(), 1);
    assert_eq!(owner.active().unwrap().graph().cuts()[0].target, None);
    let revision = owner.active_revision();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let work =
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), move || {
            Ok::<_, std::io::Error>(make(51))
        });
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(work))
            .unwrap(),
        SheetCompletion::Failed(GraphLoadError::Assembly(OwnedGraphError::Expansion(
            LoadedGraphError::UnresolvedImport { sheet: 0, entry: 0 }
        )))
    ));
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(owner.active().unwrap().graph().resources().len(), 1);
}

#[test]
fn collected_local_imports_publish_on_the_worker_and_retain_the_whole_failed_reload() {
    use iced_css::resources::{
        graph::GraphWorkerError,
        local::{ReadBodyLimits, read_file_blocking},
    };
    use iced_css_core::{
        resources::body::BodyLimits,
        stylesheet::{collector::*, requests::ImportRequestLimits},
    };
    use std::{
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let temp = Temp(std::env::temp_dir().join(format!(
        "iced-css-import-collector-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir(&temp.0).unwrap();
    let child = temp.0.join("child.css");
    let encode = |text: &str| {
        text.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    std::fs::write(&child, encode("@import 'root.css'; a{margin:1px}")).unwrap();
    let reads = Arc::new(AtomicUsize::new(0));
    struct Provider {
        child: PathBuf,
        reads: Arc<AtomicUsize>,
    }
    impl ImportProvider for Provider {
        type Error = std::io::Error;
        fn identify(&mut self, probe: ImportProbe<'_>) -> Result<ImportTarget, Self::Error> {
            let value = match probe.request.acquisition_uri() {
                "https://assets.invalid/root.css" => 1,
                "https://assets.invalid/child.css" => 2,
                _ => return Err(std::io::Error::other("unexpected URI")),
            };
            Ok(ImportTarget {
                source: id(value),
                cycle: ImportCycleId::new(NonZeroU64::new(value * 100).unwrap()),
            })
        }
        fn acquire(
            &mut self,
            target: ImportTarget,
            probe: ImportProbe<'_>,
        ) -> Result<Option<AcquiredStylesheet>, Self::Error> {
            assert_eq!(target.source, id(2));
            assert_eq!(probe.encoding.referring, Some(b"UTF-16LE".as_slice()));
            self.reads.fetch_add(1, Ordering::Relaxed);
            let body = read_file_blocking(
                &self.child,
                ReadBodyLimits {
                    reads: n(),
                    body: BodyLimits {
                        retained_bytes: n(),
                        work: n(),
                    },
                },
            )
            .map_err(std::io::Error::other)?;
            let candidate = parse_separate_unicode_stylesheet(
                body.as_slice(),
                probe.encoding,
                context(),
                intake_limits(),
            )
            .map_err(std::io::Error::other)?;
            Ok(Some(AcquiredStylesheet {
                candidate,
                effective_base: "https://assets.invalid/child.css".into(),
            }))
        }
    }
    let root = |css: &str| CollectorRoot {
        target: ImportTarget {
            source: id(1),
            cycle: ImportCycleId::new(NonZeroU64::new(100).unwrap()),
        },
        input: AcquiredStylesheet {
            candidate: parse_separate_unicode_stylesheet(
                &encode(css),
                EncodingMetadata {
                    transport: Some(b"UTF-16LE"),
                    ..Default::default()
                },
                context(),
                intake_limits(),
            )
            .unwrap(),
            effective_base: "https://assets.invalid/root.css".into(),
        },
    };
    let collector_limits = CollectorLimits {
        entries: n(),
        record_bytes: n(),
        work: n(),
        requests: ImportRequestLimits {
            entries: n(),
            retained_bytes: n(),
            work: n(),
        },
        tracking: graph_limits().expansion.tracking,
        cycle_bytes: n(),
    };
    let mut owner = RuntimeLocatedStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
        n(),
    );
    let reader = FileReader::new(NonZeroUsize::MIN);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let input = root("@import 'child.css'; r{color:red}");
    let mut provider = Provider {
        child: child.clone(),
        reads: reads.clone(),
    };
    let work =
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), move || {
            collect_imports(input, &mut provider, collector_limits).map(Into::into)
        });
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(work))
            .unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(
        owner.active().unwrap().graph().cuts()[0].reason,
        ImportCutReason::Cycle
    );
    let revision = owner.active_revision();
    std::fs::remove_file(&child).unwrap();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let input = root("@import 'child.css'; r{color:blue}");
    let mut provider = Provider {
        child: child.clone(),
        reads: reads.clone(),
    };
    let work =
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), move || {
            collect_imports(input, &mut provider, collector_limits).map(Into::into)
        });
    assert!(
        matches!(owner.finish(iced::futures::executor::block_on(work)).unwrap(),SheetCompletion::Failed(GraphLoadError::Acquisition(GraphWorkerError::Acquisition(CollectionError::Provider{source,..}))) if source==id(1))
    );
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().graph().resources()[0]
            .candidate
            .source()
            .text(),
        "@import 'child.css'; r{color:red}"
    );
    assert_eq!(
        owner.active().unwrap().graph().resources()[1]
            .candidate
            .source()
            .text(),
        "@import 'root.css'; a{margin:1px}"
    );
    std::fs::write(&child, encode("@import 'root.css'; a{margin:2px}")).unwrap();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let input = root("@import 'child.css'; r{color:blue}");
    let mut provider = Provider {
        child,
        reads: reads.clone(),
    };
    let work =
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), move || {
            collect_imports(input, &mut provider, collector_limits).map(Into::into)
        });
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(work))
            .unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_ne!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().graph().resources()[0]
            .candidate
            .source()
            .text(),
        "@import 'child.css'; r{color:blue}"
    );
    assert_eq!(
        owner.active().unwrap().graph().resources()[1]
            .candidate
            .source()
            .text(),
        "@import 'root.css'; a{margin:2px}"
    );
    assert_eq!(reads.load(Ordering::Relaxed), 3);
}

#[cfg(unix)]
#[test]
fn unix_provider_collects_real_file_uris_and_preserves_atomic_reloads() {
    use iced_css::resources::{
        file_imports::{FileImportLimits, UnixFileImportProvider},
        file_path::{FilePathLimits, unix_file_uri},
        file_uri_read::FileUriReadLimits,
        local::ReadBodyLimits,
    };
    use iced_css_core::{
        resources::{body::BodyLimits, uri::UriLimits},
        stylesheet::{collector::*, identities::*, requests::ImportRequestLimits},
    };
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temp =
        Temp(std::env::temp_dir().join(format!("iced-css-real-provider-{}", std::process::id())));
    std::fs::create_dir(&temp.0).unwrap();
    let root_path = temp.0.join("main %.css");
    let child_path = temp.0.join("child.css");
    let encode = |text: &str| {
        text.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    let root_text = "@import 'child.css'; r{color:red}";
    std::fs::write(&root_path, encode(root_text)).unwrap();
    let path_limits = FilePathLimits {
        bytes: n(),
        work: n(),
    };
    let uri = unix_file_uri(&root_path, path_limits).unwrap();
    let ids = Arc::new(Mutex::new(
        ImportIdentityRegistry::new(IdentityLimits {
            entries: n(),
            bytes: n(),
            work: n(),
        })
        .unwrap(),
    ));
    let file_limits = FileImportLimits {
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
        dependencies: iced_css::resources::file_dependencies::FileDependencyLimits {
            entries: n(),
            retained_bytes: n(),
            work: n(),
        },
    };
    {
        use iced_css::resources::{file_imports::FileImportError, local::ReadBodyError};
        let mut guard = ids.lock().unwrap();
        let mut provider = UnixFileImportProvider::new(&mut guard, context(), file_limits);
        assert!(matches!(
            provider.read_root(
                "https://assets.invalid/root.css",
                EncodingMetadata::default()
            ),
            Err(FileImportError::Uri(_))
        ));
        assert!(matches!(
            provider.read_root(
                &uri,
                EncodingMetadata {
                    transport: Some(b"unknown-test-charset"),
                    ..Default::default()
                }
            ),
            Err(FileImportError::Intake(_))
        ));
        let mut limited = file_limits;
        limited.file.read.body.retained_bytes = NonZeroUsize::MIN;
        let mut provider = UnixFileImportProvider::new(&mut guard, context(), limited);
        assert!(matches!(
            provider.read_root(&uri, EncodingMetadata::default()),
            Err(FileImportError::Read(ReadBodyError::Body(_)))
        ));
    }
    let collector_limits = CollectorLimits {
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
    };
    let build = move || {
        let mut ids = ids
            .lock()
            .map_err(|_| std::io::Error::other("test registry poisoned"))?;
        let mut provider = UnixFileImportProvider::new(&mut ids, context(), file_limits);
        let root = provider
            .read_root(
                &uri,
                EncodingMetadata {
                    transport: Some(b"UTF-16LE"),
                    ..Default::default()
                },
            )
            .map_err(std::io::Error::other)?;
        collect_imports(root, &mut provider, collector_limits)
            .map(Into::into)
            .map_err(std::io::Error::other)
    };
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut owner = RuntimeLocatedStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
        n(),
    );
    // First load can omit an unavailable import and publish the usable root.
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let completion = iced::futures::executor::block_on(
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), build.clone()),
    );
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    let root_id = owner.active().unwrap().graph().resources()[0].resource;
    assert_eq!(
        owner.active().unwrap().graph().cuts()[0].reason,
        ImportCutReason::Unavailable
    );
    // Restore the child; its import of the real encoded root URI is a cycle.
    std::fs::write(
        &child_path,
        encode("@import 'main%20%25.css'; a{margin:1px}"),
    )
    .unwrap();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let completion = iced::futures::executor::block_on(
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), build.clone()),
    );
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    let active = owner.active().unwrap().graph();
    assert_eq!(active.resources()[0].resource, root_id);
    assert_eq!(active.resources().len(), 2);
    assert_eq!(active.cuts()[0].reason, ImportCutReason::Cycle);
    assert_eq!(
        active.resources()[1].candidate.source().text(),
        "@import 'main%20%25.css'; a{margin:1px}"
    );
    let revision = owner.active_revision();
    std::fs::remove_file(&child_path).unwrap();
    std::fs::write(&root_path, encode("@import 'child.css'; r{color:blue}")).unwrap();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let completion = iced::futures::executor::block_on(
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), build.clone()),
    );
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().graph().resources()[0]
            .candidate
            .source()
            .text(),
        root_text
    );
    assert_eq!(
        owner.active().unwrap().graph().resources()[1]
            .candidate
            .source()
            .text(),
        "@import 'main%20%25.css'; a{margin:1px}"
    );
    std::fs::remove_file(&root_path).unwrap();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let completion = iced::futures::executor::block_on(
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), build.clone()),
    );
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(owner.active_revision(), revision);
    std::fs::write(&root_path, []).unwrap();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let completion = iced::futures::executor::block_on(
        reader.load_located_stylesheet_graph_with_cycles(request, meta_limits(), build),
    );
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().graph().rules().count(), 0);
}

#[cfg(unix)]
#[test]
fn owned_file_graph_routes_requests_without_a_second_publication_gate() {
    use iced_css::host::stylesheet::file_graph::*;
    use iced_css::resources::{
        file_imports::FileImportLimits,
        file_path::{FilePathLimits, unix_file_uri},
        file_uri_read::FileUriReadLimits,
        local::ReadBodyLimits,
        stylesheet::StylesheetEncoding,
    };
    use iced_css_core::{
        resources::{
            body::BodyLimits,
            triggers::{RequestError, RequestReason, RuntimeMode},
            uri::UriLimits,
        },
        stylesheet::{
            collector::CollectorLimits, identities::IdentityLimits, requests::ImportRequestLimits,
        },
    };
    use std::path::PathBuf;
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temp = Temp(
        std::env::temp_dir().join(format!("iced-css-file-graph-owner-{}", std::process::id())),
    );
    std::fs::create_dir(&temp.0).unwrap();
    let path = temp.0.join("root.css");
    let child = temp.0.join("child.css");
    std::fs::write(&path, b"@import 'child.css'; r{color:red}").unwrap();
    std::fs::write(&child, b"a{margin:1px}").unwrap();
    let path_limits = FilePathLimits {
        bytes: n(),
        work: n(),
    };
    let uri = unix_file_uri(&path, path_limits).unwrap();
    let source = || FileGraphSource {
        uri: uri.clone(),
        encoding: StylesheetEncoding::default(),
    };
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
            dependencies: iced_css::resources::file_dependencies::FileDependencyLimits {
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
        bases: meta_limits(),
        metadata_bytes: n(),
    };
    let make = |mode| {
        RuntimeFileStylesheetGraph::new(
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
        .unwrap()
    };
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut owner = make(RuntimeMode::Manual);
    for reason in [
        RequestReason::LocalChange,
        RequestReason::CacheExpiry,
        RequestReason::CacheEviction,
        RequestReason::Reevaluation,
        RequestReason::EmbeddedNotification,
    ] {
        assert!(matches!(
            owner.request(&reader, reason, source(), settings),
            Err(RequestError::Unauthorized)
        ));
    }
    assert_eq!(
        owner.status(),
        iced_css_core::resources::publication::PublicationStatus::Unrequested
    );
    let work = owner
        .request(&reader, RequestReason::FirstAcquisition, source(), settings)
        .unwrap();
    fn owned_send<T: Send + 'static>(_: &T) {}
    owned_send(&work);
    assert!(matches!(
        owner.request(&reader, RequestReason::FirstAcquisition, source(), settings),
        Err(RequestError::Unauthorized)
    ));
    let complete = iced::futures::executor::block_on(work);
    assert!(matches!(
        owner.finish(complete).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().graph().resources().len(), 2);
    assert_eq!(owner.active_dependencies().unwrap().entries().len(), 2);
    assert!(owner.candidate_dependencies().is_none());
    let revision = owner.active_revision();
    std::fs::write(temp.0.join("older.css"), b"a{margin:3px}").unwrap();
    std::fs::write(&path, b"@import 'older.css'; r{color:blue}").unwrap();
    // A successful completion waiting in the app queue becomes obsolete after B.
    let work = owner
        .request(&reader, RequestReason::ExplicitReload, source(), settings)
        .unwrap();
    let older = iced::futures::executor::block_on(work);
    assert!(owner.is_current(&older));
    assert!(
        owner
            .pending_dependencies(&older)
            .unwrap()
            .entries()
            .iter()
            .any(|entry| entry.native_path() == temp.0.join("older.css"))
    );

    std::fs::remove_file(&child).unwrap();
    std::fs::write(&path, b"@import 'child.css'; r{color:blue}").unwrap();
    let latest = iced::futures::executor::block_on(
        owner
            .request(&reader, RequestReason::ExplicitReload, source(), settings)
            .unwrap(),
    );
    assert!(!owner.is_current(&older));
    assert!(owner.pending_dependencies(&older).is_none());
    assert!(owner.is_current(&latest));
    assert!(owner.pending_dependencies(&latest).is_some());
    assert!(matches!(
        owner.finish(latest).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert!(matches!(
        owner.finish(older).unwrap(),
        SheetCompletion::Ignored
    ));
    let report = owner.candidate_dependencies().unwrap();
    assert!(
        report
            .entries()
            .iter()
            .any(|entry| entry.native_path() == child)
    );
    assert!(
        !report
            .entries()
            .iter()
            .any(|entry| entry.native_path() == temp.0.join("older.css"))
    );
    assert_eq!(owner.active_dependencies().unwrap().entries().len(), 2);

    assert_eq!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().graph().resources()[0]
            .candidate
            .source()
            .text(),
        "@import 'child.css'; r{color:red}"
    );
    let mut oversized = source();
    oversized.uri.reserve(n().get() * 2);
    let failed = owner
        .request(&reader, RequestReason::ExplicitReload, oversized, settings)
        .unwrap();
    assert_eq!(reader.in_flight(), 0);
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(failed))
            .unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(owner.active_revision(), revision);
    std::fs::write(&path, []).unwrap();
    let work = owner
        .request(&reader, RequestReason::ExplicitReload, source(), settings)
        .unwrap();
    let foreign = iced::futures::executor::block_on(work);
    let mut auto = make(RuntimeMode::Auto);
    assert!(!auto.is_current(&foreign));
    assert!(auto.pending_dependencies(&foreign).is_none());

    assert!(matches!(
        auto.finish(foreign).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(auto.active().is_none());
    assert!(auto.active_dependencies().is_none());
    assert!(auto.candidate_dependencies().is_none());
    let work = auto
        .request(&reader, RequestReason::LocalChange, source(), settings)
        .unwrap();
    assert!(matches!(
        auto.finish(iced::futures::executor::block_on(work))
            .unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(auto.active().unwrap().graph().rules().count(), 0);
    let work = owner
        .request(&reader, RequestReason::ExplicitReload, source(), settings)
        .unwrap();
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(work))
            .unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().graph().rules().count(), 0);
    use iced_css::resources::{
        graph::GraphWorkerError,
        worker::{BackgroundReadLimits, SubmitError},
    };
    let revision = owner.active_revision();
    let blocker = reader
        .read_file(
            path.clone(),
            BackgroundReadLimits {
                path_bytes: n(),
                read: settings.file.file.read,
            },
        )
        .unwrap();
    let rejected = owner
        .request(&reader, RequestReason::ExplicitReload, source(), settings)
        .unwrap();
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(rejected))
            .unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition(GraphWorkerError::Submit(
            SubmitError::Limit
        )))
    ));
    assert_eq!(owner.active_revision(), revision);
    iced::futures::executor::block_on(blocker).unwrap();
    assert_eq!(reader.in_flight(), 0);
    let mut oversized = source();
    oversized.encoding.transport = Some(Vec::with_capacity(n().get() * 2));
    let rejected = owner
        .request(&reader, RequestReason::ExplicitReload, oversized, settings)
        .unwrap();
    assert_eq!(reader.in_flight(), 0);
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(rejected))
            .unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition(GraphWorkerError::Acquisition(
            FileGraphAcquisitionError::MetadataBytes
        )))
    ));
    assert_eq!(owner.active_revision(), revision);
    let mut once = RuntimeFileStylesheetGraph::new(
        RuntimeMode::Manual,
        context(),
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        n(),
        IdentityLimits {
            entries: n(),
            bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    let work = once
        .request(&reader, RequestReason::FirstAcquisition, source(), settings)
        .unwrap();
    once.finish(iced::futures::executor::block_on(work))
        .unwrap();
    assert!(matches!(
        once.request(&reader, RequestReason::ExplicitReload, source(), settings),
        Err(RequestError::Exhausted(_))
    ));
    assert_eq!(reader.in_flight(), 0);
    let mut limited = settings;
    limited.file.dependencies.retained_bytes = NonZeroUsize::MIN;
    let work = owner
        .request(&reader, RequestReason::ExplicitReload, source(), limited)
        .unwrap();
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(work))
            .unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(
        owner.candidate_dependencies().unwrap().rejection(),
        Some(iced_css::resources::file_dependencies::FileDependencyError::Bytes)
    );
    assert_eq!(owner.active_dependencies().unwrap().entries().len(), 1);
}

#[cfg(unix)]
#[test]
fn file_dependency_report_retains_missing_paths_and_rejects_incomplete_storage() {
    use iced_css::resources::{
        file_dependencies::*,
        file_imports::*,
        file_path::{FilePathLimits, unix_file_uri},
        file_uri_read::FileUriReadLimits,
        local::ReadBodyLimits,
    };
    use iced_css_core::{
        resources::{body::BodyLimits, uri::UriLimits},
        stylesheet::{collector::*, identities::*, requests::ImportRequestLimits},
    };
    use std::path::PathBuf;
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temp = Temp(
        std::env::temp_dir().join(format!("iced-css-file-dependencies-{}", std::process::id())),
    );
    std::fs::create_dir(&temp.0).unwrap();
    let path = temp.0.join("root.css");
    let path_limits = FilePathLimits {
        bytes: n(),
        work: n(),
    };
    let uri = unix_file_uri(&path, path_limits).unwrap();
    let settings = FileImportLimits {
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
    };
    // Root metadata is available without a reader or identity registry, including
    // a missing file. It must use the provider's exact URI/path conversion.
    let (root_result, root_report) = root_file_dependencies(&uri, settings);
    root_result.unwrap();
    assert_eq!(root_report.entries().len(), 1);
    assert_eq!(root_report.entries()[0].acquisition_uri(), uri);
    assert_eq!(root_report.entries()[0].native_path(), path);
    let (invalid, invalid_report) = root_file_dependencies("https://example.test/x.css", settings);
    assert!(matches!(invalid, Err(FileImportError::Uri(_))));
    assert!(invalid_report.entries().is_empty());
    let (bounded, bounded_report) = root_file_dependencies(
        &uri,
        FileImportLimits {
            dependencies: FileDependencyLimits {
                retained_bytes: std::num::NonZeroUsize::MIN,
                ..settings.dependencies
            },
            ..settings
        },
    );
    assert!(matches!(
        bounded,
        Err(FileImportError::Dependencies(FileDependencyError::Bytes))
    ));
    assert_eq!(bounded_report.rejection(), Some(FileDependencyError::Bytes));
    let mut ids = ImportIdentityRegistry::new(IdentityLimits {
        entries: n(),
        bytes: n(),
        work: n(),
    })
    .unwrap();
    let mut provider = UnixFileImportProvider::new(&mut ids, context(), settings);
    assert!(
        provider
            .read_root(&uri, EncodingMetadata::default())
            .is_err()
    );
    let missing = provider.into_dependencies();
    assert_eq!(missing.entries().len(), 1);
    assert_eq!(missing.entries()[0].acquisition_uri(), uri);
    assert_eq!(missing.entries()[0].native_path(), path);
    assert!(missing.rejection().is_none());
    std::fs::write(
        &path,
        b"@import 'missing%20child.css'; @import 'missing%20child.css'; r{color:red}",
    )
    .unwrap();
    let collector = CollectorLimits {
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
    };
    let mut provider = UnixFileImportProvider::new(&mut ids, context(), settings);
    let root = provider
        .read_root(&uri, EncodingMetadata::default())
        .unwrap();
    collect_imports(root, &mut provider, collector).unwrap();
    let report = provider.into_dependencies();
    assert_eq!(report.entries().len(), 2);
    assert_eq!(
        report.entries()[1].native_path(),
        temp.0.join("missing child.css")
    );
    assert!(report.rejection().is_none());
    assert!(report.retained_bytes() <= settings.dependencies.retained_bytes.get());
    for dependencies in [
        FileDependencyLimits {
            entries: NonZeroUsize::MIN,
            ..settings.dependencies
        },
        FileDependencyLimits {
            retained_bytes: NonZeroUsize::MIN,
            ..settings.dependencies
        },
        FileDependencyLimits {
            work: NonZeroUsize::MIN,
            ..settings.dependencies
        },
    ] {
        let mut provider = UnixFileImportProvider::new(
            &mut ids,
            context(),
            FileImportLimits {
                dependencies,
                ..settings
            },
        );
        let failed = match provider.read_root(&uri, EncodingMetadata::default()) {
            Ok(root) => collect_imports(root, &mut provider, collector).is_err(),
            Err(_) => true,
        };
        assert!(failed);
        let report = provider.into_dependencies();
        assert!(report.rejection().is_some());
        assert!(report.retained_bytes() <= dependencies.retained_bytes.get());
    }
    use iced_css::host::stylesheet::file_graph::*;
    use iced_css_core::resources::triggers::{RequestReason, RuntimeMode};
    let mut owner = RuntimeFileStylesheetGraph::new(
        RuntimeMode::Auto,
        context(),
        PublicationLimits {
            requests: NonZeroU64::new(3).unwrap(),
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
    let reader = FileReader::new(NonZeroUsize::MIN);
    let graph_settings = FileGraphLimits {
        file: settings,
        collector,
        graph: graph_limits(),
        bases: meta_limits(),
        metadata_bytes: n(),
    };
    let source = || FileGraphSource {
        uri: uri.clone(),
        encoding: Default::default(),
    };
    let work = owner
        .request(
            &reader,
            RequestReason::FirstAcquisition,
            source(),
            graph_settings,
        )
        .unwrap();
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(work))
            .unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().graph().resources().len(), 1);
    assert_eq!(owner.active_dependencies().unwrap().entries().len(), 2);
    std::fs::write(temp.0.join("missing child.css"), b"a{margin:2px}").unwrap();
    let work = owner
        .request(
            &reader,
            RequestReason::LocalChange,
            source(),
            graph_settings,
        )
        .unwrap();
    assert!(matches!(
        owner
            .finish(iced::futures::executor::block_on(work))
            .unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().graph().resources().len(), 2);
    assert_eq!(owner.active().unwrap().graph().rules().count(), 3);
    assert_eq!(owner.active_dependencies().unwrap().entries().len(), 2);
}

#[test]
fn report_worker_failures_do_not_fabricate_dependency_evidence() {
    use iced_css::host::stylesheet::graph::CycleLocatedGraphInputs;
    use iced_css::resources::{graph::GraphWorkerError, worker::WorkerFailure};
    let mut owner = RuntimeLocatedStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        n(),
    );
    let reader = FileReader::new(NonZeroUsize::MIN);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let work = reader.load_located_stylesheet_graph_with_report(
        request,
        meta_limits(),
        || -> (Result<CycleLocatedGraphInputs, std::io::Error>, usize) {
            panic!("worker report probe")
        },
    );
    let (completion, report) = iced::futures::executor::block_on(work);
    assert!(report.is_none());
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition(GraphWorkerError::Worker(
            WorkerFailure::Panicked
        )))
    ));
    assert_eq!(reader.in_flight(), 0);
}

#[cfg(unix)]
#[test]
fn directory_bindings_cover_active_and_candidate_reports_with_stable_ids() {
    use iced_css::resources::{
        file_dependencies::*,
        file_imports::*,
        file_path::{FilePathLimits, unix_file_uri},
        file_uri_read::FileUriReadLimits,
        local::ReadBodyLimits,
        watch_paths::*,
    };
    use iced_css_core::{
        resources::{body::BodyLimits, uri::UriLimits, watch_ownership::*},
        stylesheet::identities::*,
    };
    use std::{
        os::unix::ffi::OsStringExt,
        path::{Path, PathBuf},
    };
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temp =
        Temp(std::env::temp_dir().join(format!("iced-css-watch-paths-{}", std::process::id())));
    std::fs::create_dir(&temp.0).unwrap();
    let one = temp.0.join("one space");
    let two = temp
        .0
        .join(std::ffi::OsString::from_vec(b"raw-\xff".to_vec()));
    std::fs::create_dir(&one).unwrap();
    std::fs::create_dir(&two).unwrap();
    fn report(path: &Path, dependencies: FileDependencyLimits) -> FileDependencyReport {
        let path_limits = FilePathLimits {
            bytes: n(),
            work: n(),
        };
        let uri = unix_file_uri(path, path_limits).unwrap();
        let mut ids = ImportIdentityRegistry::new(IdentityLimits {
            entries: n(),
            bytes: n(),
            work: n(),
        })
        .unwrap();
        let mut provider = UnixFileImportProvider::new(
            &mut ids,
            context(),
            FileImportLimits {
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
                dependencies,
            },
        );
        assert!(
            provider
                .read_root(&uri, EncodingMetadata::default())
                .is_err()
        );
        provider.into_dependencies()
    }
    let trace = FileDependencyLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    };
    let a = report(&one.join("a.css"), trace);
    let b = report(&one.join("b.css"), trace);
    let c = report(&two.join("c.css"), trace);
    let registry_limits = WatchPathRegistryLimits {
        directories: n(),
        retained_bytes: n(),
    };
    let mapping_limits = WatchPathLimits {
        reports: n(),
        dependencies: n(),
        retained_bytes: n(),
        work: n(),
    };
    let mut registry = WatchPathRegistry::new(registry_limits).unwrap();
    let first = registry.map_reports(&[&a, &b], mapping_limits).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(
        registry.bindings(&first, mapping_limits).unwrap()[0].directory,
        one
    );
    let second = registry.map_reports(&[&c], mapping_limits).unwrap();
    assert_ne!(first, second);
    assert_eq!(registry.map_reports(&[&a], mapping_limits).unwrap(), first);
    let mut ownership = WatchOwnership::new(WatchOwnershipLimits {
        owners: n(),
        bindings: n(),
        dependencies: n(),
        retained_bytes: n(),
        work: n(),
        revisions: NonZeroU64::new(10).unwrap(),
    })
    .unwrap();
    ownership
        .commit(
            ownership
                .prepare_replace(WatchOwnerId::new(NonZeroU64::MIN), &first)
                .unwrap(),
        )
        .unwrap();
    let next = ownership
        .prepare_replace(WatchOwnerId::new(NonZeroU64::new(2).unwrap()), &second)
        .unwrap();
    let union = next.candidate_snapshot().unwrap().dependencies();
    let bindings = registry.bindings(union, mapping_limits).unwrap();
    assert_eq!(bindings.len(), 2);
    assert!(
        bindings
            .iter()
            .any(|binding| binding.directory.as_os_str() == one.as_os_str())
    );
    assert!(
        bindings
            .iter()
            .any(|binding| binding.directory.as_os_str() == two.as_os_str())
    );
    use iced_css::resources::{
        watch::WatchConfig,
        watch_sessions::{WatchMappingLimits, prepare_watch_session},
    };
    let prepared = iced::futures::executor::block_on(prepare_watch_session(
        next,
        bindings,
        WatchConfig {
            recovery_interval: std::time::Duration::from_millis(10),
            directories: n(),
            retained_path_bytes: n(),
        },
        WatchMappingLimits {
            bindings: n(),
            input_bytes: n(),
            work: n(),
        },
    ))
    .unwrap();
    let control = prepared.candidate_control().unwrap().clone();
    drop(prepared);
    assert!(control.is_shutdown_requested());
    assert!(registry.retained_bytes() <= registry_limits.retained_bytes.get());
    assert!(matches!(
        registry.bindings(&[first[0], first[0]], mapping_limits),
        Err(WatchPathError::DuplicateDependency(_))
    ));
    assert!(matches!(
        registry.bindings(
            &[WatchDependencyId::new(NonZeroU64::new(999).unwrap())],
            mapping_limits
        ),
        Err(WatchPathError::UnknownDependency(_))
    ));
    assert!(matches!(
        registry.bindings(
            &first,
            WatchPathLimits {
                retained_bytes: NonZeroUsize::MIN,
                ..mapping_limits
            }
        ),
        Err(WatchPathError::OutputBytes)
    ));
    assert!(matches!(
        registry.map_reports(
            &[&a, &c],
            WatchPathLimits {
                dependencies: NonZeroUsize::MIN,
                ..mapping_limits
            }
        ),
        Err(WatchPathError::Dependencies)
    ));
    assert!(matches!(
        registry.map_reports(
            &[&a, &b],
            WatchPathLimits {
                reports: NonZeroUsize::MIN,
                ..mapping_limits
            }
        ),
        Err(WatchPathError::Reports)
    ));
    assert!(matches!(
        registry.map_reports(
            &[&a],
            WatchPathLimits {
                retained_bytes: NonZeroUsize::MIN,
                ..mapping_limits
            }
        ),
        Err(WatchPathError::OutputBytes)
    ));
    assert!(matches!(
        registry.map_reports(
            &[&a],
            WatchPathLimits {
                work: NonZeroUsize::MIN,
                ..mapping_limits
            }
        ),
        Err(WatchPathError::Work)
    ));
    let rejected = report(
        &one.join("rejected.css"),
        FileDependencyLimits {
            retained_bytes: NonZeroUsize::MIN,
            ..trace
        },
    );
    assert!(matches!(
        registry.map_reports(&[&rejected], mapping_limits),
        Err(WatchPathError::RejectedReport { .. })
    ));
    let mut tiny = WatchPathRegistry::new(WatchPathRegistryLimits {
        directories: NonZeroUsize::MIN,
        ..registry_limits
    })
    .unwrap();
    let stable = tiny.map_reports(&[&a], mapping_limits).unwrap();
    assert!(matches!(
        tiny.map_reports(&[&c], mapping_limits),
        Err(WatchPathError::RegistryDirectories)
    ));
    assert_eq!(tiny.map_reports(&[&a], mapping_limits).unwrap(), stable);
    let mut tiny = WatchPathRegistry::new(WatchPathRegistryLimits {
        retained_bytes: NonZeroUsize::MIN,
        ..registry_limits
    })
    .unwrap();
    assert!(matches!(
        tiny.map_reports(&[&a], mapping_limits),
        Err(WatchPathError::RegistryBytes)
    ));
    assert!(
        WatchPathRegistry::new(WatchPathRegistryLimits {
            retained_bytes: NonZeroUsize::new(usize::MAX).unwrap(),
            ..registry_limits
        })
        .is_err()
    );
    assert_eq!(registry.map_reports(&[&a], mapping_limits).unwrap(), first);
}

#[cfg(unix)]
#[test]
fn file_watch_preparation_rechecks_both_generations_and_returns_retryable_input() {
    use iced_css::host::stylesheet::file_graph::*;
    use iced_css::resources::{
        file_imports::FileImportLimits,
        file_path::{FilePathLimits, unix_file_uri},
        file_uri_read::FileUriReadLimits,
        local::ReadBodyLimits,
        stylesheet::StylesheetEncoding,
    };
    use iced_css_core::{
        resources::{
            body::BodyLimits,
            triggers::{RequestReason, RuntimeMode},
            uri::UriLimits,
        },
        stylesheet::{
            collector::CollectorLimits, identities::IdentityLimits, requests::ImportRequestLimits,
        },
    };
    use std::path::PathBuf;
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temp = Temp(
        std::env::temp_dir().join(format!("iced-css-file-watch-bridge-{}", std::process::id())),
    );
    std::fs::create_dir(&temp.0).unwrap();
    let path = temp.0.join("root.css");
    let child = temp.0.join("child.css");
    std::fs::write(&path, b"@import 'child.css'; r{color:red}").unwrap();
    std::fs::write(&child, b"a{margin:1px}").unwrap();
    let path_limits = FilePathLimits {
        bytes: n(),
        work: n(),
    };
    let uri = unix_file_uri(&path, path_limits).unwrap();
    let source = || FileGraphSource {
        uri: uri.clone(),
        encoding: StylesheetEncoding::default(),
    };
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
            dependencies: iced_css::resources::file_dependencies::FileDependencyLimits {
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
        bases: meta_limits(),
        metadata_bytes: n(),
    };
    let make = |mode| {
        RuntimeFileStylesheetGraph::new(
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
        .unwrap()
    };
    let reader = FileReader::new(NonZeroUsize::MIN);

    use iced::futures::executor::block_on;
    use iced_css::host::stylesheet::file_watch::*;
    use iced_css::resources::{watch::WatchConfig, watch_paths::*, watch_sessions::*};
    use iced_css_core::resources::watch_ownership::*;
    let mut graph = make(RuntimeMode::Auto);
    let mut paths = WatchPathRegistry::new(WatchPathRegistryLimits {
        directories: n(),
        retained_bytes: n(),
    })
    .unwrap();
    let ownership_limits = WatchOwnershipLimits {
        owners: n(),
        bindings: n(),
        dependencies: n(),
        retained_bytes: n(),
        work: n(),
        revisions: NonZeroU64::new(100).unwrap(),
    };
    let mut watches = WatchSessionHost::new(ownership_limits).unwrap();
    let key = WatchOwnerId::new(NonZeroU64::MIN);
    let config = FileGraphWatchConfig {
        owner: key,
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
    };
    let load = |graph: &mut RuntimeFileStylesheetGraph| {
        block_on(
            graph
                .request(&reader, RequestReason::ExplicitReload, source(), settings)
                .unwrap(),
        )
    };
    let first = load(&mut graph);
    let prepared = block_on(graph.prepare_watch(first, &mut paths, &watches, config)).unwrap();
    let candidate = prepared.candidate_control().unwrap().clone();
    assert!(graph.active().is_none());
    assert_eq!(watches.ownership().revision(), 0);
    let result = graph.finish_watched(prepared, &mut watches).unwrap();
    assert!(matches!(
        result.sheet.unwrap(),
        SheetCompletion::Published(_)
    ));
    assert!(!candidate.is_shutdown_requested());
    let revision = graph.active_revision();

    // A completed old read cannot commit watch changes after another request.
    let old = load(&mut graph);
    let old = block_on(graph.prepare_watch(old, &mut paths, &watches, config)).unwrap();
    let current = load(&mut graph);
    let watch_revision = watches.ownership().revision();
    let result = graph.finish_watched(old, &mut watches).unwrap();
    assert!(matches!(result.sheet.unwrap(), SheetCompletion::Ignored));
    assert!(result.watch.is_none());
    assert_eq!(watches.ownership().revision(), watch_revision);
    assert_eq!(graph.active_revision(), revision);

    // Failure before registration returns exactly the still-eligible completion.
    let rejected = block_on(graph.prepare_watch(
        current,
        &mut paths,
        &watches,
        FileGraphWatchConfig {
            paths: WatchPathLimits {
                work: NonZeroUsize::MIN,
                ..config.paths
            },
            ..config
        },
    ))
    .unwrap_err();
    assert!(matches!(
        rejected.reason,
        FileGraphWatchError::Paths(WatchPathError::Work)
    ));
    assert!(graph.is_current(&rejected.completion));
    let prepared =
        block_on(graph.prepare_watch(rejected.completion, &mut paths, &watches, config)).unwrap();

    // Another owner changes the same watch registry while preparation is pending.
    let update = watches
        .ownership()
        .prepare_replace(WatchOwnerId::new(NonZeroU64::new(2).unwrap()), &[])
        .unwrap();
    let other = block_on(prepare_watch_session(
        update,
        vec![],
        config.watch,
        config.mapping,
    ))
    .unwrap();
    watches.commit(other).unwrap();
    let rejected = graph.finish_watched(prepared, &mut watches).unwrap_err();
    assert!(matches!(
        rejected.reason,
        FileGraphWatchError::Commit(WatchCommitFailure::Stale)
    ));
    assert!(graph.is_current(&rejected.completion));
    assert_eq!(graph.active_revision(), revision);
    let prepared =
        block_on(graph.prepare_watch(rejected.completion, &mut paths, &watches, config)).unwrap();
    assert!(matches!(
        graph
            .finish_watched(prepared, &mut watches)
            .unwrap()
            .sheet
            .unwrap(),
        SheetCompletion::Published(_)
    ));

    // Missing new imports are watched while the entire old graph stays active.
    let new_dir = temp.0.join("new");
    std::fs::create_dir(&new_dir).unwrap();
    std::fs::write(&path, b"@import 'new/missing.css'; r{color:blue}").unwrap();
    let revision = graph.active_revision();
    let failed = load(&mut graph);
    let prepared = block_on(graph.prepare_watch(failed, &mut paths, &watches, config)).unwrap();
    let control = prepared.candidate_control().unwrap().clone();
    let failed = graph.finish_watched(prepared, &mut watches).unwrap();
    assert!(matches!(failed.sheet.unwrap(), SheetCompletion::Failed(_)));
    assert_eq!(graph.active_revision(), revision);
    assert_eq!(watches.ownership().snapshot().dependencies().len(), 2);
    assert_eq!(graph.candidate_dependencies().unwrap().entries().len(), 2);
    assert!(!control.is_shutdown_requested());
    drop(failed.watch);
    assert!(candidate.is_shutdown_requested());

    // No report is manufactured for rejected worker-input storage.
    let invalid = block_on(
        graph
            .request(
                &reader,
                RequestReason::ExplicitReload,
                source(),
                FileGraphLimits {
                    file: FileImportLimits {
                        file: FileUriReadLimits {
                            uri: UriLimits {
                                bytes: NonZeroUsize::MIN,
                                work: n(),
                            },
                            ..settings.file.file
                        },
                        ..settings.file
                    },
                    ..settings
                },
            )
            .unwrap(),
    );
    let rejected =
        block_on(graph.prepare_watch(invalid, &mut paths, &watches, config)).unwrap_err();
    assert!(matches!(
        rejected.reason,
        FileGraphWatchError::MissingReport
    ));
    assert!(matches!(
        graph.finish(rejected.completion).unwrap(),
        SheetCompletion::Failed(_)
    ));

    // A new directory forces session creation, whose failure leaves old coverage.
    let third_dir = temp.0.join("third");
    std::fs::create_dir(&third_dir).unwrap();
    std::fs::write(&path, b"@import 'third/missing.css';").unwrap();
    let current = load(&mut graph);
    let before = watches.ownership().revision();
    let rejected = block_on(graph.prepare_watch(
        current,
        &mut paths,
        &watches,
        FileGraphWatchConfig {
            watch: WatchConfig {
                recovery_interval: std::time::Duration::ZERO,
                ..config.watch
            },
            ..config
        },
    ))
    .unwrap_err();
    assert!(matches!(rejected.reason, FileGraphWatchError::Session(_)));
    assert_eq!(watches.ownership().revision(), before);
    assert!(!control.is_shutdown_requested());
    let prepared =
        block_on(graph.prepare_watch(rejected.completion, &mut paths, &watches, config)).unwrap();
    let pending_control = prepared.candidate_control().unwrap().clone();
    let mut foreign = WatchSessionHost::new(ownership_limits).unwrap();
    let rejected = graph.finish_watched(prepared, &mut foreign).unwrap_err();
    assert!(matches!(
        rejected.reason,
        FileGraphWatchError::Commit(WatchCommitFailure::WrongRegistry)
    ));
    assert!(pending_control.is_shutdown_requested());
    assert_eq!(watches.ownership().revision(), before);
    assert!(graph.is_current(&rejected.completion));
    let newer = load(&mut graph);
    let obsolete = block_on(graph.prepare_watch(rejected.completion, &mut paths, &watches, config))
        .unwrap_err();
    assert!(matches!(obsolete.reason, FileGraphWatchError::Obsolete));
    let prepared = block_on(graph.prepare_watch(newer, &mut paths, &watches, config)).unwrap();
    let pending_control = prepared.candidate_control().unwrap().clone();
    let newer = load(&mut graph);
    assert!(matches!(
        graph
            .finish_watched(prepared, &mut watches)
            .unwrap()
            .sheet
            .unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(pending_control.is_shutdown_requested());
    assert!(graph.is_current(&newer));

    // Even an exhausted publication counter cannot remove old active paths.
    let mut once = RuntimeFileStylesheetGraph::new(
        RuntimeMode::Auto,
        context(),
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::MIN,
        },
        n(),
        IdentityLimits {
            entries: n(),
            bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    let mut once_watches = WatchSessionHost::new(ownership_limits).unwrap();
    std::fs::write(&path, b"r{color:red}").unwrap();
    let first = load(&mut once);
    let prepared = block_on(once.prepare_watch(first, &mut paths, &once_watches, config)).unwrap();
    let old_control = prepared.candidate_control().unwrap().clone();
    assert!(matches!(
        once.finish_watched(prepared, &mut once_watches)
            .unwrap()
            .sheet
            .unwrap(),
        SheetCompletion::Published(_)
    ));
    let revision = once.active_revision();
    std::fs::write(new_dir.join("missing.css"), b"a{color:blue}").unwrap();
    std::fs::write(&path, b"@import 'new/missing.css';").unwrap();
    let next = load(&mut once);
    let prepared = block_on(once.prepare_watch(next, &mut paths, &once_watches, config)).unwrap();
    let new_control = prepared.candidate_control().unwrap().clone();
    let result = once.finish_watched(prepared, &mut once_watches).unwrap();
    assert!(result.sheet.is_err());
    assert_eq!(once.active_revision(), revision);
    assert_eq!(once_watches.ownership().snapshot().dependencies().len(), 2);
    assert!(!old_control.is_shutdown_requested());
    assert!(!new_control.is_shutdown_requested());
    drop(result.watch);
    assert!(old_control.is_shutdown_requested());
    assert!(!new_control.is_shutdown_requested());
    // The coordinator can terminate a rejected preparation without accepting CSS.
    let rejected = block_on(graph.prepare_watch(
        newer,
        &mut paths,
        &watches,
        FileGraphWatchConfig {
            paths: WatchPathLimits {
                work: NonZeroUsize::MIN,
                ..config.paths
            },
            ..config
        },
    ))
    .unwrap_err();
    let old_revision = graph.active_revision();
    let outcome = graph.fail_watch(rejected).unwrap();
    assert!(
        matches!(
            outcome,
            SheetCompletion::Failed(GraphLoadError::Assembly(OwnedGraphError::Expansion(
                LoadedGraphError::UnavailableImport { sheet: 0, entry: 0 }
            )))
        ),
        "{outcome:?}"
    );
    assert_eq!(graph.active_revision(), old_revision);
    std::fs::write(&path, b"r{color:green}").unwrap();
    let current = load(&mut graph);
    let rejected = block_on(graph.prepare_watch(
        current,
        &mut paths,
        &watches,
        FileGraphWatchConfig {
            paths: WatchPathLimits {
                work: NonZeroUsize::MIN,
                ..config.paths
            },
            ..config
        },
    ))
    .unwrap_err();
    assert!(matches!(
        graph.fail_watch(rejected).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition(
            iced_css::resources::graph::GraphWorkerError::Acquisition(
                FileGraphAcquisitionError::Watch(FileGraphWatchError::Paths(WatchPathError::Work))
            )
        ))
    ));
    assert_eq!(graph.active_revision(), old_revision);
    assert_eq!(
        graph.status(),
        iced_css_core::resources::publication::PublicationStatus::Failed
    );
    let current = load(&mut graph);
    let rejected = block_on(graph.prepare_watch(
        current,
        &mut paths,
        &watches,
        FileGraphWatchConfig {
            paths: WatchPathLimits {
                work: NonZeroUsize::MIN,
                ..config.paths
            },
            ..config
        },
    ))
    .unwrap_err();
    let newer = load(&mut graph);
    assert!(matches!(
        graph.fail_watch(rejected).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(graph.is_current(&newer));
    let prepared = block_on(graph.prepare_watch(newer, &mut paths, &watches, config)).unwrap();
    assert!(matches!(
        graph
            .finish_watched(prepared, &mut watches)
            .unwrap()
            .sheet
            .unwrap(),
        SheetCompletion::Published(_)
    ));

    // An unwatched missing parent makes resume/repair the only notice producer.
    // Its application waker deliberately panics on the resource worker; wait for
    // that callback before polling again so normal polling cannot replace it.
    use iced::{
        advanced::subscription,
        futures::{
            StreamExt,
            channel::oneshot,
            future::{Either, select},
        },
    };
    use std::{
        sync::{Arc, mpsc},
        task::{Context, Poll, Wake, Waker},
    };
    let fault_config = FileGraphWatchConfig {
        watch: WatchConfig {
            recovery_interval: std::time::Duration::from_secs(30),
            ..config.watch
        },
        ..config
    };
    let fault_parent = temp.0.join("absent-parent");
    let fault_path = fault_parent.join("root.css");
    let fault_uri = unix_file_uri(&fault_path, path_limits).unwrap();
    let mut fault_graph = make(RuntimeMode::Auto);
    let mut fault_watches = WatchSessionHost::new(ownership_limits).unwrap();
    let initial = block_on(
        fault_graph
            .request(
                &reader,
                RequestReason::FirstAcquisition,
                FileGraphSource {
                    uri: fault_uri,
                    encoding: StylesheetEncoding::default(),
                },
                settings,
            )
            .unwrap(),
    );
    let prepared =
        block_on(fault_graph.prepare_watch(initial, &mut paths, &fault_watches, fault_config))
            .unwrap();
    assert!(matches!(
        fault_graph
            .finish_watched(prepared, &mut fault_watches)
            .unwrap()
            .sheet
            .unwrap(),
        SheetCompletion::Failed(_)
    ));
    struct PanicWake(mpsc::SyncSender<()>);
    impl Wake for PanicWake {
        fn wake(self: Arc<Self>) {
            let _ = self.0.try_send(());
            panic!("controlled watch worker wake fault");
        }
    }
    let mut stream = subscription::into_recipes(fault_watches.subscription())
        .pop()
        .unwrap()
        .stream(Box::pin(iced::futures::stream::pending()));
    let (fault_tx, fault_rx) = mpsc::sync_channel(1);
    let waker = Waker::from(Arc::new(PanicWake(fault_tx)));
    loop {
        match stream.as_mut().poll_next(&mut Context::from_waker(&waker)) {
            Poll::Pending => break,
            Poll::Ready(Some(_)) => {}
            Poll::Ready(None) => panic!("live coverage closed before fault injection"),
        }
    }
    fault_watches.active().unwrap().control().resume();
    fault_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    let (timeout_tx, timeout_rx) = oneshot::channel();
    let timer = std::thread::spawn(move || {
        if cancel_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .is_err()
        {
            let _ = timeout_tx.send(());
        }
    });
    let observed = block_on(select(
        Box::pin(async {
            let mut failed = false;
            while let Some(delivery) = stream.next().await {
                failed |= delivery.notice().worker_failed;
            }
            failed
        }),
        timeout_rx,
    ));
    drop(cancel_tx);
    timer.join().unwrap();
    assert!(matches!(observed, Either::Left((true, _))));
    assert!(
        fault_watches
            .active()
            .unwrap()
            .control()
            .is_shutdown_requested()
    );
    std::fs::create_dir(&fault_parent).unwrap();
    std::fs::write(&fault_path, b"a{color:red}").unwrap();
    let before = fault_watches.ownership().revision();
    let next = block_on(
        fault_graph
            .retry(&reader, RequestReason::LocalChange)
            .unwrap(),
    );
    let prepared =
        block_on(fault_graph.prepare_watch(next, &mut paths, &fault_watches, fault_config))
            .unwrap();
    assert!(prepared.candidate_control().is_some());
    let result = fault_graph
        .finish_watched(prepared, &mut fault_watches)
        .unwrap();
    assert!(matches!(
        result.sheet.unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(fault_watches.ownership().revision(), before + 1);
    assert!(
        !fault_watches
            .active()
            .unwrap()
            .control()
            .is_shutdown_requested()
    );
}

#[cfg(unix)]
#[test]
fn file_graph_retry_retains_requested_input_independently_of_active_content() {
    use iced_css::host::stylesheet::file_graph::*;
    use iced_css::resources::{
        file_imports::FileImportLimits,
        file_path::{FilePathLimits, unix_file_uri},
        file_uri_read::FileUriReadLimits,
        local::ReadBodyLimits,
        stylesheet::StylesheetEncoding,
    };
    use iced_css_core::{
        resources::{
            body::BodyLimits,
            triggers::{RequestError, RequestReason, RuntimeMode},
            uri::UriLimits,
        },
        stylesheet::{
            collector::CollectorLimits, identities::IdentityLimits, requests::ImportRequestLimits,
        },
    };
    use std::path::PathBuf;
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temp =
        Temp(std::env::temp_dir().join(format!("iced-css-file-retry-{}", std::process::id())));
    std::fs::create_dir(&temp.0).unwrap();
    let path = temp.0.join("root.css");
    let child = temp.0.join("child.css");
    std::fs::write(&path, b"@import 'child.css'; r{color:red}").unwrap();
    std::fs::write(&child, b"a{margin:1px}").unwrap();
    let path_limits = FilePathLimits {
        bytes: n(),
        work: n(),
    };
    let uri = unix_file_uri(&path, path_limits).unwrap();
    let source = || FileGraphSource {
        uri: uri.clone(),
        encoding: StylesheetEncoding::default(),
    };
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
            dependencies: iced_css::resources::file_dependencies::FileDependencyLimits {
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
        bases: meta_limits(),
        metadata_bytes: n(),
    };
    let make = |mode| {
        RuntimeFileStylesheetGraph::new(
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
        .unwrap()
    };
    let reader = FileReader::new(NonZeroUsize::MIN);

    use iced::futures::executor::block_on;
    let mut graph = make(RuntimeMode::Auto);
    assert!(graph.configuration().is_none());
    assert!(matches!(
        graph.retry(&reader, RequestReason::LocalChange),
        Err(FileGraphRetryError::MissingConfiguration)
    ));
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(
        graph.status(),
        iced_css_core::resources::publication::PublicationStatus::Unrequested
    );
    let first = block_on(
        graph
            .request(&reader, RequestReason::FirstAcquisition, source(), settings)
            .unwrap(),
    );
    assert!(matches!(
        graph.finish(first).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(graph.configuration().unwrap().source().uri, uri);
    let initial_revision = graph.active_revision();
    let replacement = temp.0.join("replacement.css");
    let replacement_uri = unix_file_uri(&replacement, path_limits).unwrap();
    let replacement_source = || FileGraphSource {
        uri: replacement_uri.clone(),
        encoding: StylesheetEncoding {
            linking: Some(b"UTF-8".to_vec()),
            ..StylesheetEncoding::default()
        },
    };
    let missing = block_on(
        graph
            .request(
                &reader,
                RequestReason::ExplicitReload,
                replacement_source(),
                settings,
            )
            .unwrap(),
    );
    assert!(matches!(
        graph.finish(missing).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(graph.active_revision(), initial_revision);
    assert_eq!(graph.configuration().unwrap().source().uri, replacement_uri);
    assert_eq!(
        graph
            .configuration()
            .unwrap()
            .source()
            .encoding
            .linking
            .as_deref(),
        Some(b"UTF-8".as_slice())
    );
    assert_eq!(
        graph.configuration().unwrap().limits().metadata_bytes,
        settings.metadata_bytes
    );
    std::fs::write(&replacement, b"new{color:blue}").unwrap();
    let retry = graph.retry(&reader, RequestReason::LocalChange).unwrap();
    fn owned_send<T: Send + 'static>(_: &T) {}
    owned_send(&retry);
    let restored = block_on(retry);
    assert!(matches!(
        graph.finish(restored).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_ne!(graph.active_revision(), initial_revision);
    assert_eq!(
        graph.active_dependencies().unwrap().entries()[0].native_path(),
        replacement
    );

    // Invalid owned storage cannot replace the last admitted retry target.
    let mut invalid = settings;
    invalid.file.file.uri.bytes = NonZeroUsize::MIN;
    let rejected = block_on(
        graph
            .request(&reader, RequestReason::ExplicitReload, source(), invalid)
            .unwrap(),
    );
    assert!(matches!(
        graph.finish(rejected).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(graph.configuration().unwrap().source().uri, replacement_uri);
    assert_eq!(
        graph.configuration().unwrap().limits().file.file.uri.bytes,
        settings.file.file.uri.bytes
    );
    assert!(matches!(
        graph.request(&reader, RequestReason::Reevaluation, source(), settings),
        Err(RequestError::Unauthorized)
    ));
    assert_eq!(graph.configuration().unwrap().source().uri, replacement_uri);
    let older = block_on(graph.retry(&reader, RequestReason::ExplicitReload).unwrap());
    let current = block_on(graph.retry(&reader, RequestReason::LocalChange).unwrap());
    assert!(matches!(
        graph.finish(older).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(matches!(
        graph.finish(current).unwrap(),
        SheetCompletion::Published(_)
    ));

    // Worker admission failure also retains the explicitly requested new target.
    let blocker = reader
        .read_file(
            path.clone(),
            iced_css::resources::worker::BackgroundReadLimits {
                path_bytes: n(),
                read: settings.file.file.read,
            },
        )
        .unwrap();
    let rejected = block_on(
        graph
            .request(&reader, RequestReason::ExplicitReload, source(), settings)
            .unwrap(),
    );
    assert!(matches!(
        graph.finish(rejected).unwrap(),
        SheetCompletion::Failed(GraphLoadError::Acquisition(
            iced_css::resources::graph::GraphWorkerError::Submit(
                iced_css::resources::worker::SubmitError::Limit
            )
        ))
    ));
    assert_eq!(graph.configuration().unwrap().source().uri, uri);
    block_on(blocker).unwrap();
    let restored = block_on(graph.retry(&reader, RequestReason::LocalChange).unwrap());
    assert!(matches!(
        graph.finish(restored).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(
        graph.active_dependencies().unwrap().entries()[0].native_path(),
        path
    );

    let mut manual = make(RuntimeMode::Manual);
    let first = block_on(
        manual
            .request(&reader, RequestReason::FirstAcquisition, source(), settings)
            .unwrap(),
    );
    manual.finish(first).unwrap();
    assert!(matches!(
        manual.retry(&reader, RequestReason::LocalChange),
        Err(FileGraphRetryError::Request(RequestError::Unauthorized))
    ));
    assert_eq!(manual.configuration().unwrap().source().uri, uri);
    let next = block_on(
        manual
            .retry(&reader, RequestReason::ExplicitReload)
            .unwrap(),
    );
    assert!(matches!(
        manual.finish(next).unwrap(),
        SheetCompletion::Published(_)
    ));

    let mut once = RuntimeFileStylesheetGraph::new(
        RuntimeMode::Auto,
        context(),
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        n(),
        IdentityLimits {
            entries: n(),
            bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    let first = block_on(
        once.request(&reader, RequestReason::FirstAcquisition, source(), settings)
            .unwrap(),
    );
    once.finish(first).unwrap();
    assert!(matches!(
        once.request(
            &reader,
            RequestReason::ExplicitReload,
            replacement_source(),
            settings
        ),
        Err(RequestError::Exhausted(_))
    ));
    assert!(matches!(
        once.retry(&reader, RequestReason::LocalChange),
        Err(FileGraphRetryError::Request(RequestError::Exhausted(_)))
    ));
    assert_eq!(once.configuration().unwrap().source().uri, uri);
}

#[cfg(unix)]
#[test]
fn auto_file_coordinator_routes_messages_and_coalesces_dirty_work() {
    use iced_css::host::stylesheet::file_graph::*;
    use iced_css::resources::{
        file_imports::FileImportLimits,
        file_path::{FilePathLimits, unix_file_uri},
        file_uri_read::FileUriReadLimits,
        local::ReadBodyLimits,
        stylesheet::StylesheetEncoding,
    };
    use iced_css_core::{
        resources::{
            body::BodyLimits,
            triggers::{RequestError, RequestReason, RuntimeMode},
            uri::UriLimits,
        },
        stylesheet::{
            collector::CollectorLimits, identities::IdentityLimits, requests::ImportRequestLimits,
        },
    };
    use std::path::PathBuf;
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temp = Temp(
        std::env::temp_dir().join(format!("iced-css-file-coordinator-{}", std::process::id())),
    );
    std::fs::create_dir(&temp.0).unwrap();
    let path = temp.0.join("root.css");
    let child = temp.0.join("child.css");
    std::fs::write(&path, b"@import 'child.css'; r{color:red}").unwrap();
    std::fs::write(&child, b"a{margin:1px}").unwrap();
    let path_limits = FilePathLimits {
        bytes: n(),
        work: n(),
    };
    let uri = unix_file_uri(&path, path_limits).unwrap();
    let source = || FileGraphSource {
        uri: uri.clone(),
        encoding: StylesheetEncoding::default(),
    };
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
            dependencies: iced_css::resources::file_dependencies::FileDependencyLimits {
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
        bases: meta_limits(),
        metadata_bytes: n(),
    };

    use iced::{
        advanced::subscription,
        futures::{
            StreamExt,
            channel::oneshot,
            executor::block_on,
            future::{Either, select},
        },
    };
    use iced_css::host::stylesheet::file_coordinator::*;
    use iced_css::{
        host::stylesheet::file_watch::FileGraphWatchConfig,
        resources::{watch::WatchConfig, watch_paths::*, watch_sessions::*},
    };
    use iced_css_core::resources::watch_ownership::*;
    let config = FileStylesheetHostConfig {
        work: iced_css::resources::watch_work::WatchWorkLimits {
            slots: NonZeroUsize::new(4).unwrap(),
            retained_bytes: n(),
        },
        mode: RuntimeMode::Auto,
        context: context(),
        publication: PublicationLimits {
            requests: NonZeroU64::new(30).unwrap(),
            revisions: NonZeroU64::new(30).unwrap(),
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
            revisions: NonZeroU64::new(50).unwrap(),
        },
        watch: FileGraphWatchConfig {
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
    };
    fn notice(
        stream: &mut std::pin::Pin<
            Box<dyn iced::futures::Stream<Item = FileStylesheetMessage> + Send>,
        >,
    ) -> FileStylesheetMessage {
        let (cancel_tx, cancel_rx) = std::sync::mpsc::channel::<()>();
        let (timeout_tx, timeout_rx) = oneshot::channel();
        let timer = std::thread::spawn(move || {
            if cancel_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .is_err()
            {
                let _ = timeout_tx.send(());
            }
        });
        let observed = block_on(select(stream.next(), timeout_rx));
        drop(cancel_tx);
        timer.join().unwrap();
        let Either::Left((Some(message), _)) = observed else {
            panic!("no watch message")
        };
        assert!(message.watch_notice().is_some());
        message
    }
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut host = FileStylesheetHost::new(reader.clone(), config).unwrap();
    fn settle(host: &mut FileStylesheetHost, work: FileStylesheetWork) -> FileStylesheetUpdate {
        let mut update = host.update(block_on(work));
        for _ in 0..2 {
            if update.outcome.is_some() {
                return update;
            }
            update = host.update(block_on(update.work.unwrap()));
        }
        assert!(
            update.outcome.is_some(),
            "request must terminate within three phases"
        );
        update
    }
    fn after_root(host: &mut FileStylesheetHost, work: FileStylesheetWork) -> FileStylesheetWork {
        let update = host.update(block_on(work));
        assert!(update.outcome.is_none());
        update.work.unwrap()
    }
    // A separate caller holds the shared reader's only completion lease. Root
    // coverage must already be live when this host encounters submission failure.
    let shared = FileReader::new(NonZeroUsize::MIN);
    let held = shared
        .read_file(
            path.clone(),
            iced_css::resources::worker::BackgroundReadLimits {
                read: settings.file.file.read,
                path_bytes: n(),
            },
        )
        .unwrap();
    let mut saturated = FileStylesheetHost::new(shared.clone(), config).unwrap();
    let work = saturated
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    assert_eq!(shared.in_flight(), 1);
    let root_message = block_on(work);
    let root_control = root_message.candidate_control().unwrap();
    let acquisition = saturated.update(root_message);
    assert!(acquisition.outcome.is_none());
    let mut recovery_notices = subscription::into_recipes(saturated.subscription())
        .pop()
        .expect("root coverage must precede acquisition completion")
        .stream(Box::pin(iced::futures::stream::pending()));
    assert!(!root_control.is_shutdown_requested());
    let failed = settle(&mut saturated, acquisition.work.unwrap());
    assert!(matches!(
        failed.outcome.unwrap().unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert!(!saturated.is_busy());
    assert_eq!(saturated.pending_work().unwrap(), 0);
    assert!(!root_control.is_shutdown_requested());
    block_on(held).unwrap();
    assert_eq!(shared.in_flight(), 0);
    let retry = saturated.update(notice(&mut recovery_notices));
    let recovered = settle(&mut saturated, retry.work.unwrap());
    assert!(matches!(
        recovered.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert!(saturated.active().is_some());
    drop(saturated);
    assert!(root_control.is_shutdown_requested());

    std::fs::remove_file(&path).unwrap();
    let work = host
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    assert_eq!(
        reader.in_flight(),
        0,
        "root coverage must precede file submission"
    );
    fn owned_send<T: Send + 'static>(_: &T) {}
    owned_send(&work);
    assert!(host.is_busy());
    assert!(host.active().is_none());
    let first = settle(&mut host, work);
    assert!(matches!(
        first.outcome.unwrap().unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert!(first.work.is_none());
    assert!(!host.is_busy());
    let mut notices = subscription::into_recipes(host.subscription())
        .pop()
        .unwrap()
        .stream(Box::pin(iced::futures::stream::pending()));
    std::fs::write(&path, b"r{color:red}").unwrap();
    let restore = host.update(notice(&mut notices));
    let restored = settle(&mut host, restore.work.unwrap());
    assert!(matches!(
        restored.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert!(restored.work.is_none());
    assert_eq!(host.active().unwrap().graph().rules().count(), 1);

    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let work = after_root(&mut host, work);
    let acquired = block_on(work);
    #[derive(Debug, Clone)]
    enum AppMessage {
        Styles(FileStylesheetMessage),
        Reload,
    }
    let app_message = AppMessage::Styles(acquired.clone());
    if let AppMessage::Styles(message) = app_message {
        assert!(message.is_live());
    }
    let _: iced::Element<'_, AppMessage> = iced::widget::button("Reload styles")
        .on_press(AppMessage::Reload)
        .into();
    let duplicate = acquired.clone();

    std::fs::write(&path, b"r{color:blue} a{width:1px}").unwrap();
    for _ in 0..5 {
        let update = host.update(notice(&mut notices));
        assert!(update.work.is_none() && update.outcome.is_none());
    }
    assert!(host.is_dirty() && host.is_busy());
    let prepare = host.update(acquired);
    let ignored = host.update(duplicate);
    assert!(ignored.work.is_none() && ignored.outcome.is_none() && host.is_busy());
    assert_eq!(host.pending_work().unwrap(), 1);

    let finished = host.update(block_on(prepare.work.unwrap()));
    assert!(matches!(
        finished.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert!(host.is_busy() && !host.is_dirty());
    let repeated = settle(&mut host, finished.work.unwrap());
    assert!(matches!(
        repeated.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert!(repeated.work.is_none());
    assert!(!host.is_busy());
    assert_eq!(host.active().unwrap().graph().rules().count(), 2);

    // Abandoning the newest attempt must not leave scheduling busy merely
    // because an older superseded attempt still owns a pending slot.
    let old_work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let abandoned = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    assert!(host.is_busy());
    drop(abandoned);
    assert_eq!(host.pending_work().unwrap(), 1);
    assert!(
        !host.is_busy(),
        "dropping current work must release busy scheduling"
    );
    let revision = host.active_revision();
    let retry = host.update(notice(&mut notices));
    let recovered = settle(&mut host, retry.work.unwrap());
    assert!(matches!(
        recovered.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    let current_revision = host.active_revision();
    assert_ne!(revision, current_revision);
    let stale = host.update(block_on(old_work));
    assert!(stale.work.is_none() && stale.outcome.is_none());
    assert_eq!(host.active_revision(), current_revision);

    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let acquired_work = after_root(&mut host, work);
    assert!(host.is_busy());
    drop(acquired_work);
    assert!(!host.is_busy());
    let retry = host.update(notice(&mut notices));
    settle(&mut host, retry.work.unwrap());

    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let message = block_on(work);
    let duplicate = message.clone();
    drop(message);
    assert!(host.is_busy());
    drop(duplicate);
    assert!(!host.is_busy());
    let retry = host.update(notice(&mut notices));
    settle(&mut host, retry.work.unwrap());

    // Both old acquisition and prepared Messages leave the newer attempt busy.
    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let old = block_on(work);
    let newer = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let ignored = host.update(old);
    assert!(ignored.outcome.is_none() && ignored.work.is_none() && host.is_busy());
    settle(&mut host, newer);
    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let work = after_root(&mut host, work);
    let old_acquired = block_on(work);
    let newer = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let ignored = host.update(old_acquired);
    assert!(ignored.outcome.is_none() && ignored.work.is_none() && host.is_busy());
    settle(&mut host, newer);
    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let work = after_root(&mut host, work);
    let preparation = host.update(block_on(work));
    let old = block_on(preparation.work.unwrap());
    let newer = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let watch_revision = host.watch_revision();
    let ignored = host.update(old);
    assert!(ignored.outcome.is_none() && ignored.work.is_none() && host.is_busy());
    assert_eq!(host.watch_revision(), watch_revision);
    settle(&mut host, newer);
    let revision = host.active_revision();
    std::fs::write(&path, b"@import 'unavailable.css'; r{color:green}").unwrap();
    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let failed = settle(&mut host, work);
    assert!(matches!(
        failed.outcome.unwrap().unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(host.active_revision(), revision);
    assert!(!host.is_busy());

    let mut manual = FileStylesheetHost::new(
        FileReader::new(NonZeroUsize::MIN),
        FileStylesheetHostConfig {
            mode: RuntimeMode::Manual,
            ..config
        },
    )
    .unwrap();
    assert!(subscription::into_recipes(manual.subscription()).is_empty());
    let ignored = manual.update(notice(&mut notices));
    assert!(ignored.work.is_none() && !manual.is_dirty());
    let work = manual
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    let done = manual.update(block_on(work));
    assert!(matches!(
        done.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert!(done.work.is_none() && !manual.is_busy());
    let mut foreign = FileStylesheetHost::new(FileReader::new(NonZeroUsize::MIN), config).unwrap();
    assert!(foreign.update(notice(&mut notices)).work.is_none());
    assert!(!foreign.is_dirty());

    std::fs::write(&path, b"r{color:red}").unwrap();
    let mut bad = FileStylesheetHost::new(
        FileReader::new(NonZeroUsize::MIN),
        FileStylesheetHostConfig {
            watch: FileGraphWatchConfig {
                paths: WatchPathLimits {
                    work: NonZeroUsize::MIN,
                    ..config.watch.paths
                },
                ..config.watch
            },
            ..config
        },
    )
    .unwrap();
    let work = bad
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    let failed = settle(&mut bad, work);
    assert!(matches!(
        failed.outcome.unwrap().unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert!(!bad.is_busy() && bad.active().is_none());
    assert_eq!(
        bad.status(),
        iced_css_core::resources::publication::PublicationStatus::Failed
    );
    assert!(subscription::into_recipes(bad.subscription()).is_empty());
    let mut once = FileStylesheetHost::new(
        FileReader::new(NonZeroUsize::MIN),
        FileStylesheetHostConfig {
            mode: RuntimeMode::Manual,
            publication: PublicationLimits {
                requests: NonZeroU64::MIN,
                revisions: NonZeroU64::MIN,
            },
            ..config
        },
    )
    .unwrap();
    let work = once
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    assert!(matches!(
        once.request(RequestReason::ExplicitReload, source()),
        Err(FileStylesheetRequestError::Request(
            RequestError::Exhausted(_)
        ))
    ));
    assert!(
        once.is_busy(),
        "rejection must not retire the current attempt"
    );
    once.update(block_on(work));
    assert!(matches!(
        once.request(RequestReason::ExplicitReload, source()),
        Err(FileStylesheetRequestError::Request(
            RequestError::Exhausted(_)
        ))
    ));
    assert!(!once.is_busy());
    // A one-slot pool transfers its lease between phases and releases it before
    // a dirty follow-up. Admission rejection must not consume a graph generation.
    let work_limit = iced_css::resources::watch_work::WatchWorkLimits {
        slots: NonZeroUsize::MIN,
        retained_bytes: n(),
    };
    let mut limited = FileStylesheetHost::new(
        FileReader::new(NonZeroUsize::MIN),
        FileStylesheetHostConfig {
            mode: RuntimeMode::Manual,
            work: work_limit,
            publication: PublicationLimits {
                requests: NonZeroU64::new(2).unwrap(),
                revisions: NonZeroU64::new(2).unwrap(),
            },
            ..config
        },
    )
    .unwrap();
    let first = limited
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    assert_eq!(limited.pending_work().unwrap(), 1);
    assert!(matches!(
        limited.request(RequestReason::ExplicitReload, source()),
        Err(FileStylesheetRequestError::Work(
            iced_css::resources::watch_work::WatchWorkError::Full
        ))
    ));
    assert!(limited.is_busy());
    limited.update(block_on(first));
    assert_eq!(limited.pending_work().unwrap(), 0);
    let second = limited
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    limited.update(block_on(second));
    let mut single = FileStylesheetHost::new(
        FileReader::new(NonZeroUsize::MIN),
        FileStylesheetHostConfig {
            work: work_limit,
            ..config
        },
    )
    .unwrap();
    let first = single
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    settle(&mut single, first);
    let mut single_notices = subscription::into_recipes(single.subscription())
        .pop()
        .unwrap()
        .stream(Box::pin(iced::futures::stream::pending()));
    let work = single
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let work = after_root(&mut single, work);
    let acquired = block_on(work);
    assert!(single.update(notice(&mut single_notices)).work.is_none());
    let prepare = single.update(acquired);
    assert_eq!(single.pending_work().unwrap(), 1);
    let finished = single.update(block_on(prepare.work.unwrap()));
    assert_eq!(single.pending_work().unwrap(), 1);
    settle(&mut single, finished.work.unwrap());
    assert_eq!(single.pending_work().unwrap(), 0);

    let mut duplicate_host =
        FileStylesheetHost::new(FileReader::new(NonZeroUsize::MIN), config).unwrap();
    let work = duplicate_host
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    let message = block_on(work);
    let duplicate = message.clone();
    let control = message.candidate_control().unwrap();
    assert!(foreign.update(message.clone()).work.is_none());
    assert_eq!(duplicate_host.pending_work().unwrap(), 1);
    let acquired = duplicate_host.update(message);
    let published = settle(&mut duplicate_host, acquired.work.unwrap());
    assert!(matches!(
        published.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    let revision = duplicate_host.active_revision();
    assert_eq!(duplicate_host.pending_work().unwrap(), 0);
    assert!(!control.is_shutdown_requested());
    let ignored = duplicate_host.update(duplicate);
    assert!(ignored.outcome.is_none() && ignored.work.is_none());
    assert_eq!(duplicate_host.active_revision(), revision);
    drop(duplicate_host);
    assert!(control.is_shutdown_requested());

    let mut queued = FileStylesheetHost::new(FileReader::new(NonZeroUsize::MIN), config).unwrap();
    let work = queued
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    let message = block_on(work);
    assert_eq!(queued.pending_work().unwrap(), 1);
    let control = message.candidate_control().unwrap().clone();
    assert!(!control.is_shutdown_requested());
    let queued_clone = message.clone();
    drop(queued);
    assert!(
        control.is_shutdown_requested(),
        "queued Message must not keep candidate running after host disposal"
    );
    assert!(!message.is_live());
    assert!(!queued_clone.is_live());
    assert!(
        queued_clone
            .candidate_control()
            .unwrap()
            .is_shutdown_requested()
    );
    drop(queued_clone);

    assert!(foreign.update(message).work.is_none());
    assert_eq!(foreign.pending_work().unwrap(), 0);

    let mut early = FileStylesheetHost::new(FileReader::new(NonZeroUsize::MIN), config).unwrap();
    let work = early
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    drop(early);
    let message = block_on(work);
    assert!(!message.is_live() && message.candidate_control().is_none());
    assert!(foreign.update(message).work.is_none());
    let mut early = FileStylesheetHost::new(FileReader::new(NonZeroUsize::MIN), config).unwrap();
    let work = early
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    let preparation = early.update(block_on(work));
    drop(early);
    let message = block_on(preparation.work.unwrap());
    assert!(!message.is_live() && message.candidate_control().is_none());
    assert!(foreign.update(message).work.is_none());

    // Exercise file replacement and dependency removal through real subscription
    // notices. Periodic fallback is allowed; this does not assert a native event kind.
    let nested = temp.0.join("retired-imports");
    std::fs::create_dir(&nested).unwrap();
    let nested_child = nested.join("child.css");
    std::fs::write(&nested_child, b"child{color:green}").unwrap();
    std::fs::write(&path, b"@import 'retired-imports/child.css'; r{color:red}").unwrap();
    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let published = settle(&mut host, work);
    assert!(matches!(
        published.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(host.active().unwrap().graph().rules().count(), 2);
    // Coverage changed to include a new directory: attach to the current recipe.
    let mut current_notices = subscription::into_recipes(host.subscription())
        .pop()
        .unwrap()
        .stream(Box::pin(iced::futures::stream::pending()));
    let replacement = temp.0.join("replacement.css");
    std::fs::write(&replacement, b"replacement{color:blue}").unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    let update = host.update(notice(&mut current_notices));
    let published = settle(&mut host, update.work.unwrap());
    assert!(matches!(
        published.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(host.active().unwrap().graph().rules().count(), 1);
    std::fs::remove_file(&nested_child).unwrap();
    let mut current_notices = subscription::into_recipes(host.subscription())
        .pop()
        .unwrap()
        .stream(Box::pin(iced::futures::stream::pending()));
    let update = host.update(notice(&mut current_notices));
    let published = settle(&mut host, update.work.unwrap());
    assert!(matches!(
        published.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(host.active().unwrap().graph().rules().count(), 1);
    std::fs::write(&replacement, b"").unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    let mut current_notices = subscription::into_recipes(host.subscription())
        .pop()
        .unwrap()
        .stream(Box::pin(iced::futures::stream::pending()));
    let update = host.update(notice(&mut current_notices));
    let published = settle(&mut host, update.work.unwrap());
    assert!(matches!(
        published.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(host.active().unwrap().graph().rules().count(), 0);

    let work = manual
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let _: iced::Task<FileStylesheetMessage> = work.into_task();
}

#[cfg(unix)]
#[test]
fn staged_file_requests_acquire_only_under_the_current_owner_and_generation() {
    use iced_css::host::stylesheet::file_graph::*;
    use iced_css::resources::{
        file_imports::FileImportLimits,
        file_path::{FilePathLimits, unix_file_uri},
        file_uri_read::FileUriReadLimits,
        local::ReadBodyLimits,
        stylesheet::StylesheetEncoding,
    };
    use iced_css_core::{
        resources::{
            body::BodyLimits,
            triggers::{RequestError, RequestReason, RuntimeMode},
            uri::UriLimits,
        },
        stylesheet::{
            collector::CollectorLimits, identities::IdentityLimits, requests::ImportRequestLimits,
        },
    };
    use std::path::PathBuf;
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let temp = Temp(std::env::temp_dir().join(format!(
        "iced-css-file-request-staging-{}",
        std::process::id()
    )));
    std::fs::create_dir(&temp.0).unwrap();
    let path = temp.0.join("root.css");
    let child = temp.0.join("child.css");
    std::fs::write(&path, b"@import 'child.css'; r{color:red}").unwrap();
    std::fs::write(&child, b"a{margin:1px}").unwrap();
    let path_limits = FilePathLimits {
        bytes: n(),
        work: n(),
    };
    let uri = unix_file_uri(&path, path_limits).unwrap();
    let source = || FileGraphSource {
        uri: uri.clone(),
        encoding: StylesheetEncoding::default(),
    };
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
            dependencies: iced_css::resources::file_dependencies::FileDependencyLimits {
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
        bases: meta_limits(),
        metadata_bytes: n(),
    };
    let make = |mode| {
        RuntimeFileStylesheetGraph::new(
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
        .unwrap()
    };
    let reader = FileReader::new(NonZeroUsize::MIN);

    use iced::futures::executor::block_on;
    let mut owner = make(RuntimeMode::Auto);
    let foreign = make(RuntimeMode::Auto);
    let staged = owner
        .begin(RequestReason::FirstAcquisition, source(), settings)
        .unwrap();
    assert_eq!(reader.in_flight(), 0);
    assert!(owner.is_request_current(&staged));
    assert!(!foreign.is_request_current(&staged));
    assert_eq!(staged.configuration().unwrap().source().uri, uri);
    assert!(foreign.acquire_current(&reader, staged).is_none());
    assert_eq!(reader.in_flight(), 0);
    let older = owner.begin_retry(RequestReason::LocalChange).unwrap();
    let current = owner.begin_retry(RequestReason::ExplicitReload).unwrap();
    assert!(!owner.is_request_current(&older));
    assert!(owner.acquire_current(&reader, older).is_none());
    assert_eq!(reader.in_flight(), 0);
    let work = owner.acquire_current(&reader, current).unwrap();
    fn owned_send<T: Send + 'static>(_: &T) {}
    owned_send(&work);
    assert_eq!(reader.in_flight(), 1);
    assert!(matches!(
        owner.finish(block_on(work)).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(reader.in_flight(), 0);
    let revision = owner.active_revision();
    let mut invalid = source();
    invalid.uri.reserve(n().get() * 2);
    let staged = owner
        .begin(RequestReason::ExplicitReload, invalid, settings)
        .unwrap();
    assert!(staged.configuration().is_none());
    assert_eq!(owner.configuration().unwrap().source().uri, uri);
    let work = owner.acquire_current(&reader, staged).unwrap();
    assert_eq!(reader.in_flight(), 0);
    assert!(matches!(
        owner.finish(block_on(work)).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(owner.active_revision(), revision);

    let replacement = temp.0.join("replacement.css");
    let replacement_uri = unix_file_uri(&replacement, path_limits).unwrap();
    let staged = owner
        .begin(
            RequestReason::ExplicitReload,
            FileGraphSource {
                uri: replacement_uri.clone(),
                encoding: StylesheetEncoding::default(),
            },
            settings,
        )
        .unwrap();
    assert_eq!(reader.in_flight(), 0);
    let work = owner.acquire_current(&reader, staged).unwrap();
    assert!(matches!(
        owner.finish(block_on(work)).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(owner.configuration().unwrap().source().uri, replacement_uri);
    std::fs::write(&replacement, b"new{color:blue}").unwrap();
    let staged = owner.begin_retry(RequestReason::LocalChange).unwrap();
    assert_eq!(reader.in_flight(), 0);
    let work = owner.acquire_current(&reader, staged).unwrap();
    assert!(matches!(
        owner.finish(block_on(work)).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(
        owner.active_dependencies().unwrap().entries()[0].native_path(),
        replacement
    );

    let mut once = RuntimeFileStylesheetGraph::new(
        RuntimeMode::Manual,
        context(),
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        n(),
        IdentityLimits {
            entries: n(),
            bytes: n(),
            work: n(),
        },
    )
    .unwrap();
    let staged = once
        .begin(RequestReason::FirstAcquisition, source(), settings)
        .unwrap();
    assert!(matches!(
        once.begin_retry(RequestReason::ExplicitReload),
        Err(FileGraphRetryError::Request(RequestError::Exhausted(_)))
    ));
    assert!(once.is_request_current(&staged));
    assert_eq!(reader.in_flight(), 0);
    let work = once.acquire_current(&reader, staged).unwrap();
    assert!(matches!(
        once.finish(block_on(work)).unwrap(),
        SheetCompletion::Published(_)
    ));
}

#[test]
fn embedded_graph_collection_uses_resolver_bases_versions_and_atomic_reloads() {
    use iced::futures::executor::block_on;
    use iced_css::resources::embedded::*;
    use iced_css_core::{
        resources::{body::BodyLimits, uri::UriLimits},
        stylesheet::{collector::CollectorLimits, identities::*, requests::ImportRequestLimits},
    };
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Default)]
    struct State {
        missing_child: bool,
        empty: bool,
        revision: u8,
    }
    struct Resolver {
        state: Arc<Mutex<State>>,
        calls: Arc<AtomicUsize>,
    }
    impl EmbeddedResolver for Resolver {
        type Error = std::io::Error;
        fn resolve(&self, request: EmbeddedRequest<'_>) -> Result<EmbeddedSource, Self::Error> {
            assert_eq!(std::thread::current().name(), Some("iced-css-resource"));
            self.calls.fetch_add(1, Ordering::SeqCst);
            let state = self.state.lock().unwrap();
            let (text, base) = match request.acquisition_uri {
                "app:/root.css" => {
                    assert_eq!(request.base, "app:/configured/main.css");
                    (
                        if state.empty {
                            ""
                        } else if state.revision == 0 {
                            "@import 'child.css#part'; r{color:red}"
                        } else {
                            "@import 'child.css#part'; r{color:blue}"
                        },
                        "app:/generated/main.css",
                    )
                }
                "app:/generated/child.css" => {
                    assert_eq!(request.base, "app:/generated/main.css");
                    assert_eq!(request.reference_uri, "app:/generated/child.css#part");
                    if state.missing_child {
                        return Err(std::io::Error::other("missing child"));
                    }
                    (
                        "@import 'app:/root.css'; a{color:green}",
                        "app:/nested/child.css",
                    )
                }
                _ => return Err(std::io::Error::other("unexpected URI")),
            };
            Ok(EmbeddedSource {
                bytes: text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
                version: vec![state.revision],
                dependency_base: base.into(),
            })
        }
    }
    let state = Arc::new(Mutex::new(State {
        missing_child: true,
        ..Default::default()
    }));
    let calls = Arc::new(AtomicUsize::new(0));
    let uri = UriLimits {
        bytes: n(),
        work: n(),
    };
    let reader = EmbeddedReader::new(
        "app".into(),
        Resolver {
            state: Arc::clone(&state),
            calls: Arc::clone(&calls),
        },
        NonZeroUsize::MIN,
        uri,
    )
    .unwrap();
    let ids = Arc::new(Mutex::new(
        ImportIdentityRegistry::new(IdentityLimits {
            entries: n(),
            bytes: n(),
            work: n(),
        })
        .unwrap(),
    ));
    let source = || EmbeddedGraphSource {
        base: "app:/configured/main.css".into(),
        reference: "app:/root.css".into(),
        encoding: iced_css::resources::stylesheet::StylesheetEncoding {
            transport: Some(b"UTF-16LE".to_vec()),
            ..Default::default()
        },
    };
    let settings = EmbeddedGraphLimits {
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
        bases: meta_limits(),
        dependencies: EmbeddedDependencyLimits {
            entries: n(),
            retained_bytes: n(),
            work: n(),
        },
    };
    let mut owner = RuntimeLocatedStylesheetGraph::new(
        PublicationLimits {
            requests: NonZeroU64::new(30).unwrap(),
            revisions: NonZeroU64::new(30).unwrap(),
        },
        n(),
    );
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let work =
        reader.load_stylesheet_graph(request, Arc::clone(&ids), context(), source(), settings);
    fn owned_send<T: Send + 'static>(_: &T) {}
    owned_send(&work);
    assert_eq!(reader.in_flight(), 1);
    let (completion, report) = block_on(work);
    let report = report.unwrap();
    assert_eq!(report.entries().len(), 2);
    assert_eq!(report.entries()[0].version(), Some([0].as_slice()));
    assert_eq!(report.entries()[1].version(), None);
    assert_eq!(
        report.entries()[1].acquisition_uri(),
        "app:/generated/child.css"
    );
    assert!(report.rejection().is_none());
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().graph().rules().count(), 1);
    state.lock().unwrap().missing_child = false;
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_stylesheet_graph(
        request,
        Arc::clone(&ids),
        context(),
        source(),
        settings,
    ));
    assert_eq!(
        report
            .unwrap()
            .entries()
            .iter()
            .filter(|e| e.version().is_some())
            .count(),
        2
    );
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    let active = owner.active().unwrap();
    assert_eq!(active.graph().rules().count(), 2);
    let child = active.graph().resources()[1].resource;
    assert_eq!(
        active.effective_base(child, n()).unwrap(),
        Some("app:/nested/child.css")
    );
    assert_eq!(active.graph().cuts()[0].reason, ImportCutReason::Cycle);
    let revision = owner.active_revision();
    {
        let mut state = state.lock().unwrap();
        state.revision = 1;
        state.missing_child = true;
    }
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_stylesheet_graph(
        request,
        Arc::clone(&ids),
        context(),
        source(),
        settings,
    ));
    assert_eq!(report.unwrap().entries()[0].version(), Some([1].as_slice()));
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(owner.active_revision(), revision);
    assert!(
        owner.active().unwrap().graph().resources()[0]
            .candidate
            .source()
            .text()
            .contains("color:red")
    );
    state.lock().unwrap().empty = true;
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_stylesheet_graph(
        request,
        Arc::clone(&ids),
        context(),
        source(),
        settings,
    ));
    assert_eq!(report.unwrap().entries().len(), 1);
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().graph().rules().count(), 0);
    {
        let mut state = state.lock().unwrap();
        state.empty = false;
        state.missing_child = false;
    }
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_stylesheet_graph(
        request,
        Arc::clone(&ids),
        context(),
        source(),
        EmbeddedGraphLimits {
            dependencies: EmbeddedDependencyLimits {
                entries: NonZeroUsize::MIN,
                ..settings.dependencies
            },
            ..settings
        },
    ));
    assert_eq!(
        report.unwrap().rejection(),
        Some(EmbeddedDependencyError::Entries)
    );
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(_)
    ));
    let before = calls.load(Ordering::SeqCst);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let work = reader.load_stylesheet_graph(
        request,
        Arc::clone(&ids),
        context(),
        source(),
        EmbeddedGraphLimits {
            stylesheet: EmbeddedStylesheetLimits {
                metadata_bytes: NonZeroUsize::MIN,
                ..settings.stylesheet
            },
            ..settings
        },
    );
    assert_eq!(reader.in_flight(), 0);
    let (completion, report) = block_on(work);
    assert!(report.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), before);
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(_)
    ));
    for (dependencies, expected) in [
        (
            EmbeddedDependencyLimits {
                retained_bytes: NonZeroUsize::MIN,
                ..settings.dependencies
            },
            EmbeddedDependencyError::Bytes,
        ),
        (
            EmbeddedDependencyLimits {
                work: NonZeroUsize::MIN,
                ..settings.dependencies
            },
            EmbeddedDependencyError::Work,
        ),
    ] {
        let before = calls.load(Ordering::SeqCst);
        let request = owner.request(0, context(), graph_limits()).unwrap();
        let (completion, report) = block_on(reader.load_stylesheet_graph(
            request,
            Arc::clone(&ids),
            context(),
            source(),
            EmbeddedGraphLimits {
                dependencies,
                ..settings
            },
        ));
        assert_eq!(report.unwrap().rejection(), Some(expected));
        assert_eq!(calls.load(Ordering::SeqCst), before);
        assert!(matches!(
            owner.finish(completion).unwrap(),
            SheetCompletion::Failed(_)
        ));
    }
    let before = calls.load(Ordering::SeqCst);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_stylesheet_graph(
        request,
        Arc::clone(&ids),
        context(),
        EmbeddedGraphSource {
            reference: "https://example.invalid/root.css".into(),
            ..source()
        },
        settings,
    ));
    assert!(report.unwrap().entries().is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), before);
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(_)
    ));
    let current = source();
    let held = reader
        .read_stylesheet(
            current.base,
            current.reference,
            current.encoding,
            context(),
            settings.stylesheet,
        )
        .unwrap();
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_stylesheet_graph(
        request,
        Arc::clone(&ids),
        context(),
        source(),
        settings,
    ));
    assert!(report.is_none());
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert_eq!(reader.in_flight(), 1);
    block_on(held).unwrap();
    assert_eq!(reader.in_flight(), 0);
    let request = owner.request(0, context(), graph_limits()).unwrap();
    let (completion, report) = block_on(reader.load_stylesheet_graph(
        request,
        Arc::clone(&ids),
        context(),
        EmbeddedGraphSource {
            encoding: iced_css::resources::stylesheet::StylesheetEncoding {
                transport: Some(b"unknown-test-charset".to_vec()),
                ..Default::default()
            },
            ..source()
        },
        settings,
    ));
    let report = report.unwrap();
    assert_eq!(report.entries().len(), 1);
    assert_eq!(report.entries()[0].version(), None);
    assert!(matches!(
        owner.finish(completion).unwrap(),
        SheetCompletion::Failed(_)
    ));
    use iced_css::host::stylesheet::embedded_graph::{
        EmbeddedRuntimeLimits, RuntimeEmbeddedStylesheetGraph,
    };
    use iced_css_core::resources::triggers::{RequestError, RequestReason, RuntimeMode};
    let runtime_limits = EmbeddedRuntimeLimits {
        acquisition: settings,
        graph: graph_limits(),
    };
    for mode in [RuntimeMode::Manual, RuntimeMode::Auto] {
        {
            let mut state = state.lock().unwrap();
            state.revision = 1;
            state.empty = false;
            state.missing_child = false;
        }
        let make_reader = || {
            EmbeddedReader::new(
                "app".into(),
                Resolver {
                    state: Arc::clone(&state),
                    calls: Arc::clone(&calls),
                },
                NonZeroUsize::MIN,
                uri,
            )
            .unwrap()
        };
        let mut runtime = RuntimeEmbeddedStylesheetGraph::new(
            make_reader(),
            mode,
            context(),
            PublicationLimits {
                requests: NonZeroU64::new(30).unwrap(),
                revisions: NonZeroU64::new(30).unwrap(),
            },
            n(),
            IdentityLimits {
                entries: n(),
                bytes: n(),
                work: n(),
            },
        )
        .unwrap();
        let before = calls.load(Ordering::SeqCst);
        for reason in [
            RequestReason::LocalChange,
            RequestReason::Reevaluation,
            RequestReason::CacheExpiry,
            RequestReason::CacheEviction,
        ] {
            assert!(matches!(
                runtime.request(reason, source(), runtime_limits),
                Err(RequestError::Unauthorized)
            ));
        }
        assert_eq!(calls.load(Ordering::SeqCst), before);
        let work = runtime
            .request(RequestReason::FirstAcquisition, source(), runtime_limits)
            .unwrap();
        owned_send(&work);
        let completion = block_on(work);
        assert!(runtime.is_current(&completion));
        assert!(runtime.pending_dependencies(&completion).is_some());
        assert!(matches!(
            runtime.finish(completion).unwrap(),
            SheetCompletion::Published(_)
        ));
        let revision = runtime.active_revision();
        assert_eq!(
            runtime.active_dependencies().unwrap().entries()[0].version(),
            Some([1].as_slice())
        );
        let retained_base = runtime.configuration().unwrap().source().base.as_ptr();
        assert!(matches!(
            runtime.request(RequestReason::FirstAcquisition, source(), runtime_limits),
            Err(RequestError::Unauthorized)
        ));
        {
            let mut state = state.lock().unwrap();
            state.revision = 2;
            state.missing_child = true;
        }
        let completion = block_on(runtime.retry(RequestReason::EmbeddedNotification).unwrap());
        assert_eq!(
            runtime.configuration().unwrap().source().base.as_ptr(),
            retained_base
        );
        assert!(matches!(
            runtime.finish(completion).unwrap(),
            SheetCompletion::Failed(_)
        ));
        assert_eq!(runtime.active_revision(), revision);
        assert_eq!(
            runtime.active_dependencies().unwrap().entries()[0].version(),
            Some([1].as_slice())
        );
        assert_eq!(
            runtime.candidate_dependencies().unwrap().entries()[0].version(),
            Some([2].as_slice())
        );
        let before = calls.load(Ordering::SeqCst);
        let completion = block_on(
            runtime
                .request(
                    RequestReason::ExplicitReload,
                    source(),
                    EmbeddedRuntimeLimits {
                        acquisition: EmbeddedGraphLimits {
                            stylesheet: EmbeddedStylesheetLimits {
                                metadata_bytes: NonZeroUsize::MIN,
                                ..settings.stylesheet
                            },
                            ..settings
                        },
                        ..runtime_limits
                    },
                )
                .unwrap(),
        );
        assert_eq!(calls.load(Ordering::SeqCst), before);
        assert_eq!(
            runtime.configuration().unwrap().source().base.as_ptr(),
            retained_base
        );
        assert!(matches!(
            runtime.finish(completion).unwrap(),
            SheetCompletion::Failed(_)
        ));
        assert!(runtime.candidate_dependencies().is_none());
        assert_eq!(
            runtime.active_dependencies().unwrap().entries()[0].version(),
            Some([1].as_slice())
        );
        {
            let mut state = state.lock().unwrap();
            state.empty = true;
            state.missing_child = false;
        }
        let completion = block_on(runtime.retry(RequestReason::EmbeddedNotification).unwrap());
        assert!(matches!(
            runtime.finish(completion).unwrap(),
            SheetCompletion::Published(_)
        ));
        assert!(runtime.candidate_dependencies().is_none());
        assert_eq!(runtime.active().unwrap().graph().rules().count(), 0);
        let older = block_on(runtime.retry(RequestReason::ExplicitReload).unwrap());
        {
            let mut state = state.lock().unwrap();
            state.empty = false;
            state.missing_child = true;
            state.revision = 3;
        }
        let newer = block_on(runtime.retry(RequestReason::EmbeddedNotification).unwrap());
        assert!(!runtime.is_current(&older));
        assert!(runtime.pending_dependencies(&older).is_none());
        assert!(matches!(
            runtime.finish(newer).unwrap(),
            SheetCompletion::Failed(_)
        ));
        let revision = runtime.active_revision();
        assert!(matches!(
            runtime.finish(older).unwrap(),
            SheetCompletion::Ignored
        ));
        assert_eq!(runtime.active_revision(), revision);
        assert_eq!(
            runtime.candidate_dependencies().unwrap().entries()[0].version(),
            Some([3].as_slice())
        );
        let foreign_completion = block_on(runtime.retry(RequestReason::ExplicitReload).unwrap());
        let mut foreign = RuntimeEmbeddedStylesheetGraph::new(
            make_reader(),
            mode,
            context(),
            PublicationLimits {
                requests: NonZeroU64::MIN,
                revisions: NonZeroU64::MIN,
            },
            n(),
            IdentityLimits {
                entries: n(),
                bytes: n(),
                work: n(),
            },
        )
        .unwrap();
        assert!(foreign.pending_dependencies(&foreign_completion).is_none());
        assert!(matches!(
            foreign.finish(foreign_completion).unwrap(),
            SheetCompletion::Ignored
        ));
        assert!(
            foreign.active_dependencies().is_none() && foreign.candidate_dependencies().is_none()
        );
        state.lock().unwrap().empty = true;
        let work = foreign
            .request(RequestReason::FirstAcquisition, source(), runtime_limits)
            .unwrap();
        assert!(matches!(
            foreign.request(RequestReason::ExplicitReload, source(), runtime_limits),
            Err(RequestError::Exhausted(_))
        ));
        assert!(matches!(
            foreign.finish(block_on(work)).unwrap(),
            SheetCompletion::Published(_)
        ));
    }
    use iced_css::host::stylesheet::embedded_coordinator::*;
    use iced_css::resources::watch_work::{WatchWorkError, WatchWorkLimits};
    {
        let mut state = state.lock().unwrap();
        state.empty = false;
        state.missing_child = false;
        state.revision = 4;
    }
    let make_reader = || {
        EmbeddedReader::new(
            "app".into(),
            Resolver {
                state: Arc::clone(&state),
                calls: Arc::clone(&calls),
            },
            NonZeroUsize::MIN,
            uri,
        )
        .unwrap()
    };
    let config = EmbeddedStylesheetHostConfig {
        mode: RuntimeMode::Auto,
        context: context(),
        publication: PublicationLimits {
            requests: NonZeroU64::new(30).unwrap(),
            revisions: NonZeroU64::new(30).unwrap(),
        },
        diagnostic_work: n(),
        identities: IdentityLimits {
            entries: n(),
            bytes: n(),
            work: n(),
        },
        runtime: runtime_limits,
        work: WatchWorkLimits {
            slots: NonZeroUsize::MIN,
            retained_bytes: n(),
        },
    };
    let mut host = EmbeddedStylesheetHost::new(make_reader(), config).unwrap();
    let notification = host.notification();
    let ignored = host.update(notification);
    assert!(ignored.outcome.is_none() && ignored.work.is_none());
    assert!(!host.is_busy() && host.is_dirty());
    let first = host
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    assert!(host.is_busy() && !host.is_dirty());
    assert_eq!(host.pending_work().unwrap(), 1);
    assert!(matches!(
        host.request(RequestReason::ExplicitReload, source()),
        Err(EmbeddedStylesheetRequestError::Work(WatchWorkError::Full))
    ));
    let message = block_on(first);
    #[derive(Clone, Debug)]
    enum EmbeddedAppMessage {
        Styles(EmbeddedStylesheetMessage<std::io::Error>),
        Reload,
    }
    let app_message = EmbeddedAppMessage::Styles(message.clone());
    if let EmbeddedAppMessage::Styles(message) = app_message {
        assert!(message.is_live());
    }
    let _: iced::Element<'_, EmbeddedAppMessage> = iced::widget::button("Reload embedded styles")
        .on_press(EmbeddedAppMessage::Reload)
        .into();
    let duplicate = message.clone();
    let finished = host.update(message);
    assert!(matches!(
        finished.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert!(finished.work.is_none() && !host.is_busy());
    let revision = host.active_revision();
    let ignored = host.update(duplicate);
    assert!(ignored.outcome.is_none() && ignored.work.is_none());
    assert_eq!(host.active_revision(), revision);
    let before = calls.load(Ordering::SeqCst);
    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    for _ in 0..5 {
        let notification = host.notification();
        let update = host.update(notification);
        assert!(update.outcome.is_none() && update.work.is_none());
    }
    assert!(host.is_busy() && host.is_dirty());
    let finished = host.update(block_on(work));
    assert!(matches!(
        finished.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert!(host.is_busy() && !host.is_dirty());
    assert_eq!(host.pending_work().unwrap(), 1);
    let repeated = host.update(block_on(finished.work.unwrap()));
    assert!(matches!(
        repeated.outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    assert!(repeated.work.is_none() && !host.is_busy());
    assert_eq!(host.pending_work().unwrap(), 0);
    assert_eq!(calls.load(Ordering::SeqCst) - before, 4);
    let notification = host.notification();
    let duplicate = notification.clone();
    let update = host.update(notification);
    let ignored = host.update(duplicate);
    assert!(ignored.work.is_none() && !host.is_dirty());
    host.update(block_on(update.work.unwrap()));
    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let message = block_on(work);
    let duplicate = message.clone();
    drop(message);
    assert!(host.is_busy());
    drop(duplicate);
    assert!(!host.is_busy());
    let notification = host.notification();
    let update = host.update(notification);
    assert!(matches!(
        host.update(block_on(update.work.unwrap()))
            .outcome
            .unwrap()
            .unwrap(),
        SheetCompletion::Published(_)
    ));
    let two_slots = EmbeddedStylesheetHostConfig {
        work: WatchWorkLimits {
            slots: NonZeroUsize::new(2).unwrap(),
            ..config.work
        },
        ..config
    };
    let mut current = EmbeddedStylesheetHost::new(make_reader(), two_slots).unwrap();
    let older = block_on(
        current
            .request(RequestReason::FirstAcquisition, source())
            .unwrap(),
    );
    let newer = current
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let ignored = current.update(older);
    assert!(ignored.outcome.is_none() && ignored.work.is_none() && current.is_busy());
    let message = block_on(newer);
    assert!(host.update(message.clone()).work.is_none());
    assert!(matches!(
        current.update(message).outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    let mut queued = EmbeddedStylesheetHost::new(make_reader(), config).unwrap();
    let message = block_on(
        queued
            .request(RequestReason::FirstAcquisition, source())
            .unwrap(),
    );
    let duplicate = message.clone();
    drop(queued);
    assert!(!message.is_live() && !duplicate.is_live());
    let ignored = host.update(message);
    assert!(ignored.outcome.is_none() && ignored.work.is_none());
    drop(duplicate);
    let mut early = EmbeddedStylesheetHost::new(make_reader(), config).unwrap();
    let work = early
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    drop(early);
    let closed = block_on(work);
    assert!(!closed.is_live());
    assert!(host.update(closed).work.is_none());
    let mut once = EmbeddedStylesheetHost::new(
        make_reader(),
        EmbeddedStylesheetHostConfig {
            publication: PublicationLimits {
                requests: NonZeroU64::MIN,
                revisions: NonZeroU64::MIN,
            },
            ..two_slots
        },
    )
    .unwrap();
    let first = once
        .request(RequestReason::FirstAcquisition, source())
        .unwrap();
    assert!(matches!(
        once.request(RequestReason::ExplicitReload, source()),
        Err(EmbeddedStylesheetRequestError::Request(
            RequestError::Exhausted(_)
        ))
    ));
    assert!(once.is_busy());
    assert!(matches!(
        once.update(block_on(first)).outcome.unwrap().unwrap(),
        SheetCompletion::Published(_)
    ));
    let work = host
        .request(RequestReason::ExplicitReload, source())
        .unwrap();
    let _task: iced::Task<EmbeddedStylesheetMessage<std::io::Error>> = work.into_task();
}

#[path = "located_runtime/http_graph.rs"]
mod http_graph;
