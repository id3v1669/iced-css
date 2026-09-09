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
    decode::DecodeLimits,
    encoding_selection::{EncodingMetadata, SelectionLimits},
    unicode_intake::IntakeLimits,
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

use iced_css::host::stylesheet::RuntimeStylesheet;
use iced_css_core::{
    resources::publication::PublicationLimits, stylesheet::publication::SheetCompletion,
};
use std::sync::Mutex;
struct Logger(Mutex<Vec<String>>);
static LOGGER: Logger = Logger(Mutex::new(Vec::new()));
impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target() == "iced_css::stylesheet"
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            assert_eq!(record.level(), log::Level::Error);
            self.0.lock().unwrap().push(record.args().to_string());
        }
    }
    fn flush(&self) {}
}
#[test]
fn rejected_requests_log_once_and_keep_the_last_good_sheet() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Error);
    let mut owner = RuntimeStylesheet::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::MIN,
        },
        n(),
    );
    let failed = owner.request().unwrap();
    let duplicate = failed.clone();
    assert!(matches!(
        owner
            .finish(failed, Err::<UnicodeStylesheetCandidate, _>("read failed"))
            .unwrap(),
        SheetCompletion::Failed("read failed")
    ));
    owner
        .finish(duplicate, Err::<UnicodeStylesheetCandidate, _>("duplicate"))
        .unwrap();
    assert!(owner.active().is_none());
    let first = owner.request().unwrap();
    let candidate = parse_separate_unicode_stylesheet(
        b"a{color:red}",
        EncodingMetadata::default(),
        context(),
        intake_limits(),
    )
    .unwrap();
    owner.finish::<&str>(first, Ok(candidate)).unwrap();
    let stale = owner.request().unwrap();
    let latest = owner.request().unwrap();
    owner
        .finish(
            latest,
            Err::<UnicodeStylesheetCandidate, _>("decode failed"),
        )
        .unwrap();
    owner
        .finish(stale, Err::<UnicodeStylesheetCandidate, _>("stale"))
        .unwrap();
    assert_eq!(owner.active().unwrap().source().text(), "a{color:red}");
    let exhausted = owner.request().unwrap();
    let candidate = parse_separate_unicode_stylesheet(
        b"",
        EncodingMetadata::default(),
        context(),
        intake_limits(),
    )
    .unwrap();
    assert!(owner.finish::<&str>(exhausted, Ok(candidate)).is_err());
    assert_eq!(owner.active().unwrap().source().text(), "a{color:red}");
    let mut limited = RuntimeStylesheet::new(
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        n(),
    );
    limited.request().unwrap();
    assert!(limited.request().is_err());
    let logs = LOGGER.0.lock().unwrap();
    assert_eq!(logs.len(), 4);
    assert!(logs[0].contains("read failed"));
    assert!(logs[1].contains("decode failed"));
    assert!(logs[2].contains("Revisions"));
    assert!(logs[3].contains("Requests"));
    drop(logs);
    let mut recovering = RuntimeStylesheet::new(
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
        n(),
    );
    fn publish(owner: &mut RuntimeStylesheet, css: &str) {
        let candidate = parse_separate_unicode_stylesheet(
            css.as_bytes(),
            EncodingMetadata::default(),
            context(),
            intake_limits(),
        )
        .unwrap();
        let ticket = owner.request().unwrap();
        assert!(matches!(
            owner.finish::<&str>(ticket, Ok(candidate)).unwrap(),
            SheetCompletion::Published(_)
        ));
    }
    publish(&mut recovering, "a{width:bogus;height:2px}");
    assert_eq!(LOGGER.0.lock().unwrap().len(), 5);
    publish(&mut recovering, "a{width:bogus;height:2px}");
    assert_eq!(LOGGER.0.lock().unwrap().len(), 5);
    publish(&mut recovering, "a{color:red;width:bogus;height:2px}");
    assert_eq!(LOGGER.0.lock().unwrap().len(), 5);
    publish(&mut recovering, "a{width:bogus;width:bogus;height:2px}");
    assert_eq!(LOGGER.0.lock().unwrap().len(), 6);
    publish(&mut recovering, "a{height:2px}");
    publish(&mut recovering, "a{width:bogus;height:2px}");
    assert_eq!(LOGGER.0.lock().unwrap().len(), 7);
    let mut bounded = RuntimeStylesheet::new(
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
        NonZeroUsize::MIN,
    );
    publish(&mut bounded, "a{width:bogus;height:2px}");
    assert_eq!(
        bounded
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
    let logs = LOGGER.0.lock().unwrap();
    assert_eq!(logs.len(), 8);
    assert!(logs[4].contains("InvalidValue"));
    assert!(logs[7].contains("Work"));
}
