use iced::{
    Element, Length, Rectangle, Size,
    advanced::{
        Layout, Widget, layout, mouse, renderer,
        widget::{
            Tree,
            tree::{State, Tag},
        },
    },
};
use iced_css::host::{
    initialize::{InitializationError, prepare_native_initialization},
    reconcile::{SlotError, SlotLimits},
};
use std::{cell::RefCell, num::NonZeroUsize, rc::Rc};
type Trace = Rc<RefCell<Vec<(u8, &'static str)>>>;
fn limits() -> SlotLimits {
    SlotLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(10000).unwrap(),
    }
}
struct Tracked {
    id: u8,
    trace: Trace,
}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.trace.borrow_mut().push((self.id, "drop"));
    }
}
struct Probe {
    id: u8,
    trace: Trace,
    fail: bool,
}
impl Widget<(), iced::Theme, ()> for Probe {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }
    fn layout(&mut self, _: &mut Tree, _: &(), _: &layout::Limits) -> layout::Node {
        layout::Node::new(Size::ZERO)
    }
    fn draw(
        &self,
        _: &Tree,
        _: &mut (),
        _: &iced::Theme,
        _: &renderer::Style,
        _: Layout<'_>,
        _: mouse::Cursor,
        _: &Rectangle,
    ) {
    }
    fn tag(&self) -> Tag {
        self.trace.borrow_mut().push((self.id, "tag"));
        Tag::of::<Tracked>()
    }
    fn state(&self) -> State {
        self.trace.borrow_mut().push((self.id, "state"));
        assert!(!self.fail, "application native state failure");
        State::new(Tracked {
            id: self.id,
            trace: self.trace.clone(),
        })
    }
    fn children(&self) -> Vec<Tree> {
        self.trace.borrow_mut().push((self.id, "children"));
        vec![Tree::empty()]
    }
}
fn probe(id: u8, trace: &Trace, fail: bool) -> Element<'static, (), iced::Theme, ()> {
    Element::new(Probe {
        id,
        trace: trace.clone(),
        fail,
    })
}
fn pointer(tree: &Tree) -> *const () {
    match &tree.state {
        State::Some(s) => s.as_ref() as *const dyn std::any::Any as *const (),
        State::None => std::ptr::null(),
    }
}
#[test]
fn initializes_only_missing_slots_once_in_native_order_and_commit_drops_nothing() {
    let trace = Trace::default();
    let widgets = vec![
        probe(0, &trace, false),
        probe(1, &trace, false),
        probe(2, &trace, false),
    ];
    let retained = Tree::new(&widgets[1]);
    let before = pointer(&retained);
    let mut slots = vec![None, Some(retained), None];
    trace.borrow_mut().clear();
    let expected_trees = vec![Tree::new(&widgets[0]), Tree::new(&widgets[2])];
    let expected = trace.borrow().clone();
    drop(expected_trees);
    trace.borrow_mut().clear();
    let prepared = prepare_native_initialization(&mut slots, &widgets, limits()).unwrap();
    assert_eq!(*trace.borrow(), expected);
    assert!(prepared.retained_bytes() <= limits().retained_bytes.get());
    prepared.commit();
    assert_eq!(*trace.borrow(), expected);
    assert!(slots.iter().all(Option::is_some));
    assert_eq!(pointer(slots[1].as_ref().unwrap()), before);
    assert!(
        slots
            .iter()
            .all(|s| s.as_ref().unwrap().children.len() == 1)
    );
    trace.borrow_mut().clear();
    prepare_native_initialization(&mut slots, &widgets, limits())
        .unwrap()
        .commit();
    assert!(trace.borrow().is_empty());
    drop(slots);
    assert_eq!(*trace.borrow(), [(0, "drop"), (1, "drop"), (2, "drop")]);
}
#[test]
fn abandonment_and_native_panic_preserve_active_slots() {
    let trace = Trace::default();
    let widgets = vec![
        probe(0, &trace, false),
        probe(1, &trace, false),
        probe(2, &trace, false),
    ];
    let retained = Tree::new(&widgets[1]);
    let before = pointer(&retained);
    let mut slots = vec![None, Some(retained), None];
    trace.borrow_mut().clear();
    drop(prepare_native_initialization(&mut slots, &widgets, limits()).unwrap());
    assert!(slots[0].is_none() && slots[2].is_none());
    assert_eq!(pointer(slots[1].as_ref().unwrap()), before);
    assert_eq!(
        trace
            .borrow()
            .iter()
            .filter(|(_, e)| *e == "drop")
            .copied()
            .collect::<Vec<_>>(),
        [(0, "drop"), (2, "drop")]
    );
    let widgets = vec![
        probe(0, &trace, false),
        probe(1, &trace, false),
        probe(2, &trace, true),
    ];
    trace.borrow_mut().clear();
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = prepare_native_initialization(&mut slots, &widgets, limits());
    }))
    .unwrap_err();
    assert_eq!(
        failure.downcast_ref::<&str>(),
        Some(&"application native state failure")
    );
    assert!(slots[0].is_none() && slots[2].is_none());
    assert_eq!(pointer(slots[1].as_ref().unwrap()), before);
    assert_eq!(
        *trace.borrow(),
        [
            (0, "tag"),
            (0, "state"),
            (0, "children"),
            (2, "tag"),
            (2, "state"),
            (0, "drop")
        ]
    );
}
#[test]
fn host_preflight_rejects_before_native_callbacks_and_accepts_exact_work() {
    let trace = Trace::default();
    let widgets = vec![probe(0, &trace, false), probe(1, &trace, false)];
    let mut slots = vec![None, None];
    assert!(matches!(
        prepare_native_initialization(&mut slots, &widgets[..1], limits()),
        Err(InitializationError::Dimensions)
    ));
    for (l, error) in [
        (
            SlotLimits {
                entries: NonZeroUsize::new(1).unwrap(),
                ..limits()
            },
            SlotError::Entries,
        ),
        (
            SlotLimits {
                retained_bytes: NonZeroUsize::new(1).unwrap(),
                ..limits()
            },
            SlotError::Bytes,
        ),
        (
            SlotLimits {
                work: NonZeroUsize::new(3).unwrap(),
                ..limits()
            },
            SlotError::Work,
        ),
        (
            SlotLimits {
                work: NonZeroUsize::new(7).unwrap(),
                ..limits()
            },
            SlotError::Work,
        ),
    ] {
        assert!(
            matches!(prepare_native_initialization(&mut slots,&widgets,l),Err(InitializationError::Slots(e)) if e==error)
        );
        assert!(trace.borrow().is_empty());
        assert!(slots.iter().all(Option::is_none));
    }
    prepare_native_initialization(
        &mut slots,
        &widgets,
        SlotLimits {
            work: NonZeroUsize::new(8).unwrap(),
            ..limits()
        },
    )
    .unwrap()
    .commit();
    assert!(slots.iter().all(Option::is_some));
    trace.borrow_mut().clear();
    prepare_native_initialization(
        &mut slots,
        &widgets,
        SlotLimits {
            retained_bytes: NonZeroUsize::new(1).unwrap(),
            work: NonZeroUsize::new(4).unwrap(),
            ..limits()
        },
    )
    .unwrap()
    .commit();
    assert!(trace.borrow().is_empty());
    prepare_native_initialization(&mut [], &widgets[..0], limits())
        .unwrap()
        .commit();
    assert!(trace.borrow().is_empty());
}
#[test]
fn fresh_and_retained_input_states_work_with_native_keyed_parent_diff() {
    use iced::widget::{keyed, text_input};
    use iced_css::host::{
        keyed_column_state::prepare_keyed_column_state, reconcile::prepare_native_slots,
    };
    fn input() -> Element<'static, (), iced::Theme, ()> {
        text_input("", "value").on_input(|_| ()).into()
    }
    let old: keyed::Column<'_, u8, (), iced::Theme, ()> =
        keyed::Column::from_vecs(vec![1, 2], vec![input(), input()]);
    let mut tree = Tree::new(&old as &dyn Widget<(), iced::Theme, ()>);
    tree.children[1]
        .state
        .downcast_mut::<text_input::State<()>>()
        .focus();
    let before = pointer(&tree.children[1]);
    let mut previous = std::mem::take(&mut tree.children)
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
    let mut slots = prepare_native_slots(&mut previous, &[None, Some(1), Some(0)], limits())
        .unwrap()
        .apply();
    let widgets = vec![input(), input(), input()];
    prepare_native_initialization(&mut slots, &widgets, limits())
        .unwrap()
        .commit();
    let incoming = keyed::Column::from_vecs(vec![3u8, 2, 1], widgets);
    let prepared = prepare_keyed_column_state(&incoming, &tree.tag, &mut tree.state).unwrap();
    tree.children = slots.into_iter().map(Option::unwrap).collect();
    let retired = prepared.commit();
    tree.diff(&incoming as &dyn Widget<(), iced::Theme, ()>);
    assert_eq!(pointer(&tree.children[1]), before);
    let focused = tree
        .children
        .iter()
        .map(|t| t.state.downcast_ref::<text_input::State<()>>().is_focused())
        .collect::<Vec<_>>();
    assert_eq!(focused, [false, true, false]);
    drop(retired);
    let mut previous = std::mem::take(&mut tree.children)
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
    let mut slots = prepare_native_slots(&mut previous, &[Some(1)], limits())
        .unwrap()
        .apply();
    let widgets = vec![input()];
    prepare_native_initialization(&mut slots, &widgets, limits())
        .unwrap()
        .commit();
    let incoming = keyed::Column::from_vecs(vec![2u8], widgets);
    let prepared = prepare_keyed_column_state(&incoming, &tree.tag, &mut tree.state).unwrap();
    tree.children = slots.into_iter().map(Option::unwrap).collect();
    let retired = prepared.commit();
    tree.diff(&incoming as &dyn Widget<(), iced::Theme, ()>);
    assert_eq!(pointer(&tree.children[0]), before);
    assert!(
        tree.children[0]
            .state
            .downcast_ref::<text_input::State<()>>()
            .is_focused()
    );
    assert!(previous[0].is_some() && previous[1].is_none() && previous[2].is_some());
    drop((retired, previous));
}
