//! Move source tokens after caller-established parent and adapter compatibility.
use super::reconcile::SlotLimits;
use crate::animation::engine::{EngineError, PreparedSourceBindings, ProgressEngine};
use iced_css_core::identity::{InstanceId, SourceChangeError, SourceChangeLimits, SourceRef};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSlotError {
    Entries,
    Bytes,
    Work,
    Allocation,
    Missing { incoming: usize, previous: usize },
    OutOfBounds { incoming: usize, previous: usize },
    DuplicateMapping { first: usize, second: usize },
    DuplicateInstance { first: usize, second: usize },
    Engine(EngineError),
}
impl std::fmt::Display for SourceSlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source slot preparation: {self:?}")
    }
}
impl std::error::Error for SourceSlotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(e) => Some(e),
            _ => None,
        }
    }
}
fn reserve<T>(count: usize, bytes: &mut usize) -> Result<Vec<T>, SourceSlotError> {
    let needed = count
        .checked_mul(size_of::<T>())
        .ok_or(SourceSlotError::Bytes)?;
    if needed > *bytes || needed > isize::MAX as usize {
        return Err(SourceSlotError::Bytes);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| SourceSlotError::Allocation)?;
    let actual = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(SourceSlotError::Bytes)?;
    *bytes = bytes.checked_sub(actual).ok_or(SourceSlotError::Bytes)?;
    Ok(values)
}
/// A reserved incoming source order, not a complete native/style generation.
///
/// ```compile_fail
/// use iced_css::host::source_slots::PreparedSourceSlots;
/// fn twice(prepared:PreparedSourceSlots<'_, '_>) {
///     prepared.commit();
///     prepared.commit();
/// }
/// ```
#[must_use = "commit incoming source slots or drop the reservation"]
pub struct PreparedSourceSlots<'engine, 'slots> {
    bindings: PreparedSourceBindings<'engine>,
    previous: &'slots mut [Option<SourceRef>],
    mapping: Vec<Option<usize>>,
    destination: Vec<SourceRef>,
}
impl PreparedSourceSlots<'_, '_> {
    pub fn removed_instances(&self) -> &[InstanceId] {
        self.bindings.removed_instances()
    }
    /// Source/progress removal commits before fresh tokens become observable.
    /// All owner storage is reserved; dependency panic still needs host faulting.
    pub fn commit(mut self) -> Vec<SourceRef> {
        let mut fresh = self.bindings.commit().into_iter();
        for old in self.mapping {
            let token = match old {
                Some(index) => self.previous[index]
                    .take()
                    .expect("unique live source reuse preflighted"),
                None => fresh.next().expect("new source count preflighted"),
            };
            self.destination.push(token);
        }
        for slot in self.previous {
            slot.take();
        }
        self.destination
    }
}
/// Mapping indices must already reflect matching source parents and full adapter
/// compatibility. This validates scope, membership and movement, not those facts.
/// Slot bytes include validation scratch; identity staging has its separate limit.
pub fn prepare_source_slots<'engine, 'slots>(
    engine: &'engine mut ProgressEngine,
    previous: &'slots mut [Option<SourceRef>],
    mapping: &[Option<usize>],
    limits: SlotLimits,
    identity_limits: SourceChangeLimits,
) -> Result<PreparedSourceSlots<'engine, 'slots>, SourceSlotError> {
    let n = previous.len();
    let m = mapping.len();
    let entries = n.checked_add(m).ok_or(SourceSlotError::Entries)?;
    if entries > limits.entries.get() {
        return Err(SourceSlotError::Entries);
    }
    let global = engine.live_source_count();
    if global > identity_limits.entries.get() {
        return Err(SourceSlotError::Engine(EngineError::Sources(
            SourceChangeError::Entries,
        )));
    }
    // Bound registry validation against actual global membership, plus duplicate
    // checks, two removal scans and fixed per-slot copying/validation visits.
    let work = global
        .checked_add(1)
        .and_then(|v| v.checked_mul(n))
        .and_then(|v| n.checked_mul(n).and_then(|x| v.checked_add(x)))
        .and_then(|v| m.checked_mul(m).and_then(|x| v.checked_add(x)))
        .and_then(|v| {
            n.checked_mul(m)
                .and_then(|x| x.checked_mul(2))
                .and_then(|x| v.checked_add(x))
        })
        .and_then(|v| entries.checked_mul(3).and_then(|x| v.checked_add(x)))
        .and_then(|v| v.checked_add(1))
        .ok_or(SourceSlotError::Work)?;
    if work > limits.work.get() {
        return Err(SourceSlotError::Work);
    }
    let mut bytes = limits.retained_bytes.get();
    let mut ids = reserve(n, &mut bytes)?;
    for (second, source) in previous.iter().enumerate() {
        let id = source
            .as_ref()
            .map(|source| engine.instance(source))
            .transpose()
            .map_err(SourceSlotError::Engine)?;
        if id.is_some()
            && let Some(first) = ids.iter().position(|old| *old == id)
        {
            return Err(SourceSlotError::DuplicateInstance { first, second });
        }
        ids.push(id);
    }
    let mut owned_mapping = reserve(m, &mut bytes)?;
    let mut insertions = 0;
    for (incoming, index) in mapping.iter().copied().enumerate() {
        if let Some(previous) = index {
            let id = ids
                .get(previous)
                .ok_or(SourceSlotError::OutOfBounds { incoming, previous })?;
            if id.is_none() {
                return Err(SourceSlotError::Missing { incoming, previous });
            }
            if let Some(first) = owned_mapping.iter().position(|old| *old == index) {
                return Err(SourceSlotError::DuplicateMapping {
                    first,
                    second: incoming,
                });
            }
        } else {
            insertions += 1;
        }
        owned_mapping.push(index);
    }
    let removed_count = ids
        .iter()
        .enumerate()
        .filter(|(index, id)| id.is_some() && !owned_mapping.contains(&Some(*index)))
        .count();
    let mut removed = reserve(removed_count, &mut bytes)?;
    for (index, source) in previous.iter().enumerate() {
        if !owned_mapping.contains(&Some(index))
            && let Some(source) = source
        {
            removed.push(source.clone());
        }
    }
    let destination = reserve(m, &mut bytes)?;
    let bindings = engine
        .prepare_source_changes(&removed, insertions, identity_limits)
        .map_err(SourceSlotError::Engine)?;
    Ok(PreparedSourceSlots {
        bindings,
        previous,
        mapping: owned_mapping,
        destination,
    })
}
