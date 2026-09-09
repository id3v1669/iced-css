//! Restricted CSS22 17.2.1 stage-1 suppression; no source projection or wrappers.
use super::display::Display;
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairKind {
    Box(Display),
    /// Caller-certified anonymous whitespace inline, admitted only as a leaf.
    AnonymousWhitespace,
    /// Zero-size inline stand-in; actual out-of-flow content remains caller-owned.
    OutOfFlowPlaceholder,
}
impl RepairKind {
    fn display(self) -> Display {
        match self {
            Self::Box(display) => display,
            Self::AnonymousWhitespace | Self::OutOfFlowPlaceholder => Display::Inline,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairNode {
    pub parent: Option<u32>,
    pub kind: RepairKind,
}
#[derive(Debug, Clone, Copy)]
pub struct RepairLimits {
    pub nodes: NonZeroUsize,
    /// Combined actual capacities of scratch and output at their peak overlap.
    pub retained_bytes: NonZeroUsize,
    /// Exactly 4*n+1 prepaid logical entries; see `suppress_table_boxes`.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Survives,
    ColumnChild,
    NonColumnGroupChild,
    TabularWhitespace,
    BetweenInternalWhitespace,
    /// First directly suppressed ancestor; descendants are never spliced upward.
    Ancestor {
        index: u32,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairError {
    Nodes,
    Bytes,
    Work,
    Allocation,
    ExternalNone,
    NoneDisplay { index: u32 },
    Parent { index: u32, parent: u32 },
    LeafParent { index: u32, parent: u32 },
    AdjacentWhitespace { previous: u32, index: u32 },
}
impl std::fmt::Display for RepairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table repair stage 1: {self:?}")
    }
}
impl std::error::Error for RepairError {}

/// A stage-1 result borrows its unchanged input; local indices are not source IDs.
///
/// ```compile_fail
/// use iced_css_core::formatting::{display::Display, table_repair_stage1::*};
/// use std::num::NonZeroUsize;
/// let result;
/// {
///     let input = vec![RepairNode { parent: None, kind: RepairKind::Box(Display::Block) }];
///     let n = NonZeroUsize::new(1000).unwrap();
///     result = suppress_table_boxes(Display::Block, &input,
///         RepairLimits { nodes: n, retained_bytes: n, work: n }).unwrap();
/// }
/// assert_eq!(result.input().len(), 1);
/// ```
#[derive(Debug)]
pub struct Stage1<'a> {
    input: &'a [RepairNode],
    external_parent: Display,
    dispositions: Vec<Disposition>,
    peak_retained_bytes: usize,
}
impl<'a> Stage1<'a> {
    pub fn input(&self) -> &'a [RepairNode] {
        self.input
    }
    pub fn external_parent(&self) -> Display {
        self.external_parent
    }
    pub fn dispositions(&self) -> &[Disposition] {
        &self.dispositions
    }
    pub fn retained_bytes(&self) -> usize {
        self.dispositions.capacity() * size_of::<Disposition>()
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.peak_retained_bytes
    }
}
#[derive(Clone, Copy, Default)]
struct Links {
    previous: Option<u32>,
    next: Option<u32>,
}
fn bytes<T>(count: usize) -> Result<usize, RepairError> {
    count.checked_mul(size_of::<T>()).ok_or(RepairError::Bytes)
}
fn reserve<T>(count: usize, used: &mut usize, limit: usize) -> Result<Vec<T>, RepairError> {
    if used
        .checked_add(bytes::<T>(count)?)
        .is_none_or(|sum| sum > limit)
    {
        return Err(RepairError::Bytes);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| RepairError::Allocation)?;
    *used = used
        .checked_add(bytes::<T>(result.capacity())?)
        .ok_or(RepairError::Bytes)?;
    if *used > limit {
        return Err(RepairError::Bytes);
    }
    Ok(result)
}
fn group(role: Display) -> bool {
    matches!(
        role,
        Display::TableRowGroup | Display::TableHeaderGroup | Display::TableFooterGroup
    )
}
fn internal_or_caption(role: Display) -> bool {
    matches!(
        role,
        Display::TableCaption
            | Display::TableCell
            | Display::TableRow
            | Display::TableColumn
            | Display::TableColumnGroup
    ) || group(role)
}
fn tabular(role: Display) -> bool {
    matches!(
        role,
        Display::Table | Display::InlineTable | Display::TableRow
    ) || group(role)
}
fn proper(parent: Display, child: Display) -> bool {
    internal_or_caption(child)
        && match parent {
            Display::Table | Display::InlineTable => true,
            Display::TableRow => child == Display::TableCell,
            _ if group(parent) => matches!(child, Display::TableRow | Display::TableCell),
            _ => false,
        }
}
fn direct(parent: Display, node: RepairNode, links: Links, input: &[RepairNode]) -> Disposition {
    if parent == Display::TableColumn {
        return Disposition::ColumnChild;
    }
    if parent == Display::TableColumnGroup && node.kind != RepairKind::Box(Display::TableColumn) {
        return Disposition::NonColumnGroupChild;
    }
    if node.kind != RepairKind::AnonymousWhitespace {
        return Disposition::Survives;
    }
    let previous = links.previous.map(|i| input[i as usize].kind.display());
    let next = links.next.map(|i| input[i as usize].kind.display());
    if tabular(parent)
        && previous.is_none_or(|role| proper(parent, role))
        && next.is_none_or(|role| proper(parent, role))
    {
        Disposition::TabularWhitespace
    } else if previous.is_some_and(internal_or_caption) && next.is_some_and(internal_or_caption) {
        Disposition::BetweenInternalWhitespace
    } else {
        Disposition::Survives
    }
}

/// Suppress irrelevant boxes over a complete projected child forest.
///
/// All roots are the COMPLETE ordered children of `external_parent`, and each
/// ordinary node has its complete projected child list. The caller establishes
/// that the external parent and its ancestors survive stage 1 (or handles their
/// suppression before calling). These completeness obligations cannot be checked
/// here: omitting an Inline sibling from [Inline, whitespace, Cell] under Table
/// would change the whitespace result. Input has already omitted display:none
/// subtrees and represented out-of-flow elements with non-whitespace placeholders.
/// Whitespace content classification and real containing blocks remain caller-owned.
///
/// Parents precede children; increasing input indices define sibling order but
/// need not be depth-first. Certified whitespace and placeholders are leaves.
/// Adjacent whitespace siblings are unsupported and rejected even inside a
/// suppressed subtree. Original siblings therefore suffice for stage-1 decisions.
///
/// Prepay 4*n+1 logical entries: initialize n+1 last-child slots and n sibling-link
/// records, validate/link n nodes, then emit n dispositions. Each entry does bounded
/// constant work. The byte limit covers combined actual Vec capacities for those
/// two scratch arrays plus output. All three requested capacities are preflighted;
/// each actual capacity is checked after fallible reservation. Allocator-internal
/// transient storage is excluded. Scratch is released before return.
///
/// # Errors
/// Rejects invalid topology/roles/leaves/adjacency and exhausted resource limits.
/// All input validation finishes before output construction; no partial result
/// escapes on any error. No source membership, input or computed style is mutated.
pub fn suppress_table_boxes(
    external_parent: Display,
    input: &[RepairNode],
    limits: RepairLimits,
) -> Result<Stage1<'_>, RepairError> {
    let count = input.len();
    if count > limits.nodes.get() || u32::try_from(count).is_err() {
        return Err(RepairError::Nodes);
    }
    if external_parent == Display::None {
        return Err(RepairError::ExternalNone);
    }
    if count
        .checked_mul(4)
        .and_then(|n| n.checked_add(1))
        .is_none_or(|n| n > limits.work.get())
    {
        return Err(RepairError::Work);
    }
    let slots = count.checked_add(1).ok_or(RepairError::Bytes)?;
    let requested = bytes::<Option<u32>>(slots)?
        .checked_add(bytes::<Links>(count)?)
        .and_then(|n| n.checked_add(bytes::<Disposition>(count).ok()?))
        .ok_or(RepairError::Bytes)?;
    let limit = limits.retained_bytes.get();
    if limit > isize::MAX as usize || requested > limit {
        return Err(RepairError::Bytes);
    }
    let mut used = 0;
    let mut last = reserve::<Option<u32>>(slots, &mut used, limit)?;
    last.resize(slots, None);
    let mut links = reserve::<Links>(count, &mut used, limit)?;
    links.resize(count, Links::default());
    for (position, node) in input.iter().enumerate() {
        // count was checked against u32::MAX before allocating.
        let index = position as u32;
        if node.kind == RepairKind::Box(Display::None) {
            return Err(RepairError::NoneDisplay { index });
        }
        let slot = match node.parent {
            Some(parent) => {
                if parent >= index {
                    return Err(RepairError::Parent { index, parent });
                }
                if !matches!(input[parent as usize].kind, RepairKind::Box(_)) {
                    return Err(RepairError::LeafParent { index, parent });
                }
                parent as usize
            }
            None => count,
        };
        if let Some(previous) = last[slot] {
            if node.kind == RepairKind::AnonymousWhitespace
                && input[previous as usize].kind == RepairKind::AnonymousWhitespace
            {
                return Err(RepairError::AdjacentWhitespace { previous, index });
            }
            links[previous as usize].next = Some(index);
            links[position].previous = Some(previous);
        }
        last[slot] = Some(index);
    }
    let mut dispositions = reserve::<Disposition>(count, &mut used, limit)?;
    for (index, node) in input.iter().copied().enumerate() {
        let inherited = node
            .parent
            .and_then(|parent| match dispositions[parent as usize] {
                Disposition::Survives => None,
                Disposition::Ancestor { index } => Some(index),
                _ => Some(parent),
            });
        dispositions.push(match inherited {
            Some(index) => Disposition::Ancestor { index },
            None => direct(
                node.parent
                    .map_or(external_parent, |p| input[p as usize].kind.display()),
                node,
                links[index],
                input,
            ),
        });
    }
    drop(last);
    drop(links);
    Ok(Stage1 {
        input,
        external_parent,
        dispositions,
        peak_retained_bytes: used,
    })
}
