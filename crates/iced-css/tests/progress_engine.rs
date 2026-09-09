use iced_css::animation::{clock::AnimationTiming, engine::*};
use iced_css_core::{
    animation::registry::AnimationSelection, identity::SourceRegistry, values::registry::PropertyId,
};
use std::{
    num::{NonZeroU64, NonZeroUsize},
    time::{Duration, Instant},
};
fn id(name: &str) -> PropertyId {
    PropertyId::lookup(name).unwrap()
}
fn selected(names: &[&str]) -> AnimationSelection {
    AnimationSelection::new(
        &names.iter().map(|name| id(name)).collect::<Vec<_>>(),
        NonZeroUsize::new(100000).unwrap(),
    )
    .unwrap()
}
fn engine(addresses: u64, bindings: usize) -> ProgressEngine {
    ProgressEngine::new(
        SourceRegistry::new(NonZeroU64::new(100).unwrap()),
        ProgressLimits {
            addresses: NonZeroU64::new(addresses).unwrap(),
            bindings: NonZeroUsize::new(bindings).unwrap(),
            tracks: NonZeroUsize::new(100).unwrap(),
            retained_bytes: NonZeroUsize::new(1000000).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
    )
}
fn timing() -> AnimationTiming {
    AnimationTiming::new(Duration::from_secs(1), Duration::ZERO)
}
#[test]
fn sampling_and_accepted_builds_preserve_progress_while_explicit_restart_is_isolated() {
    let mut engine = engine(10, 10);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    engine
        .start_progress(&source, &selected(&["margin-top", "color"]), timing(), t)
        .unwrap();
    engine.advance(t + Duration::from_millis(250));
    for _ in 0..4 {
        engine.accepted_build();
        assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.25));
    }
    engine
        .start_progress(
            &source,
            &selected(&["margin-top"]),
            timing(),
            t + Duration::from_millis(250),
        )
        .unwrap();
    assert_eq!(engine.sample(&source, id("margin-top")).unwrap(), Some(0.0));
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.25));
    engine.advance(t + Duration::from_millis(500));
    assert_eq!(
        engine.sample(&source, id("margin-top")).unwrap(),
        Some(0.25)
    );
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.5));
    engine.advance(t + Duration::from_millis(1250));
    assert_eq!(engine.sample(&source, id("margin-top")).unwrap(), None);
    assert!(!engine.is_animating());
}
#[test]
fn foreign_and_removed_tokens_cannot_sample_restart_or_remove_local_owners() {
    let mut a = engine(10, 10);
    let mut b = engine(10, 10);
    let foreign = a.register_source().unwrap();
    let local = b.register_source().unwrap();
    let t = Instant::now();
    b.start_progress(&local, &selected(&["color"]), timing(), t)
        .unwrap();
    b.advance(t + Duration::from_millis(250));
    assert!(
        b.start_progress(
            &foreign,
            &selected(&["color"]),
            timing(),
            t + Duration::from_millis(500)
        )
        .is_err()
    );
    assert!(b.sample(&foreign, id("color")).is_err());
    assert!(b.remove_source(&foreign).is_err());
    assert!(b.cancel(&foreign, &selected(&["color"])).is_err());
    assert_eq!(b.sample(&local, id("color")).unwrap(), Some(0.25));
    let stale = local.clone();
    b.remove_source(&local).unwrap();
    let replacement = b.register_source().unwrap();
    assert!(
        b.start_progress(&stale, &selected(&["color"]), timing(), t)
            .is_err()
    );
    assert!(b.sample(&stale, id("color")).is_err());
    assert_eq!(b.sample(&replacement, id("color")).unwrap(), None);
    assert!(!b.is_animating());
}
#[test]
fn address_exhaustion_rejects_the_whole_batch_without_advancing_existing_tracks() {
    let mut engine = engine(2, 10);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    engine
        .start_progress(&source, &selected(&["color"]), timing(), t)
        .unwrap();
    engine.advance(t + Duration::from_millis(250));
    assert!(
        engine
            .start_progress(
                &source,
                &selected(&["color", "margin-top", "margin-left"]),
                timing(),
                t + Duration::from_millis(500)
            )
            .is_err()
    );
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.25));
    assert_eq!(engine.sample(&source, id("margin-top")).unwrap(), None);
    engine
        .start_progress(
            &source,
            &selected(&["margin-top"]),
            timing(),
            t + Duration::from_millis(250),
        )
        .unwrap();
    engine
        .start_progress(
            &source,
            &selected(&["color"]),
            timing(),
            t + Duration::from_millis(250),
        )
        .unwrap();
    engine.cancel(&source, &selected(&["margin-top"])).unwrap();
    assert!(
        engine
            .start_progress(&source, &selected(&["margin-left"]), timing(), t)
            .is_err()
    );
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.0));
}
#[test]
fn storage_and_timestamp_failure_do_not_partially_start_a_batch() {
    let mut engine = engine(10, 1);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    assert!(
        engine
            .start_progress(&source, &selected(&["color", "margin-top"]), timing(), t)
            .is_err()
    );
    assert!(!engine.is_animating());
    assert_eq!(engine.sample(&source, id("color")).unwrap(), None);
    engine
        .start_progress(&source, &selected(&["color"]), timing(), t)
        .unwrap();
    engine.advance(t + Duration::from_millis(250));
    let impossible = AnimationTiming::new(Duration::MAX, Duration::MAX);
    assert!(
        engine
            .start_progress(&source, &selected(&["color"]), impossible, t)
            .is_err()
    );
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.25));
}
#[test]
fn one_monotonic_clock_handles_equal_times_and_zero_duration_after_delay() {
    let mut engine = engine(10, 10);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    engine.advance(t + Duration::from_millis(500));
    let accepted = engine
        .start_progress(
            &source,
            &selected(&["color"]),
            AnimationTiming::new(Duration::ZERO, Duration::from_millis(100)),
            t,
        )
        .unwrap();
    assert_eq!(accepted, t + Duration::from_millis(500));
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.0));
    engine.advance(t);
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.0));
    assert!(engine.is_animating());
    engine.advance(t + Duration::from_millis(600));
    assert_eq!(engine.sample(&source, id("color")).unwrap(), None);
    assert!(!engine.is_animating());
}

