use iced_css_core::{
    cascade::*,
    syntax::StringLimits,
    values::{
        NumericLimits, color::ColorLimits, initial::InitialReference, property::*,
        registry::PropertyId,
    },
};
use std::num::NonZeroUsize;
fn limits() -> ColorLimits {
    let n = |n| NonZeroUsize::new(n).unwrap();
    ColorLimits {
        syntax: StringLimits {
            retained_bytes: n(4096),
            work: n(100000),
        },
        numeric: NumericLimits {
            digits: n(100),
            work: n(100000),
        },
    }
}
fn id(name: &str) -> PropertyId {
    PropertyId::lookup(name).unwrap()
}
fn winning(name: &str, value: SpecifiedValue) -> Option<ParsedProperty> {
    Some(ParsedProperty {
        property: id(name),
        value,
        important: false,
    })
}
#[test]
fn absent_inherited_values_bind_parent_computed_values_only() {
    assert_eq!(
        default_value(id("font-size"), None, true, limits()).unwrap(),
        DefaultedValue::ParentComputed
    );
    assert!(matches!(
        default_value(id("margin-top"), None, true, limits()).unwrap(),
        DefaultedValue::Compute(_)
    ));
    let DefaultedValue::Compute(SpecifiedValue::TextDecoration(value)) =
        default_value(id("text-decoration"), None, true, limits()).unwrap()
    else {
        panic!()
    };
    assert!(value.is_none());
}
#[test]
fn explicit_inherit_and_root_initial_follow_css_rules() {
    assert_eq!(
        default_value(
            id("width"),
            winning("width", SpecifiedValue::Inherit),
            true,
            limits()
        )
        .unwrap(),
        DefaultedValue::ParentComputed
    );
    assert_eq!(
        default_value(
            id("color"),
            winning("color", SpecifiedValue::Inherit),
            false,
            limits()
        )
        .unwrap(),
        DefaultedValue::Compute(SpecifiedValue::InitialReference(
            InitialReference::UserAgentColor
        ))
    );
    assert_eq!(
        default_value(
            id("color"),
            winning("color", SpecifiedValue::Initial),
            true,
            limits()
        )
        .unwrap(),
        DefaultedValue::Compute(SpecifiedValue::InitialReference(
            InitialReference::UserAgentColor
        ))
    );
}
#[test]
fn defaulting_rejects_wrong_owner_and_shorthand_inputs() {
    assert!(matches!(
        default_value(
            id("width"),
            winning("height", SpecifiedValue::Inherit),
            true,
            limits()
        ),
        Err(DefaultingError::WrongProperty { .. })
    ));
    assert_eq!(
        default_value(id("margin"), None, false, limits()),
        Err(DefaultingError::Shorthand(id("margin")))
    );
    assert_eq!(
        default_value(
            id("width"),
            winning("width", SpecifiedValue::Expanded(vec![])),
            true,
            limits()
        ),
        Err(DefaultingError::UnexpandedShorthand)
    );
}
