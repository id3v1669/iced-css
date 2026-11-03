use iced::{
    advanced::subscription::{self, Hasher},
    futures::{self, Stream},
};
use iced_css::diagnostics::DiagnosticHost;
use iced_css_core::{
    diagnostics::{Admission, Delivery, Diagnostic, Origin, QueueLimits},
    values::{NonnegativeF32, RangeEvent, registry::PropertyId},
};
use std::{
    num::NonZeroU64,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};
fn host() -> DiagnosticHost {
    DiagnosticHost::new(NonZeroU64::MIN, QueueLimits::new(2, 100_000).unwrap())
}
fn report() -> Diagnostic {
    Diagnostic::range(
        NonnegativeF32 {
            value: f32::MAX,
            event: Some(RangeEvent::UpperSaturation),
        },
        Origin::Synthetic("fixture".into()),
        None,
        PropertyId::lookup("font-size").unwrap(),
    )
    .unwrap()
}
fn stream(host: &DiagnosticHost) -> Pin<Box<dyn Stream<Item = Delivery> + Send>> {
    let mut recipes = subscription::into_recipes(host.subscription());
    assert_eq!(recipes.len(), 1, "one stable diagnostic recipe per host");
    recipes
        .pop()
        .unwrap()
        .stream(Box::pin(futures::stream::pending()))
}
#[derive(Default)]
struct Wakes(AtomicUsize);
impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn poll(
    stream: &mut Pin<Box<dyn Stream<Item = Delivery> + Send>>,
    waker: &Waker,
) -> Poll<Option<Delivery>> {
    stream.as_mut().poll_next(&mut Context::from_waker(waker))
}
#[test]
fn pending_stream_wakes_and_hands_each_report_off_once() {
    let host = host();
    let producer = host.producer();
    let mut stream = stream(&host);
    let wakes = Arc::new(Wakes::default());
    let waker = Waker::from(Arc::clone(&wakes));
    assert!(poll(&mut stream, &waker).is_pending());
    assert_eq!(producer.report(report()).unwrap(), Admission::Queued);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert!(matches!(
        poll(&mut stream, &waker),
        Poll::Ready(Some(Delivery::Report(_)))
    ));
    assert!(poll(&mut stream, &waker).is_pending());
}
#[test]
fn canceled_subscription_keeps_reports_for_reattachment() {
    let host = host();
    let producer = host.producer();
    let waker = Waker::noop();
    let mut first = stream(&host);
    assert!(poll(&mut first, waker).is_pending());
    drop(first);
    assert_eq!(producer.report(report()).unwrap(), Admission::Queued);
    let mut second = stream(&host);
    assert!(matches!(
        poll(&mut second, waker),
        Poll::Ready(Some(Delivery::Report(_)))
    ));
    drop(second);
    let mut third = stream(&host);
    assert!(poll(&mut third, waker).is_pending());
}
#[test]
fn replacement_stream_retires_old_receiver_without_stealing_pending_data() {
    let host = host();
    let producer = host.producer();
    let waker = Waker::noop();
    let mut old = stream(&host);
    let mut replacement = stream(&host);
    assert_eq!(producer.report(report()).unwrap(), Admission::Queued);
    assert!(matches!(poll(&mut old, waker), Poll::Ready(None)));
    drop(old);
    assert!(matches!(
        poll(&mut replacement, waker),
        Poll::Ready(Some(Delivery::Report(_)))
    ));
}
#[test]
fn permanent_host_drop_closes_stream_and_late_producers() {
    let host = host();
    let producer = host.producer();
    let mut stream = stream(&host);
    let wakes = Arc::new(Wakes::default());
    let waker = Waker::from(Arc::clone(&wakes));
    assert!(poll(&mut stream, &waker).is_pending());
    drop(host);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert_eq!(producer.report(report()).unwrap(), Admission::Closed);
    assert!(matches!(poll(&mut stream, &waker), Poll::Ready(None)));
}
#[test]
fn subscription_identity_is_stable_per_host_and_distinct_across_hosts() {
    use std::hash::Hasher as _;
    fn hash(host: &DiagnosticHost) -> u64 {
        let mut recipes = subscription::into_recipes(host.subscription());
        assert_eq!(recipes.len(), 1);
        let mut hasher = Hasher::default();
        recipes.pop().unwrap().hash(&mut hasher);
        hasher.finish()
    }
    let first = host();
    let second = host();
    assert_eq!(hash(&first), hash(&first));
    assert_ne!(hash(&first), hash(&second));
}

