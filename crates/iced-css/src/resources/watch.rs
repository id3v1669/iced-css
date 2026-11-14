//! Fixed-parent-directory native watch sessions with conservative local rescans.
//! Notices never authorize HTTP refresh or replace explicit reload generations.
use iced::{
    Subscription,
    futures::{Stream, channel::oneshot},
};
use notify::{
    EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode},
};
use std::{
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};
const NATIVE: u8 = 1;
const LOSS: u8 = 2;
const PERIODIC: u8 = 4;
const RESUMED: u8 = 8;
const FAILED: u8 = 16;
#[derive(Debug, Clone, Copy)]
pub struct WatchConfig {
    pub recovery_interval: Duration,
    pub directories: NonZeroUsize,
    /// Input Vec record capacity plus owned native path capacities. Backend/OS
    /// allocations are separate; the product event mailbox retains no paths.
    pub retained_path_bytes: NonZeroUsize,
}
#[derive(Debug)]
pub enum WatchError {
    Interval,
    Directories,
    Paths,
    RelativePath { index: usize },
    Spawn(std::io::Error),
    Stopped,
    Work(super::watch_work::WatchWorkError),
}
impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "local watcher: {self:?}")
    }
}
impl std::error::Error for WatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(e) => Some(e),
            Self::Work(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchNotice {
    pub dirty_all_local: bool,
    pub native_change: bool,
    pub registration_loss: bool,
    pub periodic_rescan: bool,
    pub resumed: bool,
    pub worker_failed: bool,
}
impl WatchNotice {
    fn from_flags(flags: u8) -> Self {
        Self {
            dirty_all_local: true,
            native_change: flags & NATIVE != 0,
            registration_loss: flags & LOSS != 0,
            periodic_rescan: flags & PERIODIC != 0,
            resumed: flags & RESUMED != 0,
            worker_failed: flags & FAILED != 0,
        }
    }
}
struct Shared {
    flags: AtomicU8,
    stop: AtomicBool,
    closed: AtomicBool,
    receiver: Mutex<ReceiverState>,
}
impl Shared {
    fn raise(&self, flags: u8) {
        if flags != 0 && !self.stop.load(Ordering::Acquire) {
            self.flags.fetch_or(flags, Ordering::AcqRel);
            self.wake();
        }
    }
    fn wake(&self) {
        let waker = self
            .receiver
            .lock()
            .ok()
            .and_then(|mut state| state.waker.take());
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    fn close_owner(&self) {
        let waker = self.receiver.lock().ok().and_then(|mut state| {
            state.alive = false;
            state.waker.take()
        });
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    fn take(&self) -> Option<WatchNotice> {
        let flags = self.flags.swap(0, Ordering::AcqRel);
        (flags != 0).then(|| WatchNotice::from_flags(flags))
    }
}
struct ReceiverState {
    alive: bool,
    current: Weak<()>,
    waker: Option<Waker>,
}
impl ReceiverState {
    fn new() -> Self {
        Self {
            alive: true,
            current: Weak::new(),
            waker: None,
        }
    }
    fn is_current(&self, token: &Arc<()>) -> bool {
        self.current
            .upgrade()
            .is_some_and(|current| Arc::ptr_eq(&current, token))
    }
}
struct NoticeReceiver {
    shared: Weak<Shared>,
    token: Arc<()>,
}
impl NoticeReceiver {
    fn connect(shared: Weak<Shared>) -> Self {
        let token = Arc::new(());
        let waker = shared.upgrade().and_then(|shared| {
            shared.receiver.lock().ok().and_then(|mut state| {
                if state.alive {
                    state.current = Arc::downgrade(&token);
                }
                state.waker.take()
            })
        });
        if let Some(waker) = waker {
            waker.wake();
        }
        Self { shared, token }
    }
}
impl Stream for NoticeReceiver {
    type Item = WatchNotice;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<WatchNotice>> {
        let Some(shared) = self.shared.upgrade() else {
            return Poll::Ready(None);
        };
        // Clone and destroy arbitrary application wakers outside the receiver lock.
        let next_waker = cx.waker().clone();
        let (result, old_waker) = {
            let Ok(mut state) = shared.receiver.lock() else {
                return Poll::Ready(None);
            };
            if !state.alive || !state.is_current(&self.token) {
                return Poll::Ready(None);
            }
            if let Some(notice) = shared.take() {
                (Poll::Ready(Some(notice)), state.waker.take())
            } else if shared.closed.load(Ordering::Acquire) {
                (Poll::Ready(None), state.waker.take())
            } else {
                (Poll::Pending, state.waker.replace(next_waker))
            }
        };
        drop(old_waker);
        result
    }
}
impl Drop for NoticeReceiver {
    fn drop(&mut self) {
        let waker = self.shared.upgrade().and_then(|shared| {
            shared.receiver.lock().ok().and_then(|mut state| {
                if state.is_current(&self.token) {
                    state.current = Weak::new();
                    state.waker.take()
                } else {
                    None
                }
            })
        });
        drop(waker);
    }
}
/// A coalesced notice tagged with its originating coverage session. Validate it
/// against current coverage in the application's Message path before acting.
#[derive(Clone)]
pub struct WatchDelivery {
    origin: Weak<Shared>,
    notice: WatchNotice,
}
impl WatchDelivery {
    pub fn notice(&self) -> WatchNotice {
        self.notice
    }
}
impl std::fmt::Debug for WatchDelivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatchDelivery")
            .field("notice", &self.notice)
            .finish_non_exhaustive()
    }
}
struct WatchSubscriptionData {
    shared: Weak<Shared>,
}
impl Hash for WatchSubscriptionData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.shared.as_ptr().hash(state);
    }
}
struct WatchDeliveryStream {
    receiver: NoticeReceiver,
}
fn connect_subscription(data: &WatchSubscriptionData) -> WatchDeliveryStream {
    WatchDeliveryStream {
        receiver: NoticeReceiver::connect(data.shared.clone()),
    }
}
impl Stream for WatchDeliveryStream {
    type Item = WatchDelivery;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<WatchDelivery>> {
        Pin::new(&mut self.receiver).poll_next(cx).map(|notice| {
            notice.map(|notice| WatchDelivery {
                origin: self.receiver.shared.clone(),
                notice,
            })
        })
    }
}
/// A recovery request is coalescible local watch maintenance, not ReloadStyles.
#[derive(Clone)]
pub struct WatchControl {
    shared: Arc<Shared>,
    wake: mpsc::SyncSender<()>,
}
impl WatchControl {
    pub(crate) fn request_shutdown(&self) {
        self.shared.stop.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
    }
    pub fn resume(&self) {
        if !self.is_shutdown_requested() {
            let _ = self.wake.try_send(());
        }
    }
    /// Reports the shutdown request, not completion of native OS teardown.
    pub fn is_shutdown_requested(&self) -> bool {
        self.shared.stop.load(Ordering::Acquire)
    }
}
struct Cancel(WatchControl);
impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.request_shutdown();
    }
}
/// Stream ownership controls session lifetime. Cloned controls cannot keep a
/// dropped stream's native watchers running. No UI drop joins a native thread.
pub struct LocalWatchStream {
    shared: Arc<Shared>,
    receiver: NoticeReceiver,
    _cancel: Cancel,
}
impl LocalWatchStream {
    /// Rebuilds share recipe identity. Connecting a receiver retires the prior
    /// receiver (including direct stream polling); cancellation retains flags.
    /// This owner, not the subscription, controls native coverage lifetime.
    pub fn subscription(&self) -> Subscription<WatchDelivery> {
        Subscription::run_with(
            WatchSubscriptionData {
                shared: Arc::downgrade(&self.shared),
            },
            connect_subscription,
        )
    }
    pub fn accepts(&self, delivery: &WatchDelivery) -> bool {
        std::ptr::eq(Arc::as_ptr(&self.shared), delivery.origin.as_ptr())
    }
    pub async fn start(
        directories: Vec<PathBuf>,
        config: WatchConfig,
    ) -> Result<(WatchControl, Self), WatchError> {
        Self::start_registered(directories, config, None).await
    }
    pub(crate) async fn start_registered(
        directories: Vec<PathBuf>,
        config: WatchConfig,
        registration: Option<super::watch_work::WatchWorkRegistrar>,
    ) -> Result<(WatchControl, Self), WatchError> {
        if config.recovery_interval.is_zero()
            || Instant::now()
                .checked_add(config.recovery_interval)
                .is_none()
        {
            return Err(WatchError::Interval);
        }
        if directories.len() > config.directories.get() {
            return Err(WatchError::Directories);
        }
        let mut bytes = directories
            .capacity()
            .checked_mul(std::mem::size_of::<PathBuf>())
            .ok_or(WatchError::Paths)?;
        for (index, path) in directories.iter().enumerate() {
            bytes = bytes
                .checked_add(path.capacity())
                .ok_or(WatchError::Paths)?;
            if !path.is_absolute() {
                return Err(WatchError::RelativePath { index });
            }
        }
        if bytes > config.retained_path_bytes.get() {
            return Err(WatchError::Paths);
        }
        let shared = Arc::new(Shared {
            flags: AtomicU8::new(0),
            stop: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            receiver: Mutex::new(ReceiverState::new()),
        });
        let (wake, commands) = mpsc::sync_channel(1);
        let control = WatchControl {
            shared: shared.clone(),
            wake,
        };
        let cancel = Cancel(control.clone());
        // Cancellation ownership precedes thread spawn and native readiness.
        if let Some(registration) = registration {
            registration
                .register(control.clone())
                .map_err(WatchError::Work)?;
        }
        if control.is_shutdown_requested() {
            return Err(WatchError::Stopped);
        }

        let (ready_tx, ready_rx) = oneshot::channel();
        let actor_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("iced-css-watch".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    if actor_shared.stop.load(Ordering::Acquire) {
                        return;
                    }
                    let mut watcher = None;
                    let mut failures = RepairFailures::new(directories.len());
                    repair(&mut watcher, &directories, &actor_shared, &mut failures);
                    if ready_tx.send(()).is_err() {
                        return;
                    }
                    while !actor_shared.stop.load(Ordering::Acquire) {
                        let reason = match commands.recv_timeout(config.recovery_interval) {
                            Ok(()) => RESUMED,
                            Err(mpsc::RecvTimeoutError::Timeout) => PERIODIC,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                        if actor_shared.stop.load(Ordering::Acquire) {
                            break;
                        }
                        repair(&mut watcher, &directories, &actor_shared, &mut failures);
                        actor_shared.raise(reason);
                    }
                    // Watcher destruction occurs on this actor, including unwinding.
                    drop(watcher);
                }));
                if let Err(payload) = result {
                    let message = payload
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                        .unwrap_or("non-string panic payload");
                    log::error!(
                        target: "iced_css::stylesheet",
                        "local watch actor panicked; native watching stopped: {message}"
                    );
                    actor_shared.flags.fetch_or(FAILED | LOSS, Ordering::AcqRel);
                }
                actor_shared.stop.store(true, Ordering::Release);
                actor_shared.closed.store(true, Ordering::Release);
                actor_shared.wake();
            })
            .map_err(WatchError::Spawn)?;
        drop(thread);
        ready_rx.await.map_err(|_| WatchError::Stopped)?;
        let receiver = NoticeReceiver::connect(Arc::downgrade(&shared));
        Ok((
            control,
            Self {
                shared,
                receiver,
                _cancel: cancel,
            },
        ))
    }
}
impl Stream for LocalWatchStream {
    type Item = WatchNotice;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}
