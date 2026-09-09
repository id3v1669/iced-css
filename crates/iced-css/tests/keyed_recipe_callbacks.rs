use iced::{
    Element,
    advanced::widget::{Tree, tree::State},
};
use iced_css::{
    capture::{CaptureLimits, KeyedColumnRecipe, adapters},
    host::{keyed_column_state::prepare_keyed_recipe_state, reconcile::SlotLimits},
};
use std::{
    cell::{Cell, RefCell},
    num::NonZeroUsize,
};
thread_local! {
    static TRACE: RefCell<Vec<(&'static str,u8)>> = const { RefCell::new(Vec::new()) };
    static FAIL: Cell<bool> = const { Cell::new(false) };
}
#[derive(Copy)]
struct Key(u8);
// Instrument native callback timing while preserving the copied value.
#[allow(clippy::non_canonical_clone_impl)]
impl Clone for Key {
    fn clone(&self) -> Self {
        TRACE.with_borrow_mut(|trace| trace.push(("clone", self.0)));
        if FAIL.get() {
            std::panic::panic_any(83u32);
        }
        *self
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        TRACE.with_borrow_mut(|trace| trace.push(("eq", self.0)));
        self.0 == other.0
    }
}
fn trace() -> Vec<(&'static str, u8)> {
    TRACE.with_borrow_mut(std::mem::take)
}
fn limits() -> SlotLimits {
    let n = NonZeroUsize::new(100_000).unwrap();
    SlotLimits {
        entries: n,
        retained_bytes: n,
        work: n,
    }
}
fn recipe(keys: Vec<Key>) -> KeyedColumnRecipe<'static, Key, (), iced::Theme, ()> {
    let n = limits().entries;
    let capture = CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    };
    let children = keys
        .iter()
        .map(|_| adapters::text("child", capture).unwrap().erase())
        .collect();
    adapters::keyed_column_from_vecs(keys, children, capture)
        .unwrap()
        .into_parts()
        .0
}
fn address(state: &State) -> *const () {
    match state {
        State::Some(value) => std::ptr::from_ref(&**value) as *const (),
        State::None => panic!("native key state"),
    }
}
#[test]
fn staging_has_no_key_callbacks_and_native_preparation_matches_stock() {
    use iced::advanced::Widget;
    let old: Element<'_, (), iced::Theme, ()> = recipe(vec![Key(1)]).into_native().into();
    let mut tree = Tree::new(&old);
    let before = address(&tree.state);
    let incoming = recipe(vec![Key(2), Key(3)]);
    trace();
    let denied = SlotLimits {
        entries: NonZeroUsize::MIN,
        ..limits()
    };
    assert!(prepare_keyed_recipe_state(&incoming, &tree.tag, &mut tree.state, denied).is_err());
    assert!(trace().is_empty());
    assert_eq!(address(&tree.state), before);
    let stock = iced::widget::keyed::Column::<Key, (), iced::Theme, ()>::from_vecs(
        vec![Key(2), Key(3)],
        Vec::new(),
    );
    let _stock_state = stock.state();
    let expected = trace();
    assert!(expected.iter().all(|(kind, _)| *kind == "clone"));
    drop(prepare_keyed_recipe_state(&incoming, &tree.tag, &mut tree.state, limits()).unwrap());
    assert_eq!(trace(), expected);
    assert_eq!(address(&tree.state), before);
}
#[test]
fn native_clone_panic_before_commit_preserves_old_parent_state() {
    let old: Element<'_, (), iced::Theme, ()> = recipe(vec![Key(1)]).into_native().into();
    let mut tree = Tree::new(&old);
    let before = address(&tree.state);
    let child = address(&tree.children[0].state);
    let incoming = recipe(vec![Key(2), Key(3)]);
    trace();
    FAIL.set(true);
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drop(prepare_keyed_recipe_state(
            &incoming,
            &tree.tag,
            &mut tree.state,
            limits(),
        ));
    }));
    FAIL.set(false);
    assert_eq!(*failure.unwrap_err().downcast::<u32>().unwrap(), 83);
    assert_eq!(trace(), [("clone", 2)]);
    assert_eq!(address(&tree.state), before);
    assert_eq!(address(&tree.children[0].state), child);
    drop(prepare_keyed_recipe_state(&incoming, &tree.tag, &mut tree.state, limits()).unwrap());
    assert_eq!(address(&tree.state), before);
}
