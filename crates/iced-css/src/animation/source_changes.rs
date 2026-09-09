//! Join identity publication with private progress-owner removal.
use super::{Binding, EngineError, ProgressEngine, quiesce};
use iced_animate::Motion;
use iced_css_core::identity::{InstanceId, PreparedSourceChanges, SourceChangeLimits, SourceRef};
use std::{marker::PhantomData, rc::Rc};

/// Prepared identity changes with owner cleanup. New source tokens are returned
/// only after removed progress bindings have been quiesced and released.
///
/// ```compile_fail
/// use iced_css::animation::engine::PreparedSourceBindings;
/// fn twice(prepared:PreparedSourceBindings<'_>) {
///     prepared.commit();
///     prepared.commit();
/// }
/// ```
/// ```compile_fail
/// use iced_css::animation::engine::PreparedSourceBindings;
/// fn require_send<T:Send>() {}
/// require_send::<PreparedSourceBindings<'static>>();
/// ```
#[must_use = "commit source bindings or drop the reservation"]
pub struct PreparedSourceBindings<'a> {
    sources: PreparedSourceChanges<'a>,
    motion: &'a Motion,
    bindings: &'a mut Vec<Binding>,
    runner_thread: PhantomData<Rc<()>>,
}
impl PreparedSourceBindings<'_> {
    /// Validated current identities, available for coordinated interaction cleanup.
    pub fn removed_instances(&self) -> &[InstanceId] {
        self.sources.removed_instances()
    }
    /// No returned admission errors remain. A dependency panic after cleanup has
    /// begun still requires the host fault policy, not a rollback claim.
    pub fn commit(self) -> Vec<SourceRef> {
        let Self {
            sources,
            motion,
            bindings,
            ..
        } = self;
        let removed = sources.removed_instances();
        bindings.retain(|binding| {
            if removed.contains(&binding.instance) {
                quiesce(motion, binding);
                false
            } else {
                true
            }
        });
        sources.commit()
    }
}
impl ProgressEngine {
    /// Prepare source liveness changes and associated progress removal together.
    /// Captured topology, native compatibility and CSS publication remain external.
    pub fn prepare_source_changes(
        &mut self,
        removals: &[SourceRef],
        insertions: usize,
        limits: SourceChangeLimits,
    ) -> Result<PreparedSourceBindings<'_>, EngineError> {
        let work = removals
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(self.bindings.len()))
            .ok_or(EngineError::Work)?;
        self.check_work(work)?;
        let sources = self
            .sources
            .prepare_changes(removals, insertions, limits)
            .map_err(EngineError::Sources)?;
        Ok(PreparedSourceBindings {
            sources,
            motion: &self.motion,
            bindings: &mut self.bindings,
            runner_thread: PhantomData,
        })
    }
}
