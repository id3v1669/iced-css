//! Private CF34 capture tests. No public batch construction or reporting seam.
use super::*;
use crate::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement, adapters, source::ConstructionState},
    diagnostics::{DiagnosticHost, border::BorderHostLimits},
    evaluation::clearance_box_facts::initial_box_fact,
    host::source_publication::{InitialSourceHost, PublicationLimits},
};
use iced_css_core::{
    diagnostics::QueueLimits, identity::SourceChangeLimits,
    source::selector_tree::SelectorTreeLimits,
};

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

// The same real five-node publication shape used by sibling projection tests.
fn source_host() -> InitialSourceHost {
    let capture = CaptureLimits {
        nodes: nz(100),
        depth: nz(100),
        retained_bytes: nz(1_000_000),
        work: nz(1_000_000),
    };
    type Element = CapturedElement<'static, (), iced::Theme, ()>;
    let leaf = || {
        adapters::row_from_vec(Vec::<Element>::new(), capture)
            .unwrap()
            .erase()
    };
    let parent = adapters::column_from_vec(vec![leaf(), leaf(), leaf()], capture)
        .unwrap()
        .erase();
    let outer = adapters::column_from_vec(vec![parent], capture).unwrap();
    let (_, construction) = outer.into_parts();
    let mut source = InitialSourceHost::new(
        NonZeroU64::new(100).unwrap(),
        ProgressLimits {
            addresses: NonZeroU64::new(100).unwrap(),
            bindings: nz(100),
            tracks: nz(100),
            retained_bytes: nz(1_000_000),
            work: nz(1_000_000),
        },
    );
    source
        .prepare(
            construction,
            vec![ConstructionState::default(); 5],
            None,
            PublicationLimits {
                nodes: nz(100),
                snapshot_bytes: nz(1_000_000),
                work: nz(1_000_000),
                selectors: SelectorTreeLimits {
                    entries: nz(100),
                    retained_bytes: nz(1_000_000),
                    work: nz(1_000_000),
                },
                identities: SourceChangeLimits {
                    entries: nz(100),
                    retained_bytes: nz(1_000_000),
                    work: nz(1_000_000),
                },
            },
        )
        .unwrap()
        .commit();
    source
}

fn host_limits() -> BorderHostLimits {
    BorderHostLimits {
        live_causes: nz(32),
        live_bytes: nz(1_000_000),
        transaction_bytes: nz(4_000_000),
        publication_work: nz(1_000_000),
    }
}
fn capture_limits() -> BorderCaptureLimits {
    BorderCaptureLimits {
        observations: nz(64),
        bindings: nz(128),
        capture_bytes: nz(262_144),
        prepared_bytes: nz(2_097_152),
        capture_work: nz(1_000_000),
        provenance_work: nz(1_000_000),
        preparation_work: nz(1_000_000),
    }
}

struct SpareReservation {
    minimum: usize,
    calls: usize,
}
impl Reservation for SpareReservation {
    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        count: usize,
    ) -> std::result::Result<(), TryReserveError> {
        self.calls += 1;
        values.try_reserve_exact(count.max(self.minimum))
    }
}
struct FailedReservation {
    calls: usize,
}
impl Reservation for FailedReservation {
    fn reserve<T>(
        &mut self,
        _values: &mut Vec<T>,
        _count: usize,
    ) -> std::result::Result<(), TryReserveError> {
        self.calls += 1;
        // This request necessarily exceeds Vec<u8>'s isize bound. It produces
        // the real reserve error type without attempting a huge allocation.
        Err(Vec::<u8>::new()
            .try_reserve_exact(usize::MAX)
            .expect_err("capacity overflow"))
    }
}

