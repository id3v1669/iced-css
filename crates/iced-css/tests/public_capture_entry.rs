use iced::{
    Element, Size,
    advanced::{layout, widget::Tree},
    widget::{Column, Text},
};
use iced_css::capture::{
    CaptureError, CaptureLimits, adapters,
    context::{CaptureContext, ContextError, PublicCaptureEntry, current_limits, public_entry},
};
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
};
fn limits(nodes: usize) -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: NonZeroUsize::new(nodes).unwrap(),
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
#[derive(Default)]
struct Calls {
    native: Cell<usize>,
    captured: Cell<usize>,
    log: RefCell<Vec<ContextError>>,
}
fn native<'a>(owned: String, label: &'a str, calls: &Calls) -> Element<'a, (), iced::Theme, ()> {
    calls.native.set(calls.native.get() + 1);
    Column::from_vec(vec![Text::new(owned).into(), Text::new(label).into()])
        .width(100)
        .height(40)
        .into()
}
fn captured<'a>(
    owned: String,
    label: &'a str,
    calls: &Calls,
    limits: CaptureLimits,
) -> Result<Element<'a, (), iced::Theme, ()>, CaptureError> {
    calls.captured.set(calls.captured.get() + 1);
    let children = vec![
        adapters::text(owned, limits)?.erase(),
        adapters::text(label, limits)?.erase(),
    ];
    Ok(adapters::column_from_vec(children, limits)?
        .width(100)
        .height(40)
        .erase()
        .into_parts()
        .0)
}
// A manually authored stand-in for the public ordinary-Element companion shape.
// Test configuration admits this captured branch; generated error handling is separate.
fn public_view<'a>(
    owned: String,
    label: &'a str,
    calls: &Calls,
) -> Element<'a, (), iced::Theme, ()> {
    match public_entry(|error| calls.log.borrow_mut().push(error)) {
        PublicCaptureEntry::Capture(limits) => captured(owned, label, calls, limits).unwrap(),
        PublicCaptureEntry::Unstyled => native(owned, label, calls),
    }
}
fn observe(mut element: Element<'_, (), iced::Theme, ()>) -> (iced::Rectangle, usize) {
    let mut tree = Tree::new(&element);
    let layout = element.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
    );
    (layout.bounds(), tree.children.len())
}
#[test]
fn missing_context_logs_and_runs_only_the_ordinary_borrowed_view() {
    let calls = Calls::default();
    let label = String::from("borrowed");
    let actual = public_view(String::from("moved"), &label, &calls);
    assert_eq!(*calls.log.borrow(), [ContextError::Missing]);
    assert_eq!(calls.native.get(), 1);
    assert_eq!(calls.captured.get(), 0);
    let control = Calls::default();
    let expected = native(String::from("moved"), &label, &control);
    assert_eq!(observe(actual), observe(expected));
    assert_eq!(calls.native.get(), 1);
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
}
#[test]
fn configured_and_nested_entries_keep_limits_and_never_fallback_after_capture_failure() {
    let calls = Calls::default();
    let label = String::from("borrowed");
    let actual =
        CaptureContext::new(limits(3)).run(|| public_view(String::from("moved"), &label, &calls));
    assert_eq!(calls.captured.get(), 1);
    assert_eq!(calls.native.get(), 0);
    assert!(calls.log.borrow().is_empty());
    assert_eq!(
        observe(actual),
        observe(native(String::from("moved"), &label, &Calls::default()))
    );
    CaptureContext::new(limits(3)).run(|| {
        CaptureContext::new(limits(1)).run(|| {
            let result=match public_entry(|error|calls.log.borrow_mut().push(error)) {
                PublicCaptureEntry::Capture(limits)=>{assert_eq!(limits.nodes.get(),1);captured(String::from("moved"),&label,&calls,limits)},
                PublicCaptureEntry::Unstyled=>Ok(native(String::from("moved"),&label,&calls)),
            };
            assert!(matches!(result,Err(CaptureError::Nodes)));
        });
        assert!(matches!(public_entry(|e|calls.log.borrow_mut().push(e)),PublicCaptureEntry::Capture(l) if l.nodes.get()==3));
    });
    assert!(calls.log.borrow().is_empty());
    assert_eq!(calls.native.get(), 0);
    assert_eq!(calls.captured.get(), 2);
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
}
#[test]
fn reporter_panic_propagates_before_view_construction_without_installing_a_scope() {
    let calls = Calls::default();
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match public_entry(|_| std::panic::panic_any(57u32)) {
            PublicCaptureEntry::Unstyled => {
                calls.native.set(1);
            }
            PublicCaptureEntry::Capture(_) => {
                calls.captured.set(1);
            }
        }
    }))
    .unwrap_err();
    assert_eq!(*failure.downcast::<u32>().unwrap(), 57);
    assert_eq!(calls.native.get(), 0);
    assert_eq!(calls.captured.get(), 0);
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
}
#[test]
fn async_entry_is_selected_at_poll_time_with_no_scope_or_with_explicit_scope() {
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };
    let calls = Calls::default();
    let mut cx = Context::from_waker(Waker::noop());
    let future = CaptureContext::new(limits(3))
        .run(|| async { public_view(String::from("moved"), "borrowed", &calls) });
    let mut future = pin!(future);
    let Poll::Ready(element) = future.as_mut().poll(&mut cx) else {
        panic!("ready view")
    };
    assert_eq!(observe(element).1, 2);
    assert_eq!(*calls.log.borrow(), [ContextError::Missing]);
    let future = async { public_view(String::from("moved"), "borrowed", &calls) };
    let mut future = pin!(future);
    let context = CaptureContext::new(limits(3));
    let mut scoped = context.scope_future(future.as_mut());
    let Poll::Ready(element) = std::pin::Pin::new(&mut scoped).poll(&mut cx) else {
        panic!("ready view")
    };
    assert_eq!(observe(element).1, 2);
    assert_eq!(calls.native.get(), 1);
    assert_eq!(calls.captured.get(), 1);
    assert_eq!(*calls.log.borrow(), [ContextError::Missing]);
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
}

