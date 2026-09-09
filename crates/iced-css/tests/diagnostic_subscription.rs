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
