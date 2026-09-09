use iced_css_core::cascade::sheets::*;
use std::num::{NonZeroU64, NonZeroUsize};

fn sheet(id: u64) -> SheetId {
    SheetId::new(NonZeroU64::new(id).unwrap())
}
fn set(id: u64) -> SetId {
    SetId::new(NonZeroU64::new(id).unwrap())
}
fn limits() -> SheetLimits {
    SheetLimits {
        entries: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(10000).unwrap(),
        work: NonZeroUsize::new(10000).unwrap(),
    }
}
fn registrations() -> Vec<AuthorSheet> {
    vec![
        AuthorSheet {
            id: sheet(1),
            enabled: true,
            membership: Membership::Persistent,
        },
        AuthorSheet {
            id: sheet(2),
            enabled: true,
            membership: Membership::Alternate(vec![set(1)]),
        },
        AuthorSheet {
            id: sheet(3),
            enabled: true,
            membership: Membership::Alternate(vec![set(1), set(2)]),
        },
    ]
}

#[test]
fn persistent_and_selected_alternates_follow_explicit_controls() {
    let mut sheets = AuthorSheets::new(
        registrations(),
        vec![set(1), set(2)],
        true,
        Some(set(1)),
        limits(),
    )
    .unwrap();
    for id in 1..=3 {
        assert!(sheets.applies(sheet(id)).unwrap());
    }
    assert!(sheets.select_set(Some(set(2))).unwrap());
    assert!(sheets.applies(sheet(1)).unwrap());
    assert!(!sheets.applies(sheet(2)).unwrap());
    assert!(sheets.applies(sheet(3)).unwrap());
    assert!(!sheets.select_set(Some(set(2))).unwrap());
    assert!(sheets.set_sheet_enabled(sheet(3), false).unwrap());
    assert!(!sheets.applies(sheet(3)).unwrap());
    assert!(sheets.select_set(Some(set(1))).unwrap());
    assert!(!sheets.applies(sheet(3)).unwrap());
    assert_eq!(
        sheets.sheet_ids().collect::<Vec<_>>(),
        vec![sheet(1), sheet(2), sheet(3)]
    );
}

#[test]
fn global_disabling_gates_native_hints_and_retains_individual_settings() {
    let mut sheets = AuthorSheets::new(
        registrations(),
        vec![set(1), set(2)],
        true,
        Some(set(1)),
        limits(),
    )
    .unwrap();
    assert!(sheets.native_hints_apply());
    assert!(sheets.set_author_enabled(false).unwrap());
    assert!(!sheets.native_hints_apply());
    for id in 1..=3 {
        assert!(!sheets.applies(sheet(id)).unwrap());
    }
    assert!(!sheets.set_sheet_enabled(sheet(2), false).unwrap());
    assert!(!sheets.select_set(Some(set(2))).unwrap());
    assert!(sheets.set_author_enabled(true).unwrap());
    assert!(sheets.native_hints_apply());
    assert!(!sheets.applies(sheet(2)).unwrap());
    assert!(sheets.applies(sheet(3)).unwrap());
    assert!(sheets.select_set(None).unwrap());
    assert!(!sheets.applies(sheet(3)).unwrap());
    assert!(sheets.applies(sheet(1)).unwrap());
}

#[test]
fn invalid_registration_or_control_request_never_changes_active_state() {
    let mut duplicate = registrations();
    duplicate[1].id = sheet(1);
    assert!(AuthorSheets::new(duplicate, vec![set(1), set(2)], true, None, limits()).is_err());
    assert!(AuthorSheets::new(registrations(), vec![set(1)], true, None, limits()).is_err());
    assert!(
        AuthorSheets::new(
            registrations(),
            vec![set(1), set(1), set(2)],
            true,
            None,
            limits()
        )
        .is_err()
    );
    assert!(
        AuthorSheets::new(
            registrations(),
            vec![set(1), set(2)],
            true,
            Some(set(3)),
            limits()
        )
        .is_err()
    );
    let mut empty = registrations();
    empty[1].membership = Membership::Alternate(vec![]);
    assert!(AuthorSheets::new(empty, vec![set(1), set(2)], true, None, limits()).is_err());
    let mut sheets = AuthorSheets::new(
        registrations(),
        vec![set(1), set(2)],
        true,
        Some(set(1)),
        limits(),
    )
    .unwrap();
    assert!(sheets.select_set(Some(set(99))).is_err());
    assert!(sheets.set_sheet_enabled(sheet(99), false).is_err());
    for id in 1..=3 {
        assert!(sheets.applies(sheet(id)).unwrap());
    }
}

#[test]
fn descriptor_capacities_entries_and_work_are_explicitly_bounded() {
    let mut small = limits();
    small.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        AuthorSheets::new(registrations(), vec![set(1), set(2)], true, None, small),
        Err(SheetError::Bytes)
    ));
    small = limits();
    small.entries = NonZeroUsize::MIN;
    assert!(matches!(
        AuthorSheets::new(registrations(), vec![set(1), set(2)], true, None, small),
        Err(SheetError::Entries)
    ));
    small = limits();
    small.work = NonZeroUsize::MIN;
    assert!(matches!(
        AuthorSheets::new(registrations(), vec![set(1), set(2)], true, None, small),
        Err(SheetError::Work)
    ));
}

#[test]
fn failed_control_work_is_atomic_and_unused_capacity_is_still_charged() {
    let mut small = limits();
    small.work = NonZeroUsize::new(2).unwrap();
    let mut sheets = AuthorSheets::new(
        vec![AuthorSheet {
            id: sheet(1),
            enabled: true,
            membership: Membership::Persistent,
        }],
        vec![set(1)],
        true,
        None,
        small,
    )
    .unwrap();
    assert_eq!(
        sheets.set_sheet_enabled(sheet(1), false),
        Err(SheetError::Work)
    );
    assert!(sheets.sheet_enabled(sheet(1)).unwrap());
    assert_eq!(sheets.select_set(Some(set(1))), Err(SheetError::Work));
    assert_eq!(sheets.selected_set(), None);
    let reserved = Vec::with_capacity(2000);
    assert!(matches!(
        AuthorSheets::new(vec![], reserved, true, None, limits()),
        Err(SheetError::Bytes)
    ));
    let mut duplicate = registrations();
    duplicate[1].membership = Membership::Alternate(vec![set(1), set(1)]);
    assert!(matches!(
        AuthorSheets::new(duplicate, vec![set(1), set(2)], true, None, limits()),
        Err(SheetError::DuplicateMembership)
    ));
}
