use iced_css_core::cascade::{sheets::*, *};
use std::num::{NonZeroU64, NonZeroUsize};
fn sheet() -> SheetId {
    SheetId::new(NonZeroU64::new(1).unwrap())
}
fn controls() -> AuthorSheets {
    AuthorSheets::new(
        vec![AuthorSheet {
            id: sheet(),
            enabled: true,
            membership: Membership::Persistent,
        }],
        vec![],
        true,
        None,
        SheetLimits {
            entries: NonZeroUsize::new(10).unwrap(),
            retained_bytes: NonZeroUsize::new(4096).unwrap(),
            work: NonZeroUsize::new(1000).unwrap(),
        },
    )
    .unwrap()
}
fn candidate(level: Level, order: u64, value: u8) -> Candidate<u8> {
    Candidate {
        key: CascadeKey {
            level,
            specificity: Specificity::ZERO,
            order,
        },
        declaration: NonZeroU64::new(order + 1).unwrap(),
        value,
    }
}
fn evaluate(controls: &AuthorSheets, user_important: bool) -> u8 {
    let mut winner = Winner::new();
    winner
        .offer(candidate(
            if user_important {
                Level::UserImportant
            } else {
                Level::UserNormal
            },
            0,
            1,
        ))
        .unwrap();
    offer_native_hint(controls, &mut winner, candidate(Level::AuthorNormal, 1, 2)).unwrap();
    offer_author_sheet(
        controls,
        sheet(),
        &mut winner,
        candidate(Level::AuthorImportant, 2, 3),
    )
    .unwrap();
    winner.candidate().unwrap().value
}

#[test]
fn reevaluation_gates_sheet_and_hint_candidates_without_changing_user_precedence() {
    let mut controls = controls();
    assert_eq!(evaluate(&controls, false), 3);
    assert_eq!(evaluate(&controls, true), 1);
    controls.set_sheet_enabled(sheet(), false).unwrap();
    assert_eq!(evaluate(&controls, false), 2);
    controls.set_author_enabled(false).unwrap();
    assert_eq!(evaluate(&controls, false), 1);
    controls.set_author_enabled(true).unwrap();
    assert_eq!(evaluate(&controls, false), 2);
}

#[test]
fn wrong_origin_or_hint_specificity_is_an_error_even_when_excluded() {
    let mut controls = controls();
    controls.set_author_enabled(false).unwrap();
    let mut winner = Winner::new();
    assert!(matches!(
        offer_author_sheet(
            &controls,
            sheet(),
            &mut winner,
            candidate(Level::UserImportant, 0, 1)
        ),
        Err(AdmissionError::WrongOrigin)
    ));
    assert!(matches!(
        offer_native_hint(
            &controls,
            &mut winner,
            candidate(Level::AuthorImportant, 0, 1)
        ),
        Err(AdmissionError::InvalidHint)
    ));
    let mut hint = candidate(Level::AuthorNormal, 0, 1);
    hint.key.specificity = Specificity::selector(0, 1, 0);
    assert!(matches!(
        offer_native_hint(&controls, &mut winner, hint),
        Err(AdmissionError::InvalidHint)
    ));
    assert!(winner.candidate().is_none());
    assert_eq!(
        offer_author_sheet(
            &controls,
            sheet(),
            &mut winner,
            candidate(Level::AuthorNormal, 1, 2)
        )
        .unwrap(),
        AuthorAdmission::Excluded
    );
}

#[test]
fn membership_and_cascade_errors_preserve_the_existing_winner() {
    let controls = controls();
    let mut winner = Winner::new();
    winner.offer(candidate(Level::AuthorNormal, 1, 1)).unwrap();
    assert!(matches!(
        offer_author_sheet(
            &controls,
            sheet(),
            &mut winner,
            candidate(Level::AuthorNormal, 1, 2)
        ),
        Err(AdmissionError::ConflictingOrder)
    ));
    let unknown = SheetId::new(NonZeroU64::new(99).unwrap());
    assert!(matches!(
        offer_author_sheet(
            &controls,
            unknown,
            &mut winner,
            candidate(Level::AuthorImportant, 2, 3)
        ),
        Err(AdmissionError::Membership(SheetError::UnknownSheet))
    ));
    assert_eq!(winner.candidate().unwrap().value, 1);
}
