use iced::{
    Element,
    advanced::widget::{Tree, operation::focusable::Focusable},
    widget::{TextInput, pane_grid},
};
use iced_css::host::pane_roles::{PaneRoles, prepare_pane_role_removal};
fn view(state: &pane_grid::State<()>, roles: PaneRoles) -> Element<'_, (), iced::Theme, ()> {
    pane_grid::PaneGrid::new(state, move |_, _, _| {
        let input = || TextInput::new("", "value").on_input(|_| ());
        let content = pane_grid::Content::new(input());
        match roles {
            PaneRoles::NoTitle => content,
            PaneRoles::Title => content.title_bar(pane_grid::TitleBar::new(input())),
            PaneRoles::FullControls => content.title_bar(
                pane_grid::TitleBar::new(input()).controls(pane_grid::Controls::new(input())),
            ),
            PaneRoles::DynamicControls => content.title_bar(
                pane_grid::TitleBar::new(input())
                    .controls(pane_grid::Controls::dynamic(input(), input())),
            ),
        }
    })
    .into()
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
fn full(roles: PaneRoles) -> bool {
    matches!(roles, PaneRoles::FullControls | PaneRoles::DynamicControls)
}
#[test]
fn all_presence_transitions_remove_only_omitted_native_roles() {
    let (state, _) = pane_grid::State::new(());
    let variants = [
        PaneRoles::NoTitle,
        PaneRoles::Title,
        PaneRoles::FullControls,
        PaneRoles::DynamicControls,
    ];
    for previous in variants {
        for incoming in variants {
            let old = view(&state, previous);
            let mut tree = Tree::new(&old);
            tree.diff(&old);
            let content = &mut tree.children[0];
            focus(&mut content.children[0]);
            if previous != PaneRoles::NoTitle {
                focus(&mut content.children[1].children[0]);
                if full(previous) {
                    focus(&mut content.children[1].children[1]);
                }
                if previous == PaneRoles::DynamicControls {
                    focus(&mut content.children[1].children[2]);
                }
            }
            let removed = prepare_pane_role_removal(content, previous, incoming)
                .unwrap()
                .commit();
            let title_removed = previous != PaneRoles::NoTitle && incoming == PaneRoles::NoTitle;
            let full_removed = full(previous) && incoming == PaneRoles::Title;
            let compact_removed = previous == PaneRoles::DynamicControls
                && matches!(incoming, PaneRoles::Title | PaneRoles::FullControls);
            assert_eq!(removed.title.is_some(), title_removed);
            assert_eq!(removed.full.is_some(), full_removed);
            assert_eq!(removed.compact.is_some(), compact_removed);
            if let Some(title) = &removed.title {
                assert!(focused(&title.children[0]));
            }
            if let Some(full) = &removed.full {
                assert!(focused(full));
            }
            if let Some(compact) = &removed.compact {
                assert!(focused(compact));
            }
            let new = view(&state, incoming);
            tree.diff(&new);
            tree.diff(&old);
            let content = &tree.children[0];
            assert!(focused(&content.children[0]));
            if previous != PaneRoles::NoTitle {
                assert_eq!(focused(&content.children[1].children[0]), !title_removed);
                if full(previous) {
                    assert_eq!(
                        focused(&content.children[1].children[1]),
                        !title_removed && !full_removed
                    );
                }
                if previous == PaneRoles::DynamicControls {
                    assert_eq!(
                        focused(&content.children[1].children[2]),
                        !title_removed && !compact_removed
                    );
                }
            }
        }
    }
}
#[test]
fn native_resurrection_remains_if_preparation_is_abandoned() {
    let (state, _) = pane_grid::State::new(());
    let old = view(&state, PaneRoles::DynamicControls);
    let mut tree = Tree::new(&old);
    tree.diff(&old);
    focus(&mut tree.children[0].children[1].children[2]);
    drop(
        prepare_pane_role_removal(
            &mut tree.children[0],
            PaneRoles::DynamicControls,
            PaneRoles::FullControls,
        )
        .unwrap(),
    );
    tree.diff(view(&state, PaneRoles::FullControls));
    tree.diff(&old);
    assert!(focused(&tree.children[0].children[1].children[2]));
}

