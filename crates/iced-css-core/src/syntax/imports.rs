//! Serial import traversal bookkeeping. Resource identities are supplied by the
//! resolver; no URI canonicalization, acquisition or stylesheet parsing occurs here.
use std::num::{NonZeroU64, NonZeroUsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceId(NonZeroU64);
impl ResourceId {
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
/// Scoped to one tracker lifetime. The owner must not mix tracker identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OccurrenceId(NonZeroU64);
impl OccurrenceId {
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visit {
    pub occurrence: OccurrenceId,
    pub needs_acquisition: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportEntry {
    Visit(Visit),
    Cycle,
    MediaExcluded,
}
#[derive(Debug, Clone, Copy)]
pub struct ImportLimits {
    pub entries: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
    pub occurrences: NonZeroU64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportTrackError {
    Entries,
    Bytes,
    Work,
    Allocation,
    Occurrences,
    NotCurrent,
}
impl std::fmt::Display for ImportTrackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "import traversal: {self:?}")
    }
}
impl std::error::Error for ImportTrackError {}

#[derive(Clone, Copy)]
struct Frame {
    resource: ResourceId,
    occurrence: OccurrenceId,
}
fn charge(work: &mut usize, count: usize) -> Result<(), ImportTrackError> {
    *work = work.checked_sub(count).ok_or(ImportTrackError::Work)?;
    Ok(())
}
fn bytes<T>(values: &Vec<T>) -> Result<usize, ImportTrackError> {
    values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(ImportTrackError::Bytes)
}
/// Bounds retained capacities; temporary old/new buffer overlap during growth is
/// excluded, as in the other fallible bounded-vector primitives.
fn reserve<T: Copy>(
    values: &mut Vec<T>,
    other_bytes: usize,
    limits: ImportLimits,
    work: &mut usize,
) -> Result<(), ImportTrackError> {
    if values.len() < values.capacity() {
        return Ok(());
    }
    let available = limits
        .retained_bytes
        .get()
        .checked_sub(other_bytes)
        .ok_or(ImportTrackError::Bytes)?;
    let max = available.min(isize::MAX as usize) / size_of::<T>();
    let needed = values
        .len()
        .checked_add(1)
        .filter(|needed| *needed <= max)
        .ok_or(ImportTrackError::Bytes)?;
    let target = needed.max(values.capacity().saturating_mul(2).min(max));
    charge(work, values.len())?;
    let mut next = Vec::new();
    next.try_reserve_exact(target)
        .map_err(|_| ImportTrackError::Allocation)?;
    if next.capacity() > max {
        return Err(ImportTrackError::Bytes);
    }
    next.extend_from_slice(values);
    *values = next;
    Ok(())
}

pub struct ImportTracker {
    limits: ImportLimits,
    active: Vec<Frame>,
    acquired: Vec<ResourceId>,
    admitted: u64,
}
impl ImportTracker {
    pub fn new(limits: ImportLimits) -> Self {
        Self {
            limits,
            active: Vec::new(),
            acquired: Vec::new(),
            admitted: 0,
        }
    }
    fn room(&self) -> Result<(), ImportTrackError> {
        self.active
            .len()
            .checked_add(self.acquired.len())
            .and_then(|count| count.checked_add(1))
            .filter(|count| *count <= self.limits.entries.get())
            .map(|_| ())
            .ok_or(ImportTrackError::Entries)
    }
    fn acquired(&self, resource: ResourceId, work: &mut usize) -> Result<bool, ImportTrackError> {
        for current in &self.acquired {
            charge(work, 1)?;
            if *current == resource {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn active_contains(
        &self,
        resource: ResourceId,
        work: &mut usize,
    ) -> Result<bool, ImportTrackError> {
        for frame in &self.active {
            charge(work, 1)?;
            if frame.resource == resource {
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// Bounded read-only activity query. It allocates no storage and does not
    /// admit an occurrence, including when the occurrence allowance is exhausted.
    pub fn is_active(&self, resource: ResourceId) -> Result<bool, ImportTrackError> {
        let mut work = self.limits.work.get();
        charge(&mut work, 1)?;
        self.active_contains(resource, &mut work)
    }
    /// Enter a root or import occurrence. Roots also count toward the lifetime
    /// occurrence limit. Excluded media and cyclic edges create no admitted visit.
    pub fn enter(
        &mut self,
        resource: ResourceId,
        applies_to_screen: bool,
    ) -> Result<ImportEntry, ImportTrackError> {
        let mut work = self.limits.work.get();
        charge(&mut work, 1)?;
        if !applies_to_screen {
            return Ok(ImportEntry::MediaExcluded);
        }
        if self.active_contains(resource, &mut work)? {
            return Ok(ImportEntry::Cycle);
        }
        let needs_acquisition = !self.acquired(resource, &mut work)?;
        let next = self
            .admitted
            .checked_add(1)
            .filter(|next| *next <= self.limits.occurrences.get())
            .ok_or(ImportTrackError::Occurrences)?;
        self.room()?;
        reserve(
            &mut self.active,
            bytes(&self.acquired)?,
            self.limits,
            &mut work,
        )?;
        let occurrence =
            OccurrenceId(NonZeroU64::new(next).expect("checked increment is positive"));
        self.active.push(Frame {
            resource,
            occurrence,
        });
        self.admitted = next;
        Ok(ImportEntry::Visit(Visit {
            occurrence,
            needs_acquisition,
        }))
    }
    /// Mark the current resource only after the owner successfully retains its
    /// acquisition. The owner must keep those bytes available for tracker lifetime.
    pub fn mark_acquired(&mut self, resource: ResourceId) -> Result<bool, ImportTrackError> {
        let mut work = self.limits.work.get();
        charge(&mut work, 1)?;
        if self
            .active
            .last()
            .is_none_or(|frame| frame.resource != resource)
        {
            return Err(ImportTrackError::NotCurrent);
        }
        if self.acquired(resource, &mut work)? {
            return Ok(false);
        }
        self.room()?;
        reserve(
            &mut self.acquired,
            bytes(&self.active)?,
            self.limits,
            &mut work,
        )?;
        self.acquired.push(resource);
        Ok(true)
    }
    pub fn finish(&mut self, occurrence: OccurrenceId) -> Result<(), ImportTrackError> {
        if self
            .active
            .last()
            .is_none_or(|frame| frame.occurrence != occurrence)
        {
            return Err(ImportTrackError::NotCurrent);
        }
        self.active.pop();
        Ok(())
    }
    pub fn admitted_occurrences(&self) -> u64 {
        self.admitted
    }
    pub fn acquired_count(&self) -> usize {
        self.acquired.len()
    }
    pub fn active_depth(&self) -> usize {
        self.active.len()
    }
}
