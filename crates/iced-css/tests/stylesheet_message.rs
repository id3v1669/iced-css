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
            "iced-css-sheet-message-{}-{}",
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
use iced_css::host::stylesheet::file::{FileStylesheetCompletion, FileStylesheetError};
use iced_css_core::stylesheet::publication::SheetCompletion;
fn owner() -> RuntimeStylesheet {
    RuntimeStylesheet::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
        n(),
    )
}
#[test]
fn local_messages_publish_only_when_delivered_and_preserve_failed_reload() {
    let temp = Temp::new();
    let path = temp.0.join("theme.css");
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut owner = owner();
    std::fs::write(&path, b"a{width:bogus;height:2px}").unwrap();
    let work = owner
        .request_file(
            &reader,
            path.clone(),
            StylesheetEncoding::default(),
            context(),
            io_limits(),
        )
        .unwrap();
    fn transport<T: Send + 'static>(_: &T) {}
    transport(&work);
    let message = iced::futures::executor::block_on(work);
    transport(&message);
    assert!(owner.active().is_none());
    assert!(matches!(
        owner.finish_file(message).unwrap(),
        SheetCompletion::Published(_)
    ));
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
        1
    );
    let revision = owner.active_revision();
    std::fs::remove_file(&path).unwrap();
    let failed = iced::futures::executor::block_on(
        owner
            .request_file(
                &reader,
                path.clone(),
                StylesheetEncoding::default(),
                context(),
                io_limits(),
            )
            .unwrap(),
    );
    assert!(matches!(
        owner.finish_file(failed).unwrap(),
        SheetCompletion::Failed(FileStylesheetError::Background(
            BackgroundStylesheetError::Read(_)
        ))
    ));
    assert_eq!(owner.active_revision(), revision);
    assert_eq!(
        owner.active().unwrap().source().text(),
        "a{width:bogus;height:2px}"
    );
    std::fs::write(&path, b"").unwrap();
    let empty = iced::futures::executor::block_on(
        owner
            .request_file(
                &reader,
                path,
                StylesheetEncoding::default(),
                context(),
                io_limits(),
            )
            .unwrap(),
    );
    assert!(matches!(
        owner.finish_file(empty).unwrap(),
        SheetCompletion::Published(_)
    ));
    assert_eq!(owner.active().unwrap().source().text(), "");
}
#[test]
fn rejected_latest_submission_supersedes_unread_older_success() {
    let temp = Temp::new();
    let path = temp.0.join("theme.css");
    std::fs::write(&path, b"a{color:red}").unwrap();
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut owner = owner();
    let old = owner
        .request_file(
            &reader,
            path.clone(),
            StylesheetEncoding::default(),
            context(),
            io_limits(),
        )
        .unwrap();
    let latest = owner
        .request_file(
            &reader,
            path,
            StylesheetEncoding::default(),
            context(),
            io_limits(),
        )
        .unwrap();
    let message = iced::futures::executor::block_on(latest);
    assert!(matches!(
        owner.finish_file(message).unwrap(),
        SheetCompletion::Failed(FileStylesheetError::Submit(_))
    ));
    let old = iced::futures::executor::block_on(old);
    assert!(matches!(
        owner.finish_file(old).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(owner.active().is_none());
    assert_eq!(reader.in_flight(), 0);
}
#[test]
fn local_completion_cannot_publish_to_another_owner() {
    let temp = Temp::new();
    let path = temp.0.join("theme.css");
    std::fs::write(&path, b"a{color:red}").unwrap();
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut original = owner();
    let work = original
        .request_file(
            &reader,
            path,
            StylesheetEncoding::default(),
            context(),
            io_limits(),
        )
        .unwrap();
    let message = iced::futures::executor::block_on(work);
    let mut other = owner();
    other.request().unwrap();
    assert!(matches!(
        other.finish_file(message).unwrap(),
        SheetCompletion::Ignored
    ));
    assert!(other.active().is_none());
    assert!(original.active().is_none());
}
#[test]
fn request_exhaustion_starts_no_worker_and_future_fits_iced_task() {
    let temp = Temp::new();
    let path = temp.0.join("theme.css");
    let reader = FileReader::new(NonZeroUsize::MIN);
    let mut owner = RuntimeStylesheet::new(
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        n(),
    );
    let work = owner
        .request_file(
            &reader,
            path.clone(),
            StylesheetEncoding::default(),
            context(),
            io_limits(),
        )
        .unwrap();
    let message = iced::futures::executor::block_on(work);
    assert!(matches!(
        owner.finish_file(message).unwrap(),
        SheetCompletion::Failed(_)
    ));
    assert!(owner.active().is_none());
    assert!(
        owner
            .request_file(
                &reader,
                path.clone(),
                StylesheetEncoding::default(),
                context(),
                io_limits()
            )
            .is_err()
    );
    assert_eq!(reader.in_flight(), 0);
    let mut next = RuntimeStylesheet::new(
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        n(),
    );
    let future = next
        .request_file(
            &reader,
            path,
            StylesheetEncoding::default(),
            context(),
            io_limits(),
        )
        .unwrap();
    let task: iced::Task<FileStylesheetCompletion> =
        iced::Task::perform(future, std::convert::identity);
    drop(task);
    assert!(next.active().is_none());
}
