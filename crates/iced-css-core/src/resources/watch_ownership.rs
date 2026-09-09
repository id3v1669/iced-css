//! Active/candidate ownership of logical local watch dependencies. Native path
//! identity, registration acknowledgement and session replacement are host work.
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatchOwnerId(NonZeroU64);
impl WatchOwnerId {
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatchDependencyId(NonZeroU64);
impl WatchDependencyId {
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
#[derive(Debug, Clone, Copy)]
pub struct WatchOwnershipLimits {
    pub owners: NonZeroUsize,
    pub bindings: NonZeroUsize,
    pub dependencies: NonZeroUsize,
    /// Combined capacities of one prepared snapshot and its added/removed lists.
    /// Existing snapshots, simultaneous preparations and fixed Arc metadata are separate.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
    pub revisions: NonZeroU64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchOwnershipError {
    Owners,
    Bindings,
    Dependencies,
    Bytes,
    Work,
    Allocation,
    Revisions,
    UnknownOwner,
    SameOwner,
}
impl std::fmt::Display for WatchOwnershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "watch ownership: {self:?}")
    }
}
impl std::error::Error for WatchOwnershipError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Binding {
    owner: WatchOwnerId,
    dependency: WatchDependencyId,
}
#[derive(Debug, Default)]
pub struct WatchSnapshot {
    owners: Vec<WatchOwnerId>,
    bindings: Vec<Binding>,
    dependencies: Vec<WatchDependencyId>,
}
impl WatchSnapshot {
    pub fn owners(&self) -> &[WatchOwnerId] {
        &self.owners
    }
    pub fn dependencies(&self) -> &[WatchDependencyId] {
        &self.dependencies
    }
    /// Allocation-free scan bounded by this snapshot's binding count.
    pub fn owner_dependencies(
        &self,
        owner: WatchOwnerId,
    ) -> impl Iterator<Item = WatchDependencyId> + '_ {
        self.bindings
            .iter()
            .filter_map(move |b| (b.owner == owner).then_some(b.dependency))
    }
}
#[derive(Debug, Default)]
pub struct WatchChanges {
    added: Vec<WatchDependencyId>,
    removed: Vec<WatchDependencyId>,
}
impl WatchChanges {
    pub fn added(&self) -> &[WatchDependencyId] {
        &self.added
    }
    pub fn removed(&self) -> &[WatchDependencyId] {
        &self.removed
    }
}
#[derive(Debug)]
struct Marker;
#[derive(Debug)]
pub struct PreparedWatchUpdate {
    marker: Arc<Marker>,
    base_revision: u64,
    next_revision: u64,
    snapshot: Option<WatchSnapshot>,
    changes: WatchChanges,
    refresh: bool,
}
impl PreparedWatchUpdate {
    /// Explicit coverage replacement, even when the physical dependency union
    /// is unchanged. Such preparations always carry a complete snapshot.
    pub fn refreshes_coverage(&self) -> bool {
        self.refresh
    }
    pub fn changes(&self) -> &WatchChanges {
        &self.changes
    }
    /// None denotes a semantic no-op; native coverage can remain unchanged.
    pub fn candidate_snapshot(&self) -> Option<&WatchSnapshot> {
        self.snapshot.as_ref()
    }
    pub fn next_revision(&self) -> u64 {
        self.next_revision
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchCommitFailure {
    WrongRegistry,
    Stale,
}
#[derive(Debug)]
pub struct RejectedWatchUpdate {
    pub reason: WatchCommitFailure,
    pub prepared: PreparedWatchUpdate,
}
impl std::fmt::Display for RejectedWatchUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "watch commit rejected: {:?}", self.reason)
    }
}
impl std::error::Error for RejectedWatchUpdate {}
#[derive(Debug)]
pub struct WatchCommit {
    pub revision: u64,
    pub changes: WatchChanges,
    pub retired: Option<WatchSnapshot>,
}
pub struct WatchOwnership {
    marker: Arc<Marker>,
    revision: u64,
    snapshot: WatchSnapshot,
    limits: WatchOwnershipLimits,
}
enum Operation<'a> {
    Replace {
        owner: WatchOwnerId,
        dependencies: &'a [WatchDependencyId],
    },
    Release(WatchOwnerId),
    Promote {
        candidate: WatchOwnerId,
        active: WatchOwnerId,
    },
}
impl WatchOwnership {
    /// Owner/dependency IDs are host-scoped; the host must retain their identity
    /// across pending work and must not reuse them for unrelated live resources.
    pub fn new(limits: WatchOwnershipLimits) -> Result<Self, WatchOwnershipError> {
        if limits.retained_bytes.get() > isize::MAX as usize {
            return Err(WatchOwnershipError::Bytes);
        }
        Ok(Self {
            marker: Arc::new(Marker),
            revision: 0,
            snapshot: WatchSnapshot::default(),
            limits,
        })
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn snapshot(&self) -> &WatchSnapshot {
        &self.snapshot
    }
    pub fn prepare_replace(
        &self,
        owner: WatchOwnerId,
        dependencies: &[WatchDependencyId],
    ) -> Result<PreparedWatchUpdate, WatchOwnershipError> {
        self.prepare(
            Operation::Replace {
                owner,
                dependencies,
            },
            false,
        )
    }
    /// Prepare the replacement owner set and require fresh native coverage.
    /// Even an equal set consumes a revision and retains a full bounded snapshot;
    /// this does not invent added/removed dependency changes or acknowledge an OS.
    pub fn prepare_refresh_replace(
        &self,
        owner: WatchOwnerId,
        dependencies: &[WatchDependencyId],
    ) -> Result<PreparedWatchUpdate, WatchOwnershipError> {
        self.prepare(
            Operation::Replace {
                owner,
                dependencies,
            },
            true,
        )
    }
    pub fn prepare_release(
        &self,
        owner: WatchOwnerId,
    ) -> Result<PreparedWatchUpdate, WatchOwnershipError> {
        self.prepare(Operation::Release(owner), false)
    }
    pub fn prepare_promote(
        &self,
        candidate: WatchOwnerId,
        active: WatchOwnerId,
    ) -> Result<PreparedWatchUpdate, WatchOwnershipError> {
        self.prepare(Operation::Promote { candidate, active }, false)
    }
    fn prepare(
        &self,
        operation: Operation<'_>,
        refresh: bool,
    ) -> Result<PreparedWatchUpdate, WatchOwnershipError> {
        let mut budget = Budget {
            bytes: 0,
            work: self.limits.work.get(),
            maximum: self.limits.retained_bytes.get(),
        };
        let mut next = WatchSnapshot::default();
        let (owner, replacement, promote) = match operation {
            Operation::Replace {
                owner,
                dependencies,
            } => {
                if dependencies.len() > self.limits.bindings.get() {
                    return Err(WatchOwnershipError::Bindings);
                }
                (owner, Some(dependencies), None)
            }
            Operation::Release(owner) => (owner, None, None),
            Operation::Promote { candidate, active } => {
                if candidate == active {
                    return Err(WatchOwnershipError::SameOwner);
                }
                if !budget.contains(&self.snapshot.owners, &candidate)? {
                    return Err(WatchOwnershipError::UnknownOwner);
                }
                (candidate, None, Some(active))
            }
        };
        for old in &self.snapshot.owners {
            budget.spend(1)?;
            if *old != owner && Some(*old) != promote {
                budget.push(
                    &mut next.owners,
                    *old,
                    self.limits.owners.get(),
                    WatchOwnershipError::Owners,
                )?;
            }
        }
        if replacement.is_some() || promote.is_some() {
            budget.push(
                &mut next.owners,
                promote.unwrap_or(owner),
                self.limits.owners.get(),
                WatchOwnershipError::Owners,
            )?;
        }
        for old in &self.snapshot.bindings {
            budget.spend(1)?;
            let binding = if let Some(active) = promote {
                if old.owner == active {
                    continue;
                }
                Binding {
                    owner: if old.owner == owner {
                        active
                    } else {
                        old.owner
                    },
                    dependency: old.dependency,
                }
            } else {
                if old.owner == owner {
                    continue;
                }
                *old
            };
            budget.push(
                &mut next.bindings,
                binding,
                self.limits.bindings.get(),
                WatchOwnershipError::Bindings,
            )?;
        }
        if let Some(dependencies) = replacement {
            for dependency in dependencies {
                let binding = Binding {
                    owner,
                    dependency: *dependency,
                };
                if !budget.contains(&next.bindings, &binding)? {
                    budget.push(
                        &mut next.bindings,
                        binding,
                        self.limits.bindings.get(),
                        WatchOwnershipError::Bindings,
                    )?;
                }
            }
        }
        for binding in &next.bindings {
            if !budget.contains(&next.dependencies, &binding.dependency)? {
                budget.push(
                    &mut next.dependencies,
                    binding.dependency,
                    self.limits.dependencies.get(),
                    WatchOwnershipError::Dependencies,
                )?;
            }
        }
        let mut same = next.owners.len() == self.snapshot.owners.len()
            && next.bindings.len() == self.snapshot.bindings.len();
        budget.spend(1)?;
        if same {
            for owner in &next.owners {
                if !budget.contains(&self.snapshot.owners, owner)? {
                    same = false;
                    break;
                }
            }
            if same {
                for binding in &next.bindings {
                    if !budget.contains(&self.snapshot.bindings, binding)? {
                        same = false;
                        break;
                    }
                }
            }
        }
        if same && !refresh {
            return Ok(PreparedWatchUpdate {
                marker: self.marker.clone(),
                base_revision: self.revision,
                next_revision: self.revision,
                snapshot: None,
                changes: WatchChanges::default(),
                refresh: false,
            });
        }
        let revision = self
            .revision
            .checked_add(1)
            .filter(|r| *r <= self.limits.revisions.get())
            .ok_or(WatchOwnershipError::Revisions)?;
        let mut changes = WatchChanges::default();
        for dependency in &next.dependencies {
            if !budget.contains(&self.snapshot.dependencies, dependency)? {
                budget.push(
                    &mut changes.added,
                    *dependency,
                    self.limits.dependencies.get(),
                    WatchOwnershipError::Dependencies,
                )?;
            }
        }
        for dependency in &self.snapshot.dependencies {
            if !budget.contains(&next.dependencies, dependency)? {
                budget.push(
                    &mut changes.removed,
                    *dependency,
                    self.limits.dependencies.get(),
                    WatchOwnershipError::Dependencies,
                )?;
            }
        }
        Ok(PreparedWatchUpdate {
            marker: self.marker.clone(),
            base_revision: self.revision,
            next_revision: revision,
            snapshot: Some(next),
            changes,
            refresh,
        })
    }
    /// Call only after the host has prepared required native coverage. This
    /// registry cannot verify an OS acknowledgement. Retired storage is returned
    /// for disposal after the infallible snapshot/revision transition.
    // Return the original preparation without a new allocation on rejection.
    #[allow(clippy::result_large_err)]
    pub fn commit(
        &mut self,
        prepared: PreparedWatchUpdate,
    ) -> Result<WatchCommit, RejectedWatchUpdate> {
        let failure = if !Arc::ptr_eq(&self.marker, &prepared.marker) {
            Some(WatchCommitFailure::WrongRegistry)
        } else if self.revision != prepared.base_revision {
            Some(WatchCommitFailure::Stale)
        } else {
            None
        };
        if let Some(reason) = failure {
            return Err(RejectedWatchUpdate { reason, prepared });
        }
        let retired = prepared
            .snapshot
            .map(|next| std::mem::replace(&mut self.snapshot, next));
        self.revision = prepared.next_revision;
        Ok(WatchCommit {
            revision: self.revision,
            changes: prepared.changes,
            retired,
        })
    }
}
struct Budget {
    bytes: usize,
    work: usize,
    maximum: usize,
}
impl Budget {
    fn spend(&mut self, n: usize) -> Result<(), WatchOwnershipError> {
        self.work = self.work.checked_sub(n).ok_or(WatchOwnershipError::Work)?;
        Ok(())
    }
    fn contains<T: PartialEq>(
        &mut self,
        items: &[T],
        item: &T,
    ) -> Result<bool, WatchOwnershipError> {
        self.spend(1)?;
        for candidate in items {
            self.spend(1)?;
            if candidate == item {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn push<T>(
        &mut self,
        items: &mut Vec<T>,
        item: T,
        maximum: usize,
        count_error: WatchOwnershipError,
    ) -> Result<(), WatchOwnershipError> {
        self.spend(1)?;
        if items.len() == maximum {
            return Err(count_error);
        }
        if items.len() == items.capacity() {
            let size = std::mem::size_of::<T>();
            // All private callers use nonzero-sized numeric IDs/bindings.
            let old = items.capacity();
            let available = (self.maximum - self.bytes) / size;
            let bound = old.saturating_add(available).min(maximum);
            if old >= bound {
                return Err(WatchOwnershipError::Bytes);
            }
            let next = old.saturating_mul(2).max(1).min(bound);
            self.spend(
                items
                    .len()
                    .checked_mul(size)
                    .ok_or(WatchOwnershipError::Work)?,
            )?;
            items
                .try_reserve_exact(next - items.len())
                .map_err(|_| WatchOwnershipError::Allocation)?;
            if items.capacity() > bound {
                return Err(WatchOwnershipError::Bytes);
            }
            self.bytes = self
                .bytes
                .checked_add((items.capacity() - old) * size)
                .ok_or(WatchOwnershipError::Bytes)?;
        }
        items.push(item);
        Ok(())
    }
}
