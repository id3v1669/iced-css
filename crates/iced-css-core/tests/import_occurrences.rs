use iced_css_core::syntax::imports::*;
use std::num::{NonZeroU64, NonZeroUsize};
fn resource(id: u64) -> ResourceId {
    ResourceId::new(NonZeroU64::new(id).unwrap())
}
fn limits() -> ImportLimits {
    ImportLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(10000).unwrap(),
        occurrences: NonZeroU64::new(100).unwrap(),
    }
}
fn enter(tracker: &mut ImportTracker, id: u64) -> Visit {
    match tracker.enter(resource(id), true).unwrap() {
        ImportEntry::Visit(visit) => visit,
        other => panic!("{other:?}"),
    }
}

#[test]
fn repeated_imports_have_distinct_occurrences_and_share_completed_acquisition() {
    let mut tracker = ImportTracker::new(limits());
    let root = enter(&mut tracker, 1);
    tracker.mark_acquired(resource(1)).unwrap();
    let first = enter(&mut tracker, 2);
    assert!(first.needs_acquisition);
    tracker.mark_acquired(resource(2)).unwrap();
    tracker.finish(first.occurrence).unwrap();
    let second = enter(&mut tracker, 2);
    assert!(!second.needs_acquisition);
    assert_ne!(first.occurrence, second.occurrence);
    tracker.finish(second.occurrence).unwrap();
    tracker.finish(root.occurrence).unwrap();
    assert_eq!(tracker.admitted_occurrences(), 3);
    assert_eq!(tracker.acquired_count(), 2);
    assert_eq!(tracker.active_depth(), 0);
}

#[test]
fn cycles_use_the_active_path_and_nonmatching_media_does_not_enter() {
    let mut tracker = ImportTracker::new(limits());
    let root = enter(&mut tracker, 1);
    let nested = enter(&mut tracker, 2);
    assert_eq!(
        tracker.enter(resource(1), true).unwrap(),
        ImportEntry::Cycle
    );
    assert_eq!(
        tracker.enter(resource(3), false).unwrap(),
        ImportEntry::MediaExcluded
    );
    assert_eq!(tracker.admitted_occurrences(), 2);
    tracker.finish(nested.occurrence).unwrap();
    assert!(matches!(
        tracker.enter(resource(2), true).unwrap(),
        ImportEntry::Visit(_)
    ));
    assert_eq!(
        tracker.finish(root.occurrence),
        Err(ImportTrackError::NotCurrent)
    );
    assert_eq!(tracker.active_depth(), 2);
}

#[test]
fn acquisition_is_not_remembered_before_success_and_finish_tokens_do_not_recur() {
    let mut tracker = ImportTracker::new(limits());
    let failed = enter(&mut tracker, 1);
    assert_eq!(
        tracker.mark_acquired(resource(2)),
        Err(ImportTrackError::NotCurrent)
    );
    tracker.finish(failed.occurrence).unwrap();
    let retry = enter(&mut tracker, 1);
    assert!(retry.needs_acquisition);
    assert_eq!(
        tracker.finish(failed.occurrence),
        Err(ImportTrackError::NotCurrent)
    );
    assert!(tracker.mark_acquired(resource(1)).unwrap());
    assert!(!tracker.mark_acquired(resource(1)).unwrap());
    tracker.finish(retry.occurrence).unwrap();
}

#[test]
fn occurrence_and_storage_limits_fail_without_admitting_partial_visits() {
    let mut small = limits();
    small.occurrences = NonZeroU64::new(1).unwrap();
    let mut tracker = ImportTracker::new(small);
    let first = enter(&mut tracker, 1);
    tracker.finish(first.occurrence).unwrap();
    assert_eq!(
        tracker.enter(resource(1), true),
        Err(ImportTrackError::Occurrences)
    );
    assert_eq!(tracker.admitted_occurrences(), 1);
    small = limits();
    small.retained_bytes = NonZeroUsize::MIN;
    let mut tracker = ImportTracker::new(small);
    assert_eq!(
        tracker.enter(resource(1), true),
        Err(ImportTrackError::Bytes)
    );
    assert_eq!(tracker.admitted_occurrences(), 0);
    assert_eq!(tracker.active_depth(), 0);
    small = limits();
    small.entries = NonZeroUsize::MIN;
    let mut tracker = ImportTracker::new(small);
    let first = enter(&mut tracker, 1);
    assert_eq!(
        tracker.mark_acquired(resource(1)),
        Err(ImportTrackError::Entries)
    );
    tracker.finish(first.occurrence).unwrap();
    assert_eq!(tracker.acquired_count(), 0);
}

#[test]
fn active_and_acquired_storage_share_bytes_and_work_failures_preserve_state() {
    let mut small = limits();
    // One frame is two nonzero u64 identities. The acquisition ID needs eight
    // additional bytes while the frame remains active.
    small.retained_bytes = NonZeroUsize::new(23).unwrap();
    let mut tracker = ImportTracker::new(small);
    let root = enter(&mut tracker, 1);
    assert_eq!(
        tracker.mark_acquired(resource(1)),
        Err(ImportTrackError::Bytes)
    );
    assert_eq!(tracker.acquired_count(), 0);
    assert_eq!(tracker.active_depth(), 1);
    tracker.finish(root.occurrence).unwrap();
    small = limits();
    small.work = NonZeroUsize::new(2).unwrap();
    let mut tracker = ImportTracker::new(small);
    let root = enter(&mut tracker, 1);
    tracker.mark_acquired(resource(1)).unwrap();
    assert_eq!(
        tracker.enter(resource(2), true),
        Err(ImportTrackError::Work)
    );
    assert_eq!(tracker.admitted_occurrences(), 1);
    assert_eq!(tracker.active_depth(), 1);
    assert_eq!(tracker.acquired_count(), 1);
    tracker.finish(root.occurrence).unwrap();
}

#[test]
fn activity_queries_do_not_allocate_or_consume_occurrences() {
    let mut bound = limits();
    bound.occurrences = NonZeroU64::MIN;
    let mut tracker = ImportTracker::new(bound);
    let root = enter(&mut tracker, 1);
    assert!(tracker.is_active(resource(1)).unwrap());
    assert!(!tracker.is_active(resource(2)).unwrap());
    assert_eq!(tracker.admitted_occurrences(), 1);
    assert_eq!(tracker.active_depth(), 1);
    tracker.finish(root.occurrence).unwrap();
    assert!(!tracker.is_active(resource(1)).unwrap());
    assert_eq!(tracker.admitted_occurrences(), 1);
    bound.work = NonZeroUsize::MIN;
    let mut tracker = ImportTracker::new(bound);
    let root = enter(&mut tracker, 1);
    assert_eq!(tracker.is_active(resource(1)), Err(ImportTrackError::Work));
    assert_eq!(tracker.active_depth(), 1);
    tracker.finish(root.occurrence).unwrap();
    assert!(!tracker.is_active(resource(1)).unwrap());
}
