use iced::{
    Element, Size,
    advanced::{
        layout,
        widget::{Tree, text::State},
    },
};
use iced_css::{
    capture::{CaptureLimits, CapturedElement, ColumnRecipe, RowRecipe, adapters},
    host::{
        initialize::InitializationError,
        reconcile::{SlotError, SlotLimits, prepare_native_slots},
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
enum Recipe {
    Row(RowRecipe<'static, (), iced::Theme, ()>),
    Column(ColumnRecipe<'static, (), iced::Theme, ()>),
}
impl Recipe {
    fn children(&self) -> &[Element<'static, (), iced::Theme, ()>] {
        match self {
            Self::Row(r) => r.native_children(),
            Self::Column(r) => r.native_children(),
        }
    }
    fn lower(self) -> Element<'static, (), iced::Theme, ()> {
        match self {
            Self::Row(r) => r.into_native().into(),
            Self::Column(r) => r.into_native().into(),
        }
    }
}
fn recipe(column: bool, names: &[&'static str]) -> Recipe {
    let children: Vec<CapturedElement<'static, (), iced::Theme, ()>> = names
        .iter()
        .map(|name| {
            adapters::text(*name, limits())
                .unwrap()
                .source_key(name.as_bytes(), limits())
                .unwrap()
                .erase()
        })
        .collect();
    let (recipe, source) = if column {
        let (r, c) = adapters::column_from_vec(children, limits())
            .unwrap()
            .into_parts();
        (Recipe::Column(r), c)
    } else {
        let (r, c) = adapters::row_from_vec(children, limits())
            .unwrap()
            .into_parts();
        (Recipe::Row(r), c)
    };
    assert_eq!(
        source
            .nodes()
            .iter()
            .filter_map(|n| n.source_key())
            .collect::<Vec<_>>(),
        names.iter().map(|n| n.as_bytes()).collect::<Vec<_>>()
    );
    recipe
}
fn layout(element: &mut Element<'_, (), iced::Theme, ()>, tree: &mut Tree) {
    element.as_widget_mut().layout(
        tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
    );
}
fn paragraph(tree: &Tree) -> &State<()> {
    tree.state.downcast_ref::<State<()>>()
}
#[test]
fn ordered_recipe_children_prepare_state_before_parent_lowering() {
    for column in [false, true] {
        let mut old = recipe(column, &["A", "B"]).lower();
        let mut old_tree = Tree::new(&old);
        layout(&mut old, &mut old_tree);
        assert_eq!(paragraph(&old_tree.children[0]).content(), "A");
        let a = std::ptr::from_ref(paragraph(&old_tree.children[0]));
        let b = std::ptr::from_ref(paragraph(&old_tree.children[1]));
        let mut previous: Vec<_> = old_tree.children.into_iter().map(Some).collect();
        let incoming = recipe(column, &["B", "C", "A"]);
        let pending = prepare_native_slots(&mut previous, &[Some(1), None, Some(0)], slots())
            .unwrap()
            .with_initialization(incoming.children(), slots())
            .unwrap();
        let initialized = pending.apply();
        assert!(previous.iter().all(Option::is_none));
        let children: Vec<_> = initialized.into_iter().map(Option::unwrap).collect();
        assert_eq!(std::ptr::from_ref(paragraph(&children[0])), b);
        assert_eq!(std::ptr::from_ref(paragraph(&children[2])), a);
        assert_eq!(paragraph(&children[0]).content(), "B");
        assert_eq!(paragraph(&children[1]).content(), "");
        let mut native = incoming.lower();
        let mut tree = Tree {
            tag: native.as_widget().tag(),
            state: native.as_widget().state(),
            children,
        };
        native.as_widget().diff(&mut tree);
        layout(&mut native, &mut tree);
        assert_eq!(std::ptr::from_ref(paragraph(&tree.children[0])), b);
        assert_eq!(std::ptr::from_ref(paragraph(&tree.children[2])), a);
        assert_eq!(paragraph(&tree.children[1]).content(), "C");
    }
}
#[test]
fn abandoned_or_rejected_recipe_preparation_keeps_active_states() {
    for column in [false, true] {
        let mut old = recipe(column, &["A"]).lower();
        let mut tree = Tree::new(&old);
        layout(&mut old, &mut tree);
        let address = std::ptr::from_ref(paragraph(&tree.children[0]));
        let mut previous: Vec<_> = tree.children.into_iter().map(Some).collect();
        let incoming = recipe(column, &["B", "A"]);
        let pending = prepare_native_slots(&mut previous, &[None, Some(0)], slots())
            .unwrap()
            .with_initialization(incoming.children(), slots())
            .unwrap();
        drop(pending);
        assert_eq!(
            std::ptr::from_ref(paragraph(previous[0].as_ref().unwrap())),
            address
        );
        assert!(matches!(
            prepare_native_slots(&mut previous, &[None, Some(0)], slots())
                .unwrap()
                .with_initialization(
                    incoming.children(),
                    SlotLimits {
                        retained_bytes: NonZeroUsize::MIN,
                        ..slots()
                    }
                ),
            Err(InitializationError::Slots(SlotError::Bytes))
        ));
        assert_eq!(paragraph(previous[0].as_ref().unwrap()).content(), "A");
        assert_eq!(
            std::ptr::from_ref(paragraph(previous[0].as_ref().unwrap())),
            address
        );
    }
}
