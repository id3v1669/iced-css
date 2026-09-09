//! CSS22 17.5 cell placement from already repaired, explicitly registered spans.
//! Logical intervals start at the table's direction-start edge. DP26 shifts cells
//! to avoid the overlap expressly permitted by the specification's alternative.
use super::block::Direction;
use std::{
    mem::size_of,
    num::{NonZeroU32, NonZeroUsize},
    ops::Range,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellSpan {
    pub row: u32,
    pub row_span: NonZeroU32,
    pub column_span: NonZeroU32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPlacement {
    pub row_start: u32,
    pub row_end: u32,
    pub column_start: u32,
    pub column_end: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct TablePlacementLimits {
    pub rows: NonZeroUsize,
    pub columns: NonZeroUsize,
    pub cells: NonZeroUsize,
    /// Actual output vector capacity; input slices and fixed stack state are separate.
    pub retained_bytes: NonZeroUsize,
    /// Admission plus row/cell validation, candidate positions and prior-cell visits.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TablePlacementError {
    Rows,
    Columns,
    Cells,
    Group { row: u32, end: u32 },
    Row { cell: usize, row: u32 },
    Order { cell: usize },
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for TablePlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table cell placement: {self:?}")
    }
}
impl std::error::Error for TablePlacementError {}
#[derive(Debug)]
pub struct TablePlacement {
    rows: u32,
    columns: u32,
    cells: Vec<CellPlacement>,
}
impl TablePlacement {
    pub fn rows(&self) -> u32 {
        self.rows
    }
    pub fn columns(&self) -> u32 {
        self.columns
    }
    /// One rectangle per input cell, in the same order; never duplicate source cells.
    pub fn cells(&self) -> &[CellPlacement] {
        &self.cells
    }
    pub fn retained_bytes(&self) -> usize {
        self.cells.capacity() * size_of::<CellPlacement>()
    }
    pub fn physical_columns(&self, cell: usize, direction: Direction) -> Option<Range<u32>> {
        let cell = self.cells.get(cell)?;
        Some(match direction {
            Direction::Ltr => cell.column_start..cell.column_end,
            Direction::Rtl => (self.columns - cell.column_end)..(self.columns - cell.column_start),
        })
    }
}
struct Work(usize);
impl Work {
    fn charge(&mut self, amount: usize) -> Result<(), TablePlacementError> {
        self.0 = self
            .0
            .checked_sub(amount)
            .ok_or(TablePlacementError::Work)?;
        Ok(())
    }
}
fn admit_columns(count: u32, limits: TablePlacementLimits) -> Result<(), TablePlacementError> {
    if usize::try_from(count).map_err(|_| TablePlacementError::Columns)? > limits.columns.get() {
        return Err(TablePlacementError::Columns);
    }
    Ok(())
}
/// Place an already repaired row/source-ordered sequence without dense grid storage.
/// Each entry in `row_scope_ends` is the exclusive end of that row's contiguous
/// row-group/table scope. This routine neither invents spans nor repairs source.
///
/// # Errors
/// Rejects invalid row scopes/order, coordinate overflow, explicit bounds and
/// allocation failure. Work exhaustion discards private placements without changing
/// the borrowed inputs. Width/height allocation and visibility collapse are separate.
pub fn place_table_cells(
    row_scope_ends: &[u32],
    minimum_columns: u32,
    cells: &[CellSpan],
    limits: TablePlacementLimits,
) -> Result<TablePlacement, TablePlacementError> {
    if row_scope_ends.len() > limits.rows.get() {
        return Err(TablePlacementError::Rows);
    }
    let row_count = u32::try_from(row_scope_ends.len()).map_err(|_| TablePlacementError::Rows)?;
    if cells.len() > limits.cells.get() || u32::try_from(cells.len()).is_err() {
        return Err(TablePlacementError::Cells);
    }
    admit_columns(minimum_columns, limits)?;
    let mut work = Work(limits.work.get());
    work.charge(
        row_scope_ends
            .len()
            .checked_add(cells.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(TablePlacementError::Work)?,
    )?;
    let mut scope_end = 0;
    for (index, &end) in row_scope_ends.iter().enumerate() {
        // Row count was admitted to u32 above.
        let row = index as u32;
        if row == scope_end {
            scope_end = end;
        }
        if end != scope_end || end <= row || end > row_count {
            return Err(TablePlacementError::Group { row, end });
        }
    }
    let mut previous_row = 0;
    for (index, cell) in cells.iter().enumerate() {
        if cell.row >= row_count {
            return Err(TablePlacementError::Row {
                cell: index,
                row: cell.row,
            });
        }
        if cell.row < previous_row {
            return Err(TablePlacementError::Order { cell: index });
        }
        previous_row = cell.row;
    }
    let check_bytes = |count: usize| {
        count
            .checked_mul(size_of::<CellPlacement>())
            .filter(|n| *n <= limits.retained_bytes.get() && *n <= isize::MAX as usize)
            .ok_or(TablePlacementError::Bytes)
    };
    check_bytes(cells.len())?;
    let mut placed: Vec<CellPlacement> = Vec::new();
    placed
        .try_reserve_exact(cells.len())
        .map_err(|_| TablePlacementError::Allocation)?;
    check_bytes(placed.capacity())?;
    let mut columns = minimum_columns;
    let mut cursor = 0u32;
    let mut active_row = None;
    for cell in cells {
        if active_row != Some(cell.row) {
            cursor = 0;
            active_row = Some(cell.row);
        }
        let mut candidate = cursor;
        let (start, end) = loop {
            work.charge(1)?;
            let end = candidate
                .checked_add(cell.column_span.get())
                .ok_or(TablePlacementError::Columns)?;
            admit_columns(end, limits)?;
            let mut next = candidate;
            for prior in &placed {
                work.charge(1)?;
                if prior.row_end > cell.row
                    && candidate < prior.column_end
                    && prior.column_start < end
                {
                    next = next.max(prior.column_end);
                }
            }
            if next == candidate {
                break (candidate, end);
            }
            candidate = next;
        };
        // The admitted row index originated within a usize-sized slice.
        let scope_end = row_scope_ends[cell.row as usize];
        let row_end = cell.row + cell.row_span.get().min(scope_end - cell.row);
        placed.push(CellPlacement {
            row_start: cell.row,
            row_end,
            column_start: start,
            column_end: end,
        });
        cursor = end;
        columns = columns.max(end);
    }
    Ok(TablePlacement {
        rows: row_count,
        columns,
        cells: placed,
    })
}
