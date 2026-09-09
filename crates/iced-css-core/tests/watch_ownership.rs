use iced_css_core::resources::watch_ownership::*;
use std::num::{NonZeroU64, NonZeroUsize};
fn n(x: usize) -> NonZeroUsize {
    NonZeroUsize::new(x).unwrap()
}
fn owner(x: u64) -> WatchOwnerId {
    WatchOwnerId::new(NonZeroU64::new(x).unwrap())
}
fn key(x: u64) -> WatchDependencyId {
    WatchDependencyId::new(NonZeroU64::new(x).unwrap())
}
fn limits() -> WatchOwnershipLimits {
    WatchOwnershipLimits {
        owners: n(10),
        bindings: n(30),
        dependencies: n(20),
        retained_bytes: n(10000),
        work: n(100000),
        revisions: NonZeroU64::new(100).unwrap(),
    }
}
fn replace(registry: &mut WatchOwnership, o: u64, keys: &[u64]) -> WatchCommit {
    let keys: Vec<_> = keys.iter().copied().map(key).collect();
    let update = registry.prepare_replace(owner(o), &keys).unwrap();
    registry.commit(update).unwrap()
}
#[test]
fn active_and_pending_owners_retain_shared_watches_until_atomic_promotion() {
    let mut registry = WatchOwnership::new(limits()).unwrap();
    let first = replace(&mut registry, 1, &[10, 20]);
    assert_eq!(first.changes.added(), &[key(10), key(20)]);
    let candidate = replace(&mut registry, 2, &[20, 30]);
    assert_eq!(candidate.changes.added(), &[key(30)]);
    assert!(candidate.changes.removed().is_empty());
    let prepared = registry.prepare_promote(owner(2), owner(1)).unwrap();
    assert_eq!(prepared.changes().removed(), &[key(10)]);
    assert_eq!(registry.snapshot().dependencies().len(), 3);
    let committed = registry.commit(prepared).unwrap();
    assert!(committed.retired.is_some());
    assert_eq!(registry.snapshot().owners(), &[owner(1)]);
    assert_eq!(
        registry
            .snapshot()
            .owner_dependencies(owner(1))
            .collect::<Vec<_>>(),
        vec![key(20), key(30)]
    );
    replace(&mut registry, 3, &[20]);
    let release = registry.prepare_release(owner(1)).unwrap();
    assert_eq!(release.changes().removed(), &[key(30)]);
    registry.commit(release).unwrap();
    assert_eq!(registry.snapshot().dependencies(), &[key(20)]);
}
#[test]
fn empty_candidates_are_known_owners_and_duplicate_order_changes_are_noops() {
    let mut registry = WatchOwnership::new(limits()).unwrap();
    replace(&mut registry, 1, &[10, 20]);
    let revision = registry.revision();
    let same = replace(&mut registry, 1, &[20, 10, 20]);
    assert_eq!(same.revision, revision);
    assert!(same.retired.is_none());
    replace(&mut registry, 2, &[]);
    let promote = registry.prepare_promote(owner(2), owner(1)).unwrap();
    assert_eq!(promote.changes().removed().len(), 2);
    registry.commit(promote).unwrap();
    assert_eq!(registry.snapshot().owners(), &[owner(1)]);
    assert!(registry.snapshot().dependencies().is_empty());
    assert!(matches!(
        registry.prepare_promote(owner(99), owner(1)),
        Err(WatchOwnershipError::UnknownOwner)
    ));
    assert!(matches!(
        registry.prepare_promote(owner(1), owner(1)),
        Err(WatchOwnershipError::SameOwner)
    ));
}
#[test]
fn stale_and_cross_registry_preparations_cannot_mutate_the_current_union() {
    let mut registry = WatchOwnership::new(limits()).unwrap();
    let mut other = WatchOwnership::new(limits()).unwrap();
    let prepared = registry.prepare_replace(owner(1), &[key(10)]).unwrap();
    let rejected = other.commit(prepared).unwrap_err();
    assert_eq!(rejected.reason, WatchCommitFailure::WrongRegistry);
    assert!(other.snapshot().owners().is_empty());
    registry.commit(rejected.prepared).unwrap();
    let stale = registry.prepare_release(owner(1)).unwrap();
    replace(&mut registry, 2, &[20]);
    let rejected = registry.commit(stale).unwrap_err();
    assert_eq!(rejected.reason, WatchCommitFailure::Stale);
    assert_eq!(registry.snapshot().dependencies(), &[key(10), key(20)]);
}
#[test]
fn preparation_bounds_preserve_current_state_and_noops_survive_revision_exhaustion() {
    let mut cap = limits();
    cap.owners = n(1);
    let mut registry = WatchOwnership::new(cap).unwrap();
    replace(&mut registry, 1, &[10]);
    assert!(matches!(
        registry.prepare_replace(owner(2), &[]),
        Err(WatchOwnershipError::Owners)
    ));
    assert_eq!(registry.snapshot().dependencies(), &[key(10)]);
    cap = limits();
    cap.bindings = n(1);
    let mut registry = WatchOwnership::new(cap).unwrap();
    replace(&mut registry, 1, &[10]);
    assert!(matches!(
        registry.prepare_replace(owner(2), &[key(10)]),
        Err(WatchOwnershipError::Bindings)
    ));
    cap = limits();
    cap.dependencies = n(1);
    let registry = WatchOwnership::new(cap).unwrap();
    assert!(matches!(
        registry.prepare_replace(owner(1), &[key(10), key(20)]),
        Err(WatchOwnershipError::Dependencies)
    ));
    cap = limits();
    cap.retained_bytes = n(1);
    let registry = WatchOwnership::new(cap).unwrap();
    assert!(matches!(
        registry.prepare_replace(owner(1), &[]),
        Err(WatchOwnershipError::Bytes)
    ));
    cap = limits();
    cap.work = n(1);
    let registry = WatchOwnership::new(cap).unwrap();
    assert!(matches!(
        registry.prepare_replace(owner(1), &[key(10)]),
        Err(WatchOwnershipError::Work)
    ));
    cap = limits();
    cap.revisions = NonZeroU64::MIN;
    let mut registry = WatchOwnership::new(cap).unwrap();
    replace(&mut registry, 1, &[10]);
    assert!(matches!(
        registry.prepare_release(owner(1)),
        Err(WatchOwnershipError::Revisions)
    ));
    let same = replace(&mut registry, 1, &[10, 10]);
    assert_eq!(same.revision, 1);
    assert!(same.retired.is_none());
}

