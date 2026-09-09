use iced::{
    Element, Event, Point, Rectangle, Size,
    advanced::{Layout, Shell, layout, widget::Tree},
    mouse,
};
use iced_css::capture::{CaptureError, CaptureLimits, adapters};
use iced_test::core::time::{Duration, Instant};
use std::{cell::Cell, num::NonZeroUsize, rc::Rc};
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
#[test]
fn sensor_retains_source_and_native_content_with_aggregate_limits() {
    let s =
        adapters::sensor::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let n = s.construction().nodes();
    assert_eq!(n.len(), 2);
    assert_eq!(n[1].canonical_type(), "sensor");
    assert_eq!(n[0].parent(), Some(1));
    assert_eq!(n[0].role(), Some("content"));
    let (native, _) = s.erase().into_parts();
    assert_eq!(Tree::new(&native).children.len(), 1);
    for (field, error) in [
        (0, CaptureError::Nodes),
        (1, CaptureError::Depth),
        (2, CaptureError::Bytes),
        (3, CaptureError::Work),
    ] {
        let mut cap = limits();
        match field {
            0 => cap.nodes = NonZeroUsize::MIN,
            1 => cap.depth = NonZeroUsize::MIN,
            2 => cap.retained_bytes = NonZeroUsize::MIN,
            _ => cap.work = NonZeroUsize::MIN,
        }
        assert!(
            matches!(adapters::sensor::<(),iced::Theme,(),_>(adapters::space(limits()).unwrap(),cap),Err(actual) if actual==error)
        );
    }
}
#[test]
fn key_setters_preserve_source_and_clone_only_when_native_state_is_created() {
    struct Key(Rc<Cell<usize>>);
    impl Clone for Key {
        fn clone(&self) -> Self {
            self.0.set(self.0.get() + 1);
            Self(self.0.clone())
        }
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }
    let clones = Rc::new(Cell::new(0));
    let s =
        adapters::sensor::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let ptr = s.construction().nodes().as_ptr();
    let s = s.key(Key(clones.clone()));
    assert_eq!(clones.get(), 0);
    assert_eq!(s.construction().nodes().as_ptr(), ptr);
    let (native, _) = s.erase().into_parts();
    assert_eq!(clones.get(), 0);
    let _ = Tree::new(&native);
    assert_eq!(clones.get(), 1);
    let key = String::from("borrowed");
    let s =
        adapters::sensor::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap();
    let ptr = s.construction().nodes().as_ptr();
    let s = s
        .key(7u32)
        .key_ref(key.as_str())
        .anticipate(2.0)
        .delay(Duration::ZERO);
    assert_eq!(s.construction().nodes().as_ptr(), ptr);
    let (native, _) = s.erase().into_parts();
    assert_eq!(Tree::new(&native).children.len(), 1);
}
#[test]
fn configuration_conversions_are_eager_and_callbacks_remain_stored() {
    struct Pixels<'a>(&'a Cell<usize>);
    impl From<Pixels<'_>> for iced::Pixels {
        fn from(v: Pixels<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self(3.0)
        }
    }
    struct Delay<'a>(&'a Cell<usize>);
    impl From<Delay<'_>> for Duration {
        fn from(v: Delay<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Self::from_millis(10)
        }
    }
    let conversions = Cell::new(0);
    let calls = Cell::new(0);
    let s =
        adapters::sensor::<(), iced::Theme, (), _>(adapters::space(limits()).unwrap(), limits())
            .unwrap()
            .anticipate(Pixels(&conversions))
            .delay(Delay(&conversions))
            .on_show(|_| calls.set(calls.get() + 1))
            .on_resize(|_| calls.set(calls.get() + 1))
            .on_hide(());
    assert_eq!(conversions.get(), 2);
    assert_eq!(calls.get(), 0);
    let (native, _) = s.erase().into_parts();
    let _ = Tree::new(&native);
    assert_eq!(calls.get(), 0);
    assert_eq!(conversions.get(), 2);
}
type Message = (&'static str, Size);
fn redraw(
    e: &mut Element<'_, Message, iced::Theme, ()>,
    state: &mut Tree,
    now: Instant,
    viewport: Rectangle,
) -> (Vec<Message>, iced::window::RedrawRequest) {
    let n = e.as_widget_mut().layout(
        state,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );
    let mut messages = Vec::new();
    let request = {
        let mut shell = Shell::new(&mut messages);
        e.as_widget_mut().update(
            state,
            &Event::Window(iced::window::Event::RedrawRequested(now)),
            Layout::new(&n),
            mouse::Cursor::Unavailable,
            &(),
            &mut iced_test::core::clipboard::Null,
            &mut shell,
            &viewport,
        );
        shell.redraw_request()
    };
    (messages, request)
}
fn keyed(key: &str, size: f32) -> Element<'_, Message, iced::Theme, ()> {
    adapters::sensor(
        adapters::space(limits()).unwrap().width(size).height(size),
        limits(),
    )
    .unwrap()
    .key_ref(key)
    .on_show(|s| ("show", s))
    .on_resize(|s| ("resize", s))
    .on_hide(("hide", Size::ZERO))
    .erase()
    .into_parts()
    .0
}
#[test]
fn borrowed_key_and_size_changes_retrigger_on_explicit_redraws() {
    let a = String::from("a");
    let b = String::from("b");
    let now = Instant::now();
    let viewport = Rectangle::with_size(Size::new(100.0, 100.0));
    let mut e = keyed(&a, 10.0);
    let mut state = Tree::new(&e);
    assert_eq!(
        redraw(&mut e, &mut state, now, viewport).0,
        [("show", Size::new(10.0, 10.0))]
    );
    assert!(redraw(&mut e, &mut state, now, viewport).0.is_empty());
    let mut next = keyed(&a, 20.0);
    state.diff(next.as_widget());
    assert_eq!(
        redraw(&mut next, &mut state, now, viewport).0,
        [("resize", Size::new(20.0, 20.0))]
    );
    let mut next = keyed(&b, 20.0);
    state.diff(next.as_widget());
    assert_eq!(
        redraw(&mut next, &mut state, now, viewport).0,
        [("show", Size::new(20.0, 20.0))]
    );
    let outside = Rectangle::new(Point::new(200.0, 200.0), Size::new(100.0, 100.0));
    assert_eq!(
        redraw(&mut next, &mut state, now, outside).0,
        [("hide", Size::ZERO)]
    );
    assert!(redraw(&mut next, &mut state, now, outside).0.is_empty());
}
#[test]
fn delay_deadlines_use_supplied_redraw_instants_and_hide_message_is_consumed() {
    let start = Instant::now();
    let inside = Rectangle::with_size(Size::new(100.0, 100.0));
    let outside = Rectangle::new(Point::new(200.0, 200.0), Size::new(100.0, 100.0));
    let mut e = adapters::sensor::<Message, iced::Theme, (), _>(
        adapters::space(limits()).unwrap().width(10.0).height(10.0),
        limits(),
    )
    .unwrap()
    .delay(Duration::from_millis(10))
    .on_show(|s| ("show", s))
    .on_hide(("hide", Size::ZERO))
    .erase()
    .into_parts()
    .0;
    let mut state = Tree::new(&e);
    let time = |n| start + Duration::from_millis(n);
    let (m, request) = redraw(&mut e, &mut state, time(0), inside);
    assert!(m.is_empty());
    assert_eq!(request, iced::window::RedrawRequest::At(time(10)));
    assert!(redraw(&mut e, &mut state, time(9), inside).0.is_empty());
    assert_eq!(
        redraw(&mut e, &mut state, time(10), inside).0,
        [("show", Size::new(10.0, 10.0))]
    );
    let (m, request) = redraw(&mut e, &mut state, time(20), outside);
    assert!(m.is_empty());
    assert_eq!(request, iced::window::RedrawRequest::At(time(30)));
    assert!(redraw(&mut e, &mut state, time(29), outside).0.is_empty());
    assert_eq!(
        redraw(&mut e, &mut state, time(30), outside).0,
        [("hide", Size::ZERO)]
    );
    assert!(redraw(&mut e, &mut state, time(40), inside).0.is_empty());
    assert_eq!(
        redraw(&mut e, &mut state, time(50), inside).0,
        [("show", Size::new(10.0, 10.0))]
    );
    assert!(redraw(&mut e, &mut state, time(60), outside).0.is_empty());
    assert!(redraw(&mut e, &mut state, time(70), outside).0.is_empty());
}

