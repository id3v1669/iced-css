//! Move already compatibility-checked, declared native slots after preflight.
use iced::advanced::widget::Tree;
use std::{borrow::Cow, num::NonZeroUsize};
#[derive(Debug, Clone, Copy)]
pub struct SlotLimits {
    pub entries: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotError {
    Entries,
    Bytes,
    Work,
    Allocation,
    OutOfBounds { incoming: usize, previous: usize },
    Missing { incoming: usize, previous: usize },
    Duplicate { first: usize, second: usize },
}
impl std::fmt::Display for SlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native slot preflight: {self:?}")
    }
}
impl std::error::Error for SlotError {}
/// Exclusive access prevents the validated previous slots changing before apply.
/// Dropping this preparation moves no Tree and leaves previous slots unchanged.
#[derive(Debug)]
pub struct PreparedNativeSlots<'state, 'mapping> {
    previous: &'state mut [Option<Tree>],
    mapping: Cow<'mapping, [Option<usize>]>,
    destination: Vec<Option<Tree>>,
}
impl PreparedNativeSlots<'_, '_> {
    /// Retained destination capacity; excludes an owned input mapping.
    pub fn retained_bytes(&self) -> usize {
        self.destination.capacity() * std::mem::size_of::<Option<Tree>>()
    }
    /// Owned mapping capacity, separately bounded by compatibility preparation.
    /// The borrowed-mapping entry point retains no mapping allocation.
    pub fn mapping_retained_bytes(&self) -> usize {
        match &self.mapping {
            Cow::Borrowed(_) => 0,
            Cow::Owned(values) => values.capacity() * size_of::<Option<usize>>(),
        }
    }
    /// No allocation or native callbacks occur here. Unselected previous slots
    /// stay owned by the caller; new destination slots remain None.
    pub fn apply(mut self) -> Vec<Option<Tree>> {
        for (destination, previous) in self.destination.iter_mut().zip(self.mapping.iter()) {
            if let Some(previous) = previous {
                *destination = self.previous[*previous].take();
            }
        }
        self.destination
    }
}
fn charge(work: &mut usize, n: usize) -> Result<(), SlotError> {
    *work = work.checked_sub(n).ok_or(SlotError::Work)?;
    Ok(())
}
/// Reserve and validate before changing any previous state. The caller supplies
/// slots from one established parent scope after complete adapter compatibility
/// checks; a source-sibling match alone is insufficient authorization for reuse.
pub fn prepare_native_slots<'state, 'mapping>(
    previous: &'state mut [Option<Tree>],
    mapping: &'mapping [Option<usize>],
    limits: SlotLimits,
) -> Result<PreparedNativeSlots<'state, 'mapping>, SlotError> {
    prepare_slots(previous, Cow::Borrowed(mapping), limits)
}

pub(crate) fn prepare_native_slots_owned(
    previous: &mut [Option<Tree>],
    mapping: Vec<Option<usize>>,
    limits: SlotLimits,
) -> Result<PreparedNativeSlots<'_, 'static>, SlotError> {
    prepare_slots(previous, Cow::Owned(mapping), limits)
}

