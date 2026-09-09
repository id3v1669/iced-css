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

use iced_css::{
    host::stylesheet::RuntimeStylesheet,
    resources::{
        local::ReadBodyLimits,
        stylesheet::*,
        worker::{BackgroundReadLimits, FileReader},
    },
};
use iced_css_core::resources::{body::BodyLimits, publication::PublicationLimits};
use std::path::PathBuf;
fn io_limits() -> StylesheetReadLimits {
    StylesheetReadLimits {
        read: BackgroundReadLimits {
            path_bytes: n(),
            read: ReadBodyLimits {
                reads: n(),
                body: BodyLimits {
                    retained_bytes: n(),
                    work: n(),
                },
            },
        },
        metadata_bytes: n(),
        intake: intake_limits(),
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "iced-css-sheet-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn actual_local_load_and_failed_replacement_use_owned_publication() {
    let temp = Temp::new();
    let path = temp.0.join("theme.css");
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut owner = RuntimeStylesheet::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
        n(),
    );
    let first = owner.request().unwrap();
    let failed = iced::futures::executor::block_on(
        reader
            .read_stylesheet_file(
                path.clone(),
                StylesheetEncoding::default(),
                context(),
                io_limits(),
            )
            .unwrap(),
    );
    assert!(matches!(&failed, Err(BackgroundStylesheetError::Read(_))));
    owner.finish(first, failed).unwrap();
    assert!(owner.active().is_none());
    std::fs::write(&path, b"a{color:red;width:bogus;height:2px}").unwrap();
    let first = owner.request().unwrap();
    let result = iced::futures::executor::block_on(
        reader
            .read_stylesheet_file(
                path.clone(),
                StylesheetEncoding::default(),
                context(),
                io_limits(),
            )
            .unwrap(),
    );
    owner.finish(first, result).unwrap();
    let revision = owner.active_revision();
    assert_eq!(
        owner
            .active()
            .unwrap()
            .sheet()
            .local_screen_rules()
            .next()
            .unwrap()
            .declarations
            .len(),
        2
    );
    std::fs::remove_file(&path).unwrap();
    let next = owner.request().unwrap();
    let failed = iced::futures::executor::block_on(
        reader
            .read_stylesheet_file(path, StylesheetEncoding::default(), context(), io_limits())
            .unwrap(),
    );
    owner.finish(next, failed).unwrap();
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().source().text(),
        "a{color:red;width:bogus;height:2px}"
    );
}
#[test]
fn stylesheet_jobs_share_operation_and_unread_completion_capacity() {
    let temp = Temp::new();
    let path = temp.0.join("theme.css");
    std::fs::write(&path, b"a{color:red}").unwrap();
    let reader = FileReader::new(NonZeroUsize::MIN);
    let first = reader
        .read_stylesheet_file(
            path.clone(),
            StylesheetEncoding::default(),
            context(),
            io_limits(),
        )
        .unwrap();
    fn send<T: Send>(_: &T) {}
    send(&first);
    assert_eq!(reader.in_flight(), 1);
    assert!(
        reader
            .read_stylesheet_file(
                path.clone(),
                StylesheetEncoding::default(),
                context(),
                io_limits()
            )
            .is_err()
    );
    assert!(reader.read_file(path, io_limits().read).is_err());
    assert_eq!(
        iced::futures::executor::block_on(first)
            .unwrap()
            .source()
            .text(),
        "a{color:red}"
    );
    assert_eq!(reader.in_flight(), 0);
}
#[test]
fn metadata_path_body_and_intake_limits_fail_without_partial_candidates() {
    let temp = Temp::new();
    let path = temp.0.join("theme.css");
    std::fs::write(&path, b"a{color:red}").unwrap();
    let reader = FileReader::new(NonZeroUsize::MIN);
    let encoding = StylesheetEncoding {
        transport: Some(b"UTF-8".to_vec()),
        linking: None,
        referring: None,
    };
    let mut small = io_limits();
    small.metadata_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        reader.read_stylesheet_file(path.clone(), encoding, context(), small),
        Err(StylesheetSubmitError::MetadataBytes)
    ));
    assert_eq!(reader.in_flight(), 0);
    let mut small = io_limits();
    small.read.path_bytes = NonZeroUsize::MIN;
    assert!(
        reader
            .read_stylesheet_file(
                path.clone(),
                StylesheetEncoding::default(),
                context(),
                small
            )
            .is_err()
    );
    assert_eq!(reader.in_flight(), 0);
    let mut small = io_limits();
    small.read.read.body.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        iced::futures::executor::block_on(
            reader
                .read_stylesheet_file(
                    path.clone(),
                    StylesheetEncoding::default(),
                    context(),
                    small
                )
                .unwrap()
        ),
        Err(BackgroundStylesheetError::Read(_))
    ));
    let mut small = io_limits();
    small.intake.intake.decode.work = NonZeroUsize::MIN;
    assert!(matches!(
        iced::futures::executor::block_on(
            reader
                .read_stylesheet_file(
                    path.clone(),
                    StylesheetEncoding::default(),
                    context(),
                    small
                )
                .unwrap()
        ),
        Err(BackgroundStylesheetError::Intake(_))
    ));
    let encoding = StylesheetEncoding {
        transport: Some(b"UTF-8".to_vec()),
        linking: None,
        referring: None,
    };
    assert!(
        iced::futures::executor::block_on(
            reader
                .read_stylesheet_file(path, encoding, context(), io_limits())
                .unwrap()
        )
        .is_ok()
    );
}