// Fails if raw admission checks requested length instead of actual capacity, or
// resizes/uses the rejected buffer before returning the deferred capture error.
fn raw_spare_capacity() {
    let source = source_host();
    let mut host = DiagnosticHost::new(NonZeroU64::MIN, QueueLimits::new(32, 1_000_000).unwrap());
    let reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let control = Collector::new(5, &reporter, capture_limits());
    assert_eq!(control.error, None);
    assert_eq!(control.raw.len(), 5);
    assert!(control.raw.iter().all(Option::is_none));
    let slots = control.raw.capacity();
    let measured = slots.checked_mul(size_of::<Option<RawRow<'_>>>()).unwrap();
    assert_eq!(control.capture_bytes, measured);
    drop(control);
    let limits = BorderCaptureLimits {
        capture_bytes: nz(measured),
        ..capture_limits()
    };
    let exact = Collector::new(5, &reporter, limits);
    assert_eq!(exact.error, None);
    assert_eq!(exact.capture_bytes, measured);
    drop(exact);

    let collector = Collector::with_reservation(
        5,
        &reporter,
        limits,
        SpareReservation {
            minimum: slots.checked_add(1).unwrap(),
            calls: 0,
        },
    );
    assert_eq!(
        collector.error,
        Some(Error::Limit(BorderLimit::CaptureBytes))
    );
    assert_eq!(collector.reservation.calls, 1);
    assert!(collector.raw.capacity() > slots);
    assert!(collector.raw.is_empty());
    assert_eq!(collector.events, 0);
    assert!(collector.stamp.is_some());
    assert_eq!(host.health(), Ok(()));
}

// The first raw allocation failure must disable collection without mutating the
// captured rows, retrying allocation, or replacing the original failure when
// subsequent ordinary rows invoke the observer. Full fact-error precedence is
// tested independently through the public evaluator integration.
fn raw_failure_atomic_allocations() {
    let source = source_host();
    let mut host = DiagnosticHost::new(NonZeroU64::MIN, QueueLimits::new(32, 1_000_000).unwrap());
    let reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    let mut collector = Collector::with_reservation(
        5,
        &reporter,
        capture_limits(),
        FailedReservation { calls: 0 },
    );
    assert_eq!(collector.error, Some(Error::Allocation));
    assert!(collector.raw.is_empty());
    assert_eq!(collector.raw.capacity(), 0);
    assert_eq!(collector.reservation.calls, 1);
    assert_eq!(collector.events, 0);
    assert!(collector.stamp.is_some());

    let winners = PropertyWinners::new();
    let row = initial_box_fact();
    let ids = [PropertyId::lookup("border-top-width").unwrap(); 29];
    // These indices would be InvalidObservation in an enabled collector. The
    // existing Allocation error must remain first and no collection may occur.
    collector.observe(usize::MAX, &winners, &row, &ids);
    collector.observe(0, &winners, &row, &ids);
    assert_eq!(collector.error, Some(Error::Allocation));
    assert_eq!(collector.reservation.calls, 1);
    assert_eq!(collector.events, 0);
    assert!(collector.raw.is_empty());
    assert_eq!(host.health(), Ok(()));

    // Failure is local to the observation, not retained in the source or host.
    let next = Collector::new(5, &reporter, capture_limits());
    assert_eq!(next.error, None);
    assert_eq!(next.raw.len(), 5);
    assert!(next.raw.iter().all(Option::is_none));
}

#[path = "border_diagnostics_fixture.rs"]
mod fixture;

use iced::advanced::subscription;
use iced::futures::{self, Stream};
use iced_css_core::{
    diagnostics::{Delivery, Diagnostic, Origin},
    values::NonnegativeF32,
};
use std::{cell::RefCell, rc::Rc};
use std::{
    pin::Pin,
    task::{Context, Poll, Waker},
};

#[derive(Debug, Clone, Copy)]
struct Attempt {
    requested: usize,
    element_bytes: usize,
    capacity: usize,
}
type Trace = Rc<RefCell<Vec<Attempt>>>;
struct TracedReservation {
    trace: Trace,
    fail_at: Option<usize>,
    oversized_at: Option<usize>,
    extra: usize,
}
impl Reservation for TracedReservation {
    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        count: usize,
    ) -> std::result::Result<(), TryReserveError> {
        let attempt = self.trace.borrow().len().checked_add(1).unwrap();
        self.trace.borrow_mut().push(Attempt {
            requested: count,
            element_bytes: size_of::<T>(),
            capacity: 0,
        });
        if self.fail_at == Some(attempt) {
            return Err(Vec::<u8>::new()
                .try_reserve_exact(usize::MAX)
                .expect_err("capacity overflow"));
        }
        let requested = if self.oversized_at == Some(attempt) {
            count.checked_add(self.extra).unwrap()
        } else {
            count
        };
        values.try_reserve_exact(requested)?;
        self.trace.borrow_mut()[attempt - 1].capacity = values.capacity();
        Ok(())
    }
}
fn traced(
    fail_at: Option<usize>,
    oversized_at: Option<usize>,
    extra: usize,
) -> (Trace, TracedReservation) {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let policy = TracedReservation {
        trace: Rc::clone(&trace),
        fail_at,
        oversized_at,
        extra,
    };
    (trace, policy)
}
fn drain(host: &DiagnosticHost) -> Vec<Delivery> {
    let mut stream: Pin<Box<dyn Stream<Item = Delivery> + Send>> =
        subscription::into_recipes(host.subscription())
            .pop()
            .unwrap()
            .stream(Box::pin(futures::stream::pending()));
    let mut delivered = Vec::new();
    while let Poll::Ready(Some(delivery)) = stream
        .as_mut()
        .poll_next(&mut Context::from_waker(Waker::noop()))
    {
        delivered.push(delivery);
    }
    delivered
}
fn seed_queue(host: &DiagnosticHost) {
    let diagnostic = Diagnostic::range(
        NonnegativeF32 {
            value: f32::MAX,
            event: Some(RangeEvent::UpperSaturation),
        },
        Origin::Synthetic("preexisting sentinel".into()),
        None,
        PropertyId::lookup("font-size").unwrap(),
    )
    .unwrap();
    assert_eq!(
        host.producer().report(diagnostic).unwrap(),
        iced_css_core::diagnostics::Admission::Queued
    );
}
fn assert_queue_unchanged(host: &DiagnosticHost) {
    let delivered = drain(host);
    assert_eq!(delivered.len(), 1, "collection changed the shared queue");
    let Delivery::Report(report) = &delivered[0] else {
        panic!("sentinel report replaced by omission")
    };
    assert!(
        matches!(&report.diagnostic.origin, Origin::Synthetic(label) if label == "preexisting sentinel")
    );
    assert_eq!(report.diagnostic.property, PropertyId::lookup("font-size"));
    assert_eq!(
        report.diagnostic.result.unwrap().to_bits(),
        f32::MAX.to_bits()
    );
}
fn assert_trace_roles(trace: &[Attempt], events: usize) {
    assert_eq!(
        trace.len(),
        3 + events,
        "raw, cache, observations, then each event limb vector"
    );
    assert_eq!(
        (trace[0].requested, trace[0].element_bytes),
        (5, size_of::<Option<RawRow<'_>>>())
    );
    assert_eq!(
        (trace[1].requested, trace[1].element_bytes),
        (2, size_of::<ResolvedBinding<'_>>())
    );
    assert_eq!(
        (trace[2].requested, trace[2].element_bytes),
        (20, size_of::<BorderObservation>())
    );
    for attempt in &trace[3..] {
        assert_eq!(
            (attempt.requested, attempt.element_bytes),
            (5, size_of::<u32>())
        );
    }
}

#[test]
fn cf34_capture_spare_capacity() {
    raw_spare_capacity();
    let source = source_host();
    let mut host = DiagnosticHost::new(NonZeroU64::MIN, QueueLimits::new(32, 1_000_000).unwrap());
    let reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    fixture::with_graph(&source, |projection, display, mappings| {
        let (trace, policy) = traced(None, None, 0);
        let baseline = fixture::collect(
            projection,
            display,
            mappings,
            &reporter,
            capture_limits(),
            policy,
        )
        .unwrap();
        assert_eq!(baseline.receipt().events, 5);
        let peak = baseline.receipt().prepared_bytes;
        let attempts = trace.borrow().len();
        assert_trace_roles(&trace.borrow(), 5);
        drop(baseline);
        let limits = BorderCaptureLimits {
            prepared_bytes: nz(peak),
            ..capture_limits()
        };
        let (_, policy) = traced(None, None, 0);
        let exact =
            fixture::collect(projection, display, mappings, &reporter, limits, policy).unwrap();
        assert_eq!(exact.receipt().prepared_bytes, peak);
        drop(exact);

        // Raw capacity has its independent account above. Every later reserve
        // contributes to the same measured preparation peak: cache, batch and
        // all five owned numeric allocations. An oversized reservation alone
        // exceeds the whole bound; failure must precede subsequent reserve calls.
        for attempt in 2..=attempts {
            seed_queue(&host);
            let (trace, policy) = traced(None, Some(attempt), peak.checked_add(1).unwrap());
            let result = fixture::collect(projection, display, mappings, &reporter, limits, policy);
            assert!(
                matches!(result, Err(Error::Limit(BorderLimit::PreparedBytes))),
                "attempt {attempt}"
            );
            let log = trace.borrow();
            assert_eq!(log.len(), attempt);
            assert!(log[attempt - 1].capacity > log[attempt - 1].requested);
            assert!(
                log[attempt - 1]
                    .capacity
                    .checked_mul(log[attempt - 1].element_bytes)
                    .unwrap()
                    > peak
            );
            drop(log);
            assert_queue_unchanged(&host);
        }
    });
}

#[test]
fn cf34_capture_failure_atomic_allocations() {
    raw_failure_atomic_allocations();
    let source = source_host();
    let mut host = DiagnosticHost::new(NonZeroU64::MIN, QueueLimits::new(32, 1_000_000).unwrap());
    let mut reporter = host.bind_border_reporting(&source, host_limits()).unwrap();
    fixture::with_graph(&source, |projection, display, mappings| {
        let (trace, policy) = traced(None, None, 0);
        let baseline = fixture::collect(
            projection,
            display,
            mappings,
            &reporter,
            capture_limits(),
            policy,
        )
        .unwrap();
        let events = baseline.receipt().events;
        assert_eq!(events, 5);
        assert_trace_roles(&trace.borrow(), events);
        let attempts = trace.borrow().len();
        drop(baseline);
        for attempt in 1..=attempts {
            seed_queue(&host);
            let (trace, policy) = traced(Some(attempt), None, 0);
            let result = fixture::collect(
                projection,
                display,
                mappings,
                &reporter,
                capture_limits(),
                policy,
            );
            assert!(
                matches!(result, Err(Error::Allocation)),
                "attempt {attempt}"
            );
            assert_eq!(trace.borrow().len(), attempt, "failed reserve was retried");
            assert_queue_unchanged(&host);
        }
        // No failed collection published or suppressed a cause. A complete real
        // observation after every injected failure still queues all five events.
        let (_, policy) = traced(None, None, 0);
        let batch = fixture::collect(
            projection,
            display,
            mappings,
            &reporter,
            capture_limits(),
            policy,
        )
        .unwrap();
        let receipt = reporter.publish(batch).unwrap();
        assert_eq!(
            (receipt.new_causes, receipt.suppressed, receipt.queued),
            (events, 0, events)
        );
        assert_eq!(drain(&host).len(), events);
    });
}
