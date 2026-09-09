use iced::advanced::widget::{
    Tree,
    tree::{State, Tag},
};
use iced_css::host::reconcile::*;
use iced_css_core::source::sibling_matches::{MatchLimits, SiblingKey, match_siblings};
use std::{cell::Cell, num::NonZeroUsize, rc::Rc};
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> SlotLimits {
    SlotLimits {
        entries: nz(100),
        retained_bytes: nz(10000),
        work: nz(10000),
    }
}
fn input(focused: bool) -> Tree {
    let widget: iced::Element<'_, (), iced::Theme, ()> = iced::widget::text_input("", "value")
        .on_input(|_| ())
        .into();
    let mut tree = Tree::new(&widget);
    if focused {
        tree.state
            .downcast_mut::<iced::widget::text_input::State<()>>()
            .focus();
    }
    tree
}
fn focused(tree: &Tree) -> bool {
    tree.state
        .downcast_ref::<iced::widget::text_input::State<()>>()
        .is_focused()
}
#[test]
fn keyed_plan_moves_complete_native_state_and_preserves_focus() {
    let keys = [SiblingKey::Explicit(b"a"), SiblingKey::Explicit(b"b")];
    let next = [keys[1], SiblingKey::Explicit(b"c"), keys[0]];
    let matches = match_siblings(
        &keys,
        &next,
        MatchLimits {
            entries: nz(100),
            retained_bytes: nz(10000),
            work: nz(10000),
        },
    )
    .unwrap();
    let mut old = [Some(input(false)), Some(input(true))];
    let mut moved = prepare_native_slots(&mut old, matches.previous_indices(), limits())
        .unwrap()
        .apply();
    let updated: iced::Element<'_, (), iced::Theme, ()> =
        iced::widget::text_input("", "updated value")
            .on_input(|_| ())
            .into();
    moved[0].as_mut().unwrap().diff(&updated);
    assert!(focused(moved[0].as_ref().unwrap()));
    assert!(moved[1].is_none());
    assert!(!focused(moved[2].as_ref().unwrap()));
    assert!(old.iter().all(Option::is_none));
}
#[test]
fn invalid_or_abandoned_preflight_leaves_native_state_intact() {
    let mut old = [Some(input(true)), None];
    for (mapping, expected) in [
        (
            vec![Some(0), Some(0)],
            SlotError::Duplicate {
                first: 0,
                second: 1,
            },
        ),
        (
            vec![Some(0), Some(2)],
            SlotError::OutOfBounds {
                incoming: 1,
                previous: 2,
            },
        ),
        (
            vec![Some(0), Some(1)],
            SlotError::Missing {
                incoming: 1,
                previous: 1,
            },
        ),
    ] {
        assert_eq!(
            prepare_native_slots(&mut old, &mapping, limits()).unwrap_err(),
            expected
        );
        assert!(focused(old[0].as_ref().unwrap()));
    }
    let before = old[0]
        .as_ref()
        .unwrap()
        .state
        .downcast_ref::<iced::widget::text_input::State<()>>() as *const _;
    drop(prepare_native_slots(&mut old, &[Some(0)], limits()).unwrap());
    let after = old[0]
        .as_ref()
        .unwrap()
        .state
        .downcast_ref::<iced::widget::text_input::State<()>>() as *const _;
    assert_eq!(before, after);
    for (field, expected) in [
        (0, SlotError::Entries),
        (1, SlotError::Bytes),
        (2, SlotError::Work),
    ] {
        let mut tiny = limits();
        match field {
            0 => tiny.entries = nz(1),
            1 => tiny.retained_bytes = nz(1),
            _ => tiny.work = nz(1),
        }
        assert_eq!(
            prepare_native_slots(&mut old, &[Some(0)], tiny).unwrap_err(),
            expected
        );
        assert!(focused(old[0].as_ref().unwrap()));
    }
}
#[derive(Debug)]
struct Dropped(Rc<Cell<usize>>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
#[test]
fn transport_moves_descendants_without_running_or_dropping_native_payloads() {
    let drops = Rc::new(Cell::new(0));
    let child = Tree {
        tag: Tag::of::<Dropped>(),
        state: State::new(Dropped(drops.clone())),
        children: vec![],
    };
    let mut old = [
        Some(Tree {
            tag: Tag::stateless(),
            state: State::None,
            children: vec![child],
        }),
        Some(Tree::empty()),
    ];
    let moved = prepare_native_slots(&mut old, &[Some(0)], limits())
        .unwrap()
        .apply();
    assert_eq!(drops.get(), 0);
    assert!(old[0].is_none());
    assert!(old[1].is_some());
    assert_eq!(moved[0].as_ref().unwrap().children.len(), 1);
    drop(moved);
    assert_eq!(drops.get(), 1);
}
