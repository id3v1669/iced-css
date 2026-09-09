use iced::advanced::widget::Tree;
use iced_css::{
    animation::{
        clock::AnimationTiming,
        engine::{ProgressEngine, ProgressLimits},
    },
    host::{
        compatibility::{CompatibilityLimits, NativeCompatibility},
        leaf_slots::*,
        reconcile::SlotLimits,
    },
};
use iced_css_core::{
    animation::registry::AnimationSelection,
    identity::{SourceChangeLimits, SourceRef, SourceRegistry},
    values::registry::PropertyId,
};
use std::{
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    time::{Duration, Instant},
};
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn engine(limit: u64) -> ProgressEngine {
    ProgressEngine::new(
        SourceRegistry::new(NonZeroU64::new(limit).unwrap()),
        ProgressLimits {
            addresses: NonZeroU64::new(100).unwrap(),
            bindings: n(100),
            tracks: n(100),
            retained_bytes: n(100000),
            work: n(100000),
        },
    )
}
fn limits() -> LeafSlotLimits {
    let slots = SlotLimits {
        entries: n(100),
        retained_bytes: n(100000),
        work: n(100000),
    };
    LeafSlotLimits {
        compatibility: CompatibilityLimits {
            entries: n(100),
            retained_bytes: n(100000),
            work: n(100000),
        },
        sources: slots,
        native: slots,
        identities: SourceChangeLimits {
            entries: n(100),
            retained_bytes: n(100000),
            work: n(100000),
        },
    }
}
fn tree(focus: bool) -> Tree {
    let element: iced::Element<'_, (), iced::Theme, ()> = iced::widget::text_input("", "value")
        .on_input(|_| ())
        .into();
    let mut tree = Tree::new(&element);
    if focus {
        tree.state
            .downcast_mut::<iced::widget::text_input::State<()>>()
            .focus();
    }
    tree
}
fn focused(tree: &Option<Tree>) -> bool {
    tree.as_ref()
        .unwrap()
        .state
        .downcast_ref::<iced::widget::text_input::State<()>>()
        .is_focused()
}
fn schema() -> NativeCompatibility<'static> {
    NativeCompatibility {
        adapter_id: "test.text-input",
        schema_version: NonZeroU32::MIN,
        content_tag: tree(false).tag,
        ordered_roles: b"test:leaf:v1",
        representation: "native",
    }
}
fn prepare<'e, 's, 't>(
    engine: &'e mut ProgressEngine,
    sources: &'s mut [Option<SourceRef>],
    native: &'t mut [Option<Tree>],
    before: &[NativeCompatibility<'_>],
    after: &[NativeCompatibility<'_>],
    mapping: &[Option<usize>],
    limits: LeafSlotLimits,
) -> Result<PreparedLeafSlots<'e, 's, 't>, LeafSlotError> {
    prepare_leaf_slots(
        engine,
        PreviousLeafSlots {
            sources,
            native,
            schema: before,
        },
        IncomingLeafSlots {
            schema: after,
            mapping,
        },
        limits,
    )
}
#[test]
fn retained_focus_and_animation_follow_the_same_source_on_reorder() {
    let mut engine = engine(10);
    let a = engine.register_source().unwrap();
    let b = engine.register_source().unwrap();
    let a_id = engine.instance(&a).unwrap();
    let b_id = engine.instance(&b).unwrap();
    let property = PropertyId::lookup("color").unwrap();
    let selection = AnimationSelection::new(&[property], n(1000)).unwrap();
    let now = Instant::now();
    engine
        .start_progress(
            &a,
            &selection,
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            now,
        )
        .unwrap();
    engine.advance(now + Duration::from_millis(250));
    let mut sources = vec![Some(a), Some(b)];
    let mut native = vec![Some(tree(true)), Some(tree(false))];
    let schemas = [schema(); 2];
    let incoming = prepare(
        &mut engine,
        &mut sources,
        &mut native,
        &schemas,
        &schemas,
        &[Some(1), Some(0)],
        limits(),
    )
    .unwrap()
    .commit();
    assert_eq!(engine.instance(&incoming.sources[0]).unwrap(), b_id);
    assert_eq!(engine.instance(&incoming.sources[1]).unwrap(), a_id);
    assert!(!focused(&incoming.native[0]));
    assert!(focused(&incoming.native[1]));
    assert_eq!(
        engine.sample(&incoming.sources[1], property).unwrap(),
        Some(0.25)
    );
    assert!(sources.iter().all(Option::is_none));
    assert!(native.iter().all(Option::is_none));
}
#[test]
fn compatibility_reset_retires_source_and_leaves_new_native_slot_empty() {
    let mut engine = engine(10);
    let a = engine.register_source().unwrap();
    let old = a.clone();
    let old_id = engine.instance(&a).unwrap();
    let mut sources = vec![Some(a)];
    let mut native = vec![Some(tree(true))];
    let changed = NativeCompatibility {
        representation: "replacement",
        ..schema()
    };
    let prepared = prepare(
        &mut engine,
        &mut sources,
        &mut native,
        &[schema()],
        &[changed],
        &[Some(0)],
        limits(),
    )
    .unwrap();
    assert_eq!(prepared.removed_instances(), &[old_id]);
    let incoming = prepared.commit();
    assert!(engine.instance(&old).is_err());
    assert_ne!(engine.instance(&incoming.sources[0]).unwrap(), old_id);
    assert!(incoming.native[0].is_none());
    assert!(focused(&native[0]));
    assert!(sources[0].is_none());
}
#[test]
fn abandonment_and_native_preparation_failure_preserve_both_owners() {
    let mut engine = engine(10);
    let a = engine.register_source().unwrap();
    let id = engine.instance(&a).unwrap();
    let mut sources = vec![Some(a)];
    let mut native = vec![Some(tree(true))];
    drop(
        prepare(
            &mut engine,
            &mut sources,
            &mut native,
            &[schema()],
            &[schema()],
            &[None],
            limits(),
        )
        .unwrap(),
    );
    let mut small = limits();
    small.native.retained_bytes = n(1);
    assert!(matches!(
        prepare(
            &mut engine,
            &mut sources,
            &mut native,
            &[schema()],
            &[schema()],
            &[None],
            small
        ),
        Err(LeafSlotError::Native(_))
    ));
    assert!(focused(&native[0]));
    assert_eq!(engine.instance(sources[0].as_ref().unwrap()).unwrap(), id);
    let incoming = prepare(
        &mut engine,
        &mut sources,
        &mut native,
        &[schema()],
        &[schema()],
        &[None],
        limits(),
    )
    .unwrap()
    .commit();
    assert_eq!(engine.instance(&incoming.sources[0]).unwrap().get(), 2);
}
#[test]
fn malformed_correspondence_and_scan_limits_reject_before_movement() {
    let mut engine = engine(10);
    let a = engine.register_source().unwrap();
    let mut sources = vec![Some(a)];
    let mut missing = vec![None];
    assert!(matches!(
        prepare(
            &mut engine,
            &mut sources,
            &mut missing,
            &[schema()],
            &[],
            &[],
            limits()
        ),
        Err(LeafSlotError::Occupancy { index: 0 })
    ));
    let mut empty = vec![];
    assert!(matches!(
        prepare(
            &mut engine,
            &mut sources,
            &mut empty,
            &[],
            &[],
            &[],
            limits()
        ),
        Err(LeafSlotError::Dimensions)
    ));
    let mut native = vec![Some(tree(true))];
    let mut small = limits();
    small.sources.work = n(1);
    assert!(matches!(
        prepare(
            &mut engine,
            &mut sources,
            &mut native,
            &[schema()],
            &[],
            &[],
            small
        ),
        Err(LeafSlotError::Sources(_))
    ));
    assert!(focused(&native[0]));
    assert!(engine.instance(sources[0].as_ref().unwrap()).is_ok());
}
#[test]
fn exhausted_identity_reservation_keeps_native_focus_and_original_token() {
    let mut engine = engine(1);
    let a = engine.register_source().unwrap();
    let mut sources = vec![Some(a)];
    let mut native = vec![Some(tree(true))];
    assert!(matches!(
        prepare(
            &mut engine,
            &mut sources,
            &mut native,
            &[schema()],
            &[schema()],
            &[None],
            limits()
        ),
        Err(LeafSlotError::Sources(_))
    ));
    assert!(focused(&native[0]));
    assert_eq!(
        engine.instance(sources[0].as_ref().unwrap()).unwrap().get(),
        1
    );
}
