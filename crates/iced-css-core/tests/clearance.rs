use iced_css_core::formatting::{
    clearance::{ClearanceError, ClearancePlacement, ClearanceState, solve_clearance},
    margins::MarginExtrema,
};
use std::num::NonZeroUsize;

fn required(
    hypothetical: f32,
    baseline: f32,
    bottom: f32,
) -> Result<ClearancePlacement, ClearanceError> {
    solve_clearance(
        hypothetical,
        ClearanceState::Required {
            zero_clearance_border_top: baseline,
            lowest_float_bottom: bottom,
        },
    )
}

#[test]
fn absent_and_present_zero_have_distinct_collapse_meanings() {
    assert_eq!(
        solve_clearance(4.0, ClearanceState::Absent).unwrap(),
        ClearancePlacement {
            border_top: 4.0,
            clearance: None
        }
    );
    assert_eq!(
        required(4.0, 6.0, 6.0).unwrap(),
        ClearancePlacement {
            border_top: 6.0,
            clearance: Some(0.0)
        }
    );
}

#[test]
fn css22_negative_clearance_example_uses_recomputed_margin_position() {
    // §9.5.2 Example 2, choosing one coordinate unit per em. The preceding
    // border bottom is zero; M1=4, float height=2, target top margin M2=3.
    let mut margins = MarginExtrema::default();
    margins
        .extend(&[4.0, 3.0], NonZeroUsize::new(2).unwrap())
        .unwrap();
    let hypothetical = margins.collapsed();
    assert_eq!(hypothetical, 4.0);
    assert_eq!(
        required(hypothetical, 4.0 + 3.0, 4.0 + 2.0).unwrap(),
        ClearancePlacement {
            border_top: 6.0,
            clearance: Some(-1.0)
        }
    );
}

#[test]
fn both_first_algorithm_bounds_are_preserved() {
    for (h, baseline, bottom, top, amount) in [
        (4.0, 2.0, 6.0, 6.0, 4.0),
        (10.0, 12.0, 6.0, 10.0, -2.0),
        (-10.0, -6.0, -8.0, -8.0, -2.0),
    ] {
        assert_eq!(
            required(h, baseline, bottom).unwrap(),
            ClearancePlacement {
                border_top: top,
                clearance: Some(amount)
            }
        );
    }
}

#[test]
fn presence_is_an_input_not_inferred_at_equal_edges() {
    assert_eq!(
        solve_clearance(6.0, ClearanceState::Absent)
            .unwrap()
            .clearance,
        None
    );
    assert_eq!(
        required(6.0, 7.0, 6.0).unwrap(),
        ClearancePlacement {
            border_top: 6.0,
            clearance: Some(-1.0)
        }
    );
}

#[test]
fn finite_extremes_and_subnormals_do_not_publish_infinity() {
    assert_eq!(
        required(f32::MAX, -f32::MAX, f32::MAX),
        Err(ClearanceError::Unrepresentable)
    );
    assert_eq!(
        required(f32::MAX, f32::MAX, -f32::MAX).unwrap().clearance,
        Some(0.0)
    );
    let tiny = f32::from_bits(1);
    assert_eq!(
        required(-tiny, -tiny, tiny).unwrap(),
        ClearancePlacement {
            border_top: tiny,
            clearance: Some(f32::from_bits(2))
        }
    );
}

#[test]
fn nonfinite_inputs_are_rejected_by_field() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            solve_clearance(bad, ClearanceState::Absent),
            Err(ClearanceError::InvalidInput("hypothetical_top"))
        );
        assert_eq!(
            required(0.0, bad, 1.0),
            Err(ClearanceError::InvalidInput("zero_clearance_border_top"))
        );
        assert_eq!(
            required(0.0, 0.0, bad),
            Err(ClearanceError::InvalidInput("lowest_float_bottom"))
        );
    }
}

#[test]
fn ties_round_once_and_border_top_is_not_reconstructed_from_rounded_amount() {
    let half_ulp = f32::EPSILON / 2.0;
    let even = required(1.0, -half_ulp, 1.0).unwrap();
    assert_eq!(even.clearance.unwrap().to_bits(), 1.0_f32.to_bits());
    assert_eq!(even.border_top.to_bits(), 1.0_f32.to_bits());
    assert_ne!(-half_ulp + even.clearance.unwrap(), even.border_top);
    let odd = f32::from_bits(0x3f80_0001);
    let rounded = required(odd, -half_ulp, odd).unwrap();
    assert_eq!(rounded.clearance.unwrap().to_bits(), 0x3f80_0002);
    assert_eq!(rounded.border_top.to_bits(), odd.to_bits());
}

#[test]
fn negative_overflow_and_signed_zero_are_explicit() {
    assert_eq!(
        required(-f32::MAX, f32::MAX, -f32::MAX),
        Err(ClearanceError::Unrepresentable)
    );
    let absent = solve_clearance(-0.0, ClearanceState::Absent).unwrap();
    assert_eq!(absent.border_top.to_bits(), 0);
    assert_eq!(absent.clearance, None);
    let present = required(-0.0, 0.0, -0.0).unwrap();
    assert_eq!(present.border_top.to_bits(), 0);
    assert_eq!(present.clearance.unwrap().to_bits(), 0);
    // The unselected H-B is below finite f32 range; selected F-B is exactly zero.
    assert_eq!(
        required(-f32::MAX, f32::MAX, f32::MAX).unwrap(),
        ClearancePlacement {
            border_top: f32::MAX,
            clearance: Some(0.0)
        }
    );
}
