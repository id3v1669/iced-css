use iced::advanced::widget::Tree;
use iced_css::host::{
    compatibility::*,
    reconcile::{SlotError, SlotLimits},
};
use iced_css_core::source::sibling_matches::{MatchLimits, SiblingKey, match_siblings};
use std::num::{NonZeroU32, NonZeroUsize};
fn cap() -> CompatibilityLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CompatibilityLimits {
        entries: n,
        retained_bytes: n,
        work: n,
    }
}
fn slots() -> SlotLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    SlotLimits {
        entries: n,
        retained_bytes: n,
        work: n,
    }
}
fn input(focus: bool) -> Tree {
    let e: iced::Element<'_, (), iced::Theme, ()> = iced::widget::text_input("", "value")
        .on_input(|_| ())
        .into();
    let mut t = Tree::new(&e);
    if focus {
        t.state
            .downcast_mut::<iced::widget::text_input::State<()>>()
            .focus();
    }
    t
}
fn focused(t: &Tree) -> bool {
    t.state
        .downcast_ref::<iced::widget::text_input::State<()>>()
        .is_focused()
}
fn schema(t: &Tree) -> NativeCompatibility<'static> {
    NativeCompatibility {
        adapter_id: "test.input",
        schema_version: NonZeroU32::MIN,
        content_tag: t.tag,
        ordered_roles: b"test-schema-v1",
        representation: "native",
    }
}
#[test]
fn keyed_matching_and_compatibility_prepare_one_exclusive_state_move() {
    let keys = [SiblingKey::Explicit(b"a"), SiblingKey::Explicit(b"b")];
    let next = [keys[1], SiblingKey::Explicit(b"c"), keys[0]];
    let l = cap();
    let matches = match_siblings(
        &keys,
        &next,
        MatchLimits {
            entries: l.entries,
            retained_bytes: l.retained_bytes,
            work: l.work,
        },
    )
    .unwrap();
    let mut previous = [Some(input(false)), Some(input(true))];
    let s = schema(previous[0].as_ref().unwrap());
    let incompatible = NativeCompatibility {
        representation: "new-variant",
        ..s
    };
    let preparation = prepare_compatible_native_slots(
        &mut previous,
        &[s, s],
        &[s, s, incompatible],
        matches.previous_indices(),
        cap(),
        slots(),
    )
    .unwrap();
    assert!(preparation.mapping_retained_bytes() > 0);
    assert!(preparation.mapping_retained_bytes() <= cap().retained_bytes.get());
    assert!(preparation.retained_bytes() <= slots().retained_bytes.get());
    drop(matches);
    let moved = preparation.apply();
    assert!(focused(moved[0].as_ref().unwrap()));
    assert!(moved[1].is_none());
    assert!(moved[2].is_none());
    assert!(previous[0].is_some());
    assert!(previous[1].is_none());
}
#[test]
fn drop_and_second_stage_failure_preserve_focus() {
    let mut previous = [Some(input(true))];
    let s = schema(previous[0].as_ref().unwrap());
    let p = prepare_compatible_native_slots(&mut previous, &[s], &[s], &[Some(0)], cap(), slots())
        .unwrap();
    drop(p);
    assert!(focused(previous[0].as_ref().unwrap()));
    let mut tiny = slots();
    tiny.retained_bytes = NonZeroUsize::MIN;
    assert_eq!(
        prepare_compatible_native_slots(&mut previous, &[s], &[s], &[Some(0)], cap(), tiny)
            .unwrap_err(),
        CompatiblePreparationError::Slots(SlotError::Bytes)
    );
    assert!(focused(previous[0].as_ref().unwrap()));
    let wrong = NativeCompatibility {
        content_tag: iced::advanced::widget::tree::Tag::stateless(),
        ..s
    };
    assert_eq!(
        prepare_compatible_native_slots(&mut previous, &[wrong], &[s], &[Some(0)], cap(), slots())
            .unwrap_err(),
        CompatiblePreparationError::Compatibility(CompatibilityError::Tag {
            incoming: 0,
            previous: 0
        })
    );
    assert!(focused(previous[0].as_ref().unwrap()));
}
