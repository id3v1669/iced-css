use iced_css_core::resources::{entity_tag::*, validation_selection::*};
use std::num::NonZeroUsize;
fn n(v: usize) -> NonZeroUsize {
    NonZeroUsize::new(v).unwrap()
}
fn tag(raw: &[u8]) -> EntityTag<'_> {
    parse_entity_tag_fields(
        &[raw],
        EntityTagLimits {
            bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap()
    .unwrap()
}
fn validators(
    raw: Option<&[u8]>,
    modified: Option<(i64, ValidatorStrength)>,
) -> ResponseValidators<'_> {
    ResponseValidators {
        entity_tag: raw.map(tag),
        last_modified: modified.map(|(utc_seconds, strength)| LastModifiedValidator {
            utc_seconds,
            strength,
        }),
    }
}
fn candidate(
    raw: Option<&[u8]>,
    modified: Option<(i64, ValidatorStrength)>,
    date: Option<i64>,
) -> ValidationCandidate<'_> {
    ValidationCandidate {
        validators: validators(raw, modified),
        date,
    }
}
fn limits() -> ValidationSelectionLimits {
    ValidationSelectionLimits {
        entries: n(20),
        index_bytes: n(10000),
        work: n(10000),
    }
}
#[test]
fn strong_validator_union_has_no_weak_fallback_and_preserves_candidate_order() {
    use ValidatorStrength::*;
    let received = validators(Some(b"\"x\""), Some((10, Strong)));
    let stored = [
        candidate(Some(b"\"x\""), None, None),
        candidate(Some(b"W/\"x\""), Some((10, Weak)), None),
        candidate(Some(b"\"other\""), Some((10, Strong)), None),
        candidate(Some(b"\"x\""), Some((10, Strong)), None),
        candidate(Some(b"\"X\""), None, None),
    ];
    assert_eq!(
        select_304_updates(received, &stored, limits()),
        Ok(ValidationSelection::Strong(vec![0, 2, 3]))
    );
    let received = validators(Some(b"\"new\""), Some((10, Weak)));
    assert_eq!(
        select_304_updates(received, &stored, limits()),
        Ok(ValidationSelection::None)
    );
    assert_eq!(
        select_304_updates(
            validators(Some(b"\"\""), None),
            &[candidate(Some(b"\"\""), None, None)],
            limits()
        ),
        Ok(ValidationSelection::Strong(vec![0]))
    );
}
#[test]
fn weak_validation_uses_admitted_date_ordering_and_never_guesses_missing_dates() {
    use ValidatorStrength::*;
    let received = validators(Some(b"W/\"x\""), Some((10, Weak)));
    let stored = [
        candidate(Some(b"\"x\""), None, Some(1)),
        candidate(Some(b"W/\"x\""), None, Some(2)),
        candidate(Some(b"\"other\""), Some((10, Strong)), Some(3)),
        candidate(None, Some((10, Weak)), Some(3)),
    ];
    assert_eq!(
        select_304_updates(received, &stored, limits()),
        Ok(ValidationSelection::Weak(2))
    );
    let one = [candidate(Some(b"\"x\""), None, None)];
    assert_eq!(
        select_304_updates(received, &one, limits()),
        Ok(ValidationSelection::Weak(0))
    );
    let ambiguous = [one[0], stored[1]];
    assert_eq!(
        select_304_updates(received, &ambiguous, limits()),
        Err(ValidationSelectionError::MissingDate { candidate: 0 })
    );
    let ambiguous = [stored[1], one[0]];
    assert_eq!(
        select_304_updates(received, &ambiguous, limits()),
        Err(ValidationSelectionError::MissingDate { candidate: 1 })
    );
    assert_eq!(
        select_304_updates(
            received,
            &[candidate(Some(b"\"different\""), None, None)],
            limits()
        ),
        Ok(ValidationSelection::None)
    );
}
#[test]
fn validator_free_singleton_and_all_selection_caps_are_explicit() {
    let none = validators(None, None);
    let empty = candidate(None, None, None);
    assert_eq!(
        select_304_updates(none, &[], limits()),
        Ok(ValidationSelection::None)
    );
    assert_eq!(
        select_304_updates(none, &[empty], limits()),
        Ok(ValidationSelection::WithoutValidator(0))
    );
    assert_eq!(
        select_304_updates(none, &[empty, empty], limits()),
        Ok(ValidationSelection::None)
    );
    assert_eq!(
        select_304_updates(
            none,
            &[candidate(None, Some((1, ValidatorStrength::Weak)), None)],
            limits()
        ),
        Ok(ValidationSelection::None)
    );
    let received = validators(Some(b"\"x\""), None);
    let stored = candidate(Some(b"\"x\""), None, None);
    assert_eq!(
        select_304_updates(
            received,
            &[stored, stored],
            ValidationSelectionLimits {
                entries: n(1),
                ..limits()
            }
        ),
        Err(ValidationSelectionError::Entries)
    );
    assert_eq!(
        select_304_updates(
            received,
            &[stored],
            ValidationSelectionLimits {
                index_bytes: n(1),
                ..limits()
            }
        ),
        Err(ValidationSelectionError::Bytes)
    );
    assert_eq!(
        select_304_updates(
            received,
            &[stored],
            ValidationSelectionLimits {
                work: n(1),
                ..limits()
            }
        ),
        Err(ValidationSelectionError::Work)
    );
    assert_eq!(
        select_304_updates(
            validators(Some(b"W/\"x\""), None),
            &[stored],
            ValidationSelectionLimits {
                index_bytes: n(1),
                ..limits()
            }
        ),
        Ok(ValidationSelection::Weak(0))
    );
}

#[test]
fn output_and_tag_comparison_budgets_fail_without_partial_selection() {
    let received = validators(Some(b"\"x\""), None);
    let stored = candidate(Some(b"\"x\""), None, None);
    let one_index = ValidationSelectionLimits {
        index_bytes: n(size_of::<usize>()),
        ..limits()
    };
    assert_eq!(
        select_304_updates(received, &[stored], one_index),
        Ok(ValidationSelection::Strong(vec![0]))
    );
    assert_eq!(
        select_304_updates(received, &[stored, stored], one_index),
        Err(ValidationSelectionError::Bytes)
    );
    assert_eq!(
        select_304_updates(
            received,
            &[candidate(Some(b"\"y\""), None, None)],
            ValidationSelectionLimits {
                index_bytes: n(1),
                ..limits()
            }
        ),
        Ok(ValidationSelection::None)
    );
    let long = format!("\"{}\"", "a".repeat(100));
    assert_eq!(
        select_304_updates(
            validators(Some(long.as_bytes()), None),
            &[candidate(Some(long.as_bytes()), None, None)],
            ValidationSelectionLimits {
                work: n(20),
                ..limits()
            }
        ),
        Err(ValidationSelectionError::Work)
    );
    assert_eq!(
        select_304_updates(
            validators(Some(b"W/\"x\""), None),
            &[
                candidate(Some(b"\"x\""), None, Some(i64::MIN)),
                candidate(Some(b"\"x\""), None, Some(i64::MAX))
            ],
            limits()
        ),
        Ok(ValidationSelection::Weak(1))
    );
}