impl Drop for LocalWatchStream {
    fn drop(&mut self) {
        self.shared.close_owner();
    }
}
fn native_flags(event: notify::Result<notify::Event>) -> u8 {
    let event = match event {
        Ok(event) => event,
        Err(error) => {
            log::warn!(
                target: "iced_css::stylesheet",
                "native watch event error; rescanning local stylesheets: {error}"
            );
            return LOSS;
        }
    };
    if event.need_rescan() {
        return LOSS | NATIVE;
    }
    match event.kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => NATIVE,
        EventKind::Access(_) => 0,
        _ => NATIVE,
    }
}
/// Failure state of the previous repair, so each failure is logged once when it
/// begins rather than on every recovery interval.
struct RepairFailures {
    watcher: bool,
    directories: Vec<bool>,
}
impl RepairFailures {
    fn new(directories: usize) -> Self {
        Self {
            watcher: false,
            directories: vec![false; directories],
        }
    }
}
fn repair(
    watcher: &mut Option<RecommendedWatcher>,
    directories: &[PathBuf],
    shared: &Arc<Shared>,
    failures: &mut RepairFailures,
) {
    if shared.stop.load(Ordering::Acquire) {
        return;
    }
    let signal = shared.clone();
    let mut candidate = match notify::recommended_watcher(move |event| {
        signal.raise(native_flags(event));
    }) {
        Ok(watcher) => watcher,
        Err(error) => {
            if !std::mem::replace(&mut failures.watcher, true) {
                log::warn!(
                    target: "iced_css::stylesheet",
                    "native watcher creation failed; relying on periodic rescans: {error}"
                );
            }
            shared.raise(LOSS);
            return;
        }
    };
    failures.watcher = false;
    for (path, failed) in directories.iter().zip(&mut failures.directories) {
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        let failure = if !path.metadata().is_ok_and(|m| m.is_dir()) {
            Some("not an existing directory".to_owned())
        } else {
            candidate
                .watch(path, RecursiveMode::NonRecursive)
                .err()
                .map(|error| error.to_string())
        };
        match failure {
            Some(reason) => {
                if !std::mem::replace(failed, true) {
                    log::warn!(
                        target: "iced_css::stylesheet",
                        "native watch registration failed for {}; relying on periodic rescans: {reason}",
                        path.display()
                    );
                }
                shared.raise(LOSS);
            }
            None => *failed = false,
        }
    }
    *watcher = Some(candidate);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_access_is_ignored_but_write_close_rescan_and_errors_dirty_the_session() {
        for kind in [
            AccessKind::Read,
            AccessKind::Open(AccessMode::Read),
            AccessKind::Close(AccessMode::Read),
        ] {
            assert_eq!(
                native_flags(Ok(notify::Event::new(EventKind::Access(kind)))),
                0
            );
        }
        assert_eq!(
            native_flags(Ok(notify::Event::new(EventKind::Access(
                AccessKind::Close(AccessMode::Write)
            )))),
            NATIVE
        );
        assert_eq!(
            native_flags(Ok(
                notify::Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan)
            )),
            LOSS | NATIVE
        );
        assert_eq!(
            native_flags(Err(notify::Error::generic("controlled loss"))),
            LOSS
        );
    }
    #[test]
    fn repeated_notices_coalesce_in_fixed_state_and_closed_sessions_ignore_callbacks() {
        let shared = Shared {
            flags: AtomicU8::new(0),
            stop: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            receiver: Mutex::new(ReceiverState::new()),
        };
        for _ in 0..10000 {
            shared.raise(NATIVE);
            shared.raise(LOSS);
        }
        let notice = shared.take().unwrap();
        assert!(notice.native_change && notice.registration_loss);
        assert!(shared.take().is_none());
        shared.stop.store(true, Ordering::Release);
        shared.raise(NATIVE);
        assert!(shared.take().is_none());
    }
}

