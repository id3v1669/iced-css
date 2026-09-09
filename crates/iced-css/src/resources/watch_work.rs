//! Bounded pending-work leases and cancellation of queued watch preparations.
use super::watch::WatchControl;
use std::{
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    sync::{Arc, Mutex, Weak},
};
#[derive(Debug, Clone, Copy)]
pub struct WatchWorkLimits {
    pub slots: NonZeroUsize,
    /// Actual slot-vector capacity; shared allocation/mutex metadata is separate.
    pub retained_bytes: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchWorkError {
    Bytes,
    Allocation,
    Full,
    Closed,
    Faulted,
    InvalidLease,
    Registered,
}
impl std::fmt::Display for WatchWorkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "watch work: {self:?}")
    }
}
impl std::error::Error for WatchWorkError {}
#[derive(Default)]
struct Slot {
    occupied: bool,
    identity: Weak<()>,
    control: Option<WatchControl>,
}
impl Slot {
    fn is_for(&self, identity: &Arc<()>) -> bool {
        self.occupied
            && self
                .identity
                .upgrade()
                .is_some_and(|current| Arc::ptr_eq(&current, identity))
    }
}
struct State {
    alive: bool,
    slots: Vec<Slot>,
    pending: usize,
}
/// A weak identity that cannot keep its owner or native coverage alive.
#[derive(Clone)]
pub struct WatchWorkOrigin {
    shared: Weak<Mutex<State>>,
}
impl WatchWorkOrigin {
    pub fn is_live(&self) -> bool {
        self.shared
            .upgrade()
            .is_some_and(|shared| shared.lock().is_ok_and(|state| state.alive))
    }
}
impl Hash for WatchWorkOrigin {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.shared.as_ptr().hash(state);
    }
}
impl std::fmt::Debug for WatchWorkOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatchWorkOrigin").finish_non_exhaustive()
    }
}
/// Unique lifetime owner. Dropping or closing it cancels registered candidates,
/// including those still held by application Messages. This never joins a worker.
/// Admission scans at most the configured number of fixed slots, with no growth.
pub struct WatchWorkPool {
    shared: Arc<Mutex<State>>,
}
impl WatchWorkPool {
    pub fn new(limits: WatchWorkLimits) -> Result<Self, WatchWorkError> {
        let size = std::mem::size_of::<Slot>();
        if limits.retained_bytes.get() > isize::MAX as usize
            || limits
                .slots
                .get()
                .checked_mul(size)
                .is_none_or(|bytes| bytes > limits.retained_bytes.get())
        {
            return Err(WatchWorkError::Bytes);
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(limits.slots.get())
            .map_err(|_| WatchWorkError::Allocation)?;
        if slots
            .capacity()
            .checked_mul(size)
            .is_none_or(|bytes| bytes > limits.retained_bytes.get())
        {
            return Err(WatchWorkError::Bytes);
        }
        slots.resize_with(limits.slots.get(), Slot::default);
        Ok(Self {
            shared: Arc::new(Mutex::new(State {
                alive: true,
                slots,
                pending: 0,
            })),
        })
    }
    pub fn admit(&self) -> Result<WatchWorkLease, WatchWorkError> {
        // Fixed identity metadata is separate from the admitted slot vector.
        let identity = Arc::new(());
        let mut state = self.shared.lock().map_err(|_| WatchWorkError::Faulted)?;
        if !state.alive {
            return Err(WatchWorkError::Closed);
        }
        let index = state
            .slots
            .iter()
            .position(|slot| !slot.occupied)
            .ok_or(WatchWorkError::Full)?;
        state.slots[index].occupied = true;
        state.slots[index].identity = Arc::downgrade(&identity);
        state.pending += 1; // Bounded by the pre-admitted slot-vector length.
        Ok(WatchWorkLease {
            shared: Arc::clone(&self.shared),
            index,
            identity,
        })
    }
    pub fn pending(&self) -> Result<usize, WatchWorkError> {
        self.shared
            .lock()
            .map(|state| state.pending)
            .map_err(|_| WatchWorkError::Faulted)
    }
    pub fn retained_bytes(&self) -> Result<usize, WatchWorkError> {
        self.shared
            .lock()
            .map(|state| state.slots.capacity() * std::mem::size_of::<Slot>())
            .map_err(|_| WatchWorkError::Faulted)
    }
    pub fn origin(&self) -> WatchWorkOrigin {
        WatchWorkOrigin {
            shared: Arc::downgrade(&self.shared),
        }
    }
    pub fn owns_origin(&self, origin: &WatchWorkOrigin) -> bool {
        std::ptr::eq(Arc::as_ptr(&self.shared), origin.shared.as_ptr()) && origin.is_live()
    }
    pub fn owns(&self, lease: &WatchWorkLease) -> bool {
        Arc::ptr_eq(&self.shared, &lease.shared) && lease.is_live()
    }
    /// Permanently close admission, invalidate origins and request shutdown of all
    /// registered candidates. Poison recovery is for terminal disposal only.
    pub fn close(&self) {
        let slots = {
            let mut state = self
                .shared
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.alive = false;
            state.pending = 0;
            std::mem::take(&mut state.slots)
        };
        for slot in slots {
            if let Some(control) = slot.control {
                control.request_shutdown();
            }
        }
    }
}
impl Drop for WatchWorkPool {
    fn drop(&mut self) {
        self.close();
    }
}
/// Move this lease through the acquisition/preparation/Message phases. It is not
/// cloneable. Release before admitting a follow-up attempt into a one-slot pool.
pub struct WatchWorkLease {
    identity: Arc<()>,
    shared: Arc<Mutex<State>>,
    index: usize,
}
impl std::fmt::Debug for WatchWorkLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatchWorkLease")
            .field("slot", &self.index)
            .finish_non_exhaustive()
    }
}
impl WatchWorkLease {
    pub fn is_live(&self) -> bool {
        self.shared.lock().is_ok_and(|state| {
            state.alive
                && state
                    .slots
                    .get(self.index)
                    .is_some_and(|slot| slot.is_for(&self.identity))
        })
    }
    /// Weak capability for registration during native startup. It does not keep
    /// this lease admitted and cannot attach to a later reuse of the same slot.
    pub fn registrar(&self) -> WatchWorkRegistrar {
        WatchWorkRegistrar {
            shared: Arc::downgrade(&self.shared),
            identity: Arc::downgrade(&self.identity),
            index: self.index,
        }
    }
    /// Advance after committing or disposing the previous candidate. Keeps the
    /// admission occupied, releases its control without stopping active coverage,
    /// and invalidates registrars from the previous phase.
    pub fn advance_registration(&mut self) -> Result<(), WatchWorkError> {
        let identity = Arc::new(());
        let control = {
            let mut state = self.shared.lock().map_err(|_| WatchWorkError::Faulted)?;
            if !state.alive {
                return Err(WatchWorkError::Closed);
            }
            let slot = state
                .slots
                .get_mut(self.index)
                .filter(|slot| slot.is_for(&self.identity))
                .ok_or(WatchWorkError::InvalidLease)?;
            let control = slot.control.take();
            slot.identity = Arc::downgrade(&identity);
            control
        };
        self.identity = identity;
        drop(control);
        Ok(())
    }
    /// Register a new candidate once. Rejection cancels the supplied candidate.
    pub fn register(&self, control: WatchControl) -> Result<(), WatchWorkError> {
        self.registrar().register(control)
    }
}
/// Weak pool/slot/lease identity. Useful before native startup has returned a
/// prepared session; cloning this handle does not extend pending-work admission.
#[derive(Clone)]
pub struct WatchWorkRegistrar {
    shared: Weak<Mutex<State>>,
    identity: Weak<()>,
    index: usize,
}
impl WatchWorkRegistrar {
    /// Whether this exact registration phase still owns an admitted live slot.
    /// This does not prove native readiness or graph-publication eligibility.
    pub fn is_live(&self) -> bool {
        let Some(shared) = self.shared.upgrade() else {
            return false;
        };
        let Some(identity) = self.identity.upgrade() else {
            return false;
        };
        shared.lock().is_ok_and(|state| {
            state.alive
                && state
                    .slots
                    .get(self.index)
                    .is_some_and(|slot| slot.is_for(&identity))
        })
    }
    pub fn register(&self, control: WatchControl) -> Result<(), WatchWorkError> {
        let mut control = Some(control);
        let result = (|| {
            let shared = self.shared.upgrade().ok_or(WatchWorkError::Closed)?;
            let mut state = shared.lock().map_err(|_| WatchWorkError::Faulted)?;
            if !state.alive {
                return Err(WatchWorkError::Closed);
            }
            let identity = self
                .identity
                .upgrade()
                .ok_or(WatchWorkError::InvalidLease)?;
            let slot = state
                .slots
                .get_mut(self.index)
                .filter(|slot| slot.is_for(&identity))
                .ok_or(WatchWorkError::InvalidLease)?;
            if slot.control.is_some() {
                return Err(WatchWorkError::Registered);
            }
            slot.control = control.take();
            Ok(())
        })();
        if let Some(control) = control {
            control.request_shutdown();
        }
        result
    }
}
impl Drop for WatchWorkLease {
    fn drop(&mut self) {
        let control = {
            // Terminal release can dispose a poisoned pool; normal operations fail.
            let mut state = self
                .shared
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some(slot) = state.slots.get_mut(self.index) else {
                return;
            };
            if !slot.is_for(&self.identity) {
                return;
            }
            slot.occupied = false;
            slot.identity = Weak::new();
            let control = slot.control.take();
            state.pending -= 1;
            control
        };
        // Releasing a lease after successful commit must keep active coverage live.
        drop(control);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn poisoned_state_rejects_normal_admission_but_terminal_cleanup_cancels_candidates() {
        let pool = WatchWorkPool::new(WatchWorkLimits {
            slots: NonZeroUsize::MIN,
            retained_bytes: NonZeroUsize::new(1000).unwrap(),
        })
        .unwrap();
        let lease = pool.admit().unwrap();
        let (control, stream) =
            iced::futures::executor::block_on(super::super::watch::LocalWatchStream::start(
                vec![],
                super::super::watch::WatchConfig {
                    recovery_interval: std::time::Duration::from_secs(30),
                    directories: NonZeroUsize::MIN,
                    retained_path_bytes: NonZeroUsize::new(1000).unwrap(),
                },
            ))
            .unwrap();
        lease.register(control.clone()).unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = pool.shared.lock().unwrap();
                panic!("controlled work-pool fault");
            }))
            .is_err()
        );
        assert!(matches!(pool.admit(), Err(WatchWorkError::Faulted)));
        assert!(!lease.is_live());
        assert!(!pool.origin().is_live());
        assert!(!control.is_shutdown_requested());
        drop(pool);
        assert!(control.is_shutdown_requested());
        drop(lease);
        drop(stream);
    }
    #[test]
    fn retained_old_marker_cannot_register_into_a_reused_slot() {
        let pool = WatchWorkPool::new(WatchWorkLimits {
            slots: NonZeroUsize::MIN,
            retained_bytes: NonZeroUsize::new(1000).unwrap(),
        })
        .unwrap();
        let old = pool.admit().unwrap();
        let registration = old.registrar();
        // Model the interval after slot release but before the old marker's last
        // strong reference is destroyed; checking weak liveness alone is insufficient.
        let marker = Arc::clone(&old.identity);
        drop(old);
        let replacement = pool.admit().unwrap();
        let (control, stream) =
            iced::futures::executor::block_on(super::super::watch::LocalWatchStream::start(
                vec![],
                super::super::watch::WatchConfig {
                    recovery_interval: std::time::Duration::from_secs(30),
                    directories: NonZeroUsize::MIN,
                    retained_path_bytes: NonZeroUsize::new(1000).unwrap(),
                },
            ))
            .unwrap();
        assert!(matches!(
            registration.register(control.clone()),
            Err(WatchWorkError::InvalidLease)
        ));
        assert!(control.is_shutdown_requested());
        assert!(replacement.is_live());
        assert_eq!(pool.pending().unwrap(), 1);
        drop(marker);
        drop(stream);
        drop(replacement);
    }
}
