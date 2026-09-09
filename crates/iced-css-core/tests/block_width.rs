use iced_css_core::formatting::block::*;
fn input() -> BlockWidthInput {
    BlockWidthInput {
        containing_width: 100.0,
        direction: Direction::Ltr,
        width: AutoLength::Value(40.0),
        margin_left: AutoLength::Auto,
        margin_right: AutoLength::Auto,
        border_left: 0.0,
        border_right: 0.0,
        padding_left: 0.0,
        padding_right: 0.0,
        min_width: 0.0,
        max_width: None,
    }
}
#[test]
fn auto_margins_and_directional_overconstraint_solve_the_css_equation() {
    let mut i = input();
    let result = solve_block_width(i).unwrap();
    assert_eq!(
        (result.width, result.margin_left, result.margin_right),
        (40.0, 30.0, 30.0)
    );
    i.margin_left = AutoLength::Value(10.0);
    i.margin_right = AutoLength::Value(20.0);
    let result = solve_block_width(i).unwrap();
    assert_eq!((result.margin_left, result.margin_right), (10.0, 50.0));
    i.direction = Direction::Rtl;
    let result = solve_block_width(i).unwrap();
    assert_eq!((result.margin_left, result.margin_right), (40.0, 20.0));
    i.margin_left = AutoLength::Auto;
    let result = solve_block_width(i).unwrap();
    assert_eq!((result.margin_left, result.margin_right), (40.0, 20.0));
}
#[test]
fn overflowing_nonauto_width_zeroes_auto_margins_before_end_margin_adjustment() {
    let mut i = input();
    i.containing_width = 20.0;
    i.width = AutoLength::Value(20.0);
    i.margin_right = AutoLength::Value(10.0);
    let result = solve_block_width(i).unwrap();
    assert_eq!((result.margin_left, result.margin_right), (0.0, 0.0));
    i.direction = Direction::Rtl;
    let result = solve_block_width(i).unwrap();
    assert_eq!((result.margin_left, result.margin_right), (-10.0, 10.0));
    i.containing_width = 10.0;
    i.margin_right = AutoLength::Auto;
    let result = solve_block_width(i).unwrap();
    assert_eq!((result.margin_left, result.margin_right), (-10.0, 0.0));
    i.direction = Direction::Ltr;
    let result = solve_block_width(i).unwrap();
    assert_eq!((result.margin_left, result.margin_right), (0.0, -10.0));
}
#[test]
fn auto_width_uses_zero_auto_margins_and_includes_all_edges() {
    let mut i = input();
    i.width = AutoLength::Auto;
    i.margin_right = AutoLength::Value(10.0);
    i.padding_left = 5.0;
    i.padding_right = 5.0;
    i.border_left = 1.0;
    i.border_right = 1.0;
    let result = solve_block_width(i).unwrap();
    assert_eq!(
        (result.width, result.margin_left, result.margin_right),
        (78.0, 0.0, 10.0)
    );
}
#[test]
fn negative_tentative_auto_width_reruns_at_minimum_and_recomputes_margins() {
    let mut i = input();
    i.containing_width = 10.0;
    i.width = AutoLength::Auto;
    i.padding_left = 10.0;
    i.padding_right = 10.0;
    let result = solve_block_width(i).unwrap();
    assert_eq!(
        (
            result.width,
            result.margin_left,
            result.margin_right,
            result.passes
        ),
        (0.0, 0.0, -10.0, 2)
    );
    i.direction = Direction::Rtl;
    let result = solve_block_width(i).unwrap();
    assert_eq!(
        (result.width, result.margin_left, result.margin_right),
        (0.0, -10.0, 0.0)
    );
    assert_eq!(i.width, AutoLength::Auto);
}
#[test]
fn min_and_max_are_category_reruns_with_minimum_winning_conflict() {
    let mut i = input();
    i.width = AutoLength::Value(80.0);
    i.min_width = 60.0;
    i.max_width = Some(40.0);
    let result = solve_block_width(i).unwrap();
    assert_eq!(
        (
            result.width,
            result.margin_left,
            result.margin_right,
            result.passes
        ),
        (60.0, 20.0, 20.0, 3)
    );
    i = input();
    i.width = AutoLength::Auto;
    i.margin_left = AutoLength::Value(10.0);
    i.margin_right = AutoLength::Value(10.0);
    i.max_width = Some(50.0);
    let result = solve_block_width(i).unwrap();
    assert_eq!(
        (result.width, result.margin_left, result.margin_right),
        (50.0, 10.0, 40.0)
    );
}
#[test]
fn invalid_domains_and_unrepresentable_signed_arithmetic_fail_explicitly() {
    let mut i = input();
    i.width = AutoLength::Value(-1.0);
    assert!(matches!(
        solve_block_width(i),
        Err(BlockWidthError::InvalidInput(_))
    ));
    i = input();
    i.padding_left = f32::INFINITY;
    assert!(matches!(
        solve_block_width(i),
        Err(BlockWidthError::InvalidInput(_))
    ));
    i = input();
    i.containing_width = f32::MAX;
    i.width = AutoLength::Auto;
    i.margin_left = AutoLength::Value(-f32::MAX);
    assert_eq!(solve_block_width(i), Err(BlockWidthError::Unrepresentable));
}

#[test]
fn exact_intermediates_preserve_cancellation_and_defer_tentative_range_checks() {
    let mut i = input();
    i.containing_width = f32::MAX;
    i.width = AutoLength::Auto;
    i.margin_left = AutoLength::Value(-f32::MAX);
    i.margin_right = AutoLength::Value(f32::MAX);
    let result = solve_block_width(i).unwrap();
    assert_eq!(result.width, f32::MAX);
    i.containing_width = 0.0;
    i.margin_left = AutoLength::Value(f32::MAX);
    let result = solve_block_width(i).unwrap();
    assert_eq!(
        (
            result.width,
            result.margin_left,
            result.margin_right,
            result.passes
        ),
        (0.0, f32::MAX, -f32::MAX, 2)
    );
}
#[test]
fn paired_auto_margins_round_half_subnormals_with_ties_even() {
    let mut i = input();
    i.width = AutoLength::Value(0.0);
    for (containing_bits, expected_bits) in [(1, 0), (3, 2), (5, 2)] {
        i.containing_width = f32::from_bits(containing_bits);
        let result = solve_block_width(i).unwrap();
        assert_eq!(result.margin_left.to_bits(), expected_bits);
        assert_eq!(result.margin_right.to_bits(), expected_bits);
    }
}
