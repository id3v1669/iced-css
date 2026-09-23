use iced_css_core::formatting::{
    clearance::{
        ClearSide, ClearanceFloatError, ClearancePresenceError, ClearanceRequirement,
        ClearanceState, classify_clearance, lowest_clearance_float_bottom, solve_clearance,
    },
    floats::{FloatSide, PlacedFloat},
};
use std::num::NonZeroUsize;

#[test]
fn strict_cases_and_no_constraint_are_classified() {
    assert_eq!(
        classify_clearance(4.0, Some(6.0)),
        Ok(ClearanceRequirement::Required)
    );
    assert_eq!(
        classify_clearance(8.0, Some(6.0)),
        Ok(ClearanceRequirement::Absent)
    );
    assert_eq!(
        classify_clearance(4.0, None),
        Ok(ClearanceRequirement::Absent)
    );
}

#[test]
fn equality_is_unsupported_including_both_signed_zeros() {
    for value in [-f32::MAX, -6.0, -f32::from_bits(1), 6.0, f32::MAX] {
        assert_eq!(
            classify_clearance(value, Some(value)),
            Err(ClearancePresenceError::UnsupportedEquality)
        );
    }
    for h in [0.0, -0.0] {
        for f in [0.0, -0.0] {
            assert_eq!(
                classify_clearance(h, Some(f)),
                Err(ClearancePresenceError::UnsupportedEquality)
            );
        }
    }
}

#[test]
fn strict_comparisons_have_no_overflow_or_tolerance() {
    for (low, high) in [
        (-f32::MAX, f32::MAX),
        (1.0, f32::from_bits(1.0_f32.to_bits() + 1)),
        (0.0, f32::from_bits(1)),
        (-f32::from_bits(1), 0.0),
    ] {
        assert_eq!(
            classify_clearance(low, Some(high)),
            Ok(ClearanceRequirement::Required)
        );
        assert_eq!(
            classify_clearance(high, Some(low)),
            Ok(ClearanceRequirement::Absent)
        );
    }
}

#[test]
fn input_errors_are_distinct_and_hypothetical_validation_comes_first() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for bottom in [None, Some(6.0), Some(bad)] {
            assert_eq!(
                classify_clearance(bad, bottom),
                Err(ClearancePresenceError::InvalidInput("hypothetical_top"))
            );
        }
        assert_eq!(
            classify_clearance(4.0, Some(bad)),
            Err(ClearancePresenceError::InvalidInput("lowest_float_bottom"))
        );
    }
}

#[test]
fn real_selector_classification_and_amount_compose_without_equality_fallback() {
    let floats = [PlacedFloat {
        side: FloatSide::Left,
        left: 0.0,
        right: 2.0,
        top: 4.0,
        bottom: 6.0,
    }];
    let budget = NonZeroUsize::new(1).unwrap();
    let bottom = lowest_clearance_float_bottom(&floats, ClearSide::Left, budget).unwrap();
    assert_eq!(
        classify_clearance(4.0, bottom),
        Ok(ClearanceRequirement::Required)
    );
    // Explicit retained-example B, not a general margin owner.
    let required = solve_clearance(
        4.0,
        ClearanceState::Required {
            zero_clearance_border_top: 7.0,
            lowest_float_bottom: bottom.unwrap(),
        },
    )
    .unwrap();
    assert_eq!((required.border_top, required.clearance), (6.0, Some(-1.0)));
    assert_eq!(
        classify_clearance(8.0, bottom),
        Ok(ClearanceRequirement::Absent)
    );
    let absent = solve_clearance(8.0, ClearanceState::Absent).unwrap();
    assert_eq!((absent.border_top, absent.clearance), (8.0, None));
    assert_eq!(
        classify_clearance(6.0, bottom),
        Err(ClearancePresenceError::UnsupportedEquality)
    );
    let no_match = lowest_clearance_float_bottom(&floats, ClearSide::Right, budget).unwrap();
    assert_eq!(
        classify_clearance(4.0, no_match),
        Ok(ClearanceRequirement::Absent)
    );
    // Selection failure is propagated before classification, never changed to None.
    let invalid = [PlacedFloat {
        bottom: f32::NAN,
        ..floats[0]
    }];
    let selected = lowest_clearance_float_bottom(&invalid, ClearSide::Left, budget);
    let classified = selected.map(|bottom| classify_clearance(4.0, bottom));
    assert_eq!(
        classified,
        Err(ClearanceFloatError::InvalidBottom { index: 0 })
    );
}
