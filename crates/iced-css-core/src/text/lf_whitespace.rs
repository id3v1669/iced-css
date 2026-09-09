//! CSS22 16.6.1 step1 in one explicit uniform-style logical inline scope.
use super::{line_edges::WhiteSpace, newlines::NormalizedText, unicode::is_bidi_control};
use std::{mem::size_of, num::NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfWhitespaceAction {
    Keep,
    Remove,
}
#[derive(Debug, Clone, Copy)]
pub struct LfWhitespaceLimits {
    pub scalars: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    /// Initialization and two directional scalar scans, prepaid.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfWhitespaceError {
    Scalars,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for LfWhitespaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "whitespace surrounding LF: {self:?}")
    }
}
impl std::error::Error for LfWhitespaceError {}
fn mark<'a>(items: impl Iterator<Item = (char, &'a mut LfWhitespaceAction)>) {
    let mut adjacent_lf = false;
    for (scalar, action) in items {
        if is_bidi_control(scalar) {
            continue;
        }
        match scalar {
            '\n' => adjacent_lf = true,
            ' ' | '\t' => {
                if adjacent_lf {
                    *action = LfWhitespaceAction::Remove;
                }
            }
            _ => adjacent_lf = false,
        }
    }
}
/// Mark spaces/tabs surrounding preserved LF, without deleting source mappings.
/// Input uses default newline normalization and one caller-established uniform
/// inline scope. Cross-element scope decisions and LF rendering conversion are
/// separate. Pre/pre-wrap retain every scalar; bidi controls always remain.
///
/// # Errors
/// Rejects admission/allocation failures without a partial action map.
pub fn remove_whitespace_around_lf(
    text: &NormalizedText<'_>,
    mode: WhiteSpace,
    limits: LfWhitespaceLimits,
) -> Result<Vec<LfWhitespaceAction>, LfWhitespaceError> {
    let count = text.scalars().len();
    if count > limits.scalars.get() {
        return Err(LfWhitespaceError::Scalars);
    }
    if count.checked_mul(3).is_none_or(|n| n > limits.work.get()) {
        return Err(LfWhitespaceError::Work);
    }
    if limits.retained_bytes.get() > isize::MAX as usize
        || count
            .checked_mul(size_of::<LfWhitespaceAction>())
            .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(LfWhitespaceError::Bytes);
    }
    let mut actions = Vec::new();
    actions
        .try_reserve_exact(count)
        .map_err(|_| LfWhitespaceError::Allocation)?;
    if actions
        .capacity()
        .checked_mul(size_of::<LfWhitespaceAction>())
        .is_none_or(|n| n > limits.retained_bytes.get())
    {
        return Err(LfWhitespaceError::Bytes);
    }
    actions.resize(count, LfWhitespaceAction::Keep);
    if mode.collapses_spaces() {
        mark(text.text().chars().zip(&mut actions));
        mark(text.text().chars().rev().zip(actions.iter_mut().rev()));
    }
    Ok(actions)
}
