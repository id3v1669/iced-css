use iced_css_core::{
    cascade::{Origin, Specificity, winner_table::*},
    syntax::{DeclarationItem, DeclarationLimits, Declarations, RecoveryLimits, StringLimits},
    values::{
        NumericLimits,
        color::ColorLimits,
        property::{ParsedProperty, PropertyResult, SpecifiedValue, parse_property},
        registry::PropertyId,
    },
};
use std::num::{NonZeroU64, NonZeroUsize};
fn parsed(css: &str) -> ParsedProperty {
    let work = NonZeroUsize::new(1000000).unwrap();
    let item = Declarations::new(
        css,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work,
                retained_bytes: work,
                nesting: work,
            },
            name_bytes: work,
        },
    )
    .next()
    .unwrap()
    .unwrap();
    let DeclarationItem::Declaration(declaration) = item else {
        panic!()
    };
    let PropertyResult::Parsed(value) = parse_property(
        &declaration,
        ColorLimits {
            syntax: StringLimits {
                work,
                retained_bytes: work,
            },
            numeric: NumericLimits { work, digits: work },
        },
    )
    .unwrap() else {
        panic!()
    };
    value
}
fn admission(origin: Origin, order: u64) -> TableAdmission {
    TableAdmission {
        origin,
        specificity: Specificity::ZERO,
        order,
        declaration: NonZeroU64::new(order).unwrap(),
    }
}
#[test]
fn shorthand_expansion_and_longhand_precedence_share_property_slots() {
    let border = parsed("border:1px solid red");
    let left = parsed("border-left-color:blue !important");
    let mut table = PropertyWinners::new();
    assert_eq!(
        table.offer(&border, admission(Origin::Author, 1)).unwrap(),
        12
    );
    assert_eq!(table.offer(&left, admission(Origin::User, 2)).unwrap(), 1);
    let selected = table
        .candidate(PropertyId::lookup("border-left-color").unwrap())
        .unwrap()
        .unwrap();
    assert!(std::ptr::eq(selected.value, &left));
    assert!(
        table
            .candidate(PropertyId::lookup("width").unwrap())
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        table.candidate(border.property),
        Err(TableError::ShorthandQuery)
    ));
}
#[test]
fn a_late_shorthand_conflict_leaves_all_earlier_target_slots_unchanged() {
    let existing = parsed("margin-left:9px");
    let group = parsed("margin:1px 2px 3px 4px");
    let key = admission(Origin::Author, 1);
    let mut table = PropertyWinners::new();
    table.offer(&existing, key).unwrap();
    assert!(matches!(
        table.offer(&group, key),
        Err(TableError::ConflictingOrder)
    ));
    assert!(
        table
            .candidate(PropertyId::lookup("margin-top").unwrap())
            .unwrap()
            .is_none()
    );
    assert!(std::ptr::eq(
        table.candidate(existing.property).unwrap().unwrap().value,
        &existing
    ));
}
#[test]
fn structural_expansion_errors_do_not_mutate_any_slot() {
    let mut group = parsed("margin:1px");
    let SpecifiedValue::Expanded(leaves) = &mut group.value else {
        panic!()
    };
    leaves.pop();
    let mut table = PropertyWinners::new();
    assert!(matches!(
        table.offer(&group, admission(Origin::Author, 1)),
        Err(TableError::InvalidExpansion)
    ));
    for property in group.property.longhands() {
        assert!(table.candidate(*property).unwrap().is_none());
    }
}
#[test]
fn repeated_borrowed_leaf_is_idempotent_but_recreated_equal_objects_conflict() {
    let value = parsed("color:red");
    let recreated = value.clone();
    let key = admission(Origin::Author, 1);
    let mut table = PropertyWinners::new();
    assert_eq!(table.offer(&value, key).unwrap(), 1);
    assert_eq!(table.offer(&value, key).unwrap(), 0);
    assert!(matches!(
        table.offer(&recreated, key),
        Err(TableError::ConflictingOrder)
    ));
    assert!(std::ptr::eq(
        table.candidate(value.property).unwrap().unwrap().value,
        &value
    ));
}

#[test]
fn all_registry_inherit_routes_touch_only_their_expanded_targets() {
    // Every property with a name of its own: custom properties are filed under one
    // identity that no declaration names.
    for property in PropertyId::all().filter(|property| *property != PropertyId::custom()) {
        let declaration = parsed(&format!("{}:inherit", property.descriptor().name));
        let mut table = PropertyWinners::new();
        assert_eq!(
            table
                .offer(&declaration, admission(Origin::Author, 1))
                .unwrap(),
            property.longhands().len()
        );
        for queried in PropertyId::all().filter(|property| !property.descriptor().shorthand) {
            assert_eq!(
                table.candidate(queried).unwrap().is_some(),
                property.longhands().contains(&queried),
                "{} -> {}",
                property.descriptor().name,
                queried.descriptor().name
            );
        }
    }
}
#[test]
fn duplicate_foreign_nested_and_inconsistent_priority_leaves_are_rejected_atomically() {
    for fault in 0..4 {
        let mut declaration = parsed("margin:1px");
        let SpecifiedValue::Expanded(leaves) = &mut declaration.value else {
            panic!()
        };
        match fault {
            0 => leaves[1].property = leaves[0].property,
            1 => leaves[1].property = PropertyId::lookup("width").unwrap(),
            2 => leaves[1].value = SpecifiedValue::Expanded(vec![]),
            _ => leaves[1].important = true,
        }
        let mut table = PropertyWinners::new();
        assert!(matches!(
            table.offer(&declaration, admission(Origin::Author, 1)),
            Err(TableError::InvalidExpansion)
        ));
        for property in declaration.property.longhands() {
            assert!(table.candidate(*property).unwrap().is_none());
        }
    }
}
