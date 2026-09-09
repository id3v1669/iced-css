use iced::futures::{
    StreamExt,
    channel::oneshot,
    executor::block_on,
    future::{Either, select},
};
use iced_css::resources::{
    watch::{LocalWatchStream, WatchConfig},
    watch_sessions::*,
};
use iced_css_core::resources::watch_ownership::*;
use std::{
    fs,
    num::{NonZeroU64, NonZeroUsize},
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn owner(v: u64) -> WatchOwnerId {
    WatchOwnerId::new(NonZeroU64::new(v).unwrap())
}
fn key(v: u64) -> WatchDependencyId {
    WatchDependencyId::new(NonZeroU64::new(v).unwrap())
}
fn host() -> WatchSessionHost {
    WatchSessionHost::new(WatchOwnershipLimits {
        owners: n(10),
        bindings: n(100),
        dependencies: n(20),
        retained_bytes: n(100000),
        work: n(1000000),
        revisions: NonZeroU64::new(100).unwrap(),
    })
    .unwrap()
}
fn config() -> WatchConfig {
    WatchConfig {
        recovery_interval: Duration::from_secs(30),
        directories: n(10),
        retained_path_bytes: n(100000),
    }
}
fn limits() -> WatchMappingLimits {
    WatchMappingLimits {
        bindings: n(20),
        input_bytes: n(100000),
        work: n(1000000),
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "iced-css-watch-session-{}-{}",
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
fn binding(id: u64, temp: &Temp) -> WatchDirectoryBinding {
    WatchDirectoryBinding {
        dependency: key(id),
        directory: temp.0.clone(),
    }
}
fn prepared(
    h: &WatchSessionHost,
    o: u64,
    keys: &[u64],
    bindings: Vec<WatchDirectoryBinding>,
) -> PreparedWatchSession {
    let keys: Vec<_> = keys.iter().copied().map(key).collect();
    let update = h.ownership().prepare_replace(owner(o), &keys).unwrap();
    block_on(prepare_watch_session(update, bindings, config(), limits())).unwrap()
}
fn native_notice(stream: &mut LocalWatchStream) {
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = oneshot::channel();
    let timer = std::thread::spawn(move || {
        if cancel_rx.recv_timeout(Duration::from_secs(5)).is_err() {
            let _ = done_tx.send(());
        }
    });
    let observed = block_on(async {
        let mut timeout = done_rx;
        loop {
            match select(stream.next(), timeout).await {
                Either::Left((Some(notice), remaining)) => {
                    if notice.native_change {
                        return true;
                    }
                    timeout = remaining;
                }
                _ => return false,
            }
        }
    });
    drop(cancel_tx);
    timer.join().unwrap();
    assert!(observed, "native coverage missing");
}
#[test]
fn new_union_is_live_before_old_session_is_returned_for_retirement() {
    let a = Temp::new();
    let b = Temp::new();
    let mut h = host();
    let first = prepared(&h, 1, &[10], vec![binding(10, &a)]);
    h.commit(first).unwrap();
    let old = h.active().unwrap().control().clone();
    let candidate = prepared(&h, 2, &[20], vec![binding(10, &a), binding(20, &b)]);
    assert!(
        !candidate
            .candidate_control()
            .unwrap()
            .is_shutdown_requested()
    );
    assert_eq!(h.ownership().snapshot().dependencies(), &[key(10)]);
    let commit = h.commit(candidate).unwrap();
    assert!(!old.is_shutdown_requested());
    assert!(commit.retired.is_some());
    drop(commit.retired);
    assert!(old.is_shutdown_requested());
    fs::write(b.0.join("candidate.css"), b"a{}").unwrap();
    native_notice(h.active_mut().unwrap().stream_mut());
    assert_eq!(h.ownership().snapshot().dependencies(), &[key(10), key(20)]);
}
#[test]
fn owner_only_changes_preserve_coverage_and_last_release_retires_it() {
    let a = Temp::new();
    let mut h = host();
    let first = prepared(&h, 1, &[10], vec![binding(10, &a)]);
    h.commit(first).unwrap();
    let control = h.active().unwrap().control().clone();
    let shared = prepared(&h, 2, &[10], vec![]);
    assert!(shared.candidate_control().is_none());
    assert!(h.commit(shared).unwrap().retired.is_none());
    assert!(!control.is_shutdown_requested());
    let release = h.ownership().prepare_release(owner(1)).unwrap();
    let release = block_on(prepare_watch_session(release, vec![], config(), limits())).unwrap();
    assert!(h.commit(release).unwrap().retired.is_none());
    let release = h.ownership().prepare_release(owner(2)).unwrap();
    let release = block_on(prepare_watch_session(release, vec![], config(), limits())).unwrap();
    let commit = h.commit(release).unwrap();
    assert!(h.active().is_none());
    assert!(!control.is_shutdown_requested());
    drop(commit.retired);
    assert!(control.is_shutdown_requested());
}
#[test]
fn stale_or_foreign_preparations_leave_current_session_live() {
    let a = Temp::new();
    let mut h = host();
    let first = prepared(&h, 1, &[10], vec![binding(10, &a)]);
    h.commit(first).unwrap();
    let active = h.active().unwrap().control().clone();
    let stale = h.ownership().prepare_release(owner(1)).unwrap();
    let stale = block_on(prepare_watch_session(stale, vec![], config(), limits())).unwrap();
    let shared = prepared(&h, 2, &[10], vec![]);
    h.commit(shared).unwrap();
    let error = h.commit(stale).unwrap_err();
    assert_eq!(error.reason, WatchCommitFailure::Stale);
    assert!(!active.is_shutdown_requested());
    drop(error);
    let foreign = host();
    let update = prepared(&foreign, 1, &[10], vec![binding(10, &a)]);
    let candidate = update.candidate_control().unwrap().clone();
    let error = h.commit(update).unwrap_err();
    assert_eq!(error.reason, WatchCommitFailure::WrongRegistry);
    assert!(!candidate.is_shutdown_requested());
    drop(error);
    assert!(candidate.is_shutdown_requested());
    assert!(!active.is_shutdown_requested());
}
#[test]
fn incomplete_ambiguous_and_unrelated_directory_mappings_cannot_prepare_coverage() {
    let a = Temp::new();
    let h = host();
    for (bindings, kind) in [
        (vec![], 0),
        (vec![binding(10, &a), binding(10, &a)], 1),
        (vec![binding(20, &a)], 2),
    ] {
        let update = h.ownership().prepare_replace(owner(1), &[key(10)]).unwrap();
        let result = block_on(prepare_watch_session(update, bindings, config(), limits()));
        assert!(matches!(
            (kind, result),
            (0, Err(WatchSessionError::MissingDependency(_)))
                | (1, Err(WatchSessionError::DuplicateDependency(_)))
                | (2, Err(WatchSessionError::UnknownDependency(_)))
        ));
        assert!(h.active().is_none());
        assert_eq!(h.ownership().revision(), 0);
    }
    let mut cap = limits();
    cap.input_bytes = n(1);
    let update = h.ownership().prepare_replace(owner(1), &[key(10)]).unwrap();
    assert!(matches!(
        block_on(prepare_watch_session(
            update,
            vec![binding(10, &a)],
            config(),
            cap
        )),
        Err(WatchSessionError::InputBytes)
    ));
}

#[test]
fn directory_aliases_share_one_watch_and_preparation_limits_are_enforced() {
    let a = Temp::new();
    let h = host();
    let update = h
        .ownership()
        .prepare_replace(owner(1), &[key(10), key(20)])
        .unwrap();
    let mut one = config();
    one.directories = n(1);
    let prepared = block_on(prepare_watch_session(
        update,
        vec![binding(10, &a), binding(20, &a)],
        one,
        limits(),
    ))
    .unwrap();
    assert!(prepared.candidate_control().is_some());
    drop(prepared);
    let update = h.ownership().prepare_replace(owner(1), &[key(10)]).unwrap();
    let mut cap = limits();
    cap.work = n(1);
    assert!(matches!(
        block_on(prepare_watch_session(
            update,
            vec![binding(10, &a)],
            config(),
            cap
        )),
        Err(WatchSessionError::Work)
    ));
    let update = h.ownership().prepare_replace(owner(1), &[key(10)]).unwrap();
    let mut cap = config();
    cap.retained_path_bytes = n(1);
    assert!(matches!(
        block_on(prepare_watch_session(
            update,
            vec![binding(10, &a)],
            cap,
            limits()
        )),
        Err(WatchSessionError::OutputBytes)
    ));
    let update = h.ownership().prepare_replace(owner(1), &[]).unwrap();
    assert!(matches!(
        block_on(prepare_watch_session(
            update,
            vec![binding(10, &a)],
            config(),
            limits()
        )),
        Err(WatchSessionError::UnexpectedBindings)
    ));
    assert_eq!(h.ownership().revision(), 0);
}

#[test]
fn iced_subscription_deliveries_are_checked_against_current_coverage() {
    use iced::advanced::subscription;
    let one = Temp::new();
    let two = Temp::new();
    let mut h = host();
    assert!(subscription::into_recipes(h.subscription()).is_empty());
    let next = prepared(&h, 1, &[1], vec![binding(1, &one)]);
    h.commit(next).unwrap();
    let mut stream = subscription::into_recipes(h.subscription())
        .pop()
        .unwrap()
        .stream(Box::pin(iced::futures::stream::pending()));
    h.active().unwrap().control().resume();
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = oneshot::channel();
    let timer = std::thread::spawn(move || {
        if cancel_rx.recv_timeout(Duration::from_secs(5)).is_err() {
            let _ = done_tx.send(());
        }
    });
    let observed = block_on(select(stream.next(), done_rx));
    drop(cancel_tx);
    timer.join().unwrap();
    let Either::Left((Some(delivery), _)) = observed else {
        panic!("watch subscription failed to deliver")
    };
    assert!(h.accepts(&delivery));
    assert!(!host().accepts(&delivery));
    let next = prepared(&h, 1, &[2], vec![binding(2, &two)]);
    let old = h.commit(next).unwrap().retired.unwrap();
    assert!(!old.control().is_shutdown_requested());
    assert!(
        !h.accepts(&delivery),
        "retained retired coverage is not current"
    );
    drop(old);
    assert!(block_on(stream.next()).is_none());
    let release = h.ownership().prepare_release(owner(1)).unwrap();
    h.commit(block_on(prepare_watch_session(release, vec![], config(), limits())).unwrap())
        .unwrap();
    assert!(subscription::into_recipes(h.subscription()).is_empty());
    assert!(!h.accepts(&delivery));
}

#[test]
fn explicit_refresh_replaces_equal_coverage_and_requires_the_complete_union() {
    let one = Temp::new();
    let two = Temp::new();
    let mut h = host();
    let first = prepared(&h, 1, &[1], vec![binding(1, &one)]);
    h.commit(first).unwrap();
    let second = prepared(&h, 2, &[2], vec![binding(1, &one), binding(2, &two)]);
    h.commit(second).unwrap();
    let old_control = h.active().unwrap().control().clone();
    let update = h
        .ownership()
        .prepare_refresh_replace(owner(1), &[key(1)])
        .unwrap();
    assert!(matches!(
        block_on(prepare_watch_session(
            update,
            vec![binding(1, &one)],
            config(),
            limits()
        )),
        Err(WatchSessionError::MissingDependency(_))
    ));
    assert!(!old_control.is_shutdown_requested());
    let update = h
        .ownership()
        .prepare_refresh_replace(owner(1), &[key(1)])
        .unwrap();
    let next = block_on(prepare_watch_session(
        update,
        vec![binding(1, &one), binding(2, &two)],
        config(),
        limits(),
    ))
    .unwrap();
    let new_control = next.candidate_control().unwrap().clone();
    assert!(!old_control.is_shutdown_requested());
    assert!(!new_control.is_shutdown_requested());
    assert!(next.changes().added().is_empty() && next.changes().removed().is_empty());
    let before = h.ownership().revision();
    let commit = h.commit(next).unwrap();
    assert_eq!(h.ownership().revision(), before + 1);
    assert!(commit.retired.is_some());
    assert!(!old_control.is_shutdown_requested());
    drop(commit.retired);
    assert!(old_control.is_shutdown_requested());
    assert!(!new_control.is_shutdown_requested());
    let same = prepared(&h, 1, &[1], vec![]);
    assert!(same.candidate_control().is_none());
    assert!(h.commit(same).unwrap().retired.is_none());
}
