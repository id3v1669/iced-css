use iced_css_core::resources::{publication::*, triggers::*};
use std::num::NonZeroU64;
fn limits() -> PublicationLimits {
    PublicationLimits {
        requests: NonZeroU64::new(10).unwrap(),
        revisions: NonZeroU64::new(10).unwrap(),
    }
}
#[test]
fn first_web_acquisition_is_allowed_once_and_does_not_reset_after_failure() {
    for mode in [RuntimeMode::Manual, RuntimeMode::Auto] {
        let mut gate = RuntimeResourceGate::new(1, mode, ResourceKind::Web, limits());
        let first = gate.request(RequestReason::FirstAcquisition).unwrap();
        assert_eq!(first.generation(), 1);
        gate.finish(first, false).unwrap();
        assert_eq!(
            gate.request(RequestReason::FirstAcquisition),
            Err(RequestError::Unauthorized)
        );
        assert_eq!(gate.status(), PublicationStatus::Failed);
        let explicit = gate.request(RequestReason::ExplicitReload).unwrap();
        assert_eq!(explicit.generation(), 2);
    }
}
#[test]
fn local_changes_are_allowed_only_for_auto_local_resources() {
    for mode in [RuntimeMode::Manual, RuntimeMode::Auto] {
        for kind in [
            ResourceKind::LocalFile,
            ResourceKind::Web,
            ResourceKind::Embedded,
        ] {
            let mut gate = RuntimeResourceGate::new(1, mode, kind, limits());
            let first = gate.request(RequestReason::FirstAcquisition).unwrap();
            gate.finish(first, true).unwrap();
            let result = gate.request(RequestReason::LocalChange);
            if mode == RuntimeMode::Auto && kind == ResourceKind::LocalFile {
                assert_eq!(result.unwrap().generation(), 2);
            } else {
                assert_eq!(result, Err(RequestError::Unauthorized));
                assert_eq!(gate.status(), PublicationStatus::Ready);
            }
        }
    }
}
#[test]
fn passive_causes_never_authorize_new_or_replacement_requests() {
    for reason in [
        RequestReason::Reevaluation,
        RequestReason::CacheExpiry,
        RequestReason::CacheEviction,
    ] {
        for kind in [
            ResourceKind::LocalFile,
            ResourceKind::Web,
            ResourceKind::Embedded,
        ] {
            for mode in [RuntimeMode::Manual, RuntimeMode::Auto] {
                let mut gate = RuntimeResourceGate::new(1, mode, kind, limits());
                assert_eq!(gate.request(reason), Err(RequestError::Unauthorized));
                assert_eq!(gate.status(), PublicationStatus::Unrequested);
                let first = gate.request(RequestReason::FirstAcquisition).unwrap();
                assert_eq!(gate.request(reason), Err(RequestError::Unauthorized));
                assert_eq!(gate.status(), PublicationStatus::Loading);
                assert!(matches!(
                    gate.finish(first, true),
                    Ok(PublicationDecision::Publish(_))
                ));
                assert_eq!(gate.request(reason), Err(RequestError::Unauthorized));
                assert_eq!(gate.active_revision().unwrap().get(), 1);
            }
        }
    }
}
#[test]
fn authorized_requests_still_obey_latest_completion_and_lifetime_limits() {
    let mut gate = RuntimeResourceGate::new(
        1,
        RuntimeMode::Manual,
        ResourceKind::Web,
        PublicationLimits {
            requests: NonZeroU64::new(2).unwrap(),
            ..limits()
        },
    );
    let first = gate.request(RequestReason::ExplicitReload).unwrap();
    let next = gate.request(RequestReason::ExplicitReload).unwrap();
    assert_eq!(gate.finish(first, true), Ok(PublicationDecision::Ignored));
    assert_eq!(
        gate.request(RequestReason::ExplicitReload),
        Err(RequestError::Exhausted(PublicationError::Requests))
    );
    assert!(matches!(
        gate.finish(next, true),
        Ok(PublicationDecision::Publish(_))
    ));
    assert_eq!(gate.active_revision().unwrap().get(), 1);
}

#[test]
fn explicit_embedded_notifications_are_scoped_to_embedded_resources_in_both_modes() {
    for mode in [RuntimeMode::Manual, RuntimeMode::Auto] {
        for kind in [
            ResourceKind::LocalFile,
            ResourceKind::Web,
            ResourceKind::Embedded,
        ] {
            let mut gate = RuntimeResourceGate::new(1, mode, kind, limits());
            let first = gate.request(RequestReason::FirstAcquisition).unwrap();
            gate.finish(first, true).unwrap();
            let result = gate.request(RequestReason::EmbeddedNotification);
            if kind == ResourceKind::Embedded {
                let next = result.unwrap();
                assert_eq!(next.generation(), 2);
                assert_eq!(gate.finish(next, false), Ok(PublicationDecision::Failed));
                assert_eq!(gate.active_revision().unwrap().get(), 1);
            } else {
                assert_eq!(result, Err(RequestError::Unauthorized));
                assert_eq!(gate.status(), PublicationStatus::Ready);
            }
        }
    }
}