#[test]
fn registry_scan_and_storage_budgets_reject_before_installation() {
    for (work, bytes, want) in [
        (100, 1000000, EngineError::Work),
        (100000, 1, EngineError::Bytes),
    ] {
        let mut engine = ProgressEngine::new(
            SourceRegistry::new(NonZeroU64::new(10).unwrap()),
            ProgressLimits {
                addresses: NonZeroU64::new(10).unwrap(),
                bindings: NonZeroUsize::new(10).unwrap(),
                tracks: NonZeroUsize::new(100).unwrap(),
                retained_bytes: NonZeroUsize::new(bytes).unwrap(),
                work: NonZeroUsize::new(work).unwrap(),
            },
        );
        let source = engine.register_source().unwrap();
        assert_eq!(
            engine.start_progress(&source, &selected(&["color"]), timing(), Instant::now()),
            Err(want)
        );
        assert_eq!(engine.sample(&source, id("color")).unwrap(), None);
        assert!(!engine.is_animating());
    }
}

#[test]
fn abandoned_preparation_preserves_clock_tracks_and_address_capacity() {
    let mut engine = engine(2, 2);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    engine
        .start_progress(&source, &selected(&["color"]), timing(), t)
        .unwrap();
    engine.advance(t + Duration::from_millis(250));
    let both = selected(&["color", "margin-top"]);
    for _ in 0..3 {
        let prepared = engine
            .prepare_progress(&source, &both, timing(), t + Duration::from_millis(500))
            .unwrap();
        drop(prepared);
        assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.25));
        assert_eq!(engine.sample(&source, id("margin-top")).unwrap(), None);
    }
    engine
        .start_progress(&source, &both, timing(), t + Duration::from_millis(250))
        .unwrap();
    assert_eq!(engine.sample(&source, id("margin-top")).unwrap(), Some(0.0));
}
#[test]
fn outgoing_capture_and_new_delay_share_one_acceptance_timestamp() {
    use iced_css_core::{
        animation::{border::combine_border_width, combine::Combination},
        formatting::table_borders::BorderStyle,
    };
    let mut engine = engine(2, 2);
    let source = engine.register_source().unwrap();
    let property = id("border-top-width");
    let selection = selected(&["border-top-width"]);
    let t = Instant::now();
    engine
        .start_progress(&source, &selection, timing(), t)
        .unwrap();
    let next_timing = AnimationTiming::new(Duration::from_secs(1), Duration::from_millis(100));
    let mut prepared = engine
        .prepare_progress(
            &source,
            &selection,
            next_timing,
            t + Duration::from_millis(500),
        )
        .unwrap();
    assert!(matches!(
        prepared.sample_outgoing(&source, property),
        Err(EngineError::OutgoingNotAdvanced)
    ));
    assert_eq!(prepared.advance_outgoing(), t + Duration::from_millis(500));
    let old_progress = prepared
        .sample_outgoing(&source, property)
        .unwrap()
        .unwrap();
    assert_eq!(old_progress, 0.5);
    let capture = combine_border_width(4.0, 20.0, Combination::Interpolate(old_progress))
        .unwrap()
        .resolve(BorderStyle::Solid, 100.0)
        .unwrap()
        .value;
    assert_eq!(capture, 12.0);
    assert_eq!(prepared.commit(), t + Duration::from_millis(500));
    assert_eq!(engine.sample(&source, property).unwrap(), Some(0.0));
    engine.advance(t + Duration::from_millis(1100));
    let p = engine.sample(&source, property).unwrap().unwrap();
    assert_eq!(p, 0.5);
    let sample = combine_border_width(capture, 40.0, Combination::Interpolate(p))
        .unwrap()
        .resolve(BorderStyle::Solid, 100.0)
        .unwrap();
    assert_eq!(sample.value, 26.0);
}
#[test]
fn dropping_after_outgoing_advance_keeps_time_but_installs_no_new_starts() {
    let mut engine = engine(2, 2);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    engine
        .start_progress(&source, &selected(&["color"]), timing(), t)
        .unwrap();
    let incoming = selected(&["color", "margin-top"]);
    let mut prepared = engine
        .prepare_progress(&source, &incoming, timing(), t + Duration::from_millis(500))
        .unwrap();
    prepared.advance_outgoing();
    drop(prepared);
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.5));
    assert_eq!(engine.sample(&source, id("margin-top")).unwrap(), None);
    engine
        .start_progress(&source, &incoming, timing(), t)
        .unwrap();
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.0));
}

