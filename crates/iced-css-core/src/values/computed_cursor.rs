//! Ordered computed cursor URLs with bounded storage and a validated fallback.
use super::property::{Keyword, ParsedProperty, SpecifiedValue, cursor_keyword_allowed};
use crate::resources::uri::{self, UriError};
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy)]
pub struct CursorComputationLimits {
    pub uris: NonZeroUsize,
    /// Combined URI vector and String capacities, excluding fixed inline fields.
    pub retained_bytes: NonZeroUsize,
    /// One list admission plus each entry and cumulative URI resolver work.
    pub work: NonZeroUsize,
}
#[derive(Debug)]
pub struct ComputedCursor {
    uris: Vec<String>,
    fallback: Keyword,
    retained_bytes: usize,
}
impl ComputedCursor {
    pub fn uris(&self) -> &[String] {
        &self.uris
    }
    pub fn fallback(&self) -> Keyword {
        self.fallback
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorComputationError {
    Unresolved,
    InvalidFallback,
    Uris,
    Bytes,
    Work,
    Allocation,
    Uri(UriError),
}
impl std::fmt::Display for CursorComputationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cursor computation: {self:?}")
    }
}
impl std::error::Error for CursorComputationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Uri(error) => Some(error),
            _ => None,
        }
    }
}
/// Resolve URLs against an absolute ASCII stylesheet base without retrieving them.
/// Non-ASCII URI conversion and native cursor capability/fallback selection are
/// separate. Empty URL lists do not inspect the unused base. Failures publish no
/// partially computed list and never mutate the parsed source.
pub fn compute_cursor(
    property: &ParsedProperty,
    stylesheet_base: &str,
    limits: CursorComputationLimits,
) -> Result<ComputedCursor, CursorComputationError> {
    if property.property.descriptor().name != "cursor" {
        return Err(CursorComputationError::Unresolved);
    }
    let SpecifiedValue::Cursor(cursor) = &property.value else {
        return Err(CursorComputationError::Unresolved);
    };
    if !cursor_keyword_allowed(cursor.fallback) {
        return Err(CursorComputationError::InvalidFallback);
    }
    if cursor.uris.len() > limits.uris.get() {
        return Err(CursorComputationError::Uris);
    }
    let bytes = limits.retained_bytes.get();
    if bytes > isize::MAX as usize {
        return Err(CursorComputationError::Bytes);
    }
    let admission = cursor
        .uris
        .len()
        .checked_add(1)
        .ok_or(CursorComputationError::Work)?;
    let mut work = limits
        .work
        .get()
        .checked_sub(admission)
        .ok_or(CursorComputationError::Work)?;
    let available = bytes / size_of::<String>();
    if cursor.uris.len() > available {
        return Err(CursorComputationError::Bytes);
    }
    let mut uris = Vec::new();
    uris.try_reserve_exact(cursor.uris.len())
        .map_err(|_| CursorComputationError::Allocation)?;
    if uris.capacity() > available {
        return Err(CursorComputationError::Bytes);
    }
    let mut retained_bytes = uris.capacity() * size_of::<String>();
    for reference in &cursor.uris {
        let remaining =
            NonZeroUsize::new(bytes - retained_bytes).ok_or(CursorComputationError::Bytes)?;
        let resolved = uri::resolve_with_work(stylesheet_base, reference, remaining, &mut work)
            .map_err(CursorComputationError::Uri)?;
        retained_bytes += resolved.capacity();
        uris.push(resolved);
    }
    Ok(ComputedCursor {
        uris,
        fallback: cursor.fallback,
        retained_bytes,
    })
}
