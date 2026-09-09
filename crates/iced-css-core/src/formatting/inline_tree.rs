//! Parent-before-child inline forest, partitioned into CSS aligned subtrees.
use super::*;

#[derive(Debug, Clone, Copy)]
pub struct InlineNode {
    pub parent: Option<u32>,
    pub bounds: InlineBounds,
    pub alignment: LineAlignment,
}
#[derive(Debug, Clone, Copy)]
pub struct InlineTreeLimits {
    pub nodes: NonZeroUsize,
    /// Combined actual capacities of metadata, aligned groups and output.
    pub retained_bytes: NonZeroUsize,
    /// At most three node passes plus the parent strut, prepaid.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineTreeError {
    Nodes,
    Parent { node: u32, parent: u32 },
    Bytes,
    Work,
    Allocation,
    Geometry(LineGeometryError),
}
impl std::fmt::Display for InlineTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "inline tree geometry: {self:?}")
    }
}
impl std::error::Error for InlineTreeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Geometry(error) => Some(error),
            _ => None,
        }
    }
}
impl From<LineGeometryError> for InlineTreeError {
    fn from(error: LineGeometryError) -> Self {
        Self::Geometry(error)
    }
}
#[derive(Clone, Copy)]
struct Local {
    group: usize,
    origin: Exact,
}
fn reserve<T>(count: usize, bytes: &mut usize, limit: usize) -> Result<Vec<T>, InlineTreeError> {
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(InlineTreeError::Bytes)?;
    if bytes.checked_add(requested).is_none_or(|n| n > limit) {
        return Err(InlineTreeError::Bytes);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| InlineTreeError::Allocation)?;
    *bytes = result
        .capacity()
        .checked_mul(size_of::<T>())
        .and_then(|n| bytes.checked_add(n))
        .ok_or(InlineTreeError::Bytes)?;
    if *bytes > limit {
        return Err(InlineTreeError::Bytes);
    }
    Ok(result)
}
/// Assemble already resolved inline participants into aligned subtrees.
/// Every parent must precede its child; `None` denotes the line-level parent.
/// Independently top/bottom-aligned descendants start separate groups. Numeric
/// shifts accumulate through other parents without intermediate f32 rounding.
/// This does not perform shaping, horizontal breaking or phantom-line removal.
///
/// # Errors
/// Rejects invalid parents, nonfinite numeric raises, exhausted limits,
/// allocation failures and unrepresentable final geometry. No partial result escapes.
pub fn layout_inline_tree(
    strut: InlineBounds,
    nodes: &[InlineNode],
    limits: InlineTreeLimits,
) -> Result<LineGeometry, InlineTreeError> {
    if nodes.len() > limits.nodes.get() || u32::try_from(nodes.len()).is_err() {
        return Err(InlineTreeError::Nodes);
    }
    if nodes
        .len()
        .checked_mul(3)
        .and_then(|n| n.checked_add(1))
        .is_none_or(|n| n > limits.work.get())
    {
        return Err(InlineTreeError::Work);
    }
    if limits.retained_bytes.get() > isize::MAX as usize {
        return Err(InlineTreeError::Bytes);
    }
    let mut bytes = 0;
    let mut locals: Vec<Local> = reserve(nodes.len(), &mut bytes, limits.retained_bytes.get())?;
    let group_count = nodes.len().checked_add(1).ok_or(InlineTreeError::Bytes)?;
    let mut groups: Vec<LineParticipant> =
        reserve(group_count, &mut bytes, limits.retained_bytes.get())?;
    let mut placements = reserve(nodes.len(), &mut bytes, limits.retained_bytes.get())?;
    groups.push(LineParticipant {
        bounds: strut,
        alignment: LineAlignment::Baseline { raise: 0.0 },
    });
    for (index, node) in nodes.iter().enumerate() {
        let parent = match node.parent {
            Some(parent) => {
                let parent_index = usize::try_from(parent).map_err(|_| InlineTreeError::Nodes)?;
                if parent_index >= index {
                    return Err(InlineTreeError::Parent {
                        node: u32::try_from(index).map_err(|_| InlineTreeError::Nodes)?,
                        parent,
                    });
                }
                locals[parent_index]
            }
            None => Local {
                group: 0,
                origin: Exact::ZERO,
            },
        };
        let raise = match node.alignment {
            LineAlignment::Baseline { raise } => {
                if !raise.is_finite() {
                    return Err(LineGeometryError::Alignment.into());
                }
                Some(Exact::from_f32(raise))
            }
            LineAlignment::Resolved(ResolvedRaise(raise)) => Some(raise),
            LineAlignment::Top | LineAlignment::Bottom => None,
        };
        let local = if let Some(raise) = raise {
            let origin = parent.origin.sub(raise);
            let bounds = &mut groups[parent.group].bounds;
            bounds.top = bounds.top.min(node.bounds.top.add(origin));
            bounds.bottom = bounds.bottom.max(node.bounds.bottom.add(origin));
            Local {
                group: parent.group,
                origin,
            }
        } else {
            let group = groups.len();
            groups.push(LineParticipant {
                bounds: node.bounds,
                alignment: node.alignment,
            });
            Local {
                group,
                origin: Exact::ZERO,
            }
        };
        locals.push(local);
    }
    let (height, baseline) = exact_extents(strut, &groups)?;
    let rounded_height = rounded(height)?;
    let rounded_baseline = rounded(baseline)?;
    for (node, local) in nodes.iter().zip(locals) {
        let origin = exact_origin(groups[local.group], height, baseline).add(local.origin);
        placements.push(LinePlacement {
            top: rounded(node.bounds.top.add(origin))?,
            bottom: rounded(node.bounds.bottom.add(origin))?,
            origin: rounded(origin)?,
        });
    }
    Ok(LineGeometry {
        height: rounded_height,
        baseline: rounded_baseline,
        placements,
    })
}
