//! Project captured Table content roots onto stock Table's flat native children.
use super::reconcile::{SlotError, SlotLimits};
use crate::capture::{Construction, ConstructionNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRow {
    Header,
    Body(usize),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCellSlot {
    pub native_index: usize,
    pub row: TableRow,
    pub column: usize,
    pub cell_root: usize,
    pub content_root: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSlotError {
    Root,
    Schema,
    Slots(SlotError),
}
impl std::fmt::Display for TableSlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table native projection: {self:?}")
    }
}
impl std::error::Error for TableSlotError {}
impl From<SlotError> for TableSlotError {
    fn from(error: SlotError) -> Self {
        Self::Slots(error)
    }
}
/// Positions are valid only for this borrowed construction snapshot. No identity,
/// compatibility decision, native state mutation or permission to reuse is implied.
#[derive(Debug)]
pub struct TableSlots<'a> {
    _construction: &'a Construction,
    cells: Vec<TableCellSlot>,
    columns: usize,
    rows: usize,
}
impl<'a> TableSlots<'a> {
    /// Project one captured table, excluding any nested table's own child slots.
    /// Work charges one root lookup plus two fixed-depth classification visits
    /// per construction node. Bounds cover only the retained descriptor vector.
    ///
    /// The projection cannot outlive the construction it describes:
    /// ```compile_fail,E0505
    /// use iced_css::{capture::Construction, host::{TableSlots, reconcile::SlotLimits}};
    /// fn discard(c: Construction, limits: SlotLimits) {
    ///     let projection = TableSlots::from_construction(&c, c.root(), limits).unwrap();
    ///     drop(c);
    ///     let _ = projection.cells();
    /// }
    /// ```
    pub fn from_construction(
        construction: &'a Construction,
        root: usize,
        limits: SlotLimits,
    ) -> Result<Self, TableSlotError> {
        let nodes = construction.nodes();
        if nodes
            .get(root)
            .is_none_or(|n| n.canonical_type() != "table")
        {
            return Err(TableSlotError::Root);
        }
        let work = nodes
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or(SlotError::Work)?;
        if work > limits.work.get() {
            return Err(SlotError::Work.into());
        }
        let mut groups = [0usize; 2];
        let mut header_rows = 0;
        let mut rows = 0;
        let mut columns = 0;
        let mut count = 0;
        for (index, node) in nodes.iter().enumerate() {
            if node.parent() == Some(root) {
                let group = match node.canonical_type() {
                    "table_header_group" => 0,
                    "table_row_group" => 1,
                    _ => return Err(TableSlotError::Schema),
                };
                if node.role_position() != Some(group) {
                    return Err(TableSlotError::Schema);
                }
                groups[group] += 1;
            }
            if node.canonical_type() == "table_row"
                && let Some(group) = node.parent().and_then(|p| nodes.get(p))
                && group.parent() == Some(root)
            {
                match group.canonical_type() {
                    "table_header_group" => {
                        if node.role_position() != Some(0) {
                            return Err(TableSlotError::Schema);
                        }
                        header_rows += 1;
                    }
                    "table_row_group" => {
                        if node.role_position() != Some(rows) {
                            return Err(TableSlotError::Schema);
                        }
                        rows += 1;
                    }
                    _ => return Err(TableSlotError::Schema),
                }
            }
            if let Some(cell) = inspect(nodes, index, root) {
                count += 1;
                if cell.row == TableRow::Header {
                    columns += 1;
                }
            }
        }
        let expected = rows
            .checked_add(1)
            .and_then(|n| n.checked_mul(columns))
            .ok_or(TableSlotError::Schema)?;
        if groups != [1, 1] || header_rows != 1 || expected != count {
            return Err(TableSlotError::Schema);
        }
        if count > limits.entries.get() {
            return Err(SlotError::Entries.into());
        }
        let bytes = |capacity: usize| {
            capacity
                .checked_mul(size_of::<TableCellSlot>())
                .filter(|n| *n <= limits.retained_bytes.get())
                .ok_or(SlotError::Bytes)
        };
        bytes(count)?;
        let mut cells = Vec::<TableCellSlot>::new();
        cells
            .try_reserve_exact(count)
            .map_err(|_| SlotError::Allocation)?;
        bytes(cells.capacity())?;
        for index in 0..nodes.len() {
            if let Some(mut cell) = inspect(nodes, index, root) {
                let slot = cells.len();
                if columns == 0 || slot >= count {
                    return Err(TableSlotError::Schema);
                }
                let row = if slot < columns {
                    TableRow::Header
                } else {
                    TableRow::Body(slot / columns - 1)
                };
                if cell.row != row || cell.column != slot % columns {
                    return Err(TableSlotError::Schema);
                }
                cell.native_index = slot;
                cells.push(cell);
            }
        }
        Ok(Self {
            _construction: construction,
            cells,
            columns,
            rows,
        })
    }
    pub fn cells(&self) -> &[TableCellSlot] {
        &self.cells
    }
    pub fn columns(&self) -> usize {
        self.columns
    }
    /// Number of source data rows, excluding the header row.
    pub fn rows(&self) -> usize {
        self.rows
    }
    pub fn retained_bytes(&self) -> usize {
        self.cells.capacity() * size_of::<TableCellSlot>()
    }
}
fn inspect(nodes: &[ConstructionNode], content_root: usize, root: usize) -> Option<TableCellSlot> {
    let content = nodes.get(content_root)?;
    if content.role() != Some("content") {
        return None;
    }
    let cell_root = content.parent()?;
    let cell = nodes.get(cell_root)?;
    let row = nodes.get(cell.parent()?)?;
    if row.canonical_type() != "table_row" {
        return None;
    }
    let group = nodes.get(row.parent()?)?;
    if group.parent() != Some(root) {
        return None;
    }
    let row = match (group.canonical_type(), cell.canonical_type()) {
        ("table_header_group", "table_header_cell") if row.role_position() == Some(0) => {
            TableRow::Header
        }
        ("table_row_group", "table_cell") => TableRow::Body(row.role_position()?),
        _ => return None,
    };
    Some(TableCellSlot {
        native_index: 0,
        row,
        column: cell.role_position()?,
        cell_root,
        content_root,
    })
}
