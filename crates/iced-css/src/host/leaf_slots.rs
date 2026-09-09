//! Joint source/native transport for one parent scope of managed leaf sources.
use super::{
    compatibility::{
        CompatibilityError, CompatibilityLimits, NativeCompatibility, filter_native_compatibility,
    },
    reconcile::{PreparedNativeSlots, SlotError, SlotLimits, prepare_native_slots_owned},
    source_slots::{PreparedSourceSlots, SourceSlotError, prepare_source_slots},
};
use crate::animation::engine::ProgressEngine;
use iced::advanced::widget::Tree;
use iced_css_core::identity::{InstanceId, SourceChangeLimits, SourceRef};
use std::num::NonZeroUsize;

/// Each occupied source has exactly one complete native Tree. Native internal
/// children are allowed; separately managed source descendants are not.
/// The caller establishes the parent scope and trusted adapter declarations.
pub struct PreviousLeafSlots<'sources, 'native, 'schema, 'descriptor> {
    pub sources: &'sources mut [Option<SourceRef>],
    pub native: &'native mut [Option<Tree>],
    pub schema: &'schema [NativeCompatibility<'descriptor>],
}
pub struct IncomingLeafSlots<'schema, 'descriptor, 'mapping> {
    pub schema: &'schema [NativeCompatibility<'descriptor>],
    /// Source-parent-local sibling matches, before native compatibility filtering.
    pub mapping: &'mapping [Option<usize>],
}
/// Independent stage budgets. Compatibility owns the native mapping allocation;
/// source slots own their mapping copy. Existing state storage is not included.
#[derive(Debug, Clone, Copy)]
pub struct LeafSlotLimits {
    pub compatibility: CompatibilityLimits,
    pub sources: SlotLimits,
    pub native: SlotLimits,
    pub identities: SourceChangeLimits,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafSlotError {
    Dimensions,
    Occupancy { index: usize },
    Compatibility(CompatibilityError),
    Sources(SourceSlotError),
    Native(SlotError),
}
impl std::fmt::Display for LeafSlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "leaf slot preparation: {self:?}")
    }
}
impl std::error::Error for LeafSlotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compatibility(e) => Some(e),
            Self::Sources(e) => Some(e),
            Self::Native(e) => Some(e),
            _ => None,
        }
    }
}
/// Equal-length arrays in incoming order. New native slots remain None until
/// materialized by the caller; this is not a published or drawable generation.
pub struct LeafSlots {
    pub sources: Vec<SourceRef>,
    pub native: Vec<Option<Tree>>,
}
/// Dropping the guard leaves both active arrays and source/progress ownership
/// unchanged. No prospective token is exposed before commit.
///
/// ```compile_fail
/// use iced_css::host::leaf_slots::PreparedLeafSlots;
/// fn twice(prepared: PreparedLeafSlots<'_, '_, '_>) {
///     prepared.commit();
///     prepared.commit();
/// }
/// ```
#[must_use = "commit both prepared slot orders or abandon the candidate"]
pub struct PreparedLeafSlots<'engine, 'sources, 'native> {
    sources: PreparedSourceSlots<'engine, 'sources>,
    native: PreparedNativeSlots<'native, 'static>,
}
impl PreparedLeafSlots<'_, '_, '_> {
    pub fn removed_instances(&self) -> &[InstanceId] {
        self.sources.removed_instances()
    }
    /// Commit identity/progress changes, then transport retained native state.
    /// No native callbacks run. Unselected old native Trees remain with the
    /// caller for cleanup. Dependency panic still requires host fault handling;
    /// this does not publish interaction/style/geometry state or promise rollback.
    pub fn commit(self) -> LeafSlots {
        let sources = self.sources.commit();
        let native = self.native.apply();
        LeafSlots { sources, native }
    }
}
/// Reserve both state transports under exclusive borrows and one filtered map.
/// Parent matching and leaf/adapter declarations remain caller obligations.
pub fn prepare_leaf_slots<'engine, 'sources, 'native>(
    engine: &'engine mut ProgressEngine,
    previous: PreviousLeafSlots<'sources, 'native, '_, '_>,
    incoming: IncomingLeafSlots<'_, '_, '_>,
    mut limits: LeafSlotLimits,
) -> Result<PreparedLeafSlots<'engine, 'sources, 'native>, LeafSlotError> {
    if previous.sources.len() != previous.native.len() {
        return Err(LeafSlotError::Dimensions);
    }
    let count = previous.sources.len();
    if count > limits.sources.entries.get() {
        return Err(LeafSlotError::Sources(SourceSlotError::Entries));
    }
    // Prepay the correspondence scan; preserve a positive remainder for the
    // existing source preparation's own validation and movement work.
    limits.sources.work = limits
        .sources
        .work
        .get()
        .checked_sub(count)
        .and_then(NonZeroUsize::new)
        .ok_or(LeafSlotError::Sources(SourceSlotError::Work))?;
    for (index, (source, native)) in previous
        .sources
        .iter()
        .zip(previous.native.iter())
        .enumerate()
    {
        if source.is_some() != native.is_some() {
            return Err(LeafSlotError::Occupancy { index });
        }
    }
    let compatible = filter_native_compatibility(
        previous.native,
        previous.schema,
        incoming.schema,
        incoming.mapping,
        limits.compatibility,
    )
    .map_err(LeafSlotError::Compatibility)?;
    let sources = prepare_source_slots(
        engine,
        previous.sources,
        compatible.previous_indices(),
        limits.sources,
        limits.identities,
    )
    .map_err(LeafSlotError::Sources)?;
    let native = prepare_native_slots_owned(
        previous.native,
        compatible.into_previous_indices(),
        limits.native,
    )
    .map_err(LeafSlotError::Native)?;
    Ok(PreparedLeafSlots { sources, native })
}
