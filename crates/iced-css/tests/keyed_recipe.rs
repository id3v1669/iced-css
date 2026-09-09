use iced::{
    Element, Size,
    advanced::{
        layout,
        widget::{Tree, text::State},
    },
    widget::keyed::Column,
};
use iced_css::{
    capture::{CaptureLimits, adapters},
    host::{
        keyed_column_state::prepare_keyed_column_state,
        reconcile::{SlotLimits, prepare_native_slots},
    },
};
use std::num::NonZeroUsize;
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100000).unwrap()
}
fn limits() -> CaptureLimits {
    CaptureLimits {
        nodes: n(),
        depth: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn slots() -> SlotLimits {
    SlotLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn capture(
    keys: Vec<u8>,
    names: &[&'static str],
) -> iced_css::capture::CapturedKeyedColumn<'static, u8, (), iced::Theme, ()> {
    let children = names
        .iter()
        .map(|name| adapters::text(*name, limits()).unwrap().erase())
        .collect();
    adapters::keyed_column_from_vecs(keys, children, limits()).unwrap()
}
fn layout(element: &mut Element<'_, (), iced::Theme, ()>, tree: &mut Tree) {
    element.as_widget_mut().layout(
        tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
    );
}
fn state(tree: &Tree) -> &State<()> {
    tree.state.downcast_ref::<State<()>>()
}
#[test]
fn keyed_recipe_coordinates_parent_key_state_and_fresh_children_before_lowering() {
    let mut old = capture(vec![1, 2], &["A", "B"]).erase().into_parts().0;
    let mut old_tree = Tree::new(&old);
    layout(&mut old, &mut old_tree);
    let a = std::ptr::from_ref(state(&old_tree.children[0]));
    let b = std::ptr::from_ref(state(&old_tree.children[1]));
    let mut previous: Vec<_> = std::mem::take(&mut old_tree.children)
        .into_iter()
        .map(Some)
        .collect();
    let (incoming, _source) = capture(vec![2, 3, 1], &["B", "C", "A"]).into_parts();
    assert_eq!(incoming.native_keys(), [2, 3, 1]);
    let skeleton: Column<'_, u8, (), iced::Theme, ()> =
        Column::from_vecs(incoming.native_keys().to_vec(), vec![]);
    let parent = prepare_keyed_column_state(&skeleton, &old_tree.tag, &mut old_tree.state).unwrap();
    let children = prepare_native_slots(&mut previous, &[Some(1), None, Some(0)], slots())
        .unwrap()
        .with_initialization(incoming.native_children(), slots())
        .unwrap();
    let prepared = children.apply();
    let _retired_parent = parent.commit();
    let mut native: Element<'_, (), iced::Theme, ()> = incoming.into_native().into();
    old_tree.children = prepared.into_iter().map(Option::unwrap).collect();
    assert_eq!(state(&old_tree.children[1]).content(), "");
    native.as_widget().diff(&mut old_tree);
    layout(&mut native, &mut old_tree);
    assert_eq!(std::ptr::from_ref(state(&old_tree.children[0])), b);
    assert_eq!(std::ptr::from_ref(state(&old_tree.children[2])), a);
    assert_eq!(
        old_tree
            .children
            .iter()
            .map(|t| state(t).content())
            .collect::<Vec<_>>(),
        ["B", "C", "A"]
    );
    assert!(previous.iter().all(Option::is_none));
}