#[test]
fn dropping_stale_receiver_does_not_remove_replacements_waker() {
    let host = host();
    let producer = host.producer();
    let old = stream(&host);
    let mut next = stream(&host);
    let wakes = Arc::new(Wakes::default());
    let waker = Waker::from(Arc::clone(&wakes));
    assert!(poll(&mut next, &waker).is_pending());
    drop(old);
    assert_eq!(producer.report(report()).unwrap(), Admission::Queued);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert!(matches!(
        poll(&mut next, &waker),
        Poll::Ready(Some(Delivery::Report(_)))
    ));
}
#[test]
fn cancellation_after_wakeup_before_yield_does_not_lose_the_report() {
    let host = host();
    let producer = host.producer();
    let mut old = stream(&host);
    let waker = Waker::noop();
    assert!(poll(&mut old, waker).is_pending());
    assert_eq!(producer.report(report()).unwrap(), Admission::Queued);
    drop(old);
    let mut next = stream(&host);
    assert!(matches!(
        poll(&mut next, waker),
        Poll::Ready(Some(Delivery::Report(_)))
    ));
    assert!(poll(&mut next, waker).is_pending());
}
#[test]
fn application_message_mapping_uses_iced_subscription_without_an_adapter_dependency() {
    enum Message {
        Diagnostic(Delivery),
    }
    let host = host();
    let producer = host.producer();
    let mut recipes = subscription::into_recipes(host.subscription().map(Message::Diagnostic));
    let mut stream = recipes
        .pop()
        .unwrap()
        .stream(Box::pin(futures::stream::pending()));
    assert_eq!(producer.report(report()).unwrap(), Admission::Queued);
    assert!(matches!(
        stream
            .as_mut()
            .poll_next(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Some(Message::Diagnostic(Delivery::Report(_))))
    ));
    assert_eq!(host.health(), Ok(()));
}

#[path = "support/clearance_fixture.rs"]
mod cf34_fixture;

fn border_host_limits() -> iced_css::diagnostics::border::BorderHostLimits {
    use cf34_fixture::nz;
    iced_css::diagnostics::border::BorderHostLimits {
        live_causes: nz(32),
        live_bytes: nz(1_048_576),
        transaction_bytes: nz(4_194_304),
        publication_work: nz(1_000_000),
    }
}

