use super::*;
use iced_css_core::identity::IdAllocator;

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn limits() -> BorderHostLimits {
    BorderHostLimits {
        live_causes: nz(1),
        live_bytes: nz(1_048_576),
        transaction_bytes: nz(4_194_304),
        publication_work: nz(1_000_000),
    }
}
fn queue() -> DiagnosticQueue {
    DiagnosticQueue::new(
        NonZeroU64::MIN,
        iced_css_core::diagnostics::QueueLimits::new(8, 1_048_576).unwrap(),
    )
}
fn observation(generation: u64, key: Option<u64>, extra_limbs: usize) -> BorderObservationBatch {
    let instance = IdAllocator::new(NonZeroU64::new(10).unwrap())
        .allocate()
        .unwrap();
    let event = key.map(|key| {
        let mut limbs = Vec::new();
        limbs.try_reserve_exact(5 + extra_limbs).unwrap();
        limbs.extend_from_slice(&[0, 0, 0, 0, 1000]); // exact 10^39, base10^9.
        BorderPendingEvent {
            logical_key: NonZeroU64::new(key).unwrap(),
            source: Arc::new(Provenance {
                logical_name: "private-fixture.css".into(),
                revision: NonZeroU64::MIN,
            }),
            span: SourceSpan {
                bytes: 0..64,
                line: nz(1),
                column: nz(1),
            },
            value: OwnedBorderValue::Absolute {
                negative: false,
                limbs,
                denominator: [0; 4],
            },
            result_bits: f32::MAX.to_bits(),
            event: RangeEvent::UpperSaturation,
        }
    });
    BorderObservationBatch {
        owner: Weak::new(),
        capture_generation: generation,
        observations: vec![BorderObservation {
            instance,
            property: PropertyId::lookup("border-top-width").unwrap(),
            event,
        }],
        receipt: BorderCaptureReceipt {
            observations: 1,
            events: usize::from(key.is_some()),
            capture_work: 0,
            provenance_work: 0,
            preparation_work: 0,
            capture_bytes: 0,
            prepared_bytes: 0,
        },
    }
}

struct ReserveProbe {
    calls: usize,
    fail: Option<usize>,
}
impl ReservePolicy for ReserveProbe {
    fn reserve<T>(&mut self, value: &mut Vec<T>, additional: usize) -> Result<()> {
        self.calls += 1;
        if self.fail == Some(self.calls) {
            return Err(BorderReportingError::Allocation);
        }
        value
            .try_reserve_exact(additional)
            .map_err(|_| BorderReportingError::Allocation)
    }
}

// A single-slot replacement is allowed; rejecting it as a temporary second live
// cause is wrong. Temporary byte failure must leave the existing cause intact.
#[test]
fn cf34_transaction_one_slot_replace() {
    let queue = queue();
    let empty = BorderState::new(limits()).unwrap();
    let first = empty
        .prepare_with(&observation(1, Some(1), 0), &queue, &mut ExactReserve)
        .unwrap();
    let mut old = first.replacement;
    let prior = old.causes.keys().to_vec();
    let generation = old.generation;
    let next = old.next_semantic;
    let changed = observation(generation, Some(2), 0);
    let prepared = old
        .prepare_with(&changed, &queue, &mut ExactReserve)
        .unwrap();
    assert_eq!(
        (prepared.receipt.new_causes, prepared.receipt.cleared),
        (1, 1)
    );
    assert_eq!(prepared.replacement.causes.live_count(), 1);
    old.limits.transaction_bytes = nz(prepared.receipt.transaction_bytes - 1);
    assert!(matches!(
        old.prepare_with(&changed, &queue, &mut ExactReserve),
        Err(BorderReportingError::Limit(BorderLimit::TransactionBytes))
    ));
    assert_eq!(old.causes.keys(), prior);
    assert_eq!((old.generation, old.next_semantic), (generation, next));
    assert_eq!(queue.pending_count(), 0);
}

