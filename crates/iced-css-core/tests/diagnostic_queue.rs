use iced_css_core::{
    diagnostics::*,
    values::{NonnegativeF32, RangeEvent, registry::PropertyId},
};
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
};
fn host() -> NonZeroU64 {
    NonZeroU64::new(7).unwrap()
}
fn queue(count: usize, bytes: usize) -> DiagnosticQueue {
    DiagnosticQueue::new(host(), QueueLimits::new(count, bytes).unwrap())
}
fn source() -> Arc<Provenance> {
    Arc::new(Provenance {
        logical_name: "theme.css".into(),
        revision: NonZeroU64::new(1).unwrap(),
    })
}
fn report(source: &Arc<Provenance>) -> Diagnostic {
    Diagnostic::range(
        NonnegativeF32 {
            value: f32::MAX,
            event: Some(RangeEvent::UpperSaturation),
        },
        Origin::Authored {
            source: Arc::clone(source),
            span: SourceSpan {
                bytes: 4..12,
                line: NonZeroUsize::new(2).unwrap(),
                column: NonZeroUsize::new(3).unwrap(),
            },
        },
        None,
        PropertyId::lookup("font-size").unwrap(),
    )
    .unwrap()
}
#[test]
fn full_queue_preserves_old_report_and_yields_a_separate_omission_interval() {
    let source = source();
    let mut state = queue(1, 100_000);
    assert_eq!(state.admit(report(&source)), Admission::Queued);
    assert_eq!(
        state.admit(report(&source)),
        Admission::Omitted(OmissionReason::Count)
    );
    assert_eq!(state.pending_count(), 1);
    let Some(Delivery::Omissions { host: id, summary }) = state.handoff() else {
        panic!()
    };
    assert_eq!(id, host());
    assert_eq!(
        summary,
        Omissions {
            count: 1,
            overflow: false
        }
    );
    assert_eq!(state.omissions(), Omissions::default());
    assert_eq!(
        state.admit(report(&source)),
        Admission::Omitted(OmissionReason::Count)
    );
    let Some(Delivery::Report(old)) = state.handoff() else {
        panic!()
    };
    assert_eq!(old.host, host());
    assert_eq!(old.diagnostic.result, Some(f32::MAX));
    let Some(Delivery::Omissions { summary, .. }) = state.handoff() else {
        panic!()
    };
    assert_eq!(summary.count, 1);
    assert!(state.handoff().is_none());
    assert_eq!(state.retained_bytes(), 0);
}
#[test]
fn exact_byte_boundary_and_oversized_single_report_do_not_truncate_payload() {
    let source = source();
    let mut measure = queue(2, 100_000);
    assert_eq!(measure.admit(report(&source)), Admission::Queued);
    let required = measure.retained_bytes();
    assert!(required > 1);
    let mut exact = queue(2, required);
    assert_eq!(exact.admit(report(&source)), Admission::Queued);
    let mut short = queue(2, required - 1);
    assert_eq!(
        short.admit(report(&source)),
        Admission::Omitted(OmissionReason::Bytes)
    );
    assert_eq!(short.pending_count(), 0);
    assert_eq!(short.retained_bytes(), 0);
    let Some(Delivery::Report(received)) = exact.handoff() else {
        panic!()
    };
    let Origin::Authored { source, span } = received.diagnostic.origin else {
        panic!()
    };
    assert_eq!(source.logical_name, "theme.css");
    assert_eq!(span.bytes, 4..12);
}
#[test]
fn shared_provenance_is_charged_once_and_unshared_copies_are_distinct() {
    let shared = source();
    let mut a = queue(4, 100_000);
    let mut b = queue(4, 100_000);
    for _ in 0..2 {
        assert_eq!(a.admit(report(&shared)), Admission::Queued);
        assert_eq!(b.admit(report(&source())), Admission::Queued);
    }
    assert!(b.retained_bytes() > a.retained_bytes());
    let before = a.retained_bytes();
    let handed = a.handoff().unwrap();
    assert!(a.retained_bytes() <= before);
    assert!(a.retained_bytes() > 0);
    a.handoff().unwrap();
    assert_eq!(a.retained_bytes(), 0);
    // Pending accounting does not include reports already handed off.
    drop(handed);
}
#[test]
fn retained_string_and_related_location_capacities_count_even_when_empty() {
    let shared = source();
    let mut small = queue(4, 100_000);
    let mut larger = queue(4, 100_000);
    assert_eq!(small.admit(report(&shared)), Admission::Queued);
    let mut payload = report(&shared);
    payload.reason.reserve_exact(4096);
    payload.related.reserve_exact(8);
    assert_eq!(larger.admit(payload), Admission::Queued);
    assert!(
        larger.retained_bytes()
            >= small.retained_bytes() + 4096 + 8 * std::mem::size_of::<Origin>()
    );
}
#[test]
fn delayed_delivery_keeps_original_source_revision_and_captured_value() {
    let old = source();
    let mut state = queue(2, 100_000);
    assert_eq!(state.admit(report(&old)), Admission::Queued);
    // Subscription absence/preparation does not take ownership of a pending report.
    assert_eq!(state.pending_count(), 1);
    assert_eq!(Arc::strong_count(&old), 2);
    let newer = Arc::new(Provenance {
        logical_name: "theme.css".into(),
        revision: NonZeroU64::new(2).unwrap(),
    });
    let Some(Delivery::Report(received)) = state.handoff() else {
        panic!()
    };
    let Origin::Authored { source, .. } = &received.diagnostic.origin else {
        panic!()
    };
    assert_eq!(source.revision.get(), 1);
    assert_ne!(source.revision, newer.revision);
    assert_eq!(received.diagnostic.result, Some(f32::MAX));
    assert_eq!(state.pending_count(), 0);
}
#[test]
fn host_close_discards_pending_data_and_late_producers_cannot_revive_it() {
    let shared = source();
    let mut state = queue(1, 100_000);
    assert_eq!(state.admit(report(&shared)), Admission::Queued);
    state.admit(report(&shared));
    state.close();
    assert_eq!(state.pending_count(), 0);
    assert_eq!(state.retained_bytes(), 0);
    assert_eq!(state.omissions(), Omissions::default());
    assert_eq!(Arc::strong_count(&shared), 1);
    assert_eq!(state.admit(report(&shared)), Admission::Closed);
    assert!(state.handoff().is_none());
}
#[test]
fn invalid_queue_configuration_does_not_select_defaults() {
    assert!(QueueLimits::new(1, 1).is_ok());
    assert!(matches!(
        QueueLimits::new(0, 1),
        Err(QueueConfigError::InvalidCount)
    ));
    assert!(matches!(
        QueueLimits::new(1, 0),
        Err(QueueConfigError::InvalidBytes)
    ));
    assert!(matches!(
        QueueLimits::new(usize::MAX, 1),
        Err(QueueConfigError::InvalidCount)
    ));
    assert!(matches!(
        QueueLimits::new(1, usize::MAX),
        Err(QueueConfigError::InvalidBytes)
    ));
    let mut tiny = queue(1, 1);
    assert_eq!(
        tiny.admit(report(&source())),
        Admission::Omitted(OmissionReason::Bytes)
    );
}

#[test]
fn interleaved_new_omissions_cannot_starve_an_already_pending_report() {
    let shared = source();
    let mut state = queue(1, 100_000);
    assert_eq!(state.admit(report(&shared)), Admission::Queued);
    let mut delivered = false;
    for _ in 0..2 {
        assert_eq!(
            state.admit(report(&shared)),
            Admission::Omitted(OmissionReason::Count)
        );
        if let Some(Delivery::Report(_)) = state.handoff() {
            delivered = true;
            break;
        }
    }
    assert!(
        delivered,
        "a ready consumer must receive the old report despite continuing omissions"
    );
}
