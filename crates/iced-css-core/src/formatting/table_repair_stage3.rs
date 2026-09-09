//! CSS22 17.2.1 missing parents over the exact Stage2 owner.
use super::{display::Display, table_repair_stage1::RepairKind, table_repair_stage2::Stage2};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableWrapperKind {
    Table,
    InlineTable,
}
impl TableWrapperKind {
    fn display(self) -> Display {
        match self {
            Self::Table => Display::Table,
            Self::InlineTable => Display::InlineTable,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage3Origin {
    /// Exact Stage2 index, including its original/generated provenance chain.
    Existing {
        index: u32,
    },
    GeneratedRow,
    GeneratedTable(TableWrapperKind),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stage3Node {
    /// Output-local parent; None denotes the unchanged external context.
    pub parent: Option<u32>,
    pub origin: Stage3Origin,
}
#[derive(Debug, Clone, Copy)]
pub struct Stage3Limits {
    pub input_nodes: NonZeroUsize,
    /// Exact existing-plus-generated count; also bounded by u32::MAX.
    pub output_nodes: NonZeroUsize,
    /// Combined actual capacities of run state, mapping and output at peak overlap.
    pub retained_bytes: NonZeroUsize,
    /// Nonempty: 5*n+3+2*m entries. Empty: one admission entry.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage3Error {
    InputNodes,
    OutputNodes,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for Stage3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table repair stage 3: {self:?}")
    }
}
impl std::error::Error for Stage3Error {}

/// Structural parent repair retaining its exact Stage2 owner.
///
/// ```compile_fail
/// use iced_css_core::formatting::{display::Display, table_repair_stage1::*, table_repair_stage2::*, table_repair_stage3::*};
/// use std::num::NonZeroUsize;
/// let n = NonZeroUsize::new(1000).unwrap();
/// let input = [RepairNode { parent: None, kind: RepairKind::Box(Display::TableCell) }];
/// let first = suppress_table_boxes(Display::Inline, &input,
///     RepairLimits { nodes: n, retained_bytes: n, work: n }).unwrap();
/// let result;
/// {
///     let second = wrap_table_children(&first,
///         Stage2Limits { input_nodes: n, output_nodes: n, retained_bytes: n, work: n }).unwrap();
///     result = wrap_table_parents(&second,
///         Stage3Limits { input_nodes: n, output_nodes: n, retained_bytes: n, work: n }).unwrap();
/// }
/// assert_eq!(result.nodes().len(), 3);
/// ```
#[derive(Debug)]
pub struct Stage3<'stage2, 'stage1, 'input> {
    stage2: &'stage2 Stage2<'stage1, 'input>,
    nodes: Vec<Stage3Node>,
    previous_mapping: Vec<u32>,
    peak_retained_bytes: usize,
}
impl<'stage2, 'stage1, 'input> Stage3<'stage2, 'stage1, 'input> {
    pub fn stage2(&self) -> &'stage2 Stage2<'stage1, 'input> {
        self.stage2
    }
    pub fn external_parent(&self) -> Display {
        self.stage2.external_parent()
    }
    /// Parent-before-child; increasing indices define sibling order, not DFS.
    pub fn nodes(&self) -> &[Stage3Node] {
        &self.nodes
    }
    /// One output index for every Stage2 node; Stage3 suppresses none.
    pub fn previous_mapping(&self) -> &[u32] {
        &self.previous_mapping
    }
    /// Constant-time composition; no duplicate original-source mapping is allocated.
    /// None means either a suppressed original or an out-of-range original index.
    pub fn original_output(&self, index: u32) -> Option<u32> {
        let previous = (*self
            .stage2
            .original_mapping()
            .get(usize::try_from(index).ok()?)?)?;
        self.previous_mapping.get(previous as usize).copied()
    }
    pub fn kind(&self, index: u32) -> Option<RepairKind> {
        Some(match self.nodes.get(usize::try_from(index).ok()?)?.origin {
            Stage3Origin::Existing { index } => return self.stage2.kind(index),
            Stage3Origin::GeneratedRow => RepairKind::Box(Display::TableRow),
            Stage3Origin::GeneratedTable(kind) => RepairKind::Box(kind.display()),
        })
    }
    pub fn retained_bytes(&self) -> usize {
        self.nodes.capacity() * size_of::<Stage3Node>()
            + self.previous_mapping.capacity() * size_of::<u32>()
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
}
#[derive(Clone, Copy, Default)]
struct Runs {
    table: Option<u32>,
    row: Option<u32>,
}
fn role(kind: RepairKind) -> Display {
    match kind {
        RepairKind::Box(role) => role,
        RepairKind::AnonymousWhitespace | RepairKind::OutOfFlowPlaceholder => Display::Inline,
    }
}
fn group(role: Display) -> bool {
    matches!(
        role,
        Display::TableRowGroup | Display::TableHeaderGroup | Display::TableFooterGroup
    )
}
fn misparented(parent: Display, child: Display) -> bool {
    match child {
        Display::TableRow => {
            !(matches!(parent, Display::Table | Display::InlineTable) || group(parent))
        }
        Display::TableColumn => !matches!(
            parent,
            Display::Table | Display::InlineTable | Display::TableColumnGroup
        ),
        Display::TableRowGroup
        | Display::TableHeaderGroup
        | Display::TableFooterGroup
        | Display::TableColumnGroup
        | Display::TableCaption => !matches!(parent, Display::Table | Display::InlineTable),
        _ => false,
    }
}
/// Identical grouping transitions assign virtual indices and then emit real nodes.
fn append(
    parent_role: Display,
    parent: Option<u32>,
    child: Display,
    existing: u32,
    runs: &mut Runs,
    emit: &mut impl FnMut(Option<u32>, Stage3Origin) -> Result<u32, Stage3Error>,
) -> Result<u32, Stage3Error> {
    let missing_row = child == Display::TableCell && parent_role != Display::TableRow;
    let effective = if missing_row {
        Display::TableRow
    } else {
        child
    };
    let parent = if misparented(parent_role, effective) {
        let table = if let Some(table) = runs.table {
            table
        } else {
            let kind = if parent_role == Display::Inline {
                TableWrapperKind::InlineTable
            } else {
                TableWrapperKind::Table
            };
            let table = emit(parent, Stage3Origin::GeneratedTable(kind))?;
            runs.table = Some(table);
            runs.row = None;
            table
        };
        Some(table)
    } else {
        // Admitted Stage2 has no mixed valid/misparented proper-child run.
        // Nonqualifiers (including surviving whitespace) also terminate both runs.
        *runs = Runs::default();
        parent
    };
    let parent = if missing_row {
        let row = if let Some(row) = runs.row {
            row
        } else {
            let row = emit(parent, Stage3Origin::GeneratedRow)?;
            runs.row = Some(row);
            row
        };
        Some(row)
    } else {
        runs.row = None;
        parent
    };
    emit(parent, Stage3Origin::Existing { index: existing })
}
struct Work(usize);
impl Work {
    fn charge(&mut self, n: usize) -> Result<(), Stage3Error> {
        self.0 = self.0.checked_sub(n).ok_or(Stage3Error::Work)?;
        Ok(())
    }
}
fn bytes<T>(count: usize) -> Result<usize, Stage3Error> {
    count.checked_mul(size_of::<T>()).ok_or(Stage3Error::Bytes)
}
fn reserve<T>(count: usize, used: &mut usize, limit: usize) -> Result<Vec<T>, Stage3Error> {
    if used
        .checked_add(bytes::<T>(count)?)
        .is_none_or(|n| n > limit)
    {
        return Err(Stage3Error::Bytes);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| Stage3Error::Allocation)?;
    *used = used
        .checked_add(bytes::<T>(output.capacity())?)
        .ok_or(Stage3Error::Bytes)?;
    if *used > limit {
        return Err(Stage3Error::Bytes);
    }
    Ok(output)
}

/// Wrap missing parents after Stage2 using actual formatting parent roles.
///
/// Inherits the complete-child-list, surviving-external-parent and certified
/// whitespace obligations of Stage1/Stage2. The private owner chain has no
/// surviving whitespace bridges between internal/caption siblings and no mixed
/// correctly/misparented proper-child runs. Thus nonqualifiers end positive runs.
/// This is not a generic forest repair API. Placeholders stay non-whitespace
/// Inline participants; real containing blocks remain caller-owned.
///
/// Conceptually insert missing Rows, then Tables/InlineTables. Shared streaming
/// transitions emit the outer table before a new row, preserving that final
/// topology without intermediate trees. Per-Stage2-parent state (including the
/// external slot) preserves runs across interleaved descendant storage.
///
/// For n>0 and exact output count m, work is 5*n+3+2*m: admission one, state
/// initialization n+1, count visits n and virtual emissions m, reset n+1, mapping
/// initialization n, actual visits n and emissions m. The final traversals are
/// prepaid before emission. Empty input costs one and allocates nothing. Mapping
/// lookup composes original indices on demand in constant time; no original-size
/// scan or allocation occurs. Count/index arithmetic and limits are checked.
///
/// Bytes cover combined actual capacities of run state, mapping and exact output;
/// scratch is released before return. Borrowed prior-owner memory and allocator-
/// internal transient overlap are excluded. Each vector reserves once fallibly;
/// requested totals and actual capacities are checked, and emission cannot grow.
/// Source styles, anonymous styles, span defaults and geometry are not produced.
///
/// # Errors
/// Rejects count/index, byte, work and allocation limits without publishing a
/// partial result or changing the borrowed Stage2/Stage1/source inputs.
pub fn wrap_table_parents<'stage2, 'stage1, 'input>(
    stage2: &'stage2 Stage2<'stage1, 'input>,
    limits: Stage3Limits,
) -> Result<Stage3<'stage2, 'stage1, 'input>, Stage3Error> {
    let input = stage2.nodes();
    let n = input.len();
    if n > limits.input_nodes.get() || u32::try_from(n).is_err() {
        return Err(Stage3Error::InputNodes);
    }
    let mut work = Work(limits.work.get());
    work.charge(1)?;
    if n == 0 {
        return Ok(Stage3 {
            stage2,
            nodes: Vec::new(),
            previous_mapping: Vec::new(),
            peak_retained_bytes: 0,
        });
    }
    let slots = n.checked_add(1).ok_or(Stage3Error::Bytes)?;
    work.charge(slots)?;
    let limit = limits.retained_bytes.get();
    if limit > isize::MAX as usize {
        return Err(Stage3Error::Bytes);
    }
    let mut used = 0;
    let mut runs = reserve::<Runs>(slots, &mut used, limit)?;
    runs.resize(slots, Runs::default());
    let mut count = 0usize;
    for (index, node) in input.iter().enumerate() {
        work.charge(1)?;
        let parent_role = node.parent.map_or(stage2.external_parent(), |p| {
            role(stage2.kind(p).expect("Stage2 parent kind"))
        });
        let child = role(stage2.kind(index as u32).expect("Stage2 node kind"));
        let slot = node.parent.map_or(n, |p| p as usize);
        append(
            parent_role,
            None,
            child,
            index as u32,
            &mut runs[slot],
            &mut |_, _| {
                work.charge(1)?;
                let next = count.checked_add(1).ok_or(Stage3Error::OutputNodes)?;
                if next > limits.output_nodes.get() || u32::try_from(next).is_err() {
                    return Err(Stage3Error::OutputNodes);
                }
                let index = count as u32;
                count = next;
                Ok(index)
            },
        )?;
    }
    work.charge(
        slots
            .checked_add(n)
            .and_then(|v| v.checked_add(n))
            .and_then(|v| v.checked_add(count))
            .ok_or(Stage3Error::Work)?,
    )?;
    let requested = used
        .checked_add(bytes::<u32>(n)?)
        .and_then(|v| v.checked_add(bytes::<Stage3Node>(count).ok()?))
        .ok_or(Stage3Error::Bytes)?;
    if requested > limit {
        return Err(Stage3Error::Bytes);
    }
    let mut mapping = reserve::<u32>(n, &mut used, limit)?;
    let mut nodes = reserve::<Stage3Node>(count, &mut used, limit)?;
    runs.fill(Runs::default());
    mapping.resize(n, 0);
    for (index, node) in input.iter().enumerate() {
        let parent_role = node.parent.map_or(stage2.external_parent(), |p| {
            role(stage2.kind(p).expect("Stage2 parent kind"))
        });
        let child = role(stage2.kind(index as u32).expect("Stage2 node kind"));
        // Stage2 parents precede children, so the parent's entry was assigned.
        let parent = node.parent.map(|p| mapping[p as usize]);
        let slot = node.parent.map_or(n, |p| p as usize);
        mapping[index] = append(
            parent_role,
            parent,
            child,
            index as u32,
            &mut runs[slot],
            &mut |parent, origin| {
                if nodes.len() >= count {
                    return Err(Stage3Error::OutputNodes);
                }
                let index = nodes.len() as u32;
                nodes.push(Stage3Node { parent, origin });
                Ok(index)
            },
        )?;
    }
    debug_assert_eq!(nodes.len(), count);
    drop(runs);
    Ok(Stage3 {
        stage2,
        nodes,
        previous_mapping: mapping,
        peak_retained_bytes: used,
    })
}
