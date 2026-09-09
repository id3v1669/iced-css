//! Initialize unmatched native slots from their actual incoming widget payloads.
use super::reconcile::{SlotError, SlotLimits};
use iced::{
    Element,
    advanced::{Renderer, widget::Tree},
};
use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationError {
    Dimensions,
    Slots(SlotError),
}
impl std::fmt::Display for InitializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native slot initialization: {self:?}")
    }
}
impl std::error::Error for InitializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dimensions => None,
            Self::Slots(e) => Some(e),
        }
    }
}
impl From<SlotError> for InitializationError {
    fn from(error: SlotError) -> Self {
        Self::Slots(error)
    }
}

/// Pending Trees are not installed until commit. Abandonment preserves all active
/// slots and drops only the freshly constructed pending Trees.
///
/// The incoming payload cannot be moved away while preparation is still used:
/// ```compile_fail,E0505
/// use iced::{Element, advanced::widget::Tree};
/// use iced_css::host::{initialize::prepare_native_initialization, reconcile::SlotLimits};
/// fn move_incoming(widgets:Vec<Element<'_,(),iced::Theme,()>>,slots:&mut [Option<Tree>],limits:SlotLimits) {
///     let pending=prepare_native_initialization(slots,&widgets,limits).unwrap();
///     drop(widgets);
///     pending.commit();
/// }
/// ```
#[must_use = "commit the pending native state, or abandon it without changing active slots"]
pub struct PreparedNativeInitialization<'state, 'incoming> {
    slots: &'state mut [Option<Tree>],
    pending: Vec<(usize, Tree)>,
    _incoming: PhantomData<&'incoming ()>,
}
impl PreparedNativeInitialization<'_, '_> {
    /// Retained staging capacity, excluding native payload/descendant allocations.
    pub fn retained_bytes(&self) -> usize {
        self.pending.capacity() * size_of::<(usize, Tree)>()
    }
    /// Fill previously empty slots without allocating, calling native widget
    /// methods, or retiring old Trees. No source/style generation is published.
    pub fn commit(self) {
        for (index, tree) in self.pending {
            self.slots[index] = Some(tree);
        }
    }
}

/// Prepare exactly one native Tree for each None slot, in incoming order.
///
/// The caller establishes the slot/view correspondence, source parent and semantic
/// compatibility of retained Trees. This never queries retained widgets, runs diff,
/// or discovers private children hidden inside an already constructed parent.
/// Keep the matching incoming view until subsequent native diff has completed.
///
/// Entries bounds the number of supplied slots. Work prepays two slot scans plus
/// one construction dispatch and one commit visit per missing slot (2*n + 2*m).
/// Bytes bounds only the pending (index, Tree) vector's actual capacity. Native
/// callbacks, recursive state allocations and destructors are outside these limits;
/// native/application panic propagates and external side effects cannot roll back.
pub fn prepare_native_initialization<'state, 'incoming, Message, Theme, R>(
    slots: &'state mut [Option<Tree>],
    incoming: &'incoming [Element<'_, Message, Theme, R>],
    limits: SlotLimits,
) -> Result<PreparedNativeInitialization<'state, 'incoming>, InitializationError>
where
    R: Renderer,
{
    prepare_filtered_initialization(slots, incoming, None, limits)
}

// A mapping entry selects a retained Tree that will move only at transport commit.
// Such a destination must not receive a speculative fresh Tree during preparation.
pub(super) fn prepare_filtered_initialization<'state, 'incoming, Message, Theme, R>(
    slots: &'state mut [Option<Tree>],
    incoming: &'incoming [Element<'_, Message, Theme, R>],
    mapping: Option<&[Option<usize>]>,
    limits: SlotLimits,
) -> Result<PreparedNativeInitialization<'state, 'incoming>, InitializationError>
where
    R: Renderer,
{
    if slots.len() != incoming.len() || mapping.is_some_and(|m| m.len() != slots.len()) {
        return Err(InitializationError::Dimensions);
    }
    if slots.len() > limits.entries.get() {
        return Err(SlotError::Entries.into());
    }
    let scan_work = slots.len().checked_mul(2).ok_or(SlotError::Work)?;
    let remaining = limits
        .work
        .get()
        .checked_sub(scan_work)
        .ok_or(SlotError::Work)?;
    let needs_state = |index: usize, slot: &Option<Tree>| {
        slot.is_none() && mapping.is_none_or(|m| m[index].is_none())
    };
    let missing = slots
        .iter()
        .enumerate()
        .filter(|(index, slot)| needs_state(*index, slot))
        .count();
    if missing.checked_mul(2).ok_or(SlotError::Work)? > remaining {
        return Err(SlotError::Work.into());
    }
    let bytes = missing
        .checked_mul(size_of::<(usize, Tree)>())
        .ok_or(SlotError::Bytes)?;
    if bytes > limits.retained_bytes.get() {
        return Err(SlotError::Bytes.into());
    }
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(missing)
        .map_err(|_| SlotError::Allocation)?;
    let bytes = pending
        .capacity()
        .checked_mul(size_of::<(usize, Tree)>())
        .ok_or(SlotError::Bytes)?;
    if bytes > limits.retained_bytes.get() {
        return Err(SlotError::Bytes.into());
    }
    for (index, (slot, widget)) in slots.iter().zip(incoming).enumerate() {
        if needs_state(index, slot) {
            pending.push((index, Tree::new(widget)));
        }
    }
    Ok(PreparedNativeInitialization {
        slots,
        pending,
        _incoming: PhantomData,
    })
}
