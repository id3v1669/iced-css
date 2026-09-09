use iced_css_core::identity::*;
use std::num::{NonZeroU64, NonZeroUsize};
fn registry(maximum: u64) -> SourceRegistry {
    SourceRegistry::new(NonZeroU64::new(maximum).unwrap())
}
fn limits() -> SourceChangeLimits {
    SourceChangeLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(100000).unwrap(),
        work: NonZeroUsize::new(100000).unwrap(),
    }
}
#[test]
fn source_changes_commit_new_scope_tokens_and_remove_only_selected_instances() {
    let mut registry = registry(10);
    let old = registry.insert().unwrap();
    let retained = registry.insert().unwrap();
    let retained_id = registry.validate(&retained).unwrap();
    let removal = [old.clone()];
    let prepared = registry.prepare_changes(&removal, 2, limits()).unwrap();
    assert_eq!(prepared.removed_instances().len(), 1);
    let new = prepared.commit();
    assert_eq!(registry.validate(&old), Err(IdentityError::DeadInstance));
    assert_eq!(registry.validate(&retained).unwrap(), retained_id);
    assert_eq!(
        new.iter()
            .map(|t| registry.validate(t).unwrap().get())
            .collect::<Vec<_>>(),
        vec![3, 4]
    );
    let other = SourceRegistry::new(NonZeroU64::new(10).unwrap());
    for token in &new {
        assert_eq!(other.validate(token), Err(IdentityError::WrongHost));
    }
    let next = registry.insert().unwrap();
    assert_eq!(registry.validate(&next).unwrap().get(), 5);
}
#[test]
fn abandoned_preparation_does_not_remove_sources_or_consume_fresh_ids() {
    let mut registry = registry(2);
    let old = registry.insert().unwrap();
    for _ in 0..3 {
        let prepared = registry
            .prepare_changes(std::slice::from_ref(&old), 1, limits())
            .unwrap();
        drop(prepared);
        assert_eq!(registry.validate(&old).unwrap().get(), 1);
    }
    let new = registry
        .prepare_changes(std::slice::from_ref(&old), 1, limits())
        .unwrap()
        .commit();
    assert_eq!(registry.validate(&new[0]).unwrap().get(), 2);
    assert_eq!(registry.validate(&old), Err(IdentityError::DeadInstance));
    let removed = registry
        .prepare_changes(&new, 0, limits())
        .unwrap()
        .commit();
    assert!(removed.is_empty());
    assert!(matches!(registry.insert(), Err(IdentityError::Exhausted)));
}
#[test]
fn invalid_removals_and_range_exhaustion_leave_live_state_unchanged() {
    let mut registry = registry(2);
    let old = registry.insert().unwrap();
    let mut foreign = SourceRegistry::new(NonZeroU64::new(2).unwrap());
    let foreign = foreign.insert().unwrap();
    assert!(matches!(
        registry.prepare_changes(&[old.clone(), foreign], 0, limits()),
        Err(SourceChangeError::Identity(IdentityError::WrongHost))
    ));
    assert!(matches!(
        registry.prepare_changes(&[old.clone(), old.clone()], 0, limits()),
        Err(SourceChangeError::DuplicateRemoval(_))
    ));
    assert!(matches!(
        registry.prepare_changes(std::slice::from_ref(&old), 2, limits()),
        Err(SourceChangeError::Identity(IdentityError::Exhausted))
    ));
    assert_eq!(registry.validate(&old).unwrap().get(), 1);
    let second = registry.insert().unwrap();
    assert_eq!(registry.validate(&second).unwrap().get(), 2);
    registry.remove(&second).unwrap();
    assert!(matches!(
        registry.prepare_changes(&[old.clone(), second], 0, limits()),
        Err(SourceChangeError::Identity(IdentityError::DeadInstance))
    ));
    assert!(registry.validate(&old).is_ok());
}
#[test]
fn each_source_change_budget_fails_before_liveness_or_counter_mutation() {
    let mut registry = registry(3);
    let old = registry.insert().unwrap();
    let mut entries = limits();
    entries.entries = NonZeroUsize::new(1).unwrap();
    let mut bytes = limits();
    bytes.retained_bytes = NonZeroUsize::new(1).unwrap();
    let mut work = limits();
    work.work = NonZeroUsize::new(1).unwrap();
    for limit in [entries, bytes, work] {
        assert!(
            registry
                .prepare_changes(std::slice::from_ref(&old), 1, limit)
                .is_err()
        );
        assert!(registry.validate(&old).is_ok());
    }
    let new = registry.prepare_changes(&[], 2, limits()).unwrap().commit();
    assert_eq!(
        new.iter()
            .map(|t| registry.validate(t).unwrap().get())
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
}
#[test]
fn unordered_removals_keep_membership_valid_across_later_single_source_changes() {
    let mut registry = registry(10);
    let tokens = registry.prepare_changes(&[], 5, limits()).unwrap().commit();
    let new = registry
        .prepare_changes(&[tokens[3].clone(), tokens[1].clone()], 2, limits())
        .unwrap()
        .commit();
    for index in [0, 2, 4] {
        assert!(registry.validate(&tokens[index]).is_ok());
    }
    for index in [1, 3] {
        assert_eq!(
            registry.validate(&tokens[index]),
            Err(IdentityError::DeadInstance)
        );
    }
    assert_eq!(
        new.iter()
            .map(|t| registry.validate(t).unwrap().get())
            .collect::<Vec<_>>(),
        vec![6, 7]
    );
    registry.remove(&new[0]).unwrap();
    let last = registry.insert().unwrap();
    assert_eq!(registry.validate(&last).unwrap().get(), 8);
    assert!(registry.validate(&new[1]).is_ok());
}
