use iced_css_core::{
    diagnostics::{CauseError, CauseKey, CauseLimits, CauseTracker, Observation},
    identity::IdAllocator,
    values::{RangeEvent, registry::PropertyId},
};
use std::num::NonZeroU64;
fn key() -> CauseKey {
    let mut ids = IdAllocator::new(NonZeroU64::new(10).unwrap());
    CauseKey {
        instance: ids.allocate().unwrap(),
        property: PropertyId::lookup("font-size").unwrap(),
        event: RangeEvent::UpperSaturation,
        semantic: NonZeroU64::new(1).unwrap(),
    }
}
fn tracker(count: usize) -> CauseTracker {
    CauseTracker::new(CauseLimits::new(count, 4096).unwrap())
}
#[test]
fn ongoing_causes_suppress_duplicates_but_resolved_causes_can_recur() {
    let mut state = tracker(5);
    let cause = key();
    assert_eq!(state.observe(cause), Ok(Observation::NewCause));
    assert_eq!(state.observe(cause), Ok(Observation::Suppressed));
    assert_eq!(state.live_count(), 1);
    assert!(state.resolve(cause));
    assert!(!state.resolve(cause));
    assert_eq!(state.live_count(), 0);
    assert_eq!(state.retained_bytes(), 0);
    assert_eq!(state.observe(cause), Ok(Observation::NewCause));
}
#[test]
fn distinct_semantic_contexts_properties_events_and_instances_do_not_coalesce() {
    let mut state = tracker(5);
    let cause = key();
    let mut ids = IdAllocator::new(NonZeroU64::new(10).unwrap());
    ids.allocate().unwrap();
    for distinct in [
        cause,
        CauseKey {
            semantic: NonZeroU64::new(2).unwrap(),
            ..cause
        },
        CauseKey {
            property: PropertyId::lookup("border-top-width").unwrap(),
            ..cause
        },
        CauseKey {
            event: RangeEvent::PositiveRoundedToZero,
            ..cause
        },
        CauseKey {
            instance: ids.allocate().unwrap(),
            ..cause
        },
    ] {
        assert_eq!(state.observe(distinct), Ok(Observation::NewCause));
        assert_eq!(state.observe(distinct), Ok(Observation::Suppressed));
    }
    assert_eq!(state.live_count(), 5);
}
#[test]
fn capacity_failure_preserves_previous_suppression_and_does_not_suppress_new_cause() {
    let mut state = tracker(1);
    let first = key();
    let second = CauseKey {
        semantic: NonZeroU64::new(2).unwrap(),
        ..first
    };
    assert_eq!(state.observe(first), Ok(Observation::NewCause));
    let before = state.retained_bytes();
    assert_eq!(state.observe(second), Err(CauseError::Capacity));
    assert_eq!(state.retained_bytes(), before);
    assert_eq!(state.observe(first), Ok(Observation::Suppressed));
    state.resolve(first);
    assert_eq!(state.observe(second), Ok(Observation::NewCause));
}
#[test]
fn removal_releases_only_the_removed_instances_causes() {
    let mut state = tracker(3);
    let first = key();
    let mut ids = IdAllocator::new(NonZeroU64::new(10).unwrap());
    ids.allocate().unwrap();
    let second = CauseKey {
        instance: ids.allocate().unwrap(),
        ..first
    };
    for cause in [
        first,
        CauseKey {
            semantic: NonZeroU64::new(2).unwrap(),
            ..first
        },
        second,
    ] {
        state.observe(cause).unwrap();
    }
    assert_eq!(state.remove_instance(first.instance), 2);
    assert_eq!(state.observe(second), Ok(Observation::Suppressed));
    assert_eq!(state.live_count(), 1);
    assert_eq!(state.remove_instance(first.instance), 0);
    assert_eq!(state.remove_instance(second.instance), 1);
    assert_eq!(state.retained_bytes(), 0);
}
#[test]
fn invalid_configuration_and_insufficient_byte_budget_remain_distinct() {
    assert!(CauseLimits::new(1, 1).is_ok());
    assert!(matches!(
        CauseLimits::new(0, 1),
        Err(CauseError::InvalidCount)
    ));
    assert!(matches!(
        CauseLimits::new(1, 0),
        Err(CauseError::InvalidBytes)
    ));
    assert!(matches!(
        CauseLimits::new(usize::MAX, 1),
        Err(CauseError::InvalidCount)
    ));
    assert!(matches!(
        CauseLimits::new(1, usize::MAX),
        Err(CauseError::InvalidBytes)
    ));
    let mut state = CauseTracker::new(CauseLimits::new(1, 1).unwrap());
    assert_eq!(state.observe(key()), Err(CauseError::Capacity));
    assert_eq!(state.live_count(), 0);
    assert_eq!(state.retained_bytes(), 0);
}

// CF34: replacement must stage an independent candidate and retain all old state
// on count/byte failure. A mutation that edits self before reserve breaks this.
#[test]
fn replacement_preserves_original_on_error() {
    let first = key();
    let second = CauseKey {
        semantic: NonZeroU64::new(2).unwrap(),
        ..first
    };
    let mut old = tracker(1);
    old.observe(first).unwrap();
    let bytes = old.retained_bytes();
    assert!(matches!(
        old.prepare_replacement(&[first, second]),
        Err(CauseError::Capacity)
    ));
    assert_eq!(old.keys(), &[first]);
    assert_eq!(old.retained_bytes(), bytes);
    assert_eq!(old.observe(first), Ok(Observation::Suppressed));
    let mut replacement = old.prepare_replacement(&[second]).unwrap();
    assert_eq!(old.keys(), &[first]);
    assert_eq!(replacement.keys(), &[second]);
    assert_eq!(replacement.observe(second), Ok(Observation::Suppressed));
    assert_eq!(replacement.observe(first), Err(CauseError::Capacity));
    let too_small = CauseTracker::new(CauseLimits::new(2, 1).unwrap());
    assert!(matches!(
        too_small.prepare_replacement(&[first]),
        Err(CauseError::Capacity)
    ));
    assert_eq!(too_small.live_count(), 0);
    assert_eq!(too_small.retained_bytes(), 0);
}

// Empty replacement must free candidate capacity without clearing the old owner.
#[test]
fn replacement_empty_releases_capacity() {
    let first = key();
    let mut old = tracker(1);
    old.observe(first).unwrap();
    let mut replacement = old.prepare_replacement(&[]).unwrap();
    assert_eq!(replacement.live_count(), 0);
    assert_eq!(replacement.retained_bytes(), 0);
    assert_eq!(old.keys(), &[first]);
    assert_eq!(replacement.observe(first), Ok(Observation::NewCause));
    assert_eq!(old.observe(first), Ok(Observation::Suppressed));
}

// Duplicate input must fail even when count/byte budgets admit both entries.
#[test]
fn replacement_rejects_duplicates() {
    let first = key();
    let mut old = tracker(3);
    old.observe(first).unwrap();
    let second = CauseKey {
        semantic: NonZeroU64::new(2).unwrap(),
        ..first
    };
    assert!(matches!(
        old.prepare_replacement(&[second, second]),
        Err(CauseError::Capacity)
    ));
    assert_eq!(old.keys(), &[first]);
    let distinct = old.prepare_replacement(&[second, first]).unwrap();
    assert_eq!(distinct.keys(), &[second, first]);
    assert_eq!(old.observe(first), Ok(Observation::Suppressed));
}
