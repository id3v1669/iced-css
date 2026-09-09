//! CSS22 17.2.1 missing child wrappers over an admitted Stage1 forest.
use super::{
    display::Display,
    table_repair_stage1::{Disposition, RepairKind, Stage1},
};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowWrapperReason {
    TableChildren,
    GroupChildren,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage2Origin {
    /// Original Stage1 input index, never an output-local or source instance ID.
    Original {
        index: u32,
    },
    GeneratedRow(RowWrapperReason),
    GeneratedCell,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stage2Node {
    /// Output-local parent; None means the unchanged external parent context.
    pub parent: Option<u32>,
    pub origin: Stage2Origin,
}
#[derive(Debug, Clone, Copy)]
pub struct Stage2Limits {
    pub input_nodes: NonZeroUsize,
    /// Exact generated-plus-original output count, also bounded by u32::MAX.
    pub output_nodes: NonZeroUsize,
    /// Combined actual run-state, mapping and node capacities at peak overlap.
    pub retained_bytes: NonZeroUsize,
    /// 5*n+3+2*m entries for nonempty input; one for empty input.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage2Error {
    InputNodes,
    OutputNodes,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for Stage2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table repair stage 2: {self:?}")
    }
}
impl std::error::Error for Stage2Error {}

/// Structural child repair borrowing the exact Stage1 owner, not just its input.
/// Original source styles and real out-of-flow content remain caller-owned.
///
/// ```compile_fail
/// use iced_css_core::formatting::{display::Display, table_repair_stage1::*, table_repair_stage2::*};
/// use std::num::NonZeroUsize;
/// let n = NonZeroUsize::new(1000).unwrap();
/// let input = [RepairNode { parent: None, kind: RepairKind::Box(Display::Inline) }];
/// let result;
/// {
///     let first = suppress_table_boxes(Display::Table, &input,
///         RepairLimits { nodes: n, retained_bytes: n, work: n }).unwrap();
///     result = wrap_table_children(&first,
///         Stage2Limits { input_nodes: n, output_nodes: n, retained_bytes: n, work: n }).unwrap();
/// }
/// assert_eq!(result.nodes().len(), 3);
/// ```
#[derive(Debug)]
pub struct Stage2<'stage1, 'input> {
    stage1: &'stage1 Stage1<'input>,
    nodes: Vec<Stage2Node>,
    original_mapping: Vec<Option<u32>>,
    peak_retained_bytes: usize,
}
impl<'stage1, 'input> Stage2<'stage1, 'input> {
    pub fn stage1(&self) -> &'stage1 Stage1<'input> {
        self.stage1
    }
    pub fn external_parent(&self) -> Display {
        self.stage1.external_parent()
    }
    /// Parents precede children; increasing indices order siblings, not a DFS.
    pub fn nodes(&self) -> &[Stage2Node] {
        &self.nodes
    }
    /// One entry per original input; suppressed originals have no output index.
    pub fn original_mapping(&self) -> &[Option<u32>] {
        &self.original_mapping
    }
    pub fn kind(&self, index: u32) -> Option<RepairKind> {
        let node = self.nodes.get(usize::try_from(index).ok()?)?;
        Some(match node.origin {
            Stage2Origin::Original { index } => self.stage1.input().get(index as usize)?.kind,
            Stage2Origin::GeneratedRow(_) => RepairKind::Box(Display::TableRow),
            Stage2Origin::GeneratedCell => RepairKind::Box(Display::TableCell),
        })
    }
    pub fn retained_bytes(&self) -> usize {
        self.nodes.capacity() * size_of::<Stage2Node>()
            + self.original_mapping.capacity() * size_of::<Option<u32>>()
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
}

#[derive(Clone, Copy, Default)]
struct Runs {
    row: Option<u32>,
    cell: Option<u32>,
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
fn proper(role: Display) -> bool {
    matches!(
        role,
        Display::TableRow
            | Display::TableColumn
            | Display::TableColumnGroup
            | Display::TableCaption
    ) || group(role)
}

/// The same transitions assign virtual indices during counting and real indices
/// during emission. Original-parent output indices do not influence grouping.
fn append(
    parent_role: Display,
    parent: Option<u32>,
    child: Display,
    original: u32,
    runs: &mut Runs,
    emit: &mut impl FnMut(Option<u32>, Stage2Origin) -> Result<u32, Stage2Error>,
) -> Result<u32, Stage2Error> {
    let row_reason = if matches!(parent_role, Display::Table | Display::InlineTable) {
        (!proper(child)).then_some(RowWrapperReason::TableChildren)
    } else if group(parent_role) {
        (child != Display::TableRow).then_some(RowWrapperReason::GroupChildren)
    } else {
        None
    };
    let row_parent = if let Some(reason) = row_reason {
        let index = if let Some(index) = runs.row {
            index
        } else {
            let index = emit(parent, Stage2Origin::GeneratedRow(reason))?;
            runs.row = Some(index);
            runs.cell = None;
            index
        };
        Some(Some(index))
    } else if parent_role == Display::TableRow {
        Some(parent)
    } else {
        *runs = Runs::default();
        None
    };
    let parent = if let Some(row) = row_parent {
        if child == Display::TableCell {
            runs.cell = None;
            row
        } else {
            let index = if let Some(index) = runs.cell {
                index
            } else {
                let index = emit(row, Stage2Origin::GeneratedCell)?;
                runs.cell = Some(index);
                index
            };
            Some(index)
        }
    } else {
        parent
    };
    emit(parent, Stage2Origin::Original { index: original })
}
struct Work(usize);
impl Work {
    fn charge(&mut self, amount: usize) -> Result<(), Stage2Error> {
        self.0 = self.0.checked_sub(amount).ok_or(Stage2Error::Work)?;
        Ok(())
    }
}
fn bytes<T>(count: usize) -> Result<usize, Stage2Error> {
    count.checked_mul(size_of::<T>()).ok_or(Stage2Error::Bytes)
}
fn reserve<T>(count: usize, used: &mut usize, limit: usize) -> Result<Vec<T>, Stage2Error> {
    if used
        .checked_add(bytes::<T>(count)?)
        .is_none_or(|bytes| bytes > limit)
    {
        return Err(Stage2Error::Bytes);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| Stage2Error::Allocation)?;
    *used = used
        .checked_add(bytes::<T>(output.capacity())?)
        .ok_or(Stage2Error::Bytes)?;
    if *used > limit {
        return Err(Stage2Error::Bytes);
    }
    Ok(output)
}

/// Wrap missing children after Stage1 suppression, including children of new rows.
///
/// Inherits Stage1's complete-child-list, surviving external-parent, whitespace
/// certification and placeholder prerequisites. Surviving originals retain their
/// exact input kind and index; suppressed descendants are never spliced upward.
/// Generated wrappers affect formatting ancestry only. This does not rerun Stage1,
/// insert Stage3 table parents, compute anonymous styles or assign cell spans.
///
/// Count and emission share run transitions with independent per-original-parent
/// state, including the external slot. Each output is counted before the single
/// exact output reservation; no conservative 3*s output admission is imposed.
/// Nonempty input charges 5*n+3+2*m entries: one admission, n+1 state initializations,
/// n count visits and m virtual outputs, n+1 resets, n mapping initializations,
/// n emission visits and m actual outputs. The last four traversals are prepaid.
/// Empty input uses one entry and allocates nothing. Byte limits include combined
/// actual capacities of state, map and nodes, excluding borrowed Stage1/input and
/// allocator-internal transient storage. State is released before return.
///
/// # Errors
/// Rejects count/index, work, byte and allocation limits without publishing a
/// partial owner or changing borrowed input. All allocation is fallible.
pub fn wrap_table_children<'stage1, 'input>(
    stage1: &'stage1 Stage1<'input>,
    limits: Stage2Limits,
) -> Result<Stage2<'stage1, 'input>, Stage2Error> {
    let input = stage1.input();
    let n = input.len();
    if n > limits.input_nodes.get() || u32::try_from(n).is_err() {
        return Err(Stage2Error::InputNodes);
    }
    let mut work = Work(limits.work.get());
    work.charge(1)?;
    if n == 0 {
        return Ok(Stage2 {
            stage1,
            nodes: Vec::new(),
            original_mapping: Vec::new(),
            peak_retained_bytes: 0,
        });
    }
    let slots = n.checked_add(1).ok_or(Stage2Error::Bytes)?;
    work.charge(slots)?;
    let limit = limits.retained_bytes.get();
    if limit > isize::MAX as usize {
        return Err(Stage2Error::Bytes);
    }
    let mut used = 0;
    let mut runs = reserve::<Runs>(slots, &mut used, limit)?;
    runs.resize(slots, Runs::default());
    let mut count = 0usize;
    for (index, node) in input.iter().enumerate() {
        work.charge(1)?;
        if stage1.dispositions()[index] != Disposition::Survives {
            continue;
        }
        let parent_role = node.parent.map_or(stage1.external_parent(), |parent| {
            role(input[parent as usize].kind)
        });
        let slot = node.parent.map_or(n, |parent| parent as usize);
        append(
            parent_role,
            None,
            role(node.kind),
            index as u32,
            &mut runs[slot],
            &mut |_, _| {
                work.charge(1)?;
                let next = count.checked_add(1).ok_or(Stage2Error::OutputNodes)?;
                if next > limits.output_nodes.get() || u32::try_from(next).is_err() {
                    return Err(Stage2Error::OutputNodes);
                }
                let index = count as u32;
                count = next;
                Ok(index)
            },
        )?;
    }
    // Reset + mapping initialization + input visits + actual emissions.
    work.charge(
        slots
            .checked_add(n)
            .and_then(|v| v.checked_add(n))
            .and_then(|v| v.checked_add(count))
            .ok_or(Stage2Error::Work)?,
    )?;
    let requested = used
        .checked_add(bytes::<Option<u32>>(n)?)
        .and_then(|v| v.checked_add(bytes::<Stage2Node>(count).ok()?))
        .ok_or(Stage2Error::Bytes)?;
    if requested > limit {
        return Err(Stage2Error::Bytes);
    }
    let mut mapping = reserve::<Option<u32>>(n, &mut used, limit)?;
    let mut nodes = reserve::<Stage2Node>(count, &mut used, limit)?;
    runs.fill(Runs::default());
    mapping.resize(n, None);
    for (index, node) in input.iter().enumerate() {
        if stage1.dispositions()[index] != Disposition::Survives {
            continue;
        }
        let parent_role = node.parent.map_or(stage1.external_parent(), |parent| {
            role(input[parent as usize].kind)
        });
        let parent = node.parent.map(|parent| {
            // Stage1 guarantees parent-before-child and ancestor suppression.
            mapping[parent as usize].expect("surviving parent was emitted")
        });
        let slot = node.parent.map_or(n, |parent| parent as usize);
        let output = append(
            parent_role,
            parent,
            role(node.kind),
            index as u32,
            &mut runs[slot],
            &mut |parent, origin| {
                // Shared transitions guarantee the count; guard against accidental
                // future divergence before a push could grow the admitted vector.
                if nodes.len() >= count {
                    return Err(Stage2Error::OutputNodes);
                }
                let index = nodes.len() as u32;
                nodes.push(Stage2Node { parent, origin });
                Ok(index)
            },
        )?;
        mapping[index] = Some(output);
    }
    debug_assert_eq!(nodes.len(), count);
    drop(runs);
    Ok(Stage2 {
        stage1,
        nodes,
        original_mapping: mapping,
        peak_retained_bytes: used,
    })
}
