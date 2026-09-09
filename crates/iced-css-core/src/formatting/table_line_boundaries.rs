//! Portions of a physical grid line outside admitted spanning-cell interiors.
use super::{block::Direction, table_cell_borders::BorderAxis, table_placement::TablePlacement};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridInterval {
    pub start: u32,
    pub end: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct TableBoundaryLimits {
    pub segments: NonZeroUsize,
    /// Actual retained interval capacity, excluding allocator-internal overlap.
    pub retained_bytes: NonZeroUsize,
    /// Admission, each cursor scan/cell visit, and each emitted interval.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableBoundaryError {
    Line { line: u32, maximum: u32 },
    Segments,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for TableBoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table grid-line boundary: {self:?}")
    }
}
impl std::error::Error for TableBoundaryError {}
#[derive(Debug)]
pub struct TableLineBoundaries<'grid> {
    _grid: &'grid TablePlacement,
    axis: BorderAxis,
    line: u32,
    direction: Direction,
    extent: u32,
    intervals: Vec<GridInterval>,
}
impl TableLineBoundaries<'_> {
    pub fn axis(&self) -> BorderAxis {
        self.axis
    }
    pub fn line(&self) -> u32 {
        self.line
    }
    pub fn direction(&self) -> Direction {
        self.direction
    }
    pub fn extent(&self) -> u32 {
        self.extent
    }
    pub fn intervals(&self) -> &[GridInterval] {
        &self.intervals
    }
    pub fn retained_bytes(&self) -> usize {
        self.intervals.capacity() * size_of::<GridInterval>()
    }
}
struct Work(usize);
impl Work {
    fn charge(&mut self, n: usize) -> Result<(), TableBoundaryError> {
        self.0 = self.0.checked_sub(n).ok_or(TableBoundaryError::Work)?;
        Ok(())
    }
}
fn emit(
    output: &mut Vec<GridInterval>,
    start: u32,
    end: u32,
    limits: TableBoundaryLimits,
    work: &mut Work,
) -> Result<(), TableBoundaryError> {
    work.charge(1)?;
    let count = output
        .len()
        .checked_add(1)
        .ok_or(TableBoundaryError::Segments)?;
    if count > limits.segments.get() {
        return Err(TableBoundaryError::Segments);
    }
    let bytes = |count: usize| {
        count
            .checked_mul(size_of::<GridInterval>())
            .filter(|n| *n <= limits.retained_bytes.get() && *n <= isize::MAX as usize)
            .ok_or(TableBoundaryError::Bytes)
    };
    bytes(count)?;
    output
        .try_reserve_exact(1)
        .map_err(|_| TableBoundaryError::Allocation)?;
    bytes(output.capacity())?;
    output.push(GridInterval { start, end });
    Ok(())
}
/// Compute geometric boundary intervals without allocating a dense occupancy grid.
/// Perimeter coincidence is retained; only strict cell interiors are excluded.
/// Candidate priorities/colors and physical track/corner geometry are separate.
///
/// The intervals remain tied to this placement owner:
/// ```compile_fail,E0505
/// use iced_css_core::formatting::{block::Direction,table_cell_borders::BorderAxis,table_placement::TablePlacement,table_line_boundaries::{TableBoundaryLimits,table_line_boundaries}};
/// fn replace(grid:TablePlacement,limits:TableBoundaryLimits) {
///     let boundaries=table_line_boundaries(&grid,BorderAxis::Horizontal,0,Direction::Ltr,limits).unwrap();
///     drop(grid);
///     std::hint::black_box(boundaries.intervals());
/// }
/// ```
///
/// # Errors
/// Rejects invalid grid lines and exhausted work/segment/storage/allocation bounds.
/// Failure discards private output and leaves the borrowed placement unchanged.
pub fn table_line_boundaries(
    grid: &TablePlacement,
    axis: BorderAxis,
    line: u32,
    direction: Direction,
    limits: TableBoundaryLimits,
) -> Result<TableLineBoundaries<'_>, TableBoundaryError> {
    let mut work = Work(limits.work.get());
    work.charge(1)?;
    let (maximum, extent) = match axis {
        BorderAxis::Horizontal => (grid.rows(), grid.columns()),
        BorderAxis::Vertical => (grid.columns(), grid.rows()),
    };
    if line > maximum {
        return Err(TableBoundaryError::Line { line, maximum });
    }
    let scan_work = grid
        .cells()
        .len()
        .checked_add(1)
        .ok_or(TableBoundaryError::Work)?;
    let mut cursor = 0;
    let mut intervals = Vec::new();
    while cursor < extent {
        work.charge(scan_work)?;
        let mut next: Option<GridInterval> = None;
        for cell in grid.cells() {
            // Placement admits each logical interval inside the final column count.
            let (left, right) = match direction {
                Direction::Ltr => (cell.column_start, cell.column_end),
                Direction::Rtl => (
                    grid.columns() - cell.column_end,
                    grid.columns() - cell.column_start,
                ),
            };
            let interval = match axis {
                BorderAxis::Horizontal if cell.row_start < line && line < cell.row_end => {
                    GridInterval {
                        start: left,
                        end: right,
                    }
                }
                BorderAxis::Vertical if left < line && line < right => GridInterval {
                    start: cell.row_start,
                    end: cell.row_end,
                },
                _ => continue,
            };
            if interval.end <= cursor {
                continue;
            }
            if next.is_none_or(|old| {
                interval.start < old.start
                    || (interval.start == old.start && interval.end > old.end)
            }) {
                next = Some(interval);
            }
        }
        if let Some(excluded) = next {
            if cursor < excluded.start {
                emit(&mut intervals, cursor, excluded.start, limits, &mut work)?;
            }
            // Selected intervals end strictly beyond the cursor; each can be
            // selected at most once, even if future placement policies overlap.
            cursor = excluded.end;
        } else {
            emit(&mut intervals, cursor, extent, limits, &mut work)?;
            break;
        }
    }
    Ok(TableLineBoundaries {
        _grid: grid,
        axis,
        line,
        direction,
        extent,
        intervals,
    })
}
