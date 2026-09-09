//! Post-publication CSS recovery diagnostics for explicitly identified resources.
use super::recovery;
use iced_css_core::stylesheet::owned_graph::OwnedStylesheetGraph;
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphRecoveryError {
    Work,
    InvalidSpan,
}
impl std::fmt::Display for GraphRecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "graph recovery diagnostics omitted: {self:?}")
    }
}
impl std::error::Error for GraphRecoveryError {}
impl From<recovery::Error> for GraphRecoveryError {
    fn from(error: recovery::Error) -> Self {
        match error {
            recovery::Error::Work => Self::Work,
            recovery::Error::InvalidSpan => Self::InvalidSpan,
        }
    }
}
/// Call after accepted graph publication, supplying the replaced graph if any.
/// Compare only active resources under their explicit stable ResourceId contract.
/// A resource logs once per parsed input, regardless of repeated import occurrences.
/// This retains no cause cache and must not be invoked as an automatic view hook.
///
/// One work budget covers matching and recovery. An omission error does not undo
/// publication; the caller can report that error through its diagnostic boundary.
/// Acquisition failures/unavailable import cuts are not CSS parser recovery.
pub fn report_graph_recovery(
    active: &OwnedStylesheetGraph,
    previous: Option<&OwnedStylesheetGraph>,
    limit: NonZeroUsize,
) -> Result<(), GraphRecoveryError> {
    let mut work = limit.get();
    for resource in active.active_resources() {
        work = work.checked_sub(1).ok_or(GraphRecoveryError::Work)?;
        let mut prior = None;
        if let Some(previous) = previous {
            for old in previous.active_resources() {
                work = work.checked_sub(1).ok_or(GraphRecoveryError::Work)?;
                if old.resource == resource.resource {
                    prior = Some(&old.candidate);
                    break;
                }
            }
        }
        recovery::report_with_work(
            &resource.candidate,
            prior,
            &mut work,
            Some(resource.resource),
        )?;
    }
    Ok(())
}
