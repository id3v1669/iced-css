use iced::futures::{StreamExt, executor::block_on};
use iced_css::resources::watch::*;
use std::{num::NonZeroUsize, sync::Mutex, time::Duration};
struct Logger(Mutex<Vec<(log::Level, String)>>);
static LOGGER: Logger = Logger(Mutex::new(Vec::new()));
impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target() == "iced_css::stylesheet"
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            self.0
                .lock()
                .unwrap()
                .push((record.level(), record.args().to_string()));
        }
    }
    fn flush(&self) {}
}
#[test]
fn registration_failure_is_logged_once_per_transition_not_per_rescan() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    let missing = std::env::temp_dir().join(format!(
        "iced-css-watch-logging-{}-missing",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&missing);
    let (_control, mut stream) = block_on(LocalWatchStream::start(
        vec![missing.clone()],
        WatchConfig {
            recovery_interval: Duration::from_millis(10),
            directories: NonZeroUsize::new(1).unwrap(),
            retained_path_bytes: NonZeroUsize::new(10000).unwrap(),
        },
    ))
    .unwrap();
    let mut periodic = 0;
    while periodic < 5 {
        let notice = block_on(stream.next()).unwrap();
        periodic += usize::from(notice.periodic_rescan);
    }
    drop(stream);
    let records = LOGGER.0.lock().unwrap().clone();
    assert_eq!(records.len(), 1, "{records:?}");
    assert_eq!(records[0].0, log::Level::Warn);
    assert!(
        records[0].1.contains(&missing.display().to_string())
            && records[0].1.contains("not an existing directory"),
        "{records:?}"
    );
}
