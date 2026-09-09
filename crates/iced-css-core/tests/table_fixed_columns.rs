use iced_css_core::formatting::table_fixed_columns::{
    FixedColumnError, FixedColumnInput, FixedColumnLimits, solve_fixed_unspanned_columns,
};
use std::num::NonZeroUsize;

fn n(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn limits() -> FixedColumnLimits {
    FixedColumnLimits {
        columns: n(100),
        retained_bytes: n(10000),
        work: n(10000),
    }
}
fn column(column_width: Option<f32>, first_row_cell_width: Option<f32>) -> FixedColumnInput {
    FixedColumnInput {
        column_width,
        first_row_cell_width,
    }
}

#[test]
fn precedence_cell_fallback_and_equal_residual_shares() {
    let input = [
        column(Some(40.), Some(90.)),
        column(None, Some(20.)),
        column(None, None),
        column(None, None),
    ];
    let result = solve_fixed_unspanned_columns(100., &input, limits()).unwrap();
    assert_eq!(result.widths(), [40., 20., 20., 20.]);
    assert_eq!(
        solve_fixed_unspanned_columns(60., &input[..2], limits())
            .unwrap()
            .widths(),
        [40., 20.]
    );
}

#[test]
fn exact_residual_rounds_only_after_division() {
    let input = [
        column(Some(2.0f32.powi(-25)), None),
        column(None, None),
        column(None, None),
        column(None, None),
    ];
    let result = solve_fixed_unspanned_columns(1., &input, limits()).unwrap();
    for width in &result.widths()[1..] {
        assert_eq!(width.to_bits(), 0x3eaaaaaa);
    }
    let thirds = solve_fixed_unspanned_columns(1., &[column(None, None); 3], limits()).unwrap();
    assert!(
        thirds
            .widths()
            .iter()
            .all(|width| width.to_bits() == 0x3eaaaaab)
    );
    for (bits, expected) in [(1, 0), (3, 2)] {
        let result =
            solve_fixed_unspanned_columns(f32::from_bits(bits), &[column(None, None); 2], limits())
                .unwrap();
        assert!(
            result
                .widths()
                .iter()
                .all(|width| width.to_bits() == expected)
        );
    }
    let result = solve_fixed_unspanned_columns(
        -0.,
        &[column(Some(-0.), None), column(None, None)],
        limits(),
    )
    .unwrap();
    assert!(result.widths().iter().all(|width| width.to_bits() == 0));
}

#[test]
fn unsupported_domains_use_exact_comparisons() {
    assert_eq!(
        solve_fixed_unspanned_columns(1., &[], limits()).unwrap_err(),
        FixedColumnError::Columns
    );
    assert_eq!(
        solve_fixed_unspanned_columns(2., &[column(Some(1.), None)], limits()).unwrap_err(),
        FixedColumnError::ExcessWithoutUnassigned
    );
    let too_wide = [
        column(Some(1.), None),
        column(Some(f32::from_bits(1)), None),
        column(None, None),
    ];
    assert_eq!(
        solve_fixed_unspanned_columns(1., &too_wide, limits()).unwrap_err(),
        FixedColumnError::AssignedExceedsTable
    );
    let max = [column(Some(f32::MAX), None); 2];
    assert_eq!(
        solve_fixed_unspanned_columns(f32::MAX, &max, limits()).unwrap_err(),
        FixedColumnError::AssignedExceedsTable
    );
}

#[test]
fn invalid_widths_include_masked_first_row_values() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.] {
        assert_eq!(
            solve_fixed_unspanned_columns(value, &[column(None, None)], limits()).unwrap_err(),
            FixedColumnError::TableWidth
        );
        assert_eq!(
            solve_fixed_unspanned_columns(1., &[column(Some(value), None)], limits()).unwrap_err(),
            FixedColumnError::ColumnWidth { column: 0 }
        );
        assert_eq!(
            solve_fixed_unspanned_columns(1., &[column(Some(1.), Some(value))], limits())
                .unwrap_err(),
            FixedColumnError::FirstRowCellWidth { column: 0 }
        );
    }
}

#[test]
fn limits_are_exact_and_failures_preserve_input() {
    let input = [
        column(Some(1.), None),
        column(None, None),
        column(None, None),
    ];
    let before = input;
    let result = solve_fixed_unspanned_columns(
        3.,
        &input,
        FixedColumnLimits {
            work: n(7),
            ..limits()
        },
    )
    .unwrap();
    let bytes = result.retained_bytes();
    assert!(
        solve_fixed_unspanned_columns(
            3.,
            &input,
            FixedColumnLimits {
                retained_bytes: n(bytes),
                ..limits()
            }
        )
        .is_ok()
    );
    for (cap, error) in [
        (
            FixedColumnLimits {
                columns: n(2),
                ..limits()
            },
            FixedColumnError::Columns,
        ),
        (
            FixedColumnLimits {
                work: n(6),
                ..limits()
            },
            FixedColumnError::Work,
        ),
        (
            FixedColumnLimits {
                retained_bytes: n(bytes - 1),
                ..limits()
            },
            FixedColumnError::Bytes,
        ),
    ] {
        assert_eq!(
            solve_fixed_unspanned_columns(3., &input, cap).unwrap_err(),
            error
        );
        assert_eq!(input, before);
    }
}