#[test]
fn default_log_entry_reports_the_setup_error_and_selects_the_real_unstyled_view() {
    use std::sync::Mutex;
    struct Sink(Mutex<Vec<String>>);
    impl log::Log for Sink {
        fn enabled(&self, m: &log::Metadata<'_>) -> bool {
            m.target() == "iced_css::capture"
        }
        fn log(&self, r: &log::Record<'_>) {
            if self.enabled(r.metadata()) {
                assert_eq!(r.level(), log::Level::Error);
                self.0.lock().unwrap().push(r.args().to_string());
            }
        }
        fn flush(&self) {}
    }
    static SINK: Sink = Sink(Mutex::new(Vec::new()));
    log::set_logger(&SINK).unwrap();
    log::set_max_level(log::LevelFilter::Error);
    let calls = Calls::default();
    let element = match iced_css::capture::context::public_entry_logged() {
        PublicCaptureEntry::Unstyled => native(String::from("moved"), "borrowed", &calls),
        PublicCaptureEntry::Capture(limits) => {
            captured(String::from("moved"), "borrowed", &calls, limits).unwrap()
        }
    };
    assert_eq!(observe(element).1, 2);
    assert_eq!(calls.native.get(), 1);
    assert_eq!(calls.captured.get(), 0);
    let logs = SINK.0.lock().unwrap();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].contains("capture requires an explicit CaptureContext"));
    assert!(logs[0].contains(file!()));
    assert!(logs[0].contains("returning the ordinary unstyled iced view"));
    drop(logs);
    CaptureContext::new(limits(3)).run(|| {
        assert!(matches!(
            iced_css::capture::context::public_entry_logged(),
            PublicCaptureEntry::Capture(_)
        ))
    });
    assert_eq!(SINK.0.lock().unwrap().len(), 1);
}
