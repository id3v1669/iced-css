//! Explicit-key matching of direct captured source children; no native traversal.
use super::Construction;
use iced_css_core::source::sibling_matches::{
    Generation, MatchError, MatchLimits, SiblingKey, SiblingMatches, match_siblings,
};
use std::num::NonZeroUsize;

/// The caller establishes that previous/incoming selections describe the same
/// persistent source parent. A construction position is not a source identity.
#[derive(Clone, Copy)]
pub struct CapturedParent<'a> {
    pub construction: &'a Construction,
    pub position: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct CapturedMatchLimits {
    /// Total scanned construction nodes, including nodes outside these parents.
    pub nodes: NonZeroUsize,
    /// Position vectors and temporary borrowed-key view capacities.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
    /// Separate budget for the existing key matcher and its result mapping.
    pub matching: MatchLimits,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapturedMatchError {
    Parent {
        generation: Generation,
        position: usize,
    },
    MissingKey {
        generation: Generation,
        position: usize,
    },
    Nodes,
    Bytes,
    Work,
    Allocation,
    Matching(MatchError),
}
impl std::fmt::Display for CapturedMatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "captured child matching: {self:?}")
    }
}
impl std::error::Error for CapturedMatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Matching(e) => Some(e),
            _ => None,
        }
    }
}
/// Source positions and sibling-index matches. Native adapter compatibility and
/// parent identity must still be established before preserving any live state.
#[derive(Debug)]
pub struct CapturedSiblingMatches {
    previous: Vec<usize>,
    incoming: Vec<usize>,
    matches: SiblingMatches,
}
impl CapturedSiblingMatches {
    pub fn previous_positions(&self) -> &[usize] {
        &self.previous
    }
    pub fn incoming_positions(&self) -> &[usize] {
        &self.incoming
    }
    /// Each index selects the previous sibling array, not a construction node.
    pub fn previous_indices(&self) -> &[Option<usize>] {
        self.matches.previous_indices()
    }
}
fn reserve<T>(count: usize, bytes: &mut usize) -> Result<Vec<T>, CapturedMatchError> {
    let needed = count
        .checked_mul(size_of::<T>())
        .ok_or(CapturedMatchError::Bytes)?;
    if needed > *bytes || needed > isize::MAX as usize {
        return Err(CapturedMatchError::Bytes);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| CapturedMatchError::Allocation)?;
    let actual = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(CapturedMatchError::Bytes)?;
    *bytes = bytes.checked_sub(actual).ok_or(CapturedMatchError::Bytes)?;
    Ok(values)
}
struct Children<'a> {
    positions: Vec<usize>,
    keys: Vec<SiblingKey<'a>>,
}
fn children<'a>(
    parent: CapturedParent<'a>,
    generation: Generation,
    bytes: &mut usize,
) -> Result<Children<'a>, CapturedMatchError> {
    let nodes = parent.construction.nodes();
    let count = nodes
        .iter()
        .filter(|node| node.parent() == Some(parent.position))
        .count();
    let mut positions = reserve(count, bytes)?;
    let mut keys = reserve(count, bytes)?;
    for (position, node) in nodes.iter().enumerate() {
        if node.parent() != Some(parent.position) {
            continue;
        }
        let key = node.source_key().ok_or(CapturedMatchError::MissingKey {
            generation,
            position,
        })?;
        positions.push(position);
        keys.push(SiblingKey::Explicit(key));
    }
    Ok(Children { positions, keys })
}
/// Match direct source children using their explicit owned key bytes. Missing keys
/// fail; generated static slots are not inferred from order or native topology.
/// Returned storage borrows neither construction and makes no source/state moves.
pub fn match_keyed_children(
    previous: CapturedParent<'_>,
    incoming: CapturedParent<'_>,
    limits: CapturedMatchLimits,
) -> Result<CapturedSiblingMatches, CapturedMatchError> {
    for (parent, generation) in [
        (previous, Generation::Previous),
        (incoming, Generation::Incoming),
    ] {
        if parent.position >= parent.construction.nodes().len() {
            return Err(CapturedMatchError::Parent {
                generation,
                position: parent.position,
            });
        }
    }
    let count = previous
        .construction
        .nodes()
        .len()
        .checked_add(incoming.construction.nodes().len())
        .ok_or(CapturedMatchError::Nodes)?;
    if count > limits.nodes.get() {
        return Err(CapturedMatchError::Nodes);
    }
    // Two complete scans and at most one pair of pushes per visited source, plus
    // parent validation. Key comparison work is charged by the existing matcher.
    let work = count
        .checked_mul(3)
        .and_then(|v| v.checked_add(2))
        .ok_or(CapturedMatchError::Work)?;
    if work > limits.work.get() {
        return Err(CapturedMatchError::Work);
    }
    let mut bytes = limits.retained_bytes.get();
    let previous = children(previous, Generation::Previous, &mut bytes)?;
    let incoming = children(incoming, Generation::Incoming, &mut bytes)?;
    let matches = match_siblings(&previous.keys, &incoming.keys, limits.matching)
        .map_err(CapturedMatchError::Matching)?;
    Ok(CapturedSiblingMatches {
        previous: previous.positions,
        incoming: incoming.positions,
        matches,
    })
}
