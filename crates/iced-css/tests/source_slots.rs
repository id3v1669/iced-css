use iced_css::{
    animation::{clock::AnimationTiming, engine::*},
    host::{reconcile::SlotLimits, source_slots::*},
};
use iced_css_core::{
    animation::registry::AnimationSelection,
    identity::{SourceChangeLimits, SourceRegistry},
    source::sibling_matches::*,
    values::registry::PropertyId,
};
use std::{
    num::{NonZeroU64, NonZeroUsize},
    time::{Duration, Instant},
};
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn engine() -> ProgressEngine {
    ProgressEngine::new(
        SourceRegistry::new(NonZeroU64::new(10).unwrap()),
        ProgressLimits {
            addresses: NonZeroU64::new(10).unwrap(),
            bindings: n(10),
            tracks: n(10),
            retained_bytes: n(100000),
            work: n(100000),
        },
    )
}
fn slots() -> SlotLimits {
    SlotLimits {
        entries: n(100),
        retained_bytes: n(100000),
        work: n(100000),
    }
}
fn identities() -> SourceChangeLimits {
    SourceChangeLimits {
        entries: n(100),
        retained_bytes: n(100000),
        work: n(100000),
    }
}
fn color() -> PropertyId {
    PropertyId::lookup("color").unwrap()
}
fn selection() -> AnimationSelection {
    AnimationSelection::new(&[color()], n(1000)).unwrap()
}
fn timing() -> AnimationTiming {
    AnimationTiming::new(Duration::from_secs(1), Duration::ZERO)
}
#[test]
fn real_sibling_reorder_moves_tokens_and_retains_their_progress() {
    let mut engine = engine();
    let a = engine.register_source().unwrap();
    let b = engine.register_source().unwrap();
    let ids = [engine.instance(&a).unwrap(), engine.instance(&b).unwrap()];
    let t = Instant::now();
    engine
        .start_progress(&a, &selection(), timing(), t)
        .unwrap();
    engine.advance(t + Duration::from_millis(250));
    let matched = match_siblings(
        &[SiblingKey::Explicit(b"a"), SiblingKey::Explicit(b"b")],
        &[SiblingKey::Explicit(b"b"), SiblingKey::Explicit(b"a")],
        MatchLimits {
            entries: n(10),
            retained_bytes: n(1000),
            work: n(1000),
        },
    )
    .unwrap();
    // These fixtures assert unchanged adapter compatibility within one parent.
    let mut previous = vec![Some(a), Some(b)];
    let incoming = prepare_source_slots(
        &mut engine,
        &mut previous,
        matched.previous_indices(),
        slots(),
        identities(),
    )
    .unwrap()
    .commit();
    assert!(previous.iter().all(Option::is_none));
    assert_eq!(engine.instance(&incoming[0]).unwrap(), ids[1]);
    assert_eq!(engine.instance(&incoming[1]).unwrap(), ids[0]);
    assert_eq!(engine.sample(&incoming[1], color()).unwrap(), Some(0.25));
    assert_eq!(engine.sample(&incoming[0], color()).unwrap(), None);
}
#[test]
fn unmatched_sources_are_retired_and_new_tokens_follow_incoming_positions() {
    let mut engine = engine();
    let a = engine.register_source().unwrap();
    let b = engine.register_source().unwrap();
    let old_id = engine.instance(&a).unwrap();
    let b_id = engine.instance(&b).unwrap();
    let old = a.clone();
    engine
        .start_progress(&a, &selection(), timing(), Instant::now())
        .unwrap();
    let mut previous = vec![Some(a), Some(b), None];
    let prepared = prepare_source_slots(
        &mut engine,
        &mut previous,
        &[None, Some(1), None],
        slots(),
        identities(),
    )
    .unwrap();
    assert_eq!(prepared.removed_instances(), &[old_id]);
    let incoming = prepared.commit();
    assert_eq!(
        incoming
            .iter()
            .map(|s| engine.instance(s).unwrap().get())
            .collect::<Vec<_>>(),
        vec![3, b_id.get(), 4]
    );
    assert!(engine.sample(&old, color()).is_err());
    assert!(!engine.is_animating());
    assert!(previous.iter().all(Option::is_none));
}
#[test]
fn dropped_and_rejected_slot_preparations_leave_tokens_and_effects_intact() {
    let mut engine = engine();
    let a = engine.register_source().unwrap();
    let t = Instant::now();
    engine
        .start_progress(&a, &selection(), timing(), t)
        .unwrap();
    engine.advance(t + Duration::from_millis(250));
    let mut previous = vec![Some(a), None];
    drop(prepare_source_slots(&mut engine, &mut previous, &[None], slots(), identities()).unwrap());
    for mapping in [vec![Some(0), Some(0)], vec![Some(1)], vec![Some(2)]] {
        assert!(
            prepare_source_slots(&mut engine, &mut previous, &mapping, slots(), identities())
                .is_err()
        );
        assert_eq!(
            engine
                .sample(previous[0].as_ref().unwrap(), color())
                .unwrap(),
            Some(0.25)
        );
    }
    let incoming = prepare_source_slots(
        &mut engine,
        &mut previous,
        &[Some(0), None],
        slots(),
        identities(),
    )
    .unwrap()
    .commit();
    assert_eq!(engine.instance(&incoming[1]).unwrap().get(), 2);
    assert_eq!(engine.sample(&incoming[0], color()).unwrap(), Some(0.25));
}
#[test]
fn duplicate_or_foreign_prior_tokens_and_each_budget_fail_before_retirement() {
    let mut engine = engine();
    let a = engine.register_source().unwrap();
    let mut duplicate = vec![Some(a.clone()), Some(a.clone())];
    assert!(
        prepare_source_slots(
            &mut engine,
            &mut duplicate,
            &[Some(0)],
            slots(),
            identities()
        )
        .is_err()
    );
    assert!(duplicate.iter().all(Option::is_some));
    let mut foreign = SourceRegistry::new(NonZeroU64::new(1).unwrap());
    let mut wrong = vec![Some(foreign.insert().unwrap())];
    assert!(prepare_source_slots(&mut engine, &mut wrong, &[], slots(), identities()).is_err());
    let mut previous = vec![Some(a)];
    let mut entries = slots();
    entries.entries = n(1);
    let mut bytes = slots();
    bytes.retained_bytes = n(1);
    let mut work = slots();
    work.work = n(1);
    for limits in [entries, bytes, work] {
        assert!(
            prepare_source_slots(&mut engine, &mut previous, &[None], limits, identities())
                .is_err()
        );
        assert!(engine.instance(previous[0].as_ref().unwrap()).is_ok());
    }
    let incoming = prepare_source_slots(
        &mut engine,
        &mut previous,
        &[Some(0)],
        slots(),
        identities(),
    )
    .unwrap()
    .commit();
    assert_eq!(engine.instance(&incoming[0]).unwrap().get(), 1);
}
