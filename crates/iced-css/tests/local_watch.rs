use iced::futures::{
    StreamExt,
    channel::oneshot,
    executor::block_on,
    future::{Either, select},
};
use iced_css::resources::watch::*;
use std::{
    fs,
    num::NonZeroUsize,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
fn n(x: usize) -> NonZeroUsize {
    NonZeroUsize::new(x).unwrap()
}
fn config(interval: Duration) -> WatchConfig {
    WatchConfig {
        recovery_interval: interval,
        directories: n(10),
        retained_path_bytes: n(10000),
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "iced-css-local-watch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn notice(stream: &mut LocalWatchStream, predicate: impl Fn(WatchNotice) -> bool) -> WatchNotice {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = oneshot::channel();
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timer = std::thread::spawn(move || {
            if cancel_rx.recv_timeout(remaining).is_err() {
                let _ = done_tx.send(());
            }
        });
        let result = block_on(select(stream.next(), done_rx));
        drop(cancel_tx);
        timer.join().unwrap();
        let value = match result {
            Either::Left((Some(value), _)) => value,
            _ => panic!("watch stream stopped or timed out"),
        };
        if predicate(value) {
            return value;
        }
    }
}
#[test]
fn native_parent_events_reach_an_iced_stream_and_control_stops_with_stream() {
    let temp = Temp::new();
    let (control, mut stream) = block_on(LocalWatchStream::start(
        vec![temp.0.clone()],
        config(Duration::from_secs(30)),
    ))
    .unwrap();
    fs::write(temp.0.join("style.css"), b"a{}").unwrap();
    let value = notice(&mut stream, |v| v.native_change);
    assert!(value.dirty_all_local);
    drop(stream);
    assert!(control.is_shutdown_requested());
    control.resume();
    assert!(control.is_shutdown_requested());
}
#[test]
fn periodic_rescan_and_explicit_resume_do_not_need_a_file_event() {
    let temp = Temp::new();
    let (control, mut stream) = block_on(LocalWatchStream::start(
        vec![temp.0.clone()],
        config(Duration::from_millis(25)),
    ))
    .unwrap();
    assert!(notice(&mut stream, |v| v.periodic_rescan).dirty_all_local);
    control.resume();
    assert!(notice(&mut stream, |v| v.resumed).dirty_all_local);
}
#[test]
fn missing_directory_keeps_recovery_alive_and_is_registered_after_restoration() {
    let temp = Temp::new();
    let missing = temp.0.join("restored");
    let (control, mut stream) = block_on(LocalWatchStream::start(
        vec![missing.clone()],
        config(Duration::from_millis(25)),
    ))
    .unwrap();
    assert!(notice(&mut stream, |v| v.registration_loss).dirty_all_local);
    fs::create_dir(&missing).unwrap();
    control.resume();
    notice(&mut stream, |v| v.resumed);
    fs::write(missing.join("style.css"), b"b{}").unwrap();
    assert!(notice(&mut stream, |v| v.native_change).dirty_all_local);
}
#[test]
fn invalid_configuration_is_rejected_before_native_registration() {
    assert!(matches!(
        block_on(LocalWatchStream::start(vec![], config(Duration::ZERO))),
        Err(WatchError::Interval)
    ));
    assert!(matches!(
        block_on(LocalWatchStream::start(
            vec![PathBuf::from("relative")],
            config(Duration::from_secs(1))
        )),
        Err(WatchError::RelativePath { index: 0 })
    ));
    let temp = Temp::new();
    let mut cap = config(Duration::from_secs(1));
    cap.retained_path_bytes = n(1);
    assert!(matches!(
        block_on(LocalWatchStream::start(vec![temp.0.clone()], cap)),
        Err(WatchError::Paths)
    ));
}

#[test]
fn auto_local_notices_drive_failed_read_retention_and_restored_publication() {
    use iced_css::resources::{
        local::ReadBodyLimits,
        worker::{BackgroundReadLimits, FileReader},
    };
    use iced_css_core::resources::{
        body::BodyLimits,
        publication::{PublicationLimits, PublicationStatus},
        slot::{ResourceCompletion, RuntimeResourceSlot},
        triggers::{RequestReason, ResourceKind, RuntimeMode},
    };
    use std::num::NonZeroU64;
    let temp = Temp::new();
    let path = temp.0.join("style.css");
    fs::write(&path, b"old").unwrap();
    let (_control, mut stream) = block_on(LocalWatchStream::start(
        vec![temp.0.clone()],
        config(Duration::from_secs(30)),
    ))
    .unwrap();
    let reader = FileReader::new(n(1));
    let read = BackgroundReadLimits {
        read: ReadBodyLimits {
            body: BodyLimits {
                retained_bytes: n(100),
                work: n(10000),
            },
            reads: n(20),
        },
        path_bytes: n(4096),
    };
    let mut slot = RuntimeResourceSlot::new(
        1,
        RuntimeMode::Auto,
        ResourceKind::LocalFile,
        PublicationLimits {
            requests: NonZeroU64::new(10).unwrap(),
            revisions: NonZeroU64::new(10).unwrap(),
        },
    );
    let first = slot.request(RequestReason::FirstAcquisition).unwrap();
    assert!(matches!(
        slot.complete(
            first,
            block_on(reader.read_file(path.clone(), read).unwrap())
        ),
        ResourceCompletion::Published { .. }
    ));
    fs::remove_file(&path).unwrap();
    notice(&mut stream, |v| v.native_change);
    let removed = slot.request(RequestReason::LocalChange).unwrap();
    assert!(matches!(
        slot.complete(
            removed,
            block_on(reader.read_file(path.clone(), read).unwrap())
        ),
        ResourceCompletion::Failed(_)
    ));
    assert_eq!(slot.active().unwrap().1.as_slice(), b"old");
    assert_eq!(slot.status(), PublicationStatus::Failed);
    fs::write(&path, b"restored").unwrap();
    notice(&mut stream, |v| v.native_change);
    let restored = slot.request(RequestReason::LocalChange).unwrap();
    assert!(matches!(
        slot.complete(restored, block_on(reader.read_file(path, read).unwrap())),
        ResourceCompletion::Published { .. }
    ));
    assert_eq!(slot.active().unwrap().1.as_slice(), b"restored");
    assert_eq!(slot.active().unwrap().0.get(), 2);
}
