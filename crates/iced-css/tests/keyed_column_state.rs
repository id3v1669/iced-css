use iced::{
    Element,
    advanced::{
        Widget,
        widget::{Tree, operation::focusable::Focusable, tree::State},
    },
    widget::{TextInput, keyed},
};
use iced_css::host::keyed_column_state::{KeyedColumnStateError, prepare_keyed_column_state};
type Column = keyed::Column<'static, u8, (), iced::Theme, ()>;
fn input() -> Element<'static, (), iced::Theme, ()> {
    TextInput::new("", "value").on_input(|_| ()).into()
}
fn column(keys: Vec<u8>) -> Column {
    let children = keys.iter().map(|_| input()).collect();
    keyed::Column::from_vecs(keys, children)
}
fn tree_for(column: &Column) -> Tree {
    Tree::new(column as &dyn Widget<(), iced::Theme, ()>)
}
fn diff(tree: &mut Tree, column: &Column) {
    tree.diff(column as &dyn Widget<(), iced::Theme, ()>);
}
fn focus(tree: &mut Tree) {
    Focusable::focus(
        tree.state
            .downcast_mut::<iced::widget::text_input::State<()>>(),
    );
}
fn focused(tree: &Tree) -> bool {
    tree.state
        .downcast_ref::<iced::widget::text_input::State<()>>()
        .is_focused()
}
fn address(state: &State) -> *const () {
    match state {
        State::Some(value) => value.as_ref() as *const dyn std::any::Any as *const (),
        State::None => std::ptr::null(),
    }
}
#[test]
fn uncoordinated_child_insertion_reproduces_the_native_stale_key_failure() {
    let old = column(vec![1]);
    let mut tree = tree_for(&old);
    let fresh = input();
    let fresh_tree = Tree::new(&fresh);
    let incoming = keyed::Column::from_vecs(vec![1u8, 2], vec![input(), fresh]);
    tree.children.push(fresh_tree);
    let failure =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| diff(&mut tree, &incoming)))
            .unwrap_err();
    let message = failure
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(
        message.contains("index out of bounds")
            && message.contains("len is 1")
            && message.contains("index is 1")
    );
    // The deliberately faulted fixture is discarded, not reused by positive cases.
}
#[test]
fn prepared_key_state_supports_complete_child_state_insertion_reorder_and_removal() {
    let old = column(vec![1, 2]);
    let mut tree = tree_for(&old);
    diff(&mut tree, &old);
    focus(&mut tree.children[1]);
    let focus_address = address(&tree.children[1].state);
    let old_parent_address = address(&tree.state);
    let fresh = input();
    let fresh_tree = Tree::new(&fresh);
    let incoming = keyed::Column::from_vecs(vec![3u8, 2, 1], vec![fresh, input(), input()]);
    let prepared = prepare_keyed_column_state(&incoming, &tree.tag, &mut tree.state).unwrap();
    let mut previous = std::mem::take(&mut tree.children);
    tree.children = vec![fresh_tree, previous.remove(1), previous.remove(0)];
    let retired = prepared.commit();
    assert_eq!(address(&retired), old_parent_address);
    assert_ne!(address(&tree.state), old_parent_address);
    diff(&mut tree, &incoming);
    assert!(!focused(&tree.children[0]));
    assert!(focused(&tree.children[1]));
    assert!(!focused(&tree.children[2]));
    assert_eq!(address(&tree.children[1].state), focus_address);
    let incoming = column(vec![2]);
    let prepared = prepare_keyed_column_state(&incoming, &tree.tag, &mut tree.state).unwrap();
    let mut previous = std::mem::take(&mut tree.children);
    tree.children = vec![previous.remove(1)];
    let retired_again = prepared.commit();
    diff(&mut tree, &incoming);
    assert!(focused(&tree.children[0]));
    assert_eq!(address(&tree.children[0].state), focus_address);
    drop((retired, retired_again, previous));
}
#[test]
fn abandoned_preparation_and_wrong_tag_leave_active_state_unchanged() {
    let old = column(vec![1, 2]);
    let mut tree = tree_for(&old);
    focus(&mut tree.children[1]);
    let before = address(&tree.state);
    let child_before = address(&tree.children[1].state);
    let incoming = column(vec![1, 2, 3]);
    drop(prepare_keyed_column_state(&incoming, &tree.tag, &mut tree.state).unwrap());
    assert_eq!(address(&tree.state), before);
    diff(&mut tree, &old);
    assert!(focused(&tree.children[1]));
    assert_eq!(address(&tree.children[1].state), child_before);
    let wrong: keyed::Column<'_, u16, (), iced::Theme, ()> =
        keyed::Column::from_vecs(vec![1u16], vec![input()]);
    assert!(matches!(
        prepare_keyed_column_state(&wrong, &tree.tag, &mut tree.state),
        Err(KeyedColumnStateError::Tag)
    ));
    assert_eq!(address(&tree.state), before);
    assert_eq!(tree.children.len(), 2);
    assert!(focused(&tree.children[1]));
}
