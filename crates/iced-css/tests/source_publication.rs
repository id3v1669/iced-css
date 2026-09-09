use iced_css::{
    animation::engine::{EngineError, ProgressEngine, ProgressLimits},
    capture::{
        CaptureLimits, CapturedElement, Construction, adapters,
        source::{ConstructionSourceError, ConstructionState},
    },
    host::source_publication::{InitialSourceHost, PublicationError, PublicationLimits},
};
use iced_css_core::{
    identity::{IdentityError, SourceChangeError, SourceChangeLimits, SourceRef, SourceRegistry},
    selectors::{LinkState, SelectorSource},
    source::{AttributeLimits, selector_tree::SelectorTreeLimits},
};
use std::num::{NonZeroU64, NonZeroUsize};
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn capture_limits() -> CaptureLimits {
    CaptureLimits {
        nodes: nz(1000),
        depth: nz(1000),
        retained_bytes: nz(1_000_000),
        work: nz(1_000_000),
    }
}
fn attrs() -> AttributeLimits {
    AttributeLimits {
        entries: nz(100),
        retained_bytes: nz(10000),
        work: nz(10000),
    }
}
fn progress() -> ProgressLimits {
    ProgressLimits {
        addresses: NonZeroU64::new(100).unwrap(),
        bindings: nz(100),
        tracks: nz(100),
        retained_bytes: nz(100000),
        work: nz(100000),
    }
}
fn limits() -> PublicationLimits {
    PublicationLimits {
        nodes: nz(1000),
        snapshot_bytes: nz(1_000_000),
        work: nz(1_000_000),
        selectors: SelectorTreeLimits {
            entries: nz(1000),
            retained_bytes: nz(1_000_000),
            work: nz(1_000_000),
        },
        identities: SourceChangeLimits {
            entries: nz(1000),
            retained_bytes: nz(1_000_000),
            work: nz(1_000_000),
        },
    }
}
fn host() -> InitialSourceHost {
    InitialSourceHost::new(NonZeroU64::new(10000).unwrap(), progress())
}
fn text() -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::text("label", capture_limits()).unwrap().erase()
}
fn leaf() -> Construction {
    text().into_parts().1
}
fn state() -> ConstructionState {
    ConstructionState {
        link: LinkState::NotLink,
        hovered: false,
        active: false,
        focused: false,
    }
}
fn states(n: usize) -> Vec<ConstructionState> {
    vec![state(); n]
}
fn row(children: Vec<CapturedElement<'static, (), iced::Theme, ()>>) -> Construction {
    adapters::row_from_vec(children, capture_limits())
        .unwrap()
        .into_parts()
        .1
}
fn keyed(key: &[u8]) -> CapturedElement<'static, (), iced::Theme, ()> {
    text().source_key(key, capture_limits()).unwrap()
}
#[test]
fn publication_retains_real_metadata_context_and_exact_token_order() {
    let content = text()
        .style_id("cell-content", attrs())
        .unwrap()
        .style_attribute("data-span-source", "declared-only", attrs())
        .unwrap()
        .into_parts()
        .1;
    let construction = Construction::table(1, 0, vec![content], capture_limits()).unwrap();
    let n = construction.nodes().len();
    let expected: Vec<_> = construction
        .nodes()
        .iter()
        .map(|node| (node.canonical_type(), node.parent()))
        .collect();
    let mut explicit = states(n);
    explicit[0].hovered = true;
    explicit[0].focused = true;
    let mut owner = host();
    let prepared = owner
        .prepare(construction, explicit, Some("fr-CA".into()), limits())
        .unwrap();
    let expected_bytes = prepared.retained_bytes();
    let published = prepared.commit();
    assert_eq!(published.retained_bytes(), expected_bytes);
    assert_eq!(published.document_language(), Some("fr-CA"));
    assert_eq!(published.tokens().len(), n);
    assert!(published.states()[0].hovered && published.states()[0].focused);
    assert!(!published.states()[n - 1].hovered); // raw explicit context is not mutated by derived propagation
    assert_eq!(
        published
            .construction()
            .nodes()
            .iter()
            .map(|node| (node.canonical_type(), node.parent()))
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        published.construction().nodes()[0]
            .metadata()
            .attribute("data-span-source"),
        Some("declared-only")
    );
    let selector = published.selector_source(limits().selectors).unwrap();
    assert_eq!(selector.facts(0).unwrap().language, Some("fr-CA"));
    assert!(selector.facts(selector.root()).unwrap().hovered);
    assert!(published.token(n).is_none());
    let published = owner.snapshot().unwrap();
    for (index, token) in published.tokens().iter().enumerate() {
        assert_eq!(
            owner.engine().instance(token).unwrap().get(),
            index as u64 + 1
        );
    }
    assert_eq!(
        published.construction().nodes()[published.construction().root()].canonical_type(),
        "table"
    );
}
#[test]
fn initial_only_scope_wrong_host_and_no_implicit_context() {
    let mut owner = host();
    owner
        .prepare(leaf(), states(1), None, limits())
        .unwrap()
        .commit();
    assert_eq!(owner.snapshot().unwrap().document_language(), None);
    let old = owner.snapshot().unwrap().token(0).unwrap().clone();
    let id = owner.engine().instance(&old).unwrap();
    let error = owner
        .prepare(leaf(), Vec::new(), Some(String::new()), limits())
        .map(|_| ())
        .unwrap_err();
    assert_eq!(error, PublicationError::AlreadyPublished);
    assert_eq!(owner.engine().instance(&old).unwrap(), id);
    assert_eq!(
        owner.snapshot().unwrap().construction().nodes()[0].text(),
        Some("label")
    );
    let other = host();
    assert_eq!(
        other.engine().instance(&old),
        Err(EngineError::Identity(IdentityError::WrongHost))
    );
    drop(owner);
    assert_eq!(
        other.engine().instance(&old),
        Err(EngineError::Identity(IdentityError::WrongHost))
    );
    let mut fresh = host();
    assert_eq!(
        fresh
            .prepare(leaf(), Vec::new(), None, limits())
            .map(|_| ())
            .unwrap_err(),
        PublicationError::StateCount
    );
    assert_eq!(
        fresh
            .prepare(leaf(), states(1), Some(String::new()), limits())
            .map(|_| ())
            .unwrap_err(),
        PublicationError::EmptyLanguage
    );
    assert!(fresh.snapshot().is_none());
    fresh
        .prepare(leaf(), states(1), None, limits())
        .unwrap()
        .commit();
    assert_eq!(
        fresh
            .engine()
            .instance(fresh.snapshot().unwrap().token(0).unwrap())
            .unwrap()
            .get(),
        1
    );
}
#[test]
fn duplicate_metadata_or_parent_scoped_keys_fail_before_publication() {
    let mut owner = host();
    let duplicate_id = row(vec![
        text().style_id("same", attrs()).unwrap(),
        text().style_id("same", attrs()).unwrap(),
    ]);
    assert_eq!(
        owner
            .prepare(duplicate_id, states(3), None, limits())
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Source(ConstructionSourceError::DuplicateCssId)
    );
    for key in [b"".as_slice(), b"same"] {
        assert_eq!(
            owner
                .prepare(row(vec![keyed(key), keyed(key)]), states(3), None, limits())
                .map(|_| ())
                .unwrap_err(),
            PublicationError::DuplicateKey {
                first: 0,
                second: 1
            }
        );
    }
    let distinct_parents = || {
        row(vec![
            adapters::container(keyed(b"same"), capture_limits())
                .unwrap()
                .erase(),
            adapters::container(keyed(b"same"), capture_limits())
                .unwrap()
                .erase(),
        ])
    };
    let mut cap = limits();
    cap.work = nz(9);
    assert_eq!(
        owner
            .prepare(distinct_parents(), states(5), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Work
    );
    cap.work = nz(10);
    owner
        .prepare(distinct_parents(), states(5), None, cap)
        .unwrap()
        .commit();
    let mut missing_and_empty = host();
    missing_and_empty
        .prepare(row(vec![text(), keyed(b"")]), states(3), None, limits())
        .unwrap()
        .commit();
    assert_eq!(
        owner
            .engine()
            .instance(owner.snapshot().unwrap().token(0).unwrap())
            .unwrap()
            .get(),
        1
    );
}
#[test]
fn abandonment_and_identity_exhaustion_preserve_first_id() {
    let mut owner = host();
    for _ in 0..3 {
        drop(owner.prepare(leaf(), states(1), None, limits()).unwrap());
        assert!(owner.snapshot().is_none());
    }
    owner
        .prepare(leaf(), states(1), None, limits())
        .unwrap()
        .commit();
    assert_eq!(
        owner
            .engine()
            .instance(owner.snapshot().unwrap().token(0).unwrap())
            .unwrap()
            .get(),
        1
    );
    let mut small = InitialSourceHost::new(NonZeroU64::new(1).unwrap(), progress());
    let error = small
        .prepare(row(vec![text()]), states(2), None, limits())
        .map(|_| ())
        .unwrap_err();
    assert_eq!(
        error,
        PublicationError::Engine(EngineError::Sources(SourceChangeError::Identity(
            IdentityError::Exhausted
        )))
    );
    assert!(small.snapshot().is_none());
    small
        .prepare(leaf(), states(1), None, limits())
        .unwrap()
        .commit();
    assert_eq!(
        small
            .engine()
            .instance(small.snapshot().unwrap().token(0).unwrap())
            .unwrap()
            .get(),
        1
    );
}
#[test]
fn exact_snapshot_capacity_includes_owned_spare_capacity_and_prepared_tokens() {
    let construction = leaf();
    let construction_bytes = construction.retained_bytes();
    let mut explicit = Vec::with_capacity(20);
    explicit.push(state());
    let state_bytes = explicit.capacity() * std::mem::size_of::<ConstructionState>();
    let mut language = String::with_capacity(100);
    language.push_str("en");
    let language_bytes = language.capacity();
    let mut owner = host();
    let prepared = owner
        .prepare(construction, explicit, Some(language), limits())
        .unwrap();
    let actual = prepared.retained_bytes();
    let mut capacity_engine = ProgressEngine::new(
        SourceRegistry::new(NonZeroU64::new(20).unwrap()),
        progress(),
    );
    let identity = capacity_engine
        .prepare_source_changes(&[], 1, limits().identities)
        .unwrap();
    assert_eq!(
        actual,
        construction_bytes + state_bytes + language_bytes + identity.token_retained_bytes()
    );
    drop(identity);
    let published = prepared.commit();
    assert_eq!(published.retained_bytes(), actual);
    // Identically sized independent candidates determine the exact admission boundary.
    let mut trial = host();
    let prepared = trial.prepare(leaf(), states(1), None, limits()).unwrap();
    let bytes = prepared.retained_bytes();
    drop(prepared);
    let mut cap = limits();
    cap.snapshot_bytes = nz(bytes - 1);
    assert_eq!(
        trial
            .prepare(leaf(), states(1), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Bytes
    );
    assert!(trial.snapshot().is_none());
    cap.snapshot_bytes = nz(bytes);
    trial
        .prepare(leaf(), states(1), None, cap)
        .unwrap()
        .commit();
    assert_eq!(
        trial
            .engine()
            .instance(trial.snapshot().unwrap().token(0).unwrap())
            .unwrap()
            .get(),
        1
    );
}
#[test]
fn named_budget_failures_and_exact_facade_work() {
    let mut owner = host();
    let mut cap = limits();
    cap.nodes = nz(1);
    assert_eq!(
        owner
            .prepare(row(vec![text()]), states(2), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Nodes
    );
    cap = limits();
    cap.work = nz(3);
    assert_eq!(
        owner
            .prepare(leaf(), states(1), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Work
    );
    cap = limits();
    cap.snapshot_bytes = nz(1);
    assert_eq!(
        owner
            .prepare(leaf(), states(1), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Bytes
    );
    cap = limits();
    cap.selectors.retained_bytes = nz(1);
    assert_eq!(
        owner
            .prepare(leaf(), states(1), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Source(ConstructionSourceError::Bytes)
    );
    cap = limits();
    cap.selectors.entries = nz(1);
    assert_eq!(
        owner
            .prepare(row(vec![text()]), states(2), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Source(ConstructionSourceError::Entries)
    );
    cap = limits();
    cap.selectors.work = nz(1);
    assert_eq!(
        owner
            .prepare(leaf(), states(1), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Source(ConstructionSourceError::Work)
    );
    cap = limits();
    cap.identities.entries = nz(1);
    assert_eq!(
        owner
            .prepare(row(vec![text()]), states(2), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Engine(EngineError::Sources(SourceChangeError::Entries))
    );
    cap = limits();
    cap.identities.retained_bytes = nz(1);
    assert_eq!(
        owner
            .prepare(leaf(), states(1), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Engine(EngineError::Sources(SourceChangeError::Bytes))
    );
    cap = limits();
    cap.identities.work = nz(1);
    assert_eq!(
        owner
            .prepare(leaf(), states(1), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Engine(EngineError::Sources(SourceChangeError::Work))
    );
    cap = limits();
    cap.work = nz(4);
    owner
        .prepare(leaf(), states(1), None, cap)
        .unwrap()
        .commit(); // 3+n, no keys
    assert_eq!(
        owner
            .engine()
            .instance(owner.snapshot().unwrap().token(0).unwrap())
            .unwrap()
            .get(),
        1
    );
    // Three records: outer n=3, P=1 for second keyed child, B=min(1,1)+1=2 -> 9.
    let mut keyed_owner = host();
    cap = limits();
    cap.work = nz(8);
    assert_eq!(
        keyed_owner
            .prepare(row(vec![keyed(b"a"), keyed(b"b")]), states(3), None, cap)
            .map(|_| ())
            .unwrap_err(),
        PublicationError::Work
    );
    cap.work = nz(9);
    keyed_owner
        .prepare(row(vec![keyed(b"a"), keyed(b"b")]), states(3), None, cap)
        .unwrap()
        .commit();
}
#[test]
fn forwarded_identity_capacity_is_exact_before_commit() {
    for count in [0, 1, 7] {
        let mut engine = ProgressEngine::new(
            SourceRegistry::new(NonZeroU64::new(20).unwrap()),
            progress(),
        );
        let prepared = engine
            .prepare_source_changes(&[], count, limits().identities)
            .unwrap();
        let before = prepared.token_retained_bytes();
        let tokens = prepared.commit();
        assert_eq!(before, tokens.capacity() * std::mem::size_of::<SourceRef>());
        assert_eq!(tokens.len(), count);
    }
}

#[test]
fn increasing_snapshot_allowance_does_not_reject_small_payloads() {
    let mut owner = host();
    let mut cap = limits();
    cap.snapshot_bytes = nz(usize::MAX);
    owner
        .prepare(leaf(), states(1), None, cap)
        .unwrap()
        .commit();
    assert_eq!(owner.snapshot().unwrap().tokens().len(), 1);
}
