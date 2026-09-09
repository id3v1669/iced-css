//! CSS22 16.6.1 step4, after newline preprocessing and LF rendering conversion.
use super::{line_edges::WhiteSpace, unicode::is_bidi_control};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceUnit {
    Scalar(char),
    Atomic,
    /// An earlier stage removed this source entry; adjacency passes through it.
    Removed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceItem {
    pub unit: SpaceUnit,
    pub white_space: WhiteSpace,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceAction {
    Unchanged,
    TabToSpace,
    RemovedAdjacentSpace,
}
#[derive(Debug, Clone, Copy)]
pub struct SpaceCollapseLimits {
    pub items: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    /// One prepaid input-item pass; Unicode lookup is constant work.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceCollapseError {
    Items,
    Bytes,
    Work,
    Allocation,
    /// CR or a normal/nowrap LF survived the mandatory preceding stages.
    StageOrder {
        item: usize,
    },
}
impl std::fmt::Display for SpaceCollapseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "adjacent-space collapse: {self:?}")
    }
}
impl std::error::Error for SpaceCollapseError {}
/// Convert collapsible tabs and remove later spaces in compatible sequences.
/// Element boundaries do not interrupt this CSS step. Original entry positions
/// and earlier-removed/control entries retain their one-to-one action mapping.
/// Preserve pre-collapse break metadata externally; this does not determine breaks.
///
/// # Errors
/// Rejects admission/allocation failures and unapplied preceding newline stages.
/// No partial action vector escapes on a late invalid item.
pub fn collapse_adjacent_spaces(
    items: &[SpaceItem],
    limits: SpaceCollapseLimits,
) -> Result<Vec<SpaceAction>, SpaceCollapseError> {
    if items.len() > limits.items.get() {
        return Err(SpaceCollapseError::Items);
    }
    if items.len() > limits.work.get() {
        return Err(SpaceCollapseError::Work);
    }
    if limits.retained_bytes.get() > isize::MAX as usize
        || items
            .len()
            .checked_mul(size_of::<SpaceAction>())
            .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(SpaceCollapseError::Bytes);
    }
    let mut actions = Vec::new();
    actions
        .try_reserve_exact(items.len())
        .map_err(|_| SpaceCollapseError::Allocation)?;
    if actions
        .capacity()
        .checked_mul(size_of::<SpaceAction>())
        .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(SpaceCollapseError::Bytes);
    }
    let mut previous_collapsible = false;
    for (index, item) in items.iter().enumerate() {
        let action = match item.unit {
            SpaceUnit::Removed => SpaceAction::Unchanged,
            SpaceUnit::Atomic => {
                previous_collapsible = false;
                SpaceAction::Unchanged
            }
            SpaceUnit::Scalar(scalar) => {
                if scalar == '\r' || scalar == '\n' && !item.white_space.preserves_newlines() {
                    return Err(SpaceCollapseError::StageOrder { item: index });
                }
                if is_bidi_control(scalar) {
                    SpaceAction::Unchanged
                } else if matches!(scalar, ' ' | '\t') && item.white_space.collapses_spaces() {
                    let action = if previous_collapsible {
                        SpaceAction::RemovedAdjacentSpace
                    } else if scalar == '\t' {
                        SpaceAction::TabToSpace
                    } else {
                        SpaceAction::Unchanged
                    };
                    previous_collapsible = true;
                    action
                } else {
                    previous_collapsible = false;
                    SpaceAction::Unchanged
                }
            }
        };
        actions.push(action);
    }
    Ok(actions)
}
