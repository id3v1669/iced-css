use crate::{
    identity::InstanceId,
    values::{RangeEvent, registry::PropertyId},
};
use std::num::NonZeroU64;

/// The semantic ID identifies the canonical cause/input/context tuple within one
/// host. Producers must intern that tuple; a source revision is not this identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CauseKey {
    pub instance: InstanceId,
    pub property: PropertyId,
    pub event: RangeEvent,
    pub semantic: NonZeroU64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CauseError {
    InvalidCount,
    InvalidBytes,
    Capacity,
    Allocation,
}
impl std::fmt::Display for CauseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidCount => "invalid live-cause count limit",
            Self::InvalidBytes => "invalid live-cause byte limit",
            Self::Capacity => "live-cause capacity exceeded",
            Self::Allocation => "live-cause allocation failed",
        })
    }
}
impl std::error::Error for CauseError {}
#[derive(Debug, Clone, Copy)]
pub struct CauseLimits {
    count: usize,
    bytes: usize,
}
impl CauseLimits {
    pub fn new(count: usize, bytes: usize) -> Result<Self, CauseError> {
        if count == 0 || count > isize::MAX as usize {
            return Err(CauseError::InvalidCount);
        }
        if bytes == 0 || bytes > isize::MAX as usize {
            return Err(CauseError::InvalidBytes);
        }
        Ok(Self { count, bytes })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    NewCause,
    Suppressed,
}
pub struct CauseTracker {
    limits: CauseLimits,
    live: Vec<CauseKey>,
}
impl CauseTracker {
    pub fn new(limits: CauseLimits) -> Self {
        Self {
            limits,
            live: Vec::new(),
        }
    }
    /// Suppression precedes report admission. A NewCause can still become a queue
    /// omission; a suppressed duplicate must not increment that omission count.
    pub fn observe(&mut self, key: CauseKey) -> Result<Observation, CauseError> {
        if self.live.contains(&key) {
            return Ok(Observation::Suppressed);
        }
        let needed = self.live.len().checked_add(1).ok_or(CauseError::Capacity)?;
        let bytes = needed
            .checked_mul(std::mem::size_of::<CauseKey>())
            .ok_or(CauseError::Capacity)?;
        if needed > self.limits.count || bytes > self.limits.bytes {
            return Err(CauseError::Capacity);
        }
        if self.live.len() == self.live.capacity() {
            // Grow a candidate so allocator rounding cannot mutate retained state
            // when its actual capacity would exceed the byte budget.
            let mut candidate = Vec::new();
            candidate
                .try_reserve_exact(needed)
                .map_err(|_| CauseError::Allocation)?;
            let capacity_bytes = candidate
                .capacity()
                .checked_mul(std::mem::size_of::<CauseKey>())
                .ok_or(CauseError::Capacity)?;
            if capacity_bytes > self.limits.bytes {
                return Err(CauseError::Capacity);
            }
            candidate.extend_from_slice(&self.live);
            candidate.push(key);
            self.live = candidate;
        } else {
            self.live.push(key);
        }
        Ok(Observation::NewCause)
    }
    pub fn resolve(&mut self, key: CauseKey) -> bool {
        let Some(index) = self.live.iter().position(|known| *known == key) else {
            return false;
        };
        self.live.swap_remove(index);
        if self.live.is_empty() {
            self.live = Vec::new();
        }
        true
    }
    pub fn remove_instance(&mut self, instance: InstanceId) -> usize {
        let previous = self.live.len();
        self.live.retain(|key| key.instance != instance);
        let removed = previous - self.live.len();
        if self.live.is_empty() {
            self.live = Vec::new();
        }
        removed
    }
    pub fn retained_bytes(&self) -> usize {
        self.live.capacity() * std::mem::size_of::<CauseKey>()
    }
    pub fn live_count(&self) -> usize {
        self.live.len()
    }
}
