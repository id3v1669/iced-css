//! Physical cell perimeter edges with direction-aware collapsed-border priority.
use super::{
    block::Direction,
    table_borders::{
        BorderCandidate, BorderConflictError, BorderEdge, BorderOrder, BorderOrigin,
        BorderSegmentError, BorderStyle,
    },
    table_placement::TablePlacement,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderAxis {
    Horizontal,
    Vertical,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellBorderSide {
    Top,
    Right,
    Bottom,
    Left,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellBorderInput {
    pub style: BorderStyle,
    pub width: f32,
}
impl Default for CellBorderInput {
    fn default() -> Self {
        Self {
            style: BorderStyle::None,
            width: 0.,
        }
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CellBorders {
    pub top: CellBorderInput,
    pub right: CellBorderInput,
    pub bottom: CellBorderInput,
    pub left: CellBorderInput,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellBorderEdge {
    /// Input-cell index in TablePlacement, not a CSS source ID or native key.
    pub cell: usize,
    pub side: CellBorderSide,
    pub axis: BorderAxis,
    pub line: u32,
    pub edge: BorderEdge,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellBorderError {
    Cell { cell: usize },
    Coordinate,
    Candidate(BorderConflictError),
    Segment(BorderSegmentError),
}
impl std::fmt::Display for CellBorderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "placed table cell border: {self:?}")
    }
}
impl std::error::Error for CellBorderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Candidate(e) => Some(e),
            Self::Segment(e) => Some(e),
            _ => None,
        }
    }
}
/// Project one admitted cell onto its four physical perimeter grid lines.
/// Order is top, right, bottom, left. Source/color payloads remain caller-owned.
/// Group records by axis/line before invoking the collapsed-border splitter;
/// other table origins and aggregate work/storage admission are separate.
///
/// # Errors
/// Rejects invalid cell indices, unaddressable ranks and invalid side candidates.
/// Returns no partial edge array and performs no allocation or native callback.
pub fn placed_cell_borders(
    grid: &TablePlacement,
    cell: usize,
    borders: CellBorders,
    direction: Direction,
) -> Result<[CellBorderEdge; 4], CellBorderError> {
    let placement = grid
        .cells()
        .get(cell)
        .ok_or(CellBorderError::Cell { cell })?;
    let columns = grid
        .physical_columns(cell, direction)
        .ok_or(CellBorderError::Cell { cell })?;
    let order = BorderOrder {
        inline_start: usize::try_from(placement.column_start)
            .map_err(|_| CellBorderError::Coordinate)?,
        block_start: usize::try_from(placement.row_start)
            .map_err(|_| CellBorderError::Coordinate)?,
    };
    let edge = |side, axis, line, start, end, input: CellBorderInput| {
        let candidate = BorderCandidate::new(input.style, input.width, BorderOrigin::Cell, order)
            .map_err(CellBorderError::Candidate)?;
        let edge = BorderEdge::new(start, end, candidate).map_err(CellBorderError::Segment)?;
        Ok(CellBorderEdge {
            cell,
            side,
            axis,
            line,
            edge,
        })
    };
    Ok([
        edge(
            CellBorderSide::Top,
            BorderAxis::Horizontal,
            placement.row_start,
            columns.start,
            columns.end,
            borders.top,
        )?,
        edge(
            CellBorderSide::Right,
            BorderAxis::Vertical,
            columns.end,
            placement.row_start,
            placement.row_end,
            borders.right,
        )?,
        edge(
            CellBorderSide::Bottom,
            BorderAxis::Horizontal,
            placement.row_end,
            columns.start,
            columns.end,
            borders.bottom,
        )?,
        edge(
            CellBorderSide::Left,
            BorderAxis::Vertical,
            columns.start,
            placement.row_start,
            placement.row_end,
            borders.left,
        )?,
    ])
}
