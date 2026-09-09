//! Perimeters of nonempty, caller-admitted non-cell table formatting regions.
pub use super::table_cell_borders::{
    CellBorderInput as TableRegionBorderInput, CellBorderSide as TableRegionBorderSide,
    CellBorders as TableRegionBorders,
};
use super::{
    block::Direction,
    table_borders::{
        BorderCandidate, BorderConflictError, BorderEdge, BorderOrder, BorderOrigin,
        BorderSegmentError,
    },
    table_cell_borders::BorderAxis,
    table_placement::TablePlacement,
};

/// Logical direction-start coordinates, before any visibility-collapse geometry.
/// Group membership must be supplied by the formatting owner, not native slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableBorderRegion {
    Table,
    Row {
        row: u32,
    },
    RowGroup {
        start: u32,
        end: u32,
    },
    /// A CSS column box may occupy more than one grid column.
    Column {
        start: u32,
        end: u32,
    },
    ColumnGroup {
        start: u32,
        end: u32,
    },
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableRegionBorderEdge {
    pub region: TableBorderRegion,
    pub side: TableRegionBorderSide,
    pub axis: BorderAxis,
    pub line: u32,
    pub edge: BorderEdge,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRegionBorderError {
    Region,
    Coordinate,
    Candidate(BorderConflictError),
    Segment(BorderSegmentError),
}
impl std::fmt::Display for TableRegionBorderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table region border: {self:?}")
    }
}
impl std::error::Error for TableRegionBorderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Candidate(e) => Some(e),
            Self::Segment(e) => Some(e),
            _ => None,
        }
    }
}
/// Project four physical sides in top/right/bottom/left order without allocation.
///
/// Validates geometry only. The caller certifies CSS formatting role and group
/// membership, retains source/color payloads and groups edges by physical line
/// before conflict resolution. Copied output does not certify placement generation.
/// Apply cell-interior exclusion before painting non-cell contenders.
///
/// # Errors
/// Rejects empty/out-of-bounds regions, unaddressable ranks and invalid side
/// candidates. No partial array is returned. Rejecting an empty region does not
/// prescribe CSS empty-table painting; that requires separate degenerate geometry.
pub fn placed_region_borders(
    grid: &TablePlacement,
    region: TableBorderRegion,
    borders: TableRegionBorders,
    direction: Direction,
) -> Result<[TableRegionBorderEdge; 4], TableRegionBorderError> {
    let (top, bottom, start, end, origin) = match region {
        TableBorderRegion::Table => (0, grid.rows(), 0, grid.columns(), BorderOrigin::Table),
        TableBorderRegion::Row { row } => (
            row,
            row.checked_add(1).ok_or(TableRegionBorderError::Region)?,
            0,
            grid.columns(),
            BorderOrigin::Row,
        ),
        TableBorderRegion::RowGroup { start, end } => {
            (start, end, 0, grid.columns(), BorderOrigin::RowGroup)
        }
        TableBorderRegion::Column { start, end } => {
            (0, grid.rows(), start, end, BorderOrigin::Column)
        }
        TableBorderRegion::ColumnGroup { start, end } => {
            (0, grid.rows(), start, end, BorderOrigin::ColumnGroup)
        }
    };
    if top >= bottom || bottom > grid.rows() || start >= end || end > grid.columns() {
        return Err(TableRegionBorderError::Region);
    }
    let order = BorderOrder {
        inline_start: usize::try_from(start).map_err(|_| TableRegionBorderError::Coordinate)?,
        block_start: usize::try_from(top).map_err(|_| TableRegionBorderError::Coordinate)?,
    };
    let (left, right) = match direction {
        Direction::Ltr => (start, end),
        Direction::Rtl => (grid.columns() - end, grid.columns() - start),
    };
    let edge = |side, axis, line, start, end, input: TableRegionBorderInput| {
        let candidate = BorderCandidate::new(input.style, input.width, origin, order)
            .map_err(TableRegionBorderError::Candidate)?;
        let edge =
            BorderEdge::new(start, end, candidate).map_err(TableRegionBorderError::Segment)?;
        Ok(TableRegionBorderEdge {
            region,
            side,
            axis,
            line,
            edge,
        })
    };
    Ok([
        edge(
            TableRegionBorderSide::Top,
            BorderAxis::Horizontal,
            top,
            left,
            right,
            borders.top,
        )?,
        edge(
            TableRegionBorderSide::Right,
            BorderAxis::Vertical,
            right,
            top,
            bottom,
            borders.right,
        )?,
        edge(
            TableRegionBorderSide::Bottom,
            BorderAxis::Horizontal,
            bottom,
            left,
            right,
            borders.bottom,
        )?,
        edge(
            TableRegionBorderSide::Left,
            BorderAxis::Vertical,
            left,
            top,
            bottom,
            borders.left,
        )?,
    ])
}