#[test]
fn outgoing_completion_releases_the_override_before_a_new_start_is_installed() {
    let mut engine = engine(1, 1);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    let color = selected(&["color"]);
    engine.start_progress(&source, &color, timing(), t).unwrap();
    let mut prepared = engine
        .prepare_progress(&source, &color, timing(), t + Duration::from_secs(1))
        .unwrap();
    prepared.advance_outgoing();
    assert_eq!(
        prepared.sample_outgoing(&source, id("color")).unwrap(),
        None
    );
    prepared.commit();
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.0));
}

#[test]
fn delayed_owners_choose_the_earliest_start_and_cancelled_demand_disappears() {
    use iced_css::animation::wakeup::ProgressWakeup;
    let mut engine = engine(2, 2);
    assert_eq!(engine.next_wakeup(), ProgressWakeup::Idle);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    for (property, delay) in [("color", 300), ("margin-top", 100)] {
        engine
            .start_progress(
                &source,
                &selected(&[property]),
                AnimationTiming::new(Duration::from_secs(1), Duration::from_millis(delay)),
                t,
            )
            .unwrap();
    }
    assert_eq!(
        engine.next_wakeup(),
        ProgressWakeup::At(t + Duration::from_millis(100))
    );
    engine.cancel(&source, &selected(&["margin-top"])).unwrap();
    assert_eq!(
        engine.next_wakeup(),
        ProgressWakeup::At(t + Duration::from_millis(300))
    );
    engine.advance(t + Duration::from_millis(300));
    assert_eq!(engine.next_wakeup(), ProgressWakeup::NextFrame);
    engine.advance(t + Duration::from_millis(1300));
    assert_eq!(engine.next_wakeup(), ProgressWakeup::Idle);
    assert!(!engine.is_animating());
}
#[test]
fn active_progress_has_frame_priority_over_delay_and_clock_rewind_is_ignored() {
    use iced_css::animation::wakeup::ProgressWakeup;
    let mut engine = engine(2, 2);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    engine
        .start_progress(&source, &selected(&["color"]), timing(), t)
        .unwrap();
    engine
        .start_progress(
            &source,
            &selected(&["margin-top"]),
            AnimationTiming::new(Duration::from_secs(1), Duration::from_secs(2)),
            t,
        )
        .unwrap();
    assert_eq!(engine.next_wakeup(), ProgressWakeup::NextFrame);
    engine.advance(t + Duration::from_secs(1));
    assert_eq!(
        engine.next_wakeup(),
        ProgressWakeup::At(t + Duration::from_secs(2))
    );
    engine.advance(t);
    assert_eq!(
        engine.next_wakeup(),
        ProgressWakeup::At(t + Duration::from_secs(2))
    );
    engine.remove_source(&source).unwrap();
    assert_eq!(engine.next_wakeup(), ProgressWakeup::Idle);
}
#[test]
fn zero_duration_needs_a_wakeup_at_the_end_of_its_delay() {
    use iced_css::animation::wakeup::ProgressWakeup;
    let mut engine = engine(1, 1);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    engine
        .start_progress(
            &source,
            &selected(&["color"]),
            AnimationTiming::new(Duration::ZERO, Duration::from_millis(100)),
            t,
        )
        .unwrap();
    assert_eq!(
        engine.next_wakeup(),
        ProgressWakeup::At(t + Duration::from_millis(100))
    );
    engine.advance(t + Duration::from_millis(100));
    assert_eq!(engine.next_wakeup(), ProgressWakeup::Idle);
    assert_eq!(engine.sample(&source, id("color")).unwrap(), None);
}

