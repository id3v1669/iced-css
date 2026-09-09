//! Restricted CSS22 17.5.2.1 column precedence and equal remaining-space division.
use super::arithmetic::Exact;
use std::{
    mem::size_of,
    num::{NonZeroU32, NonZeroUsize},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedColumnInput {
    pub column_width: Option<f32>,
    pub first_row_cell_width: Option<f32>,
}
#[derive(Debug, Clone, Copy)]
pub struct FixedColumnLimits {
    pub columns: NonZeroUsize,
    /// Actual output vector capacity; borrowed input and fixed stack state excluded.
    pub retained_bytes: NonZeroUsize,
    /// One admission, one validation/selection and one output step per column.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedColumnError {
    Columns,
    TableWidth,
    ColumnWidth { column: usize },
    FirstRowCellWidth { column: usize },
    AssignedExceedsTable,
    ExcessWithoutUnassigned,
    Bytes,
    Work,
    Allocation,
    Unrepresentable,
}
impl std::fmt::Display for FixedColumnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fixed unspanned table columns: {self:?}")
    }
}
impl std::error::Error for FixedColumnError {}
#[derive(Debug)]
pub struct FixedColumns {
    widths: Vec<f32>,
}
impl FixedColumns {
    pub fn widths(&self) -> &[f32] {
        &self.widths
    }
    pub fn retained_bytes(&self) -> usize {
        self.widths.capacity() * size_of::<f32>()
    }
}
fn valid(width: f32) -> bool {
    width.is_finite() && width >= 0.0
}

/// Select column widths, then first-row cell widths, then equal exact residual shares.
///
/// The caller establishes a known nonzero column count, nonspanning first-row
/// cells, no later-row generated columns, resolved absolute widths, and zero
/// horizontal padding/borders/spacing. This kernel does not inspect source boxes.
/// Equal shares are rounded once to binary32, nearest with ties to even; no final
/// column correction redistributes rounding error. Outputs canonicalize zero.
///
/// # Errors
/// Validates both optional widths even when precedence masks one. Rejects invalid
/// widths, assigned sums exceeding the table width, and excess space when no
/// column remains unassigned. Count/work/actual-capacity bounds and allocation
/// failure are explicit. Errors preserve input and discard private output.
pub fn solve_fixed_unspanned_columns(
    table_width: f32,
    columns: &[FixedColumnInput],
    limits: FixedColumnLimits,
) -> Result<FixedColumns, FixedColumnError> {
    if columns.is_empty()
        || columns.len() > limits.columns.get()
        || u32::try_from(columns.len()).is_err()
    {
        return Err(FixedColumnError::Columns);
    }
    let work = columns
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .ok_or(FixedColumnError::Work)?;
    if work > limits.work.get() {
        return Err(FixedColumnError::Work);
    }
    if !valid(table_width) {
        return Err(FixedColumnError::TableWidth);
    }
    let mut assigned = Exact::ZERO;
    let mut unassigned = 0u32;
    for (column, input) in columns.iter().enumerate() {
        if input.column_width.is_some_and(|width| !valid(width)) {
            return Err(FixedColumnError::ColumnWidth { column });
        }
        if input
            .first_row_cell_width
            .is_some_and(|width| !valid(width))
        {
            return Err(FixedColumnError::FirstRowCellWidth { column });
        }
        if let Some(width) = input.column_width.or(input.first_row_cell_width) {
            // At most u32::MAX nonnegative finite terms: < 2^32 * f32::MAX,
            // or 310 magnitude bits in Exact's 320-bit accumulator.
            assigned = assigned.add(Exact::from_f32(width));
        } else {
            // There are at most u32::MAX admitted input columns.
            unassigned += 1;
        }
    }
    let residual = Exact::from_f32(table_width).sub(assigned);
    if residual < Exact::ZERO {
        return Err(FixedColumnError::AssignedExceedsTable);
    }
    let share = if let Some(count) = NonZeroU32::new(unassigned) {
        residual
            .divided_to_f32(count)
            .map_err(|_| FixedColumnError::Unrepresentable)?
    } else if residual == Exact::ZERO {
        0.0
    } else {
        return Err(FixedColumnError::ExcessWithoutUnassigned);
    };
    let bytes = |count: usize| {
        count
            .checked_mul(size_of::<f32>())
            .filter(|bytes| *bytes <= limits.retained_bytes.get() && *bytes <= isize::MAX as usize)
            .ok_or(FixedColumnError::Bytes)
    };
    bytes(columns.len())?;
    let mut widths = Vec::new();
    widths
        .try_reserve_exact(columns.len())
        .map_err(|_| FixedColumnError::Allocation)?;
    bytes(widths.capacity())?;
    for column in columns {
        let width = column
            .column_width
            .or(column.first_row_cell_width)
            .unwrap_or(share);
        widths.push(if width == 0.0 { 0.0 } else { width });
    }
    Ok(FixedColumns { widths })
}