#[test]
fn shape_errors_preserve_active_native_state() {
    use iced::advanced::widget::tree::Tag;
    use iced_css::host::pane_roles::PaneRoleError;
    let (state, _) = pane_grid::State::new(());
    let old = view(&state, PaneRoles::DynamicControls);
    let mut tree = Tree::new(&old);
    tree.diff(&old);
    let content = &mut tree.children[0];
    focus(&mut content.children[0]);
    focus(&mut content.children[1].children[0]);
    focus(&mut content.children[1].children[1]);
    focus(&mut content.children[1].children[2]);
    for (declared, expected) in [
        (PaneRoles::NoTitle, PaneRoleError::AbsentTitle),
        (PaneRoles::Title, PaneRoleError::AbsentFull),
        (PaneRoles::FullControls, PaneRoleError::AbsentCompact),
    ] {
        assert!(
            matches!(prepare_pane_role_removal(content,declared,PaneRoles::NoTitle),Err(e) if e==expected)
        );
        assert!(focused(&content.children[0]));
        assert!(focused(&content.children[1].children[0]));
        assert!(focused(&content.children[1].children[1]));
        assert!(focused(&content.children[1].children[2]));
    }
    content.tag = Tag::of::<u32>();
    assert!(matches!(
        prepare_pane_role_removal(content, PaneRoles::DynamicControls, PaneRoles::NoTitle),
        Err(PaneRoleError::ContentShape)
    ));
    assert!(focused(&content.children[0]));
    content.tag = Tag::stateless();
    content.children[1].children.push(Tree::empty());
    assert!(matches!(
        prepare_pane_role_removal(content, PaneRoles::DynamicControls, PaneRoles::NoTitle),
        Err(PaneRoleError::TitleShape)
    ));
    assert_eq!(content.children[1].children.len(), 4);
    assert!(focused(&content.children[1].children[2]));
}
#[test]
fn commit_detaches_state_without_running_its_destructor() {
    use iced::advanced::widget::tree::{State, Tag};
    use std::{cell::Cell, rc::Rc};
    struct Probe(Rc<Cell<usize>>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let (state, _) = pane_grid::State::new(());
    let old = view(&state, PaneRoles::FullControls);
    let mut tree = Tree::new(&old);
    tree.diff(&old);
    // A native full-controls payload may own arbitrary application state.
    let full = &mut tree.children[0].children[1].children[1];
    full.tag = Tag::of::<Probe>();
    full.state = State::new(Probe(drops.clone()));
    let prepared = prepare_pane_role_removal(
        &mut tree.children[0],
        PaneRoles::FullControls,
        PaneRoles::Title,
    )
    .unwrap();
    assert_eq!(drops.get(), 0);
    let removed = prepared.commit();
    assert_eq!(drops.get(), 0);
    assert!(removed.full.is_some());
    assert!(removed.title.is_none());
    drop(removed);
    assert_eq!(drops.get(), 1);
    drop(tree);
    assert_eq!(drops.get(), 1);
}

#[test]
fn presence_is_derived_from_direct_captured_roles_with_bounded_work() {
    use iced_css::capture::{CaptureLimits, CapturedPaneContent, adapters};
    use iced_css::host::pane_roles::PaneRoleSourceError;
    use std::num::NonZeroUsize;
    let n = NonZeroUsize::new(10000).unwrap();
    let limits = CaptureLimits {
        nodes: n,
        depth: n,
        work: n,
        retained_bytes: n,
    };
    for expected in [
        PaneRoles::NoTitle,
        PaneRoles::Title,
        PaneRoles::FullControls,
        PaneRoles::DynamicControls,
    ] {
        let space = || adapters::space(limits).unwrap();
        let content: CapturedPaneContent<'_, (), iced::Theme, ()> =
            adapters::pane_content(space(), limits).unwrap();
        let content = match expected {
            PaneRoles::NoTitle => content,
            PaneRoles::Title => content
                .title_bar(adapters::pane_title(space(), limits).unwrap(), limits)
                .unwrap(),
            PaneRoles::FullControls => content
                .title_bar(
                    adapters::pane_title(space(), limits)
                        .unwrap()
                        .controls(adapters::pane_controls(space(), limits).unwrap(), limits)
                        .unwrap(),
                    limits,
                )
                .unwrap(),
            PaneRoles::DynamicControls => content
                .title_bar(
                    adapters::pane_title(space(), limits)
                        .unwrap()
                        .controls(
                            adapters::pane_controls_dynamic(space(), space(), limits).unwrap(),
                            limits,
                        )
                        .unwrap(),
                    limits,
                )
                .unwrap(),
        };
        let c = content.construction();
        assert_eq!(
            PaneRoles::from_construction(c, c.root(), n).unwrap(),
            expected
        );
        assert_eq!(
            PaneRoles::from_construction(c, usize::MAX, n),
            Err(PaneRoleSourceError::Root)
        );
        assert_eq!(
            PaneRoles::from_construction(c, 0, n),
            Err(PaneRoleSourceError::Root)
        );
        assert_eq!(
            PaneRoles::from_construction(c, c.root(), NonZeroUsize::MIN),
            Err(PaneRoleSourceError::Work)
        );
    }
    let (state, _) = pane_grid::State::new(());
    let inner = adapters::pane_grid::<_, (), iced::Theme, ()>(
        &state,
        |_, _, _| {
            adapters::pane_content(adapters::space(limits).unwrap(), limits)
                .unwrap()
                .title_bar(
                    adapters::pane_title(adapters::space(limits).unwrap(), limits)
                        .unwrap()
                        .controls(
                            adapters::pane_controls_dynamic(
                                adapters::space(limits).unwrap(),
                                adapters::space(limits).unwrap(),
                                limits,
                            )
                            .unwrap(),
                            limits,
                        )
                        .unwrap(),
                    limits,
                )
                .unwrap()
        },
        limits,
    )
    .unwrap();
    let outer = adapters::pane_content(inner, limits).unwrap();
    let c = outer.construction();
    assert_eq!(
        PaneRoles::from_construction(c, c.root(), n).unwrap(),
        PaneRoles::NoTitle
    );
    let inner_root = c
        .nodes()
        .iter()
        .position(|node| node.pane_identity().is_some())
        .unwrap();
    assert_eq!(
        PaneRoles::from_construction(c, inner_root, n).unwrap(),
        PaneRoles::DynamicControls
    );
}