#[test]
fn explicit_refresh_prepares_a_full_snapshot_without_inventing_dependency_changes() {
    let mut registry = WatchOwnership::new(limits()).unwrap();
    replace(&mut registry, 1, &[10]);
    replace(&mut registry, 2, &[20]);
    let before = registry.revision();
    let ordinary = registry.prepare_replace(owner(1), &[key(10)]).unwrap();
    assert!(!ordinary.refreshes_coverage());
    assert!(ordinary.candidate_snapshot().is_none());
    let refresh = registry
        .prepare_refresh_replace(owner(1), &[key(10)])
        .unwrap();
    assert!(refresh.refreshes_coverage());
    assert_eq!(refresh.next_revision(), before + 1);
    assert!(refresh.changes().added().is_empty());
    assert!(refresh.changes().removed().is_empty());
    let snapshot = refresh.candidate_snapshot().unwrap();
    assert_eq!(
        snapshot.owner_dependencies(owner(1)).collect::<Vec<_>>(),
        vec![key(10)]
    );
    assert_eq!(
        snapshot.owner_dependencies(owner(2)).collect::<Vec<_>>(),
        vec![key(20)]
    );
    assert_eq!(snapshot.dependencies().len(), 2);
    assert_eq!(registry.revision(), before);
    let committed = registry.commit(refresh).unwrap();
    assert!(committed.retired.is_some());
    assert_eq!(registry.revision(), before + 1);
    assert_eq!(
        registry.commit(ordinary).unwrap_err().reason,
        WatchCommitFailure::Stale
    );

    let mut once = WatchOwnership::new(WatchOwnershipLimits {
        revisions: NonZeroU64::MIN,
        ..limits()
    })
    .unwrap();
    replace(&mut once, 1, &[10]);
    assert!(once.prepare_replace(owner(1), &[key(10)]).is_ok());
    assert!(matches!(
        once.prepare_refresh_replace(owner(1), &[key(10)]),
        Err(WatchOwnershipError::Revisions)
    ));
    assert_eq!(once.revision(), 1);
    assert_eq!(once.snapshot().dependencies(), &[key(10)]);
    assert!(matches!(
        registry.prepare_refresh_replace(owner(1), &vec![key(10); 31]),
        Err(WatchOwnershipError::Bindings)
    ));
}
