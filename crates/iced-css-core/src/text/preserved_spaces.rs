//! CSS22 16.6.1 step2 in one explicit inline scope.
use super::{line_edges::WhiteSpace, newlines::NormalizedText, unicode::is_bidi_control};
use std::{mem::size_of, num::NonZeroUsize};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreservedSpace {
    Ordinary,
    /// Rendering treatment; the original scalar remains U+0020.
    NonBreaking {
        sequence_end_opportunity: bool,
    },
}
#[derive(Debug, Clone, Copy)]
pub struct PreservedSpaceLimits {
    pub scalars: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    /// Initialization plus one reverse scalar pass, prepaid.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreservedSpaceError {
    Scalars,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for PreservedSpaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "preserved-space sequences: {self:?}")
    }
}
impl std::error::Error for PreservedSpaceError {}
/// Mark nonbreaking spaces and pre-wrap sequence-end opportunities.
/// Input belongs to one uniform-style inline scope; do not join across element
/// boundaries. Bidi controls remain mapped ordinary entries and are transparent
/// to sequence membership. This does not perform Unicode line breaking/reordering.
///
/// # Errors
/// Rejects admission/allocation failures without a partial metadata vector.
pub fn preserved_space_sequences(
    text: &NormalizedText<'_>,
    mode: WhiteSpace,
    limits: PreservedSpaceLimits,
) -> Result<Vec<PreservedSpace>, PreservedSpaceError> {
    let count = text.scalars().len();
    if count > limits.scalars.get() {
        return Err(PreservedSpaceError::Scalars);
    }
    if count.checked_mul(2).is_none_or(|n| n > limits.work.get()) {
        return Err(PreservedSpaceError::Work);
    }
    if limits.retained_bytes.get() > isize::MAX as usize
        || count
            .checked_mul(size_of::<PreservedSpace>())
            .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(PreservedSpaceError::Bytes);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| PreservedSpaceError::Allocation)?;
    if result
        .capacity()
        .checked_mul(size_of::<PreservedSpace>())
        .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(PreservedSpaceError::Bytes);
    }
    result.resize(count, PreservedSpace::Ordinary);
    if matches!(mode, WhiteSpace::Pre | WhiteSpace::PreWrap) {
        let mut followed_by_space = false;
        for (scalar, metadata) in text.text().chars().rev().zip(result.iter_mut().rev()) {
            if is_bidi_control(scalar) {
                continue;
            }
            if scalar == ' ' {
                *metadata = PreservedSpace::NonBreaking {
                    sequence_end_opportunity: mode == WhiteSpace::PreWrap && !followed_by_space,
                };
                followed_by_space = true;
            } else {
                followed_by_space = false;
            }
        }
    }
    Ok(result)
}