#[cfg(test)]
mod subscription_tests {
    use super::*;
    use iced::{
        advanced::subscription::{self, Hasher},
        futures::executor::block_on,
    };
    use std::{
        hash::Hasher as _,
        num::NonZeroUsize,
        sync::atomic::AtomicUsize,
        task::{Wake, Waker},
    };
    fn owner() -> LocalWatchStream {
        let (_, stream) = block_on(LocalWatchStream::start(
            vec![],
            WatchConfig {
                recovery_interval: Duration::from_secs(30),
                directories: NonZeroUsize::MIN,
                retained_path_bytes: NonZeroUsize::new(1000).unwrap(),
            },
        ))
        .unwrap();
        stream.shared.flags.store(0, Ordering::Release);
        stream
    }
    fn connect(owner: &LocalWatchStream) -> Pin<Box<dyn Stream<Item = WatchDelivery> + Send>> {
        subscription::into_recipes(owner.subscription())
            .pop()
            .unwrap()
            .stream(Box::pin(iced::futures::stream::pending()))
    }
    fn poll(
        stream: &mut Pin<Box<dyn Stream<Item = WatchDelivery> + Send>>,
    ) -> Poll<Option<WatchDelivery>> {
        stream
            .as_mut()
            .poll_next(&mut Context::from_waker(Waker::noop()))
    }
    #[test]
    fn recipes_keep_identity_and_reattachment_keeps_notices_without_native_cancellation() {
        fn hash(owner: &LocalWatchStream) -> u64 {
            let mut hasher = Hasher::default();
            subscription::into_recipes(owner.subscription())
                .pop()
                .unwrap()
                .hash(&mut hasher);
            hasher.finish()
        }
        let owner = owner();
        let foreign = self::owner();
        assert_eq!(hash(&owner), hash(&owner));
        assert_ne!(hash(&owner), hash(&foreign));
        let mut old = connect(&owner);
        assert!(poll(&mut old).is_pending());
        owner.shared.raise(NATIVE);
        drop(old);
        assert!(!owner.shared.stop.load(Ordering::Acquire));
        let mut next = connect(&owner);
        let Poll::Ready(Some(delivery)) = poll(&mut next) else {
            panic!("pending notice lost")
        };
        assert!(delivery.notice().native_change);
        assert!(owner.accepts(&delivery));
        assert!(!foreign.accepts(&delivery));
        assert!(poll(&mut next).is_pending());
        let mut replacement = connect(&owner);
        assert!(matches!(poll(&mut next), Poll::Ready(None)));
        drop(next);
        owner.shared.raise(LOSS | RESUMED);
        let Poll::Ready(Some(delivery)) = poll(&mut replacement) else {
            panic!("replacement lost notice")
        };
        assert!(delivery.notice().registration_loss && delivery.notice().resumed);
    }
    #[test]
    fn owner_drop_closes_receivers_but_worker_failure_can_still_be_delivered() {
        let owner = owner();
        let mut receiver = connect(&owner);
        owner.shared.flags.fetch_or(FAILED | LOSS, Ordering::AcqRel);
        owner.shared.stop.store(true, Ordering::Release);
        owner.shared.closed.store(true, Ordering::Release);
        let Poll::Ready(Some(delivery)) = poll(&mut receiver) else {
            panic!("worker failure lost")
        };
        assert!(delivery.notice().worker_failed);
        assert!(owner.accepts(&delivery));
        assert!(matches!(poll(&mut receiver), Poll::Ready(None)));
        drop(owner);
        assert!(matches!(poll(&mut receiver), Poll::Ready(None)));
    }
    #[test]
    fn owner_drop_wakes_pending_iced_receiver_and_message_mapping_works() {
        struct Wakes(AtomicUsize);
        impl Wake for Wakes {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        enum Message {
            Watch(WatchDelivery),
        }
        let owner = owner();
        let mut mapped = subscription::into_recipes(owner.subscription().map(Message::Watch))
            .pop()
            .unwrap()
            .stream(Box::pin(iced::futures::stream::pending()));
        owner.shared.raise(PERIODIC);
        let Poll::Ready(Some(Message::Watch(delivery))) = mapped
            .as_mut()
            .poll_next(&mut Context::from_waker(Waker::noop()))
        else {
            panic!("mapped notice lost")
        };
        assert!(owner.accepts(&delivery));
        let wakes = Arc::new(Wakes(AtomicUsize::new(0)));
        let waker = Waker::from(wakes.clone());
        assert!(
            mapped
                .as_mut()
                .poll_next(&mut Context::from_waker(&waker))
                .is_pending()
        );
        drop(owner);
        assert!(wakes.0.load(Ordering::SeqCst) > 0);
        assert!(matches!(
            mapped.as_mut().poll_next(&mut Context::from_waker(&waker)),
            Poll::Ready(None)
        ));
    }
    #[test]
    fn application_wakers_run_and_drop_outside_the_receiver_mutex() {
        struct Probe {
            shared: Weak<Shared>,
            calls: Arc<AtomicUsize>,
            drops: Arc<AtomicUsize>,
        }
        impl Wake for Probe {
            fn wake(self: Arc<Self>) {
                if let Some(shared) = self.shared.upgrade() {
                    assert!(shared.receiver.try_lock().is_ok());
                }
                self.calls.fetch_add(1, Ordering::SeqCst);
            }
        }
        impl Drop for Probe {
            fn drop(&mut self) {
                if let Some(shared) = self.shared.upgrade() {
                    assert!(shared.receiver.try_lock().is_ok());
                }
                self.drops.fetch_add(1, Ordering::SeqCst);
            }
        }
        let mut owner = owner();
        let mut receiver = connect(&owner);
        assert!(matches!(
            Pin::new(&mut owner).poll_next(&mut Context::from_waker(Waker::noop())),
            Poll::Ready(None)
        ));
        let calls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let waker = Waker::from(Arc::new(Probe {
            shared: Arc::downgrade(&owner.shared),
            calls: calls.clone(),
            drops: drops.clone(),
        }));
        assert!(
            receiver
                .as_mut()
                .poll_next(&mut Context::from_waker(&waker))
                .is_pending()
        );
        drop(waker);
        owner.shared.raise(NATIVE);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(matches!(poll(&mut receiver), Poll::Ready(Some(_))));
        let waker = Waker::from(Arc::new(Probe {
            shared: Arc::downgrade(&owner.shared),
            calls: calls.clone(),
            drops: drops.clone(),
        }));
        assert!(
            receiver
                .as_mut()
                .poll_next(&mut Context::from_waker(&waker))
                .is_pending()
        );
        drop(waker);
        drop(receiver);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn registered_start_is_owned_by_the_pool_and_closed_registration_is_rejected() {
        use crate::resources::watch_work::*;
        let pool = WatchWorkPool::new(WatchWorkLimits {
            slots: NonZeroUsize::MIN,
            retained_bytes: NonZeroUsize::new(1000).unwrap(),
        })
        .unwrap();
        let lease = pool.admit().unwrap();
        let config = WatchConfig {
            recovery_interval: Duration::from_secs(30),
            directories: NonZeroUsize::MIN,
            retained_path_bytes: NonZeroUsize::new(1000).unwrap(),
        };
        let (control, stream) = block_on(LocalWatchStream::start_registered(
            vec![],
            config,
            Some(lease.registrar()),
        ))
        .unwrap();
        assert!(!control.is_shutdown_requested());
        pool.close();
        assert!(control.is_shutdown_requested());
        assert!(matches!(
            block_on(LocalWatchStream::start_registered(
                vec![],
                config,
                Some(lease.registrar())
            )),
            Err(WatchError::Work(WatchWorkError::Closed))
        ));
        drop(stream);
    }
}