#[test]
fn embedded_stylesheet_intake_preserves_version_base_and_worker_admission() {
    use iced::futures::executor::block_on;
    use iced_css::resources::embedded::*;
    use iced_css_core::resources::uri::UriLimits;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Resolver(Arc<AtomicUsize>);
    impl EmbeddedResolver for Resolver {
        type Error = std::io::Error;
        fn resolve(&self, request: EmbeddedRequest<'_>) -> Result<EmbeddedSource, Self::Error> {
            assert_eq!(std::thread::current().name(), Some("iced-css-resource"));
            self.0.fetch_add(1, Ordering::SeqCst);
            if request.acquisition_uri.ends_with("missing.css") {
                return Err(std::io::Error::other("missing embedded CSS"));
            }
            let text = "a{color:red}";
            let bytes = if request.acquisition_uri.ends_with("utf16.css") {
                text.encode_utf16().flat_map(u16::to_le_bytes).collect()
            } else {
                text.as_bytes().to_vec()
            };
            Ok(EmbeddedSource {
                bytes,
                version: b"version-7".to_vec(),
                dependency_base: "app:/generated/theme.css".into(),
            })
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let uri = UriLimits {
        bytes: n(),
        work: n(),
    };
    let reader = EmbeddedReader::new(
        "app".into(),
        Resolver(Arc::clone(&calls)),
        NonZeroUsize::MIN,
        uri,
    )
    .unwrap();
    let limits = EmbeddedStylesheetLimits {
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
    };
    let request = |name: &str, encoding, limits| {
        reader.read_stylesheet(
            "app:/styles/main.css".into(),
            name.into(),
            encoding,
            context(),
            limits,
        )
    };
    let work = request("theme.css", StylesheetEncoding::default(), limits).unwrap();
    fn send<T: Send + 'static>(_: &T) {}
    send(&work);
    assert_eq!(reader.in_flight(), 1);
    assert!(matches!(
        request("theme.css", StylesheetEncoding::default(), limits),
        Err(EmbeddedSubmitError::Worker(_))
    ));
    let parsed = block_on(work).unwrap();
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(parsed.stylesheet().source().text(), "a{color:red}");
    assert_eq!(parsed.stylesheet().sheet().local_screen_rules().count(), 1);
    assert_eq!(parsed.version(), b"version-7");
    assert_eq!(parsed.dependency_base(), "app:/generated/theme.css");
    let (sheet, version, base) = parsed.into_parts();
    assert_eq!(sheet.source().text(), "a{color:red}");
    assert_eq!(version, b"version-7");
    assert_eq!(base, "app:/generated/theme.css");
    let parsed = block_on(
        request(
            "utf16.css",
            StylesheetEncoding {
                transport: Some(b"utf-16le".to_vec()),
                linking: Some(b"windows-1252".to_vec()),
                referring: Some(b"windows-1252".to_vec()),
            },
            limits,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(parsed.stylesheet().source().text(), "a{color:red}");
    assert!(matches!(
        block_on(request("missing.css", StylesheetEncoding::default(), limits).unwrap()),
        Err(EmbeddedStylesheetError::Asset(EmbeddedError::Resolver(_)))
    ));
    let before = calls.load(Ordering::SeqCst);
    assert!(matches!(
        block_on(request("other:/theme.css", StylesheetEncoding::default(), limits).unwrap()),
        Err(EmbeddedStylesheetError::Asset(EmbeddedError::Namespace))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), before);
    assert!(matches!(
        request(
            "theme.css",
            StylesheetEncoding {
                transport: Some(b"utf-8".to_vec()),
                ..StylesheetEncoding::default()
            },
            EmbeddedStylesheetLimits {
                metadata_bytes: NonZeroUsize::MIN,
                ..limits
            }
        ),
        Err(EmbeddedSubmitError::Metadata)
    ));
    assert_eq!(reader.in_flight(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), before);
    let failed = block_on(
        request(
            "theme.css",
            StylesheetEncoding::default(),
            EmbeddedStylesheetLimits {
                asset: EmbeddedLimits {
                    body: BodyLimits {
                        retained_bytes: NonZeroUsize::MIN,
                        ..limits.asset.body
                    },
                    ..limits.asset
                },
                ..limits
            },
        )
        .unwrap(),
    );
    assert!(matches!(
        failed,
        Err(EmbeddedStylesheetError::Asset(EmbeddedError::BodyBytes))
    ));
    let failed = block_on(
        request(
            "theme.css",
            StylesheetEncoding::default(),
            EmbeddedStylesheetLimits {
                intake: StylesheetIntakeLimits {
                    intake: IntakeLimits {
                        decode: DecodeLimits {
                            work: NonZeroUsize::MIN,
                            ..limits.intake.intake.decode
                        },
                        ..limits.intake.intake
                    },
                    ..limits.intake
                },
                ..limits
            },
        )
        .unwrap(),
    );
    assert!(matches!(failed, Err(EmbeddedStylesheetError::Intake(_))));
}
