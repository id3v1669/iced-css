use iced_css_core::resources::publication::*;
use std::num::NonZeroU64;
fn limits() -> PublicationLimits {
    PublicationLimits {
        requests: NonZeroU64::new(10).unwrap(),
        revisions: NonZeroU64::new(10).unwrap(),
    }
}
#[test]
fn failed_latest_request_does_not_allow_an_older_success_to_publish() {
    let mut gate = PublicationGate::new("asset", limits());
    let a = gate.request().unwrap();
    let b = gate.request().unwrap();
    assert_eq!(gate.finish(b, false), Ok(PublicationDecision::Failed));
    assert_eq!(gate.finish(a, true), Ok(PublicationDecision::Ignored));
    assert_eq!(gate.status(), PublicationStatus::Failed);
    assert_eq!(gate.active_revision(), None);
}
#[test]
fn reload_and_failure_keep_last_good_until_a_new_success_is_admitted() {
    let mut gate = PublicationGate::new("asset", limits());
    let first = gate.request().unwrap();
    let PublicationDecision::Publish(revision) = gate.finish(first, true).unwrap() else {
        panic!()
    };
    assert_eq!(revision.get(), 1);
    assert_eq!(gate.status(), PublicationStatus::Ready);
    let a = gate.request().unwrap();
    assert_eq!(gate.active_revision(), Some(revision));
    assert_eq!(gate.status(), PublicationStatus::Loading);
    let b = gate.request().unwrap();
    assert_eq!(gate.finish(a, true), Ok(PublicationDecision::Ignored));
    assert_eq!(gate.finish(b, false), Ok(PublicationDecision::Failed));
    assert_eq!(gate.active_revision(), Some(revision));
    let c = gate.request().unwrap();
    let PublicationDecision::Publish(next) = gate.finish(c, true).unwrap() else {
        panic!()
    };
    assert_eq!(next.get(), 2);
    assert_eq!(gate.active_revision(), Some(next));
}
#[test]
fn wrong_resource_and_duplicate_completions_are_ignored() {
    let mut left = PublicationGate::new("left", limits());
    let mut right = PublicationGate::new("right", limits());
    let l = left.request().unwrap();
    let r = right.request().unwrap();
    assert_eq!(right.finish(l, true), Ok(PublicationDecision::Ignored));
    assert_eq!(right.status(), PublicationStatus::Loading);
    assert!(matches!(
        right.finish(r, true),
        Ok(PublicationDecision::Publish(_))
    ));
    assert_eq!(right.finish(r, false), Ok(PublicationDecision::Ignored));
    assert_eq!(right.status(), PublicationStatus::Ready);
    assert_eq!(right.active_revision().unwrap().get(), 1);
}
#[test]
fn generation_and_revision_exhaustion_never_wrap_or_publish() {
    let bounded = PublicationLimits {
        requests: NonZeroU64::MIN,
        revisions: NonZeroU64::MIN,
    };
    let mut gate = PublicationGate::new(1, bounded);
    let first = gate.request().unwrap();
    assert_eq!(gate.request(), Err(PublicationError::Requests));
    assert_eq!(gate.status(), PublicationStatus::Loading);
    assert!(matches!(
        gate.finish(first, true),
        Ok(PublicationDecision::Publish(_))
    ));
    let mut gate = PublicationGate::new(
        1,
        PublicationLimits {
            revisions: NonZeroU64::MIN,
            ..limits()
        },
    );
    let first = gate.request().unwrap();
    gate.finish(first, true).unwrap();
    let second = gate.request().unwrap();
    assert_eq!(gate.finish(second, true), Err(PublicationError::Revisions));
    assert_eq!(gate.active_revision().unwrap().get(), 1);
    assert_eq!(gate.status(), PublicationStatus::Failed);
    assert_eq!(gate.finish(second, true), Ok(PublicationDecision::Ignored));
}
#[test]
fn every_three_request_completion_order_admits_only_the_latest_once() {
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        for latest_success in [false, true] {
            let mut gate = PublicationGate::new(1, limits());
            let tickets = [
                gate.request().unwrap(),
                gate.request().unwrap(),
                gate.request().unwrap(),
            ];
            for index in order {
                let outcome = gate
                    .finish(tickets[index], index != 2 || latest_success)
                    .unwrap();
                if index != 2 {
                    assert_eq!(outcome, PublicationDecision::Ignored);
                } else if latest_success {
                    assert!(matches!(outcome, PublicationDecision::Publish(_)));
                } else {
                    assert_eq!(outcome, PublicationDecision::Failed);
                }
            }
            assert_eq!(
                gate.active_revision().map(|r| r.get()),
                latest_success.then_some(1)
            );
        }
    }
}

#[test]
fn current_ticket_preflight_is_read_only_and_does_not_promise_revision_capacity() {
    let mut gate = PublicationGate::new(
        "a",
        PublicationLimits {
            requests: NonZeroU64::new(3).unwrap(),
            revisions: NonZeroU64::MIN,
        },
    );
    let mut other = PublicationGate::new("b", limits());
    let first = gate.request().unwrap();
    assert!(gate.is_current(first));
    assert!(gate.is_current(first));
    assert!(!gate.is_current(other.request().unwrap()));
    assert_eq!(gate.status(), PublicationStatus::Loading);
    gate.finish(first, true).unwrap();
    assert!(!gate.is_current(first));
    let second = gate.request().unwrap();
    let third = gate.request().unwrap();
    assert!(!gate.is_current(second));
    assert!(gate.is_current(third));
    assert_eq!(gate.request(), Err(PublicationError::Requests));
    assert!(gate.is_current(third));
    assert_eq!(gate.finish(third, true), Err(PublicationError::Revisions));
    assert!(!gate.is_current(third));
    assert_eq!(gate.active_revision().unwrap().get(), 1);
}
