use iced_css::capture::{
    CaptureLimits, CapturedElement, Construction, adapters, keyed_children::*,
};
use iced_css_core::source::sibling_matches::{Generation, MatchError, MatchLimits};
use std::num::NonZeroUsize;
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn capture() -> CaptureLimits {
    CaptureLimits {
        nodes: n(100),
        depth: n(100),
        retained_bytes: n(100000),
        work: n(100000),
    }
}
fn limits() -> CapturedMatchLimits {
    CapturedMatchLimits {
        nodes: n(100),
        retained_bytes: n(100000),
        work: n(100000),
        matching: MatchLimits {
            entries: n(100),
            retained_bytes: n(100000),
            work: n(100000),
        },
    }
}
fn text(key: &[u8]) -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::text("text", capture())
        .unwrap()
        .source_key(key, capture())
        .unwrap()
        .erase()
}
fn tree(keys: &[&[u8]]) -> Construction {
    adapters::row_from_vec(keys.iter().map(|key| text(key)).collect(), capture())
        .unwrap()
        .into_parts()
        .1
}
fn parent(c: &Construction) -> CapturedParent<'_> {
    CapturedParent {
        construction: c,
        position: c.root(),
    }
}
#[test]
fn direct_child_positions_and_key_matches_exclude_descendants() {
    let nested = adapters::container(text(b"grandchild"), capture())
        .unwrap()
        .source_key(b"branch", capture())
        .unwrap()
        .erase();
    let before = adapters::row_from_vec(vec![nested, text(b"sibling")], capture())
        .unwrap()
        .into_parts()
        .1;
    let after = tree(&[b"sibling", b"new", b"branch"]);
    let matched = match_keyed_children(parent(&before), parent(&after), limits()).unwrap();
    assert_eq!(matched.previous_positions(), &[1, 2]);
    assert_eq!(matched.incoming_positions(), &[0, 1, 2]);
    assert_eq!(matched.previous_indices(), &[Some(1), None, Some(0)]);
    let descendant = match_keyed_children(
        CapturedParent {
            construction: &before,
            position: 1,
        },
        CapturedParent {
            construction: &before,
            position: 1,
        },
        limits(),
    )
    .unwrap();
    assert_eq!(descendant.previous_positions(), &[0]);
    assert_eq!(descendant.previous_indices(), &[Some(0)]);
}
#[test]
fn empty_and_explicit_empty_key_are_not_implicit_identity_fallbacks() {
    let empty = tree(&[]);
    let keyed = tree(&[b""]);
    assert_eq!(
        match_keyed_children(parent(&empty), parent(&keyed), limits())
            .unwrap()
            .previous_indices(),
        &[None]
    );
    assert!(
        match_keyed_children(parent(&keyed), parent(&empty), limits())
            .unwrap()
            .previous_indices()
            .is_empty()
    );
    let unkeyed: CapturedElement<'_, (), iced::Theme, ()> =
        adapters::text("", capture()).unwrap().erase();
    let unkeyed = adapters::container(unkeyed, capture())
        .unwrap()
        .into_parts()
        .1;
    assert!(matches!(
        match_keyed_children(parent(&empty), parent(&unkeyed), limits()),
        Err(CapturedMatchError::MissingKey {
            generation: Generation::Incoming,
            position: 0
        })
    ));
    assert!(matches!(
        match_keyed_children(parent(&unkeyed), parent(&empty), limits()),
        Err(CapturedMatchError::MissingKey {
            generation: Generation::Previous,
            position: 0
        })
    ));
}
#[test]
fn duplicate_key_errors_retain_generation_and_sibling_positions() {
    let unique = tree(&[b"a"]);
    let duplicate = tree(&[b"a", b"a"]);
    assert!(matches!(
        match_keyed_children(parent(&unique), parent(&duplicate), limits()),
        Err(CapturedMatchError::Matching(MatchError::Duplicate {
            generation: Generation::Incoming,
            first: 0,
            second: 1
        }))
    ));
    assert!(matches!(
        match_keyed_children(parent(&duplicate), parent(&unique), limits()),
        Err(CapturedMatchError::Matching(MatchError::Duplicate {
            generation: Generation::Previous,
            first: 0,
            second: 1
        }))
    ));
}
#[test]
fn invalid_parent_and_each_explicit_budget_fail() {
    let before = tree(&[b"a"]);
    let after = tree(&[b"a"]);
    assert!(match_keyed_children(parent(&before), parent(&after), limits()).is_ok());
    assert!(matches!(
        match_keyed_children(
            CapturedParent {
                construction: &before,
                position: 99
            },
            parent(&after),
            limits()
        ),
        Err(CapturedMatchError::Parent {
            generation: Generation::Previous,
            position: 99
        })
    ));
    let mut nodes = limits();
    nodes.nodes = n(1);
    let mut bytes = limits();
    bytes.retained_bytes = n(1);
    let mut work = limits();
    work.work = n(1);
    let mut matcher = limits();
    matcher.matching.work = n(1);
    for bound in [nodes, bytes, work, matcher] {
        assert!(match_keyed_children(parent(&before), parent(&after), bound).is_err());
    }
    assert_eq!(before.nodes()[0].source_key(), Some(&b"a"[..]));
}

