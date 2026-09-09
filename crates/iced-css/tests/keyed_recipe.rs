use iced::{
    Element, Size,
    advanced::{
        layout,
        widget::{Tree, text::State},
    },
};
use iced_css::{
    capture::{CaptureLimits, adapters},
    host::{
        keyed_column_state::prepare_keyed_recipe_state,
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
    let parent =
        prepare_keyed_recipe_state(&incoming, &old_tree.tag, &mut old_tree.state, slots()).unwrap();
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

#[test]
fn recipe_parent_state_admission_and_abandonment_preserve_active_cache() {
    use iced_css::host::keyed_column_state::{KeyedColumnStateError, KeyedRecipeStateError};
    use iced_css::host::reconcile::SlotError;
    fn address(state: &iced::advanced::widget::tree::State) -> *const () {
        match state {
            iced::advanced::widget::tree::State::Some(state) => {
                std::ptr::from_ref(&**state) as *const ()
            }
            _ => panic!("keyed state"),
        }
    }
    let old = capture(vec![1], &["A"]).erase().into_parts().0;
    let mut tree = Tree::new(&old);
    let old_address = address(&tree.state);
    let (incoming, _) = capture(vec![2, 3, 1], &["B", "C", "A"]).into_parts();
    for (limit, want) in [
        (
            SlotLimits {
                entries: NonZeroUsize::MIN,
                ..slots()
            },
            SlotError::Entries,
        ),
        (
            SlotLimits {
                retained_bytes: NonZeroUsize::MIN,
                ..slots()
            },
            SlotError::Bytes,
        ),
        (
            SlotLimits {
                work: NonZeroUsize::new(4).unwrap(),
                ..slots()
            },
            SlotError::Work,
        ),
    ] {
        assert!(
            matches!(prepare_keyed_recipe_state(&incoming,&tree.tag,&mut tree.state,limit),Err(KeyedRecipeStateError::Slots(error)) if error==want)
        );
        assert_eq!(address(&tree.state), old_address);
    }
    let wrong: Element<'_, (), iced::Theme, ()> =
        iced::widget::keyed::Column::<u16, _, _, _>::new().into();
    let mut wrong_tree = Tree::new(&wrong);
    let wrong_address = address(&wrong_tree.state);
    assert!(matches!(
        prepare_keyed_recipe_state(&incoming, &wrong_tree.tag, &mut wrong_tree.state, slots()),
        Err(KeyedRecipeStateError::Native(KeyedColumnStateError::Tag))
    ));
    assert_eq!(address(&wrong_tree.state), wrong_address);
    let exact = SlotLimits {
        work: NonZeroUsize::new(5).unwrap(),
        ..slots()
    };
    drop(prepare_keyed_recipe_state(&incoming, &tree.tag, &mut tree.state, exact).unwrap());
    assert_eq!(address(&tree.state), old_address);
    let retired = prepare_keyed_recipe_state(&incoming, &tree.tag, &mut tree.state, exact)
        .unwrap()
        .commit();
    assert_eq!(address(&retired), old_address);
    assert_ne!(address(&tree.state), old_address);
}
