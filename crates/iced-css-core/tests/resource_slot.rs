use iced_css_core::resources::{body::*, publication::*, slot::*, triggers::*};
use std::num::{NonZeroU64, NonZeroUsize};
fn limits() -> PublicationLimits {
    PublicationLimits {
        requests: NonZeroU64::new(100).unwrap(),
        revisions: NonZeroU64::new(100).unwrap(),
    }
}
fn body(bytes: &[u8]) -> CompletedBody {
    let mut body = BodyCollector::new(
        None,
        BodyLimits {
            retained_bytes: NonZeroUsize::new(100).unwrap(),
            work: NonZeroUsize::new(1000).unwrap(),
        },
    )
    .unwrap();
    body.append(bytes).unwrap();
    body.finish(BodyEnd::Complete).unwrap()
}
#[test]
fn latest_failure_retains_real_active_bytes_and_rejects_an_older_success() {
    let mut slot = RuntimeResourceSlot::new(1, RuntimeMode::Auto, ResourceKind::Web, limits());
    let first = slot.request(RequestReason::FirstAcquisition).unwrap();
    assert!(matches!(
        slot.complete(first, Ok::<_, &str>(body(b"old"))),
        ResourceCompletion::Published { retired: None, .. }
    ));
    let a = slot.request(RequestReason::ExplicitReload).unwrap();
    let b = slot.request(RequestReason::ExplicitReload).unwrap();
    assert!(matches!(
        slot.complete(b, Err("truncated")),
        ResourceCompletion::Failed("truncated")
    ));
    assert_eq!(slot.status(), PublicationStatus::Failed);
    assert_eq!(slot.active().unwrap().1.as_slice(), b"old");
    let ResourceCompletion::Ignored(Ok(rejected)) = slot.complete(a, Ok::<_, &str>(body(b"late")))
    else {
        panic!()
    };
    assert_eq!(rejected.as_slice(), b"late");
    assert_eq!(slot.active().unwrap().1.as_slice(), b"old");
    assert_eq!(
        slot.request(RequestReason::Reevaluation),
        Err(RequestError::Unauthorized)
    );
    assert_eq!(
        slot.request(RequestReason::LocalChange),
        Err(RequestError::Unauthorized)
    );
}
#[test]
fn replacement_returns_retired_payload_without_running_its_destructor_in_publication() {
    use std::{cell::Cell, rc::Rc};
    struct Tracked(Rc<Cell<usize>>);
    impl Drop for Tracked {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let mut slot =
        RuntimeResourceSlot::new(1, RuntimeMode::Manual, ResourceKind::LocalFile, limits());
    let a = slot.request(RequestReason::FirstAcquisition).unwrap();
    let result = slot.complete(a, Ok::<_, ()>(Tracked(drops.clone())));
    assert!(matches!(
        result,
        ResourceCompletion::Published { retired: None, .. }
    ));
    drop(result);
    let b = slot.request(RequestReason::ExplicitReload).unwrap();
    let ResourceCompletion::Published { revision, retired } =
        slot.complete(b, Ok::<_, ()>(Tracked(drops.clone())))
    else {
        panic!()
    };
    assert_eq!(drops.get(), 0);
    assert_eq!(revision.get(), 2);
    assert_eq!(slot.active().unwrap().0, revision);
    drop(retired);
    assert_eq!(drops.get(), 1);
    drop(slot);
    assert_eq!(drops.get(), 2);
}
#[test]
fn wrong_resource_and_revision_exhaustion_return_candidates_without_replacing_active_state() {
    let bounded = PublicationLimits {
        requests: NonZeroU64::new(2).unwrap(),
        revisions: NonZeroU64::MIN,
    };
    let mut slot = RuntimeResourceSlot::new(1, RuntimeMode::Manual, ResourceKind::Web, bounded);
    let mut other =
        RuntimeResourceSlot::<_, u8>::new(2, RuntimeMode::Manual, ResourceKind::Web, limits());
    let foreign = other.request(RequestReason::FirstAcquisition).unwrap();
    let first = slot.request(RequestReason::FirstAcquisition).unwrap();
    assert!(matches!(
        slot.complete(foreign, Ok::<_, ()>(9)),
        ResourceCompletion::Ignored(Ok(9))
    ));
    assert!(slot.active().is_none());
    assert_eq!(slot.status(), PublicationStatus::Loading);
    assert!(matches!(
        slot.complete(first, Ok::<_, ()>(1)),
        ResourceCompletion::Published { .. }
    ));
    let second = slot.request(RequestReason::ExplicitReload).unwrap();
    assert!(matches!(
        slot.complete(second, Ok::<_, ()>(2)),
        ResourceCompletion::Exhausted {
            error: PublicationError::Revisions,
            candidate: Ok(2)
        }
    ));
    assert_eq!(slot.active().unwrap().1, &1);
    assert_eq!(slot.status(), PublicationStatus::Failed);
    assert_eq!(
        slot.request(RequestReason::ExplicitReload),
        Err(RequestError::Exhausted(PublicationError::Requests))
    );
}

#[test]
fn ready_payload_excludes_pending_failed_and_obsolete_replacements() {
    let mut slot = RuntimeResourceSlot::new(1, RuntimeMode::Auto, ResourceKind::Web, limits());
    assert!(slot.ready().is_none());
    let first = slot.request(RequestReason::FirstAcquisition).unwrap();
    assert!(slot.ready().is_none());
    slot.complete(first, Ok::<_, &str>(body(b"old")));
    assert_eq!(slot.ready().unwrap().1.as_slice(), b"old");
    for reason in [
        RequestReason::LocalChange,
        RequestReason::Reevaluation,
        RequestReason::CacheExpiry,
    ] {
        assert_eq!(slot.request(reason), Err(RequestError::Unauthorized));
        assert_eq!(slot.ready().unwrap().1.as_slice(), b"old");
    }
    let older = slot.request(RequestReason::ExplicitReload).unwrap();
    assert!(slot.ready().is_none());
    assert_eq!(slot.active().unwrap().1.as_slice(), b"old");
    let latest = slot.request(RequestReason::ExplicitReload).unwrap();
    slot.complete(latest, Err("failed refresh"));
    assert!(slot.ready().is_none());
    assert_eq!(slot.active().unwrap().1.as_slice(), b"old");
    slot.complete(older, Ok::<_, &str>(body(b"obsolete")));
    assert!(slot.ready().is_none());
    let retry = slot.request(RequestReason::ExplicitReload).unwrap();
    slot.complete(retry, Ok::<_, &str>(body(b"new")));
    assert_eq!(slot.ready().unwrap().1.as_slice(), b"new");
    let mut bounded = RuntimeResourceSlot::new(
        2,
        RuntimeMode::Manual,
        ResourceKind::Web,
        PublicationLimits {
            requests: NonZeroU64::MIN,
            revisions: NonZeroU64::MIN,
        },
    );
    let first = bounded.request(RequestReason::FirstAcquisition).unwrap();
    bounded.complete(first, Ok::<_, &str>(body(b"accepted")));
    assert_eq!(
        bounded.request(RequestReason::ExplicitReload),
        Err(RequestError::Exhausted(PublicationError::Requests))
    );
    // Rejected admission creates no newer accepted request. Callers must handle that error.
    assert_eq!(bounded.ready().unwrap().1.as_slice(), b"accepted");
}