// M08: an input's extra OWNED NUMERIC capacity must cost bytes even though its
// logical limbs and output are unchanged. The expected difference is independently
// derived from the two actual Vec capacities, not the accounting helper under test.
#[test]
fn cf34_actual_spare_capacity() {
    let queue = queue();
    let mut old = BorderState::new(limits()).unwrap();
    let compact = observation(1, Some(1), 0);
    let spare = observation(1, Some(1), 37);
    let capacity = |batch: &BorderObservationBatch| {
        let event = batch.observations[0].event.as_ref().unwrap();
        match &event.value {
            OwnedBorderValue::Absolute { limbs, .. } => limbs.capacity(),
            _ => unreachable!(),
        }
    };
    let extra = (capacity(&spare) - capacity(&compact)) * size_of::<u32>();
    assert!(extra > 0, "fixture must retain more owned numeric capacity");
    let baseline = old
        .prepare_with(&compact, &queue, &mut ExactReserve)
        .unwrap();
    old.limits.transaction_bytes = nz(baseline.receipt.transaction_bytes + extra - 1);
    assert!(matches!(
        old.prepare_with(&spare, &queue, &mut ExactReserve),
        Err(BorderReportingError::Limit(BorderLimit::TransactionBytes))
    ));
    assert_eq!(
        (
            old.causes.live_count(),
            old.semantics.len(),
            old.generation,
            old.next_semantic
        ),
        (0, 0, 1, 1)
    );
    assert_eq!(queue.pending_count(), 0);
}

// Each host-owned reservation failure preserves live state and a pending report.
// Core CauseTracker allocation is separately fallible and is not injected here.
#[test]
fn cf34_failure_atomic_allocations() {
    let mut queue = queue();
    let empty = BorderState::new(limits()).unwrap();
    let mut first = empty
        .prepare_with(&observation(1, Some(1), 0), &queue, &mut ExactReserve)
        .unwrap();
    assert_eq!(queue.admit(first.reports.pop().unwrap()), Admission::Queued);
    let old = first.replacement;
    let changed = observation(old.generation, Some(2), 0);
    let mut probe = ReserveProbe {
        calls: 0,
        fail: None,
    };
    old.prepare_with(&changed, &queue, &mut probe).unwrap();
    assert!(probe.calls >= 4);
    let keys = old.causes.keys().to_vec();
    let bytes = queue.retained_bytes();
    let counters = (old.generation, old.next_semantic);
    for fail in 1..=probe.calls {
        let mut failing = ReserveProbe {
            calls: 0,
            fail: Some(fail),
        };
        assert!(matches!(
            old.prepare_with(&changed, &queue, &mut failing),
            Err(BorderReportingError::Allocation)
        ));
        assert_eq!(old.causes.keys(), keys);
        assert_eq!((old.generation, old.next_semantic), counters);
        assert_eq!((queue.pending_count(), queue.retained_bytes()), (1, bytes));
        assert_eq!(queue.omissions().count, 0);
    }
}

// Clearing releases semantic ownership; exhausting IDs or generations cannot
// partially replace an existing cause or publish a diagnostic.
#[test]
fn cf34_interner_reclamation() {
    let queue = queue();
    let mut old = BorderState::new(limits()).unwrap();
    for key in 1..=4 {
        old = old
            .prepare_with(
                &observation(old.generation, Some(key), 0),
                &queue,
                &mut ExactReserve,
            )
            .unwrap()
            .replacement;
        assert_eq!((old.causes.live_count(), old.semantics.len()), (1, 1));
        old = old
            .prepare_with(
                &observation(old.generation, None, 0),
                &queue,
                &mut ExactReserve,
            )
            .unwrap()
            .replacement;
        assert_eq!(
            (
                old.causes.live_count(),
                old.semantics.len(),
                old.causes.retained_bytes()
            ),
            (0, 0, 0)
        );
    }
    old.next_semantic = u64::MAX;
    let captured = observation(old.generation, Some(99), 0);
    let generation = old.generation;
    assert!(matches!(
        old.prepare_with(&captured, &queue, &mut ExactReserve),
        Err(BorderReportingError::IdentityExhausted)
    ));
    assert_eq!(
        (old.generation, old.next_semantic, old.causes.live_count()),
        (generation, u64::MAX, 0)
    );
    old.generation = u64::MAX;
    assert!(matches!(
        old.prepare_with(&observation(u64::MAX, None, 0), &queue, &mut ExactReserve),
        Err(BorderReportingError::GenerationExhausted)
    ));
    assert_eq!(queue.pending_count(), 0);
}
