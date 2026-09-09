use iced::advanced::widget::{Tree, tree::Tag};
use iced_css::host::{
    compatibility::*,
    reconcile::{SlotLimits, prepare_native_slots},
};
use std::num::{NonZeroU32, NonZeroUsize};
fn limits() -> CompatibilityLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CompatibilityLimits {
        entries: n,
        retained_bytes: n,
        work: n,
    }
}
fn schema(tag: Tag) -> NativeCompatibility<'static> {
    NativeCompatibility {
        adapter_id: "test.input",
        schema_version: NonZeroU32::MIN,
        content_tag: tag,
        ordered_roles: b"declared-test-roles-v1",
        representation: "native",
    }
}
fn input() -> Tree {
    let e: iced::Element<'_, (), iced::Theme, ()> = iced::widget::text_input("", "value")
        .on_input(|_| ())
        .into();
    let mut t = Tree::new(&e);
    t.state
        .downcast_mut::<iced::widget::text_input::State<()>>()
        .focus();
    t
}
fn focused(t: &Tree) -> bool {
    t.state
        .downcast_ref::<iced::widget::text_input::State<()>>()
        .is_focused()
}
#[test]
fn all_five_compatibility_fields_gate_native_focus_reuse() {
    for field in 0..6 {
        let mut old = [Some(input())];
        let previous = [schema(old[0].as_ref().unwrap().tag)];
        let mut next = previous[0];
        match field {
            0 => next.adapter_id = "test.other",
            1 => next.schema_version = NonZeroU32::new(2).unwrap(),
            2 => next.content_tag = Tag::stateless(),
            3 => next.ordered_roles = b"different-ordered-roles",
            4 => next.representation = "different",
            _ => (),
        }
        let plan =
            filter_native_compatibility(&old, &previous, &[next], &[Some(0)], limits()).unwrap();
        assert_eq!(
            plan.previous_indices(),
            &[if field == 5 { Some(0) } else { None }]
        );
        assert!(focused(old[0].as_ref().unwrap()));
        let l = limits();
        let moved = prepare_native_slots(
            &mut old,
            plan.previous_indices(),
            SlotLimits {
                entries: l.entries,
                retained_bytes: l.retained_bytes,
                work: l.work,
            },
        )
        .unwrap()
        .apply();
        if field == 5 {
            assert!(old[0].is_none());
            assert!(focused(moved[0].as_ref().unwrap()));
        } else {
            assert!(moved[0].is_none());
            assert!(focused(old[0].as_ref().unwrap()));
        }
    }
}
#[test]
fn malformed_candidates_are_rejected_before_state_changes() {
    let old = [Some(input()), None];
    let s = schema(old[0].as_ref().unwrap().tag);
    for (mapping, incoming, expected) in [
        (
            vec![Some(0), Some(0)],
            vec![s, s],
            CompatibilityError::Duplicate {
                first: 0,
                second: 1,
            },
        ),
        (
            vec![Some(2)],
            vec![s],
            CompatibilityError::OutOfBounds {
                incoming: 0,
                previous: 2,
            },
        ),
        (
            vec![Some(1)],
            vec![s],
            CompatibilityError::Missing {
                incoming: 0,
                previous: 1,
            },
        ),
        (vec![Some(0)], vec![], CompatibilityError::Dimensions),
    ] {
        assert_eq!(
            filter_native_compatibility(&old, &[s, s], &incoming, &mapping, limits()).unwrap_err(),
            expected
        );
        assert!(focused(old[0].as_ref().unwrap()));
    }
    assert_eq!(
        filter_native_compatibility(&old, &[s], &[s], &[Some(0)], limits()).unwrap_err(),
        CompatibilityError::Dimensions
    );
    let incorrect = schema(Tag::stateless());
    assert_eq!(
        filter_native_compatibility(&old, &[incorrect, s], &[s], &[Some(0)], limits()).unwrap_err(),
        CompatibilityError::Tag {
            incoming: 0,
            previous: 0
        }
    );
}
#[test]
fn limits_and_abandoned_plans_never_move_native_state() {
    let old = [Some(input())];
    let s = schema(old[0].as_ref().unwrap().tag);
    for (field, expected) in [
        (0, CompatibilityError::Entries),
        (1, CompatibilityError::Bytes),
        (2, CompatibilityError::Work),
    ] {
        let mut l = limits();
        match field {
            0 => l.entries = NonZeroUsize::MIN,
            1 => l.retained_bytes = NonZeroUsize::MIN,
            _ => l.work = NonZeroUsize::MIN,
        }
        assert_eq!(
            filter_native_compatibility(&old, &[s], &[s], &[Some(0)], l).unwrap_err(),
            expected
        );
        assert!(focused(old[0].as_ref().unwrap()));
    }
    let plan =
        filter_native_compatibility(&old, &[s], &[s, s], &[None, Some(0)], limits()).unwrap();
    assert_eq!(plan.previous_indices(), &[None, Some(0)]);
    assert!(plan.retained_bytes() <= limits().retained_bytes.get());
    drop(plan);
    assert!(focused(old[0].as_ref().unwrap()));
}
#[test]
fn long_declared_roles_are_charged_before_comparison_completes() {
    let old = [Some(Tree::empty())];
    let roles = vec![b'a'; 1000];
    let s = NativeCompatibility {
        ordered_roles: &roles,
        ..schema(Tag::stateless())
    };
    let mut l = limits();
    l.work = NonZeroUsize::new(100).unwrap();
    assert_eq!(
        filter_native_compatibility(&old, &[s], &[s], &[Some(0)], l).unwrap_err(),
        CompatibilityError::Work
    );
    assert!(old[0].is_some());
}

#[test]
fn compatibility_matrix_checks_every_pair_of_five_independent_fields() {
    fn native(alternate: bool) -> Tree {
        if alternate { input() } else { Tree::empty() }
    }
    fn description(mask: u8, tag: Tag) -> NativeCompatibility<'static> {
        NativeCompatibility {
            adapter_id: if mask & 1 == 0 {
                "adapter.a"
            } else {
                "adapter.b"
            },
            schema_version: NonZeroU32::new(if mask & 2 == 0 { 1 } else { 2 }).unwrap(),
            content_tag: tag,
            ordered_roles: if mask & 8 == 0 { b"role-a" } else { b"role-b" },
            representation: if mask & 16 == 0 {
                "native-a"
            } else {
                "native-b"
            },
        }
    }
    for old_mask in 0..32u8 {
        for new_mask in 0..32u8 {
            let old = [Some(native(old_mask & 4 != 0))];
            let new_tag = native(new_mask & 4 != 0).tag;
            let a = description(old_mask, old[0].as_ref().unwrap().tag);
            let b = description(new_mask, new_tag);
            let result =
                filter_native_compatibility(&old, &[a], &[b], &[Some(0)], limits()).unwrap();
            assert_eq!(
                result.previous_indices(),
                &[if old_mask == new_mask { Some(0) } else { None }],
                "{old_mask} -> {new_mask}"
            );
        }
    }
}