fn track_limited(tracks: usize) -> ProgressEngine {
    ProgressEngine::new(
        SourceRegistry::new(NonZeroU64::new(10).unwrap()),
        ProgressLimits {
            addresses: NonZeroU64::new(10).unwrap(),
            bindings: NonZeroUsize::new(10).unwrap(),
            tracks: NonZeroUsize::new(tracks).unwrap(),
            retained_bytes: NonZeroUsize::new(1000000).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
    )
}
#[test]
fn cancelled_tracks_count_until_collection_and_existing_owners_can_restart_at_the_limit() {
    let mut engine = track_limited(2);
    let source = engine.register_source().unwrap();
    let t = Instant::now();
    engine
        .start_progress(&source, &selected(&["color"]), timing(), t)
        .unwrap();
    engine.cancel(&source, &selected(&["color"])).unwrap();
    engine
        .start_progress(&source, &selected(&["margin-top"]), timing(), t)
        .unwrap();
    engine.advance(t + Duration::from_millis(250));
    assert_eq!(
        engine.start_progress(
            &source,
            &selected(&["color", "margin-top", "margin-left"]),
            timing(),
            t + Duration::from_millis(500)
        ),
        Err(EngineError::Tracks)
    );
    assert_eq!(
        engine.sample(&source, id("margin-top")).unwrap(),
        Some(0.25)
    );
    assert_eq!(engine.sample(&source, id("color")).unwrap(), None);
    engine
        .start_progress(
            &source,
            &selected(&["margin-top"]),
            timing(),
            t + Duration::from_millis(250),
        )
        .unwrap();
    assert_eq!(engine.sample(&source, id("margin-top")).unwrap(), Some(0.0));
    for _ in 0..3 {
        engine.accepted_build();
    }
    engine
        .start_progress(
            &source,
            &selected(&["color"]),
            timing(),
            t + Duration::from_millis(250),
        )
        .unwrap();
    assert_eq!(engine.sample(&source, id("color")).unwrap(), Some(0.0));
}
#[test]
fn removing_a_source_does_not_bypass_outstanding_track_admission() {
    let mut engine = track_limited(1);
    let old = engine.register_source().unwrap();
    let t = Instant::now();
    engine
        .start_progress(&old, &selected(&["color"]), timing(), t)
        .unwrap();
    engine.remove_source(&old).unwrap();
    let new = engine.register_source().unwrap();
    assert_eq!(
        engine.start_progress(&new, &selected(&["color"]), timing(), t),
        Err(EngineError::Tracks)
    );
    assert!(!engine.is_animating());
    for _ in 0..3 {
        engine.accepted_build();
    }
    engine
        .start_progress(&new, &selected(&["color"]), timing(), t)
        .unwrap();
    assert_eq!(engine.sample(&new, id("color")).unwrap(), Some(0.0));
    assert!(engine.sample(&old, id("color")).is_err());
}

fn source_limits() -> iced_css_core::identity::SourceChangeLimits {
    iced_css_core::identity::SourceChangeLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(100000).unwrap(),
        work: NonZeroUsize::new(100000).unwrap(),
    }
}
#[test]
fn source_preparation_abandonment_and_commit_preserve_retained_sibling_progress() {
    let mut engine = engine(10, 10);
    let old = engine.register_source().unwrap();
    let sibling = engine.register_source().unwrap();
    let t = Instant::now();
    let color = selected(&["color"]);
    for source in [&old, &sibling] {
        engine.start_progress(source, &color, timing(), t).unwrap();
    }
    engine.advance(t + Duration::from_millis(250));
    let prepared = engine
        .prepare_source_changes(std::slice::from_ref(&old), 1, source_limits())
        .unwrap();
    drop(prepared);
    assert_eq!(engine.sample(&old, id("color")).unwrap(), Some(0.25));
    let new = engine
        .prepare_source_changes(std::slice::from_ref(&old), 1, source_limits())
        .unwrap()
        .commit();
    assert!(engine.sample(&old, id("color")).is_err());
    assert_eq!(engine.sample(&sibling, id("color")).unwrap(), Some(0.25));
    assert_eq!(engine.instance(&new[0]).unwrap().get(), 3);
    assert_eq!(engine.sample(&new[0], id("color")).unwrap(), None);
    engine
        .start_progress(&new[0], &color, timing(), t + Duration::from_millis(250))
        .unwrap();
    assert_eq!(engine.sample(&new[0], id("color")).unwrap(), Some(0.0));
    assert_eq!(engine.sample(&sibling, id("color")).unwrap(), Some(0.25));
}
#[test]
fn committed_source_removal_releases_bindings_for_normal_track_collection() {
    let mut engine = track_limited(1);
    let old = engine.register_source().unwrap();
    let t = Instant::now();
    let color = selected(&["color"]);
    engine.start_progress(&old, &color, timing(), t).unwrap();
    let new = engine
        .prepare_source_changes(std::slice::from_ref(&old), 1, source_limits())
        .unwrap()
        .commit();
    assert!(!engine.is_animating());
    assert!(engine.sample(&old, id("color")).is_err());
    assert_eq!(
        engine.start_progress(&new[0], &color, timing(), t),
        Err(EngineError::Tracks)
    );
    for _ in 0..3 {
        engine.accepted_build();
    }
    engine.start_progress(&new[0], &color, timing(), t).unwrap();
    assert_eq!(engine.sample(&new[0], id("color")).unwrap(), Some(0.0));
}
#[test]
fn rejected_source_identity_changes_leave_existing_effects_live() {
    use iced_css_core::identity::{IdentityError, SourceChangeError};
    let mut engine = ProgressEngine::new(
        SourceRegistry::new(NonZeroU64::new(1).unwrap()),
        ProgressLimits {
            addresses: NonZeroU64::new(10).unwrap(),
            bindings: NonZeroUsize::new(10).unwrap(),
            tracks: NonZeroUsize::new(10).unwrap(),
            retained_bytes: NonZeroUsize::new(100000).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
    );
    let old = engine.register_source().unwrap();
    let t = Instant::now();
    engine
        .start_progress(&old, &selected(&["color"]), timing(), t)
        .unwrap();
    engine.advance(t + Duration::from_millis(250));
    assert!(matches!(
        engine.prepare_source_changes(std::slice::from_ref(&old), 1, source_limits()),
        Err(EngineError::Sources(SourceChangeError::Identity(
            IdentityError::Exhausted
        )))
    ));
    let mut foreign = SourceRegistry::new(NonZeroU64::new(1).unwrap());
    let foreign = foreign.insert().unwrap();
    assert!(matches!(
        engine.prepare_source_changes(&[old.clone(), foreign], 0, source_limits()),
        Err(EngineError::Sources(SourceChangeError::Identity(
            IdentityError::WrongHost
        )))
    ));
    let mut tiny = source_limits();
    tiny.retained_bytes = NonZeroUsize::new(1).unwrap();
    assert!(
        engine
            .prepare_source_changes(std::slice::from_ref(&old), 0, tiny)
            .is_err()
    );
    assert_eq!(engine.sample(&old, id("color")).unwrap(), Some(0.25));
    assert!(engine.is_animating());
}
