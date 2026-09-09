use iced_css_core::cascade::*;
#[test]
fn css22_five_levels_follow_origin_and_importance() {
    assert_eq!(Origin::UserAgent.level(false), Level::UserAgent);
    assert_eq!(Origin::UserAgent.level(true), Level::UserAgent);
    let levels = [
        Origin::UserAgent.level(false),
        Origin::User.level(false),
        Origin::Author.level(false),
        Origin::Author.level(true),
        Origin::User.level(true),
    ];
    assert!(levels.windows(2).all(|pair| pair[0] < pair[1]));
}
#[test]
fn specificity_is_lexicographic_without_small_radix_carry() {
    assert_eq!(
        Specificity::selector(0, 1, 2)
            .checked_selector_sum(Specificity::selector(1, 2, 3))
            .unwrap()
            .components(),
        [0, 1, 3, 5]
    );
    assert!(Specificity::selector(1, 0, 0) > Specificity::selector(0, usize::MAX, usize::MAX));
    assert!(
        Specificity::STYLE_ATTRIBUTE > Specificity::selector(usize::MAX, usize::MAX, usize::MAX)
    );
    assert!(Specificity::selector(0, 1, 0) > Specificity::selector(0, 0, usize::MAX));
}
#[test]
fn selector_count_overflow_and_inline_mixing_are_explicit_errors() {
    assert!(
        Specificity::selector(usize::MAX, 0, 0)
            .checked_selector_sum(Specificity::selector(1, 0, 0))
            .is_err()
    );
    assert!(
        Specificity::STYLE_ATTRIBUTE
            .checked_selector_sum(Specificity::ZERO)
            .is_err()
    );
    assert!(
        Specificity::ZERO
            .checked_selector_sum(Specificity::STYLE_ATTRIBUTE)
            .is_err()
    );
}
#[test]
fn origin_precedes_specificity_and_source_order_breaks_remaining_ties() {
    let strong_author = CascadeKey {
        level: Origin::Author.level(true),
        specificity: Specificity::STYLE_ATTRIBUTE,
        order: u64::MAX,
    };
    let weak_user = CascadeKey {
        level: Origin::User.level(true),
        specificity: Specificity::ZERO,
        order: 0,
    };
    assert!(weak_user > strong_author);
    let earlier = CascadeKey {
        level: Level::AuthorNormal,
        specificity: Specificity::selector(0, 1, 0),
        order: 12,
    };
    let later = CascadeKey {
        order: 13,
        ..earlier
    };
    assert!(later > earlier);
    let specific = CascadeKey {
        specificity: Specificity::selector(1, 0, 0),
        order: 0,
        ..earlier
    };
    assert!(specific > later);
}

fn candidate(level: Level, order: u64, id: u64, value: &str) -> Candidate<String> {
    Candidate {
        key: CascadeKey {
            level,
            specificity: Specificity::ZERO,
            order,
        },
        declaration: std::num::NonZeroU64::new(id).unwrap(),
        value: value.into(),
    }
}
#[test]
fn winner_slot_keeps_only_the_highest_applicable_candidate() {
    let mut winner = Winner::new();
    assert_eq!(
        winner
            .offer(candidate(Level::AuthorImportant, 10, 1, "red"))
            .unwrap(),
        WinnerAdmission::Replaced
    );
    assert_eq!(
        winner
            .offer(candidate(Level::UserImportant, 0, 2, "blue"))
            .unwrap(),
        WinnerAdmission::Replaced
    );
    assert_eq!(
        winner
            .offer(candidate(Level::AuthorNormal, 20, 3, "green"))
            .unwrap(),
        WinnerAdmission::Retained
    );
    assert_eq!(winner.candidate().unwrap().value, "blue");
}
#[test]
fn duplicate_identity_is_idempotent_but_conflicting_order_is_atomic_error() {
    let mut winner = Winner::new();
    let first = candidate(Level::AuthorNormal, 1, 1, "red");
    winner.offer(first.clone()).unwrap();
    assert_eq!(winner.offer(first).unwrap(), WinnerAdmission::Retained);
    assert!(
        winner
            .offer(candidate(Level::AuthorNormal, 1, 2, "blue"))
            .is_err()
    );
    assert_eq!(winner.candidate().unwrap().value, "red");
    assert!(
        winner
            .offer(candidate(Level::AuthorNormal, 1, 1, "blue"))
            .is_err()
    );
    assert_eq!(winner.candidate().unwrap().value, "red");
}
