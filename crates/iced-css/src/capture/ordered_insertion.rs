//! Family-controlled ordered iteration. Never a public arbitrary capture adapter.
use super::{CaptureError, CaptureLimits, Captured};
use std::num::NonZeroUsize;

pub(super) fn extend<P, W>(
    mut parent: Captured<P>,
    children: impl IntoIterator<Item = Captured<W>>,
    limits: CaptureLimits,
    mut push: impl FnMut(Captured<P>, Captured<W>, CaptureLimits) -> Result<Captured<P>, CaptureError>,
) -> Result<Captured<P>, CaptureError> {
    parent.construction.validate_limits(limits)?;
    // NonZeroUsize guarantees the constant initial validation step fits.
    let mut work = limits.work.get() - 1;
    let mut children = children.into_iter();
    loop {
        work = work.checked_sub(1).ok_or(CaptureError::Work)?;
        let Some(child) = children.next() else {
            return Ok(parent);
        };
        let cost = parent.construction.append_cost(&child.construction)?;
        work = work.checked_sub(cost).ok_or(CaptureError::Work)?;
        let append_limits = CaptureLimits {
            work: NonZeroUsize::new(cost).ok_or(CaptureError::Work)?,
            ..limits
        };
        parent = push(parent, child, append_limits)?;
    }
}