#[test]
fn captured_siblings_feed_native_source_and_progress_preparation() {
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
        identity::{SourceChangeLimits, SourceRegistry},
        values::registry::PropertyId,
    };
    use std::{
        num::{NonZeroU32, NonZeroU64},
        time::{Duration, Instant},
    };
    let before = adapters::row_from_vec(vec![text(b"a"), text(b"b")], capture())
        .unwrap()
        .erase::<(), iced::Theme, ()>();
    let (native_view, before) = before.into_parts();
    let after = tree(&[b"b", b"new", b"a"]);
    let matches = match_keyed_children(parent(&before), parent(&after), limits()).unwrap();
    let mut native: Vec<_> = Tree::new(&native_view)
        .children
        .into_iter()
        .map(Some)
        .collect();
    assert_eq!(native.len(), 2);
    // Declared fixture contract: complete Text Tree per leaf, fixed Row::from_vec
    // child slots, no managed descendants beneath either selected Text source.
    let schema = NativeCompatibility {
        adapter_id: "fixture.text",
        schema_version: NonZeroU32::MIN,
        content_tag: native[0].as_ref().unwrap().tag,
        ordered_roles: b"fixture:text:leaf:v1",
        representation: "native",
    };
    let mut engine = ProgressEngine::new(
        SourceRegistry::new(NonZeroU64::new(10).unwrap()),
        ProgressLimits {
            addresses: NonZeroU64::new(10).unwrap(),
            bindings: n(10),
            tracks: n(10),
            retained_bytes: n(100000),
            work: n(100000),
        },
    );
    let a = engine.register_source().unwrap();
    let b = engine.register_source().unwrap();
    let ids = [engine.instance(&a).unwrap(), engine.instance(&b).unwrap()];
    let color = PropertyId::lookup("color").unwrap();
    let now = Instant::now();
    engine
        .start_progress(
            &a,
            &AnimationSelection::new(&[color], n(1000)).unwrap(),
            AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
            now,
        )
        .unwrap();
    engine.advance(now + Duration::from_millis(250));
    let mut sources = vec![Some(a), Some(b)];
    let slots = SlotLimits {
        entries: n(100),
        retained_bytes: n(100000),
        work: n(100000),
    };
    let incoming = prepare_leaf_slots(
        &mut engine,
        PreviousLeafSlots {
            sources: &mut sources,
            native: &mut native,
            schema: &[schema; 2],
        },
        IncomingLeafSlots {
            schema: &[schema; 3],
            mapping: matches.previous_indices(),
        },
        LeafSlotLimits {
            sources: slots,
            native: slots,
            compatibility: CompatibilityLimits {
                entries: n(100),
                retained_bytes: n(100000),
                work: n(100000),
            },
            identities: SourceChangeLimits {
                entries: n(100),
                retained_bytes: n(100000),
                work: n(100000),
            },
        },
    )
    .unwrap()
    .commit();
    assert_eq!(engine.instance(&incoming.sources[0]).unwrap(), ids[1]);
    assert_eq!(engine.instance(&incoming.sources[2]).unwrap(), ids[0]);
    assert_eq!(engine.instance(&incoming.sources[1]).unwrap().get(), 3);
    assert!(incoming.native[0].is_some());
    assert!(incoming.native[1].is_none());
    assert!(incoming.native[2].is_some());
    assert_eq!(
        engine.sample(&incoming.sources[2], color).unwrap(),
        Some(0.25)
    );
    assert_eq!(engine.sample(&incoming.sources[0], color).unwrap(), None);
}