fn prepare_slots<'state, 'mapping>(
    previous: &'state mut [Option<Tree>],
    mapping: Cow<'mapping, [Option<usize>]>,
    limits: SlotLimits,
) -> Result<PreparedNativeSlots<'state, 'mapping>, SlotError> {
    let entries = previous
        .len()
        .checked_add(mapping.len())
        .ok_or(SlotError::Entries)?;
    if entries > limits.entries.get() {
        return Err(SlotError::Entries);
    }
    let bytes = mapping
        .len()
        .checked_mul(std::mem::size_of::<Option<Tree>>())
        .ok_or(SlotError::Bytes)?;
    if bytes > limits.retained_bytes.get() {
        return Err(SlotError::Bytes);
    }
    let mut work = limits.work.get();
    // Prepay destination initialization and the later movement loop.
    charge(&mut work, mapping.len())?;
    charge(&mut work, mapping.len())?;
    for (incoming, slot) in mapping.iter().enumerate() {
        charge(&mut work, 1)?;
        let Some(index) = slot else {
            continue;
        };
        let value = previous.get(*index).ok_or(SlotError::OutOfBounds {
            incoming,
            previous: *index,
        })?;
        if value.is_none() {
            return Err(SlotError::Missing {
                incoming,
                previous: *index,
            });
        }
        for (first, earlier) in mapping[..incoming].iter().enumerate() {
            charge(&mut work, 1)?;
            if earlier == slot {
                return Err(SlotError::Duplicate {
                    first,
                    second: incoming,
                });
            }
        }
    }
    let mut destination = Vec::new();
    destination
        .try_reserve_exact(mapping.len())
        .map_err(|_| SlotError::Allocation)?;
    let capacity_bytes = destination
        .capacity()
        .checked_mul(std::mem::size_of::<Option<Tree>>())
        .ok_or(SlotError::Bytes)?;
    if capacity_bytes > limits.retained_bytes.get() {
        return Err(SlotError::Bytes);
    }
    destination.resize_with(mapping.len(), || None);
    Ok(PreparedNativeSlots {
        previous,
        mapping,
        destination,
    })
}

/// Native transport whose unmatched destinations already own fresh state.
/// Abandonment drops only pending fresh Trees, leaving previous Trees untouched.
///
/// ```compile_fail,E0505
/// use iced::{Element,advanced::widget::Tree};
/// use iced_css::host::reconcile::{prepare_native_slots,SlotLimits};
/// fn move_incoming(widgets:Vec<Element<'_,(),iced::Theme,()>>,old:&mut [Option<Tree>],map:&[Option<usize>],limits:SlotLimits) {
///     let pending=prepare_native_slots(old,map,limits).unwrap().with_initialization(&widgets,limits).unwrap();
///     drop(widgets);
///     pending.apply();
/// }
/// ```
#[must_use = "apply the prepared complete native order, or abandon it"]
pub struct PreparedInitializedNativeSlots<'state, 'mapping, 'incoming> {
    prepared: PreparedNativeSlots<'state, 'mapping>,
    _incoming: std::marker::PhantomData<&'incoming ()>,
}
impl PreparedInitializedNativeSlots<'_, '_, '_> {
    /// Move retained Trees into the already prepared destination. All returned
    /// slots are occupied. No native construction, diff or allocation occurs.
    /// Unselected previous Trees remain caller-owned for controlled retirement.
    pub fn apply(self) -> Vec<Option<Tree>> {
        self.prepared.apply()
    }
}
impl<'state, 'mapping> PreparedNativeSlots<'state, 'mapping> {
    /// Initialize only unmatched incoming slots before any active Tree moves.
    /// The mapping must already reflect semantic adapter compatibility. Widget
    /// correspondence is a caller obligation; native effects are not rolled back.
    /// Initialization limits independently bound the transient (index,Tree) vector
    /// and scans, excluding this transport's destination/mapping reservations and
    /// native recursive allocations. Keep the incoming view through native diff.
    pub fn with_initialization<'incoming, Message, Theme, R>(
        mut self,
        incoming: &'incoming [iced::Element<'_, Message, Theme, R>],
        limits: SlotLimits,
    ) -> Result<
        PreparedInitializedNativeSlots<'state, 'mapping, 'incoming>,
        super::initialize::InitializationError,
    >
    where
        R: iced::advanced::Renderer,
    {
        super::initialize::prepare_filtered_initialization(
            &mut self.destination,
            incoming,
            Some(&self.mapping),
            limits,
        )?
        .commit();
        Ok(PreparedInitializedNativeSlots {
            prepared: self,
            _incoming: std::marker::PhantomData,
        })
    }
}
