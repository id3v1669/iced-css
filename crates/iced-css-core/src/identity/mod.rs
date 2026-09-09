//! Host-scoped source identity. CSS IDs and animation track addresses are separate.
use std::num::NonZeroU64;
use std::sync::Arc;
mod changes;
pub use changes::{PreparedSourceChanges, SourceChangeError, SourceChangeLimits};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    Exhausted,
    WrongHost,
    DeadInstance,
    Allocation,
}
impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Exhausted => "source identity exhausted",
            Self::WrongHost => "source token belongs to another host",
            Self::DeadInstance => "source instance is no longer live",
            Self::Allocation => "source identity allocation failed",
        })
    }
}
impl std::error::Error for IdentityError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceId(NonZeroU64);
impl InstanceId {
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// Checked monotonic IDs. Removal never replenishes this lifetime budget.
pub struct IdAllocator {
    next: Option<NonZeroU64>,
    limit: NonZeroU64,
}
impl IdAllocator {
    pub fn new(limit: NonZeroU64) -> Self {
        Self {
            next: NonZeroU64::new(1),
            limit,
        }
    }
    pub fn allocate(&mut self) -> Result<InstanceId, IdentityError> {
        let current = self.next.ok_or(IdentityError::Exhausted)?;
        self.next = if current == self.limit {
            None
        } else {
            current.get().checked_add(1).and_then(NonZeroU64::new)
        };
        Ok(InstanceId(current))
    }
}

/// An owned, nonserializable token. Cloning retains the marker, not source liveness.
#[derive(Debug, Clone)]
pub struct SourceRef {
    marker: Arc<()>,
    instance: InstanceId,
}

/// Low-level host-owned registry; the facade supplies tokens through captured sources.
pub struct SourceRegistry {
    marker: Arc<()>,
    ids: IdAllocator,
    live: Vec<InstanceId>,
}
impl SourceRegistry {
    /// Current live membership count, for bounded host preflight.
    pub fn live_count(&self) -> usize {
        self.live.len()
    }
    pub fn new(limit: NonZeroU64) -> Self {
        Self {
            marker: Arc::new(()),
            ids: IdAllocator::new(limit),
            live: Vec::new(),
        }
    }
    pub fn insert(&mut self) -> Result<SourceRef, IdentityError> {
        if self.ids.next.is_none() {
            return Err(IdentityError::Exhausted);
        }
        self.live
            .try_reserve(1)
            .map_err(|_| IdentityError::Allocation)?;
        let instance = self.ids.allocate()?;
        self.live.push(instance);
        Ok(SourceRef {
            marker: Arc::clone(&self.marker),
            instance,
        })
    }
    pub fn validate(&self, token: &SourceRef) -> Result<InstanceId, IdentityError> {
        if !Arc::ptr_eq(&self.marker, &token.marker) {
            return Err(IdentityError::WrongHost);
        }
        if self.live.binary_search(&token.instance).is_err() {
            return Err(IdentityError::DeadInstance);
        }
        Ok(token.instance)
    }
    pub fn remove(&mut self, token: &SourceRef) -> Result<(), IdentityError> {
        let instance = self.validate(token)?;
        let index = self
            .live
            .binary_search(&instance)
            .expect("validated live instance");
        self.live.remove(index);
        Ok(())
    }
}
