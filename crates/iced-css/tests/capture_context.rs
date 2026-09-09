use iced_css::capture::{
    CaptureLimits, CapturedElement, adapters,
    context::{CaptureContext, ContextError, current_limits},
};
use std::num::NonZeroUsize;
fn limits(nodes: usize) -> CaptureLimits {
    let n = |v| NonZeroUsize::new(v).unwrap();
    CaptureLimits {
        nodes: n(nodes),
        depth: n(10),
        retained_bytes: n(10000),
        work: n(10000),
    }
}
#[test]
fn no_implicit_configuration_and_scoped_real_construction() {
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
    let widget: CapturedElement<'_, (), iced::Theme, ()> =
        CaptureContext::new(limits(2)).run(|| {
            let child = adapters::text("value", current_limits().unwrap())
                .unwrap()
                .erase();
            adapters::container(child, current_limits().unwrap())
                .unwrap()
                .erase()
        });
    assert_eq!(widget.construction().nodes().len(), 2);
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
    CaptureContext::new(limits(1)).run(|| {
        let child: CapturedElement<'_, (), iced::Theme, ()> =
            adapters::text("", current_limits().unwrap())
                .unwrap()
                .erase();
        assert!(adapters::container(child, current_limits().unwrap()).is_err());
    });
}
#[test]
fn nested_scopes_override_then_restore_without_changing_public_helper_signatures() {
    fn helper() -> usize {
        current_limits().unwrap().nodes.get()
    }
    CaptureContext::new(limits(2)).run(|| {
        assert_eq!(helper(), 2);
        CaptureContext::new(limits(5)).run(|| assert_eq!(helper(), 5));
        assert_eq!(helper(), 2);
    });
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
}
#[test]
fn unwind_restores_outer_scope_and_propagates_original_panic() {
    CaptureContext::new(limits(2)).run(|| {
        let error = std::panic::catch_unwind(|| {
            CaptureContext::new(limits(5)).run(|| std::panic::panic_any(37u32))
        })
        .unwrap_err();
        assert_eq!(*error.downcast::<u32>().unwrap(), 37);
        assert_eq!(current_limits().unwrap().nodes.get(), 2);
    });
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
}
#[test]
fn threads_do_not_inherit_configuration_and_borrowed_values_can_escape() {
    let text = String::from("borrowed");
    let borrowed = CaptureContext::new(limits(2)).run(|| {
        std::thread::spawn(|| {
            assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
            CaptureContext::new(limits(7))
                .run(|| assert_eq!(current_limits().unwrap().nodes.get(), 7));
        })
        .join()
        .unwrap();
        assert_eq!(current_limits().unwrap().nodes.get(), 2);
        text.as_str()
    });
    assert_eq!(borrowed, "borrowed");
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
}

#[test]
fn interleaved_future_polls_restore_the_ambient_context() {
    use std::{
        future::{Future, poll_fn},
        pin::pin,
        task::{Context, Poll, Waker},
    };
    let mut a_count = 0;
    let mut b_count = 0;
    let a = poll_fn(|_| {
        a_count += 1;
        assert_eq!(current_limits().unwrap().nodes.get(), 2);
        if a_count == 1 {
            Poll::Pending
        } else {
            Poll::Ready(17)
        }
    });
    let b = poll_fn(|_| {
        b_count += 1;
        assert_eq!(current_limits().unwrap().nodes.get(), 5);
        Poll::Ready(23)
    });
    let mut a = pin!(a);
    let mut b = pin!(b);
    let mut a = CaptureContext::new(limits(2)).scope_future(a.as_mut());
    let mut b = CaptureContext::new(limits(5)).scope_future(b.as_mut());
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
    CaptureContext::new(limits(9)).run(|| {
        assert_eq!(std::pin::Pin::new(&mut a).poll(&mut cx), Poll::Pending);
        assert_eq!(current_limits().unwrap().nodes.get(), 9);
        assert_eq!(std::pin::Pin::new(&mut b).poll(&mut cx), Poll::Ready(23));
        assert_eq!(current_limits().unwrap().nodes.get(), 9);
    });
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
    assert_eq!(std::pin::Pin::new(&mut a).poll(&mut cx), Poll::Ready(17));
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
}
#[test]
fn future_poll_panic_restores_context_without_replacing_the_panic() {
    use std::{
        future::{Future, poll_fn},
        pin::pin,
        task::{Context, Poll, Waker},
    };
    let future = poll_fn(|_| -> Poll<()> {
        assert_eq!(current_limits().unwrap().nodes.get(), 5);
        std::panic::panic_any(41u32)
    });
    let mut future = pin!(future);
    let mut scoped = CaptureContext::new(limits(5)).scope_future(future.as_mut());
    let mut cx = Context::from_waker(Waker::noop());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        std::pin::Pin::new(&mut scoped).poll(&mut cx)
    }));
    assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 41);
    assert_eq!(current_limits().unwrap_err(), ContextError::Missing);
}