#[test]
fn anticipation_boundary_and_resize_only_mode_follow_native_branches() {
    let now = Instant::now();
    let viewport = Rectangle::new(Point::new(20.0, 0.0), Size::new(100.0, 100.0));
    for distance in [9.0, 10.0] {
        let mut e = adapters::sensor::<Message, iced::Theme, (), _>(
            adapters::space(limits()).unwrap().width(10.0).height(10.0),
            limits(),
        )
        .unwrap()
        .anticipate(distance)
        .on_show(|s| ("show", s))
        .erase()
        .into_parts()
        .0;
        let mut state = Tree::new(&e);
        let m = redraw(&mut e, &mut state, now, viewport).0;
        assert_eq!(
            m,
            if distance == 10.0 {
                vec![("show", Size::new(10.0, 10.0))]
            } else {
                vec![]
            }
        );
    }
    let mut e = adapters::sensor::<Message, iced::Theme, (), _>(
        adapters::space(limits()).unwrap().width(10.0).height(10.0),
        limits(),
    )
    .unwrap()
    .on_resize(|s| ("resize", s))
    .erase()
    .into_parts()
    .0;
    let mut state = Tree::new(&e);
    assert_eq!(
        redraw(&mut e, &mut state, now, viewport).0,
        [("resize", Size::new(10.0, 10.0))]
    );
    assert!(redraw(&mut e, &mut state, now, viewport).0.is_empty());
}
