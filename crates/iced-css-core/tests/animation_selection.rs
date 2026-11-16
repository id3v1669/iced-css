use iced_css_core::{
    animation::registry::*,
    values::registry::{AnimationRule, PropertyId, Scope},
};
use std::num::NonZeroUsize;
fn id(name: &str) -> PropertyId {
    PropertyId::lookup(name).unwrap()
}
fn work() -> NonZeroUsize {
    NonZeroUsize::new(10000).unwrap()
}
#[test]
fn nonanimatable_display_rejects_the_entire_selection() {
    assert_eq!(
        AnimationSelection::new(&[id("color"), id("display")], work()).unwrap_err(),
        SelectionError::NotAnimatable(id("display"))
    );
    assert_eq!(
        AnimationSelection::new(&[], work()).unwrap_err(),
        SelectionError::Empty
    );
}
#[test]
fn shorthands_expand_once_and_keep_each_constituents_rule() {
    let selected = AnimationSelection::new(
        &[
            id("border"),
            id("font"),
            id("border-left-color"),
            id("font"),
        ],
        work(),
    )
    .unwrap();
    assert_eq!(selected.len(), 18);
    assert_eq!(
        selected.rule(id("font-weight")),
        Some(AnimationRule::FontWeight)
    );
    assert_eq!(
        selected.rule(id("font-style")),
        Some(AnimationRule::FontStyle)
    );
    assert_eq!(
        selected.rule(id("font-family")),
        Some(AnimationRule::Discrete)
    );
    assert_eq!(
        selected.rule(id("border-left-color")),
        Some(AnimationRule::ByComputedValue)
    );
    assert_eq!(selected.rule(id("border")), None);
    assert_eq!(selected.properties().count(), 18);
}
#[test]
fn every_registry_property_has_explicit_eligibility_and_work_is_bounded() {
    let mut screen = 0;
    let mut paged = 0;
    for property in PropertyId::all() {
        let result = AnimationSelection::new(&[property], work());
        if property.descriptor().scope == Scope::Paged {
            paged += usize::from(property.is_css22());
            assert_eq!(result.unwrap_err(), SelectionError::Undefined(property));
        } else {
            screen += usize::from(property.is_css22());
            // CSS Transitions 1: the transition properties do not animate, like `display`.
            let not_animatable = property.descriptor().name == "display"
                || property.descriptor().name.starts_with("transition");
            if not_animatable {
                // A shorthand names its first longhand.
                assert_eq!(
                    result.unwrap_err(),
                    SelectionError::NotAnimatable(property.longhands()[0])
                );
            } else {
                let selected = result.unwrap();
                let actual: Vec<_> = selected.properties().collect();
                for leaf in property.longhands() {
                    assert!(actual.contains(leaf));
                }
                assert_eq!(actual.len(), property.longhands().len());
            }
        }
    }
    // CSS 2.2's own; properties of later modules are checked the same way above.
    assert_eq!((screen, paged), (90, 5));
    assert_eq!(
        AnimationSelection::new(&[id("border")], NonZeroUsize::new(1).unwrap()).unwrap_err(),
        SelectionError::Work
    );
}