fn border_batch(
    source: &cf34_fixture::InitialSourceHost,
    reporter: &iced_css::diagnostics::border::BorderReporter<'_>,
) -> iced_css::diagnostics::border::BorderObservationBatch {
    use cf34_fixture::*;
    use iced_css::diagnostics::border::{BorderCaptureLimits, BorderDeclarationBinding};
    use iced_css::evaluation::sibling_box_facts::evaluate_sibling_box_facts_reporting;
    use iced_css_core::diagnostics::Provenance;
    let snapshot = source.snapshot().unwrap();
    let projection = prepare_sibling_source_projection(
        source,
        snapshot,
        SiblingProjectionLimits {
            sources: bound(),
            work: bound(),
            retained_bytes: nz(20_000_000),
        },
    )
    .unwrap();
    let ua = catalog();
    let graph = graph(
        "#p {border-top-style:solid;border-top-width:1000000000000000000000000000000000000000px}",
        context(100, 100),
    );
    let sheets = controls(&[1], true, 100);
    let bindings = [binding(&graph, GraphOrigin::Author(sheet(1)))];
    let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
    let display = evaluate_source_display(snapshot, &bundle, display_limits()).unwrap();
    let provenance = Arc::new(Provenance {
        logical_name: "wake.css".into(),
        revision: NonZeroU64::MIN,
    });
    let mut mappings = Vec::new();
    for rule in graph.rules() {
        for ordinal in 0..rule.rule.declarations.len() {
            let id =
                NonZeroU64::new(rule.context.first_declaration.get() + ordinal as u64).unwrap();
            mappings.push(BorderDeclarationBinding {
                graph: &graph,
                resource: rule.source,
                occurrence: rule.occurrence,
                declaration: id,
                logical_key: id,
                provenance: Arc::clone(&provenance),
            });
        }
    }
    evaluate_sibling_box_facts_reporting(
        &projection,
        &display,
        SiblingBoxFactsLimits {
            sources: bound(),
            retained_bytes: nz(20_000_000),
            evaluation: fact_limits(),
        },
        reporter,
        &mappings,
        BorderCaptureLimits {
            observations: nz(64),
            bindings: nz(128),
            capture_bytes: nz(262_144),
            prepared_bytes: nz(2_097_152),
            capture_work: nz(1_000_000),
            provenance_work: nz(1_000_000),
            preparation_work: nz(1_000_000),
        },
    )
    .unwrap()
    .reporting
    .unwrap()
}

// Reentrant producer admission acquires the same mutex. A wake under the border
// commit lock cannot complete this callback; callbacks must follow unlock.
struct BorderReentrantWake {
    producer: iced_css::diagnostics::DiagnosticProducer,
    calls: AtomicUsize,
}
impl Wake for BorderReentrantWake {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(self.producer.report(report()).unwrap(), Admission::Queued);
    }
}

#[test]
fn border_report_wakes_after_commit() {
    let source = cf34_fixture::captured_siblings(5);
    let mut host = DiagnosticHost::new(NonZeroU64::MIN, QueueLimits::new(32, 1_048_576).unwrap());
    let mut reporter = host
        .bind_border_reporting(&source, border_host_limits())
        .unwrap();
    let mut receiver = stream(&host);
    let wakes = Arc::new(BorderReentrantWake {
        producer: host.producer(),
        calls: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&wakes));
    assert!(poll(&mut receiver, &waker).is_pending());
    let captured = border_batch(&source, &reporter);
    assert_eq!(reporter.publish(captured).unwrap().queued, 1);
    assert_eq!(wakes.calls.load(Ordering::SeqCst), 1);
    for property in ["border-top-width", "font-size"] {
        let Poll::Ready(Some(Delivery::Report(report))) = poll(&mut receiver, Waker::noop()) else {
            panic!("expected queued report")
        };
        assert_eq!(report.diagnostic.property, PropertyId::lookup(property));
    }
    assert!(poll(&mut receiver, Waker::noop()).is_pending());
}

#[test]
fn border_report_survives_receiver_replacement() {
    let source = cf34_fixture::captured_siblings(5);
    let mut host = DiagnosticHost::new(NonZeroU64::MIN, QueueLimits::new(32, 1_048_576).unwrap());
    let mut reporter = host
        .bind_border_reporting(&source, border_host_limits())
        .unwrap();
    let mut old = stream(&host);
    let mut replacement = stream(&host);
    let captured = border_batch(&source, &reporter);
    assert_eq!(reporter.publish(captured).unwrap().queued, 1);
    assert!(matches!(poll(&mut old, Waker::noop()), Poll::Ready(None)));
    drop(old);
    let Poll::Ready(Some(Delivery::Report(report))) = poll(&mut replacement, Waker::noop()) else {
        panic!("replacement must receive border report")
    };
    assert_eq!(
        report.diagnostic.property,
        PropertyId::lookup("border-top-width")
    );
    assert!(poll(&mut replacement, Waker::noop()).is_pending());
    let repeated = border_batch(&source, &reporter);
    assert_eq!(reporter.publish(repeated).unwrap().suppressed, 1);
    assert!(poll(&mut replacement, Waker::noop()).is_pending());
}
