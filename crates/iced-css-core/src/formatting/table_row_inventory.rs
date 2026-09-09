//! Ordered structural rows and cells of one repaired table; no span interpretation.
use super::{display::Display, table_repair_stage1::RepairKind, table_repair_stage3::Stage3};
use std::{mem::size_of, num::NonZeroUsize, ops::Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableTarget {
    Node(u32),
    External,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupTreatment {
    Body,
    Header,
    Footer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandKind {
    DirectRow,
    Group(GroupTreatment),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowBand {
    pub node: u32,
    pub kind: BandKind,
    pub rows: Range<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    pub node: u32,
    pub group: Option<u32>,
    pub cells: Range<u32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCellRef {
    pub node: u32,
    pub row: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct InventoryLimits {
    pub input_nodes: NonZeroUsize,
    pub bands: NonZeroUsize,
    pub rows: NonZeroUsize,
    pub cells: NonZeroUsize,
    /// Combined actual capacities of scratch and outputs at peak overlap.
    pub retained_bytes: NonZeroUsize,
    /// Logical entries: `3*n+2+c+2*(c+h+f+g+b+r+2*k)`.
    /// Here n is the whole Stage3 node count, c all selected-table direct children,
    /// h/f each zero or one selected header/footer, g visited group rows, and
    /// b/r/k emitted bands/rows/cells. Irrelevant direct children count toward c.
    /// Empty external tables cost two entries and allocate one scratch slot.
    /// Allocator internals and repeated calls for other targets are not included.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryError {
    InputNodes,
    Bands,
    Rows,
    Cells,
    Bytes,
    Work,
    Allocation,
    TargetIndex { index: u32 },
    TargetRole { index: u32 },
    ExternalRole,
}
impl std::fmt::Display for InventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table row inventory: {self:?}")
    }
}
impl std::error::Error for InventoryError {}

/// Borrows the exact repaired owner; all record node indices address that owner.
///
/// ```compile_fail
/// use iced_css_core::formatting::{display::Display, table_repair_stage1::*, table_repair_stage2::*, table_repair_stage3::*, table_row_inventory::*};
/// use std::num::NonZeroUsize;
/// let n = NonZeroUsize::new(1000).unwrap();
/// let first = suppress_table_boxes(Display::Table, &[], RepairLimits { nodes:n, retained_bytes:n, work:n }).unwrap();
/// let second = wrap_table_children(&first, Stage2Limits { input_nodes:n, output_nodes:n, retained_bytes:n, work:n }).unwrap();
/// let inventory;
/// {
///     let third = wrap_table_parents(&second, Stage3Limits { input_nodes:n, output_nodes:n, retained_bytes:n, work:n }).unwrap();
///     inventory = collect_table_rows(&third, TableTarget::External, InventoryLimits { input_nodes:n, bands:n, rows:n, cells:n, retained_bytes:n, work:n }).unwrap();
/// }
/// assert!(inventory.rows().is_empty());
/// ```
#[derive(Debug)]
pub struct TableRowInventory<'stage3, 'stage2, 'stage1, 'input> {
    stage3: &'stage3 Stage3<'stage2, 'stage1, 'input>,
    target: TableTarget,
    bands: Vec<RowBand>,
    rows: Vec<TableRow>,
    cells: Vec<TableCellRef>,
    peak_retained_bytes: usize,
}
impl<'stage3, 'stage2, 'stage1, 'input> TableRowInventory<'stage3, 'stage2, 'stage1, 'input> {
    pub fn stage3(&self) -> &'stage3 Stage3<'stage2, 'stage1, 'input> {
        self.stage3
    }
    pub fn target(&self) -> TableTarget {
        self.target
    }
    pub fn bands(&self) -> &[RowBand] {
        &self.bands
    }
    pub fn rows(&self) -> &[TableRow] {
        &self.rows
    }
    pub fn cells(&self) -> &[TableCellRef] {
        &self.cells
    }
    pub fn retained_bytes(&self) -> usize {
        self.bands.capacity() * size_of::<RowBand>()
            + self.rows.capacity() * size_of::<TableRow>()
            + self.cells.capacity() * size_of::<TableCellRef>()
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
}
struct Work(usize);
impl Work {
    fn charge(&mut self, amount: usize) -> Result<(), InventoryError> {
        self.0 = self.0.checked_sub(amount).ok_or(InventoryError::Work)?;
        Ok(())
    }
}
fn reserve<T>(count: usize, used: &mut usize, limit: usize) -> Result<Vec<T>, InventoryError> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(InventoryError::Bytes)?;
    let requested = used.checked_add(bytes).ok_or(InventoryError::Bytes)?;
    if requested > limit || requested > isize::MAX as usize {
        return Err(InventoryError::Bytes);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| InventoryError::Allocation)?;
    let actual = out
        .capacity()
        .checked_mul(size_of::<T>())
        .and_then(|n| used.checked_add(n))
        .ok_or(InventoryError::Bytes)?;
    if actual > limit || actual > isize::MAX as usize {
        return Err(InventoryError::Bytes);
    }
    *used = actual;
    Ok(out)
}
#[derive(Default)]
struct Counts {
    bands: u32,
    rows: u32,
    cells: u32,
}
fn increment(
    value: &mut u32,
    limit: NonZeroUsize,
    error: InventoryError,
    work: &mut Work,
) -> Result<u32, InventoryError> {
    let old = *value;
    let next = old.checked_add(1).ok_or(error)?;
    if next as usize > limit.get() {
        return Err(error);
    }
    work.charge(1)?;
    *value = next;
    Ok(old)
}
enum Event {
    Band(RowBand),
    Row(TableRow),
    Cell(TableCellRef),
}
struct Walker<'a, 'stage2, 'stage1, 'input> {
    owner: &'a Stage3<'stage2, 'stage1, 'input>,
    first: &'a [Option<u32>],
    next: &'a [Option<u32>],
    slot: usize,
    header: Option<u32>,
    footer: Option<u32>,
    limits: InventoryLimits,
}
impl Walker<'_, '_, '_, '_> {
    fn row(
        &self,
        node: u32,
        group: Option<u32>,
        count: &mut Counts,
        work: &mut Work,
        sink: &mut impl FnMut(Event) -> Result<(), InventoryError>,
    ) -> Result<(), InventoryError> {
        let row = increment(
            &mut count.rows,
            self.limits.rows,
            InventoryError::Rows,
            work,
        )?;
        let start = count.cells;
        let mut child = self.first[node as usize];
        while let Some(cell) = child {
            work.charge(1)?;
            debug_assert_eq!(
                self.owner.kind(cell),
                Some(RepairKind::Box(Display::TableCell))
            );
            increment(
                &mut count.cells,
                self.limits.cells,
                InventoryError::Cells,
                work,
            )?;
            sink(Event::Cell(TableCellRef { node: cell, row }))?;
            child = self.next[cell as usize];
        }
        sink(Event::Row(TableRow {
            node,
            group,
            cells: start..count.cells,
        }))
    }
    fn band(
        &self,
        node: u32,
        kind: BandKind,
        count: &mut Counts,
        work: &mut Work,
        sink: &mut impl FnMut(Event) -> Result<(), InventoryError>,
    ) -> Result<(), InventoryError> {
        increment(
            &mut count.bands,
            self.limits.bands,
            InventoryError::Bands,
            work,
        )?;
        let start = count.rows;
        match kind {
            BandKind::DirectRow => self.row(node, None, count, work, sink)?,
            BandKind::Group(_) => {
                let mut child = self.first[node as usize];
                while let Some(row) = child {
                    work.charge(1)?;
                    debug_assert_eq!(
                        self.owner.kind(row),
                        Some(RepairKind::Box(Display::TableRow))
                    );
                    self.row(row, Some(node), count, work, sink)?;
                    child = self.next[row as usize];
                }
            }
        }
        sink(Event::Band(RowBand {
            node,
            kind,
            rows: start..count.rows,
        }))
    }
    fn walk(
        &self,
        work: &mut Work,
        mut sink: impl FnMut(Event) -> Result<(), InventoryError>,
    ) -> Result<Counts, InventoryError> {
        let mut count = Counts::default();
        if let Some(node) = self.header {
            work.charge(1)?;
            self.band(
                node,
                BandKind::Group(GroupTreatment::Header),
                &mut count,
                work,
                &mut sink,
            )?;
        }
        let mut child = self.first[self.slot];
        while let Some(node) = child {
            work.charge(1)?;
            if Some(node) != self.header && Some(node) != self.footer {
                let kind = match self.owner.kind(node) {
                    Some(RepairKind::Box(Display::TableRow)) => Some(BandKind::DirectRow),
                    Some(RepairKind::Box(
                        Display::TableRowGroup
                        | Display::TableHeaderGroup
                        | Display::TableFooterGroup,
                    )) => Some(BandKind::Group(GroupTreatment::Body)),
                    _ => None,
                };
                if let Some(kind) = kind {
                    self.band(node, kind, &mut count, work, &mut sink)?;
                }
            }
            child = self.next[node as usize];
        }
        if let Some(node) = self.footer {
            work.charge(1)?;
            self.band(
                node,
                BandKind::Group(GroupTreatment::Footer),
                &mut count,
                work,
                &mut sink,
            )?;
        }
        Ok(count)
    }
}
fn push<T>(output: &mut Vec<T>, value: T, count: u32) -> Result<(), InventoryError> {
    // An emission pass must never grow beyond the exact counting pass.
    if output.len() >= count as usize {
        return Err(InventoryError::Allocation);
    }
    output.push(value);
    Ok(())
}

/// Collect direct rows and row groups of one Table/InlineTable target.
///
/// The first direct header group precedes the middle bands; the first footer
/// group follows them. Empty groups consume this designation and remain bands;
/// later header/footer groups receive ordinary treatment. Middle bands and each
/// group's rows retain sibling order even when node storage is not depth first.
/// Each row collects only its direct cells. Captions, columns and nested tables
/// contribute no rows or cells to this target. Empty rows retain their ordinal.
///
/// The result borrows the exact Stage3 owner without changing roles or provenance.
/// Node indices address that owner; cell spans and placement are not interpreted.
/// A whole-owner child index is constructed separately for every call. Its actual
/// capacity is included in peak bytes alongside output capacities, but excluded
/// from retained output bytes. Prior owners' allocations are excluded from both.
///
/// Returns a target error for an absent node or non-table role, or the applicable
/// count, byte, work or allocation error. Errors publish no partial inventory and
/// leave every borrowed owner unchanged.
pub fn collect_table_rows<'stage3, 'stage2, 'stage1, 'input>(
    stage3: &'stage3 Stage3<'stage2, 'stage1, 'input>,
    target: TableTarget,
    limits: InventoryLimits,
) -> Result<TableRowInventory<'stage3, 'stage2, 'stage1, 'input>, InventoryError> {
    let mut work = Work(limits.work.get());
    work.charge(1)?;
    let n = stage3.nodes().len();
    if n > limits.input_nodes.get() || u32::try_from(n).is_err() {
        return Err(InventoryError::InputNodes);
    }
    let slot = match target {
        TableTarget::External => {
            if !matches!(
                stage3.external_parent(),
                Display::Table | Display::InlineTable
            ) {
                return Err(InventoryError::ExternalRole);
            }
            n
        }
        TableTarget::Node(index) => {
            let kind = stage3
                .kind(index)
                .ok_or(InventoryError::TargetIndex { index })?;
            if !matches!(kind, RepairKind::Box(Display::Table | Display::InlineTable)) {
                return Err(InventoryError::TargetRole { index });
            }
            index as usize
        }
    };
    let slots = n.checked_add(1).ok_or(InventoryError::Bytes)?;
    work.charge(slots)?;
    work.charge(n)?;
    let mut used = 0;
    let mut first = reserve(slots, &mut used, limits.retained_bytes.get())?;
    let mut next = reserve(n, &mut used, limits.retained_bytes.get())?;
    first.resize(slots, None);
    next.resize(n, None);
    for (index, node) in stage3.nodes().iter().enumerate().rev() {
        work.charge(1)?;
        let parent = node.parent.map_or(n, |p| p as usize);
        next[index] = first[parent];
        first[parent] = Some(index as u32);
    }
    let mut header = None;
    let mut footer = None;
    let mut child = first[slot];
    while let Some(node) = child {
        work.charge(1)?;
        match stage3.kind(node) {
            Some(RepairKind::Box(Display::TableHeaderGroup)) if header.is_none() => {
                header = Some(node)
            }
            Some(RepairKind::Box(Display::TableFooterGroup)) if footer.is_none() => {
                footer = Some(node)
            }
            _ => {}
        }
        child = next[node as usize];
    }
    let walker = Walker {
        owner: stage3,
        first: &first,
        next: &next,
        slot,
        header,
        footer,
        limits,
    };
    let count = walker.walk(&mut work, |_| Ok(()))?;
    let mut bands = reserve(count.bands as usize, &mut used, limits.retained_bytes.get())?;
    let mut rows = reserve(count.rows as usize, &mut used, limits.retained_bytes.get())?;
    let mut cells = reserve(count.cells as usize, &mut used, limits.retained_bytes.get())?;
    walker.walk(&mut work, |event| match event {
        Event::Band(value) => push(&mut bands, value, count.bands),
        Event::Row(value) => push(&mut rows, value, count.rows),
        Event::Cell(value) => push(&mut cells, value, count.cells),
    })?;
    Ok(TableRowInventory {
        stage3,
        target,
        bands,
        rows,
        cells,
        peak_retained_bytes: used,
    })
}
