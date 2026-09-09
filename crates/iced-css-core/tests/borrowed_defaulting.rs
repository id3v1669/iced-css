use iced_css_core::{
    cascade::{winner_table::*, *},
    values::{
        property::{ParsedProperty, SpecifiedValue},
        registry::{Inheritance, PropertyId},
    },
};
use std::num::NonZeroU64;
fn id(name: &str) -> PropertyId {
    PropertyId::lookup(name).unwrap()
}
fn declaration(property: PropertyId, value: SpecifiedValue) -> ParsedProperty {
    ParsedProperty {
        property,
        value,
        important: false,
    }
}
fn admission() -> TableAdmission {
    TableAdmission {
        origin: Origin::Author,
        specificity: Specificity::ZERO,
        order: 1,
        declaration: NonZeroU64::new(1).unwrap(),
    }
}
#[test]
fn every_longhand_defaults_without_allocating_an_initial_value() {
    let table = PropertyWinners::new();
    let mut count = 0;
    for property in PropertyId::all() {
        if property.descriptor().shorthand {
            assert_eq!(
                table.defaulted(property, true),
                Err(DefaultingError::Shorthand(property))
            );
            continue;
        }
        count += 1;
        assert_eq!(
            table.defaulted(property, false).unwrap(),
            DefaultedValueRef::Initial(property)
        );
        let expected = if property.descriptor().inheritance == Inheritance::Yes {
            DefaultedValueRef::ParentComputed
        } else {
            DefaultedValueRef::Initial(property)
        };
        assert_eq!(table.defaulted(property, true).unwrap(), expected);
        let inherit = declaration(property, SpecifiedValue::Inherit);
        assert_eq!(
            default_value_ref(property, Some(&inherit), true).unwrap(),
            DefaultedValueRef::ParentComputed
        );
        assert_eq!(
            default_value_ref(property, Some(&inherit), false).unwrap(),
            DefaultedValueRef::Initial(property)
        );
    }
    assert_eq!(count, 81);
}
#[test]
fn selected_values_borrow_source_storage_after_table_is_dropped() {
    let property = id("border-left-color");
    let parsed = declaration(
        property,
        SpecifiedValue::InitialReference(
            iced_css_core::values::initial::InitialReference::CurrentColor,
        ),
    );
    let selected = {
        let mut table = PropertyWinners::new();
        table.offer(&parsed, admission()).unwrap();
        table.defaulted(property, true).unwrap()
    };
    let DefaultedValueRef::Compute(value) = selected else {
        panic!()
    };
    assert!(std::ptr::eq(value, &parsed.value));
}
#[test]
fn shorthand_resets_and_invalid_inputs_keep_their_meaning() {
    let property = id("margin");
    let parsed = declaration(
        property,
        SpecifiedValue::Expanded(
            property
                .longhands()
                .iter()
                .map(|p| declaration(*p, SpecifiedValue::Initial))
                .collect(),
        ),
    );
    let mut table = PropertyWinners::new();
    table.offer(&parsed, admission()).unwrap();
    for leaf in property.longhands() {
        assert_eq!(
            table.defaulted(*leaf, true).unwrap(),
            DefaultedValueRef::Initial(*leaf)
        );
    }
    let wrong = declaration(id("height"), SpecifiedValue::Inherit);
    assert_eq!(
        default_value_ref(id("width"), Some(&wrong), true),
        Err(DefaultingError::WrongProperty {
            expected: id("width"),
            actual: id("height")
        })
    );
    let expanded = declaration(id("width"), SpecifiedValue::Expanded(vec![]));
    assert_eq!(
        default_value_ref(id("width"), Some(&expanded), true),
        Err(DefaultingError::UnexpandedShorthand)
    );
}
