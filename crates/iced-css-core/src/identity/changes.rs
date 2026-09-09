//! Bounded source membership preparation; source topology is validated elsewhere.
use super::{IdentityError, InstanceId, SourceRef, SourceRegistry};
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
};
#[derive(Debug, Clone, Copy)]
pub struct SourceChangeLimits {
    /// Previous live count plus removal count plus insertion count.
    pub entries: NonZeroUsize,
    /// Candidate membership, removed-ID and returned-token Vec capacities.
    /// The old active registry allocation remains separately owned until commit.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceChangeError {
    Identity(IdentityError),
    DuplicateRemoval(InstanceId),
    Entries,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for SourceChangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source change preparation: {self:?}")
    }
}
impl std::error::Error for SourceChangeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(e) => Some(e),
            _ => None,
        }
    }
}
fn reserve<T>(count: usize, bytes: &mut usize) -> Result<Vec<T>, SourceChangeError> {
    let needed = count
        .checked_mul(size_of::<T>())
        .ok_or(SourceChangeError::Bytes)?;
    if needed > *bytes || needed > isize::MAX as usize {
        return Err(SourceChangeError::Bytes);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| SourceChangeError::Allocation)?;
    let actual = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(SourceChangeError::Bytes)?;
    *bytes = bytes.checked_sub(actual).ok_or(SourceChangeError::Bytes)?;
    Ok(values)
}
/// Exclusive prepared membership. New tokens are exposed only by commit.
///
/// ```compile_fail
/// use iced_css_core::identity::PreparedSourceChanges;
/// fn twice(prepared:PreparedSourceChanges<'_>) {
///     prepared.commit();
///     prepared.commit();
/// }
/// ```
/// ```compile_fail
/// use iced_css_core::identity::{SourceRegistry,SourceChangeLimits};
/// fn interleave(registry:&mut SourceRegistry,limits:SourceChangeLimits) {
///     let prepared=registry.prepare_changes(&[],1,limits).unwrap();
///     registry.insert().unwrap();
///     prepared.commit();
/// }
/// ```
#[must_use = "commit source changes or drop the reservation"]
pub struct PreparedSourceChanges<'a> {
    registry: &'a mut SourceRegistry,
    live: Vec<InstanceId>,
    removed: Vec<InstanceId>,
    tokens: Vec<SourceRef>,
    post_next: Option<NonZeroU64>,
}
impl PreparedSourceChanges<'_> {
    /// Already validated same-host live identities, for owner cleanup integration.
    pub fn removed_instances(&self) -> &[InstanceId] {
        &self.removed
    }
    pub fn commit(self) -> Vec<SourceRef> {
        self.registry.live = self.live;
        self.registry.ids.next = self.post_next;
        self.tokens
    }
}
impl SourceRegistry {
    /// Preflight identity changes only. Candidate topology/native compatibility
    /// and animation cleanup must be coordinated by the publishing host.
    pub fn prepare_changes(
        &mut self,
        removals: &[SourceRef],
        insertions: usize,
        limits: SourceChangeLimits,
    ) -> Result<PreparedSourceChanges<'_>, SourceChangeError> {
        let n = self.live.len();
        let r = removals.len();
        let entries = n
            .checked_add(r)
            .and_then(|v| v.checked_add(insertions))
            .ok_or(SourceChangeError::Entries)?;
        if entries > limits.entries.get() {
            return Err(SourceChangeError::Entries);
        }
        // Conservative bounds for token validation, duplicate/removal searches,
        // candidate copying and new ID/token creation. All prepay before walking.
        let work = n
            .checked_add(1)
            .and_then(|v| v.checked_mul(r))
            .and_then(|v| r.checked_mul(r).and_then(|x| v.checked_add(x)))
            .and_then(|v| n.checked_mul(r).and_then(|x| v.checked_add(x)))
            .and_then(|v| entries.checked_mul(2).and_then(|x| v.checked_add(x)))
            .and_then(|v| v.checked_add(3))
            .ok_or(SourceChangeError::Work)?;
        if work > limits.work.get() {
            return Err(SourceChangeError::Work);
        }
        let mut bytes = limits.retained_bytes.get();
        let mut removed = reserve(r, &mut bytes)?;
        for token in removals {
            let instance = self.validate(token).map_err(SourceChangeError::Identity)?;
            if removed.contains(&instance) {
                return Err(SourceChangeError::DuplicateRemoval(instance));
            }
            removed.push(instance);
        }
        let post_next = if insertions == 0 {
            self.ids.next
        } else {
            let first = self
                .ids
                .next
                .ok_or(SourceChangeError::Identity(IdentityError::Exhausted))?;
            let count = u64::try_from(insertions)
                .map_err(|_| SourceChangeError::Identity(IdentityError::Exhausted))?;
            let last = first
                .get()
                .checked_add(count - 1)
                .ok_or(SourceChangeError::Identity(IdentityError::Exhausted))?;
            if last > self.ids.limit.get() {
                return Err(SourceChangeError::Identity(IdentityError::Exhausted));
            }
            if last == self.ids.limit.get() {
                None
            } else {
                last.checked_add(1).and_then(NonZeroU64::new)
            }
        };
        let count = n
            .checked_sub(r)
            .and_then(|v| v.checked_add(insertions))
            .ok_or(SourceChangeError::Entries)?;
        let mut live = reserve(count, &mut bytes)?;
        let mut tokens = reserve(insertions, &mut bytes)?;
        for instance in &self.live {
            if !removed.contains(instance) {
                live.push(*instance);
            }
        }
        let mut next = self.ids.next;
        for _ in 0..insertions {
            let value = next.expect("complete ID range preflighted");
            let instance = InstanceId(value);
            next = value.get().checked_add(1).and_then(NonZeroU64::new);
            live.push(instance);
            tokens.push(SourceRef {
                marker: Arc::clone(&self.marker),
                instance,
            });
        }
        Ok(PreparedSourceChanges {
            registry: self,
            live,
            removed,
            tokens,
            post_next,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_maximum_id_is_committed_once_and_removal_still_works_after_exhaustion() {
        let mut registry = SourceRegistry::new(NonZeroU64::new(u64::MAX).unwrap());
        registry.ids.next = NonZeroU64::new(u64::MAX);
        let limits = SourceChangeLimits {
            entries: NonZeroUsize::new(10).unwrap(),
            retained_bytes: NonZeroUsize::new(1000).unwrap(),
            work: NonZeroUsize::new(1000).unwrap(),
        };
        assert!(matches!(
            registry.prepare_changes(&[], 2, limits),
            Err(SourceChangeError::Identity(IdentityError::Exhausted))
        ));
        let token = registry
            .prepare_changes(&[], 1, limits)
            .unwrap()
            .commit()
            .pop()
            .unwrap();
        assert_eq!(registry.validate(&token).unwrap().get(), u64::MAX);
        assert!(matches!(registry.insert(), Err(IdentityError::Exhausted)));
        registry
            .prepare_changes(std::slice::from_ref(&token), 0, limits)
            .unwrap()
            .commit();
        assert_eq!(registry.validate(&token), Err(IdentityError::DeadInstance));
        assert!(matches!(
            registry.prepare_changes(&[], 1, limits),
            Err(SourceChangeError::Identity(IdentityError::Exhausted))
        ));
    }
}
