use iced::futures::executor::block_on;
use iced_css::resources::{
    watch::{LocalWatchStream, WatchConfig},
    watch_work::*,
};
use std::{num::NonZeroUsize, time::Duration};
fn n(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn limits() -> WatchWorkLimits {
    WatchWorkLimits {
        slots: n(2),
        retained_bytes: n(10000),
    }
}
fn coverage() -> (iced_css::resources::watch::WatchControl, LocalWatchStream) {
    block_on(LocalWatchStream::start(
        vec![],
        WatchConfig {
            recovery_interval: Duration::from_secs(30),
            directories: n(1),
            retained_path_bytes: n(1000),
        },
    ))
    .unwrap()
}
#[test]
fn leases_bound_pending_work_and_release_without_stopping_committed_coverage() {
    let pool = WatchWorkPool::new(limits()).unwrap();
    let foreign = WatchWorkPool::new(limits()).unwrap();
    assert_eq!(pool.pending().unwrap(), 0);
    assert!(pool.retained_bytes().unwrap() <= limits().retained_bytes.get());
    let first = pool.admit().unwrap();
    let second = pool.admit().unwrap();
    assert!(pool.owns(&first));
    assert!(!foreign.owns(&first));
    assert!(pool.owns_origin(&pool.origin()));
    assert!(!foreign.owns_origin(&pool.origin()));
    assert!(matches!(pool.admit(), Err(WatchWorkError::Full)));
    let (control, stream) = coverage();
    first.register(control.clone()).unwrap();
    drop(first);
    assert!(
        !control.is_shutdown_requested(),
        "lease retirement must not cancel committed coverage"
    );
    assert_eq!(pool.pending().unwrap(), 1);
    let replacement = pool.admit().unwrap();
    assert!(replacement.is_live());
    drop(second);
    drop(replacement);
    assert_eq!(pool.pending().unwrap(), 0);
    drop(stream);
    assert!(control.is_shutdown_requested());
}
#[test]
fn closing_pool_cancels_queued_preparations_even_while_messages_retain_their_streams() {
    let pool = WatchWorkPool::new(limits()).unwrap();
    let origin = pool.origin();
    let queued = pool.admit().unwrap();
    let pending = pool.admit().unwrap();
    let (control, stream) = coverage();
    queued.register(control.clone()).unwrap();
    drop(pool);
    assert!(!origin.is_live());
    assert!(!queued.is_live() && !pending.is_live());
    assert!(control.is_shutdown_requested());
    let (late, late_stream) = coverage();
    assert!(matches!(
        pending.register(late.clone()),
        Err(WatchWorkError::Closed)
    ));
    assert!(
        late.is_shutdown_requested(),
        "a rejected late preparation is cancelled"
    );
    drop(queued);
    drop(pending);
    drop(stream);
    drop(late_stream);
}
#[test]
fn pool_construction_rejects_unrepresentable_or_insufficient_storage() {
    assert!(matches!(
        WatchWorkPool::new(WatchWorkLimits {
            retained_bytes: n(usize::MAX),
            ..limits()
        }),
        Err(WatchWorkError::Bytes)
    ));
    assert!(matches!(
        WatchWorkPool::new(WatchWorkLimits {
            retained_bytes: n(1),
            ..limits()
        }),
        Err(WatchWorkError::Bytes)
    ));
    assert!(matches!(
        WatchWorkPool::new(WatchWorkLimits {
            slots: n(usize::MAX),
            ..limits()
        }),
        Err(WatchWorkError::Bytes)
    ));
}

#[test]
fn duplicate_registration_cancels_only_the_supplied_new_candidate() {
    let pool = WatchWorkPool::new(limits()).unwrap();
    let lease = pool.admit().unwrap();
    let (first, first_stream) = coverage();
    let (second, second_stream) = coverage();
    lease.register(first.clone()).unwrap();
    assert!(matches!(
        lease.register(second.clone()),
        Err(WatchWorkError::Registered)
    ));
    assert!(!first.is_shutdown_requested());
    assert!(second.is_shutdown_requested());
    pool.close();
    assert!(matches!(pool.admit(), Err(WatchWorkError::Closed)));
    assert!(first.is_shutdown_requested());
    assert_eq!(pool.pending().unwrap(), 0);
    assert_eq!(pool.retained_bytes().unwrap(), 0);
    assert!(!pool.owns(&lease));
    drop(lease);
    drop(first_stream);
    drop(second_stream);
}

#[test]
fn weak_registration_cannot_attach_to_a_reused_slot_or_keep_a_lease_alive() {
    let pool = WatchWorkPool::new(WatchWorkLimits {
        slots: n(1),
        ..limits()
    })
    .unwrap();
    let old = pool.admit().unwrap();
    let registration = old.registrar();
    drop(old);
    assert_eq!(pool.pending().unwrap(), 0);
    let current = pool.admit().unwrap();
    let (stale, stale_stream) = coverage();
    assert!(matches!(
        registration.register(stale.clone()),
        Err(WatchWorkError::InvalidLease)
    ));
    assert!(stale.is_shutdown_requested());
    let (active, active_stream) = coverage();
    let registration = current.registrar();
    registration.register(active.clone()).unwrap();
    assert!(!active.is_shutdown_requested());
    drop(pool);
    assert!(active.is_shutdown_requested());
    let (late, late_stream) = coverage();
    assert!(matches!(
        registration.register(late.clone()),
        Err(WatchWorkError::Closed)
    ));
    assert!(late.is_shutdown_requested());
    drop(current);
    drop(stale_stream);
    drop(active_stream);
    drop(late_stream);
}

#[test]
fn registration_phase_preserves_admission_and_retires_only_old_registration() {
    let pool = WatchWorkPool::new(WatchWorkLimits {
        slots: n(1),
        ..limits()
    })
    .unwrap();
    let mut lease = pool.admit().unwrap();
    let old_registration = lease.registrar();
    assert!(old_registration.is_live());
    let (committed, committed_stream) = coverage();
    lease.register(committed.clone()).unwrap();
    lease.advance_registration().unwrap();
    assert!(!old_registration.is_live());
    assert!(lease.registrar().is_live());
    assert_eq!(pool.pending().unwrap(), 1);
    assert!(matches!(pool.admit(), Err(WatchWorkError::Full)));
    assert!(pool.owns(&lease));
    assert!(!committed.is_shutdown_requested());
    let (late, late_stream) = coverage();
    assert_eq!(
        old_registration.register(late.clone()),
        Err(WatchWorkError::InvalidLease)
    );
    assert!(late.is_shutdown_requested());
    let (next, next_stream) = coverage();
    lease.registrar().register(next.clone()).unwrap();
    pool.close();
    assert!(next.is_shutdown_requested());
    assert!(!committed.is_shutdown_requested());
    assert_eq!(lease.advance_registration(), Err(WatchWorkError::Closed));
    drop((committed_stream, late_stream, next_stream));
}
