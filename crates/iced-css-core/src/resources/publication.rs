//! Latest-request admission for one resource, independent of transport/cache policy.
use std::num::NonZeroU64;

#[derive(Debug, Clone, Copy)]
pub struct PublicationLimits {
    pub requests: NonZeroU64,
    pub revisions: NonZeroU64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestTicket<Key> {
    key: Key,
    generation: NonZeroU64,
}
impl<Key> RequestTicket<Key> {
    pub fn generation(&self) -> u64 {
        self.generation.get()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceRevision(NonZeroU64);
impl ResourceRevision {
    pub fn get(self) -> u64 {
        self.0.get()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationStatus {
    Unrequested,
    Loading,
    Ready,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationDecision {
    Ignored,
    Failed,
    Publish(ResourceRevision),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationError {
    Requests,
    Revisions,
}
impl std::fmt::Display for PublicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "resource publication exhausted: {self:?}")
    }
}
impl std::error::Error for PublicationError {}

/// Metadata-only gate. The service supplies a key including its host/lifetime
/// namespace, does not reuse it, and retains this gate across cache eviction.
/// Matching keys are an explicit identity contract, not URI comparison or an
/// allocator supplied here. Request authorization is checked before request().
pub struct PublicationGate<Key> {
    key: Key,
    limits: PublicationLimits,
    next_request: Option<NonZeroU64>,
    next_revision: Option<NonZeroU64>,
    latest: Option<RequestTicket<Key>>,
    active: Option<ResourceRevision>,
    status: PublicationStatus,
}
impl<Key: Copy + Eq> PublicationGate<Key> {
    pub fn new(key: Key, limits: PublicationLimits) -> Self {
        Self {
            key,
            limits,
            next_request: Some(NonZeroU64::MIN),
            next_revision: Some(NonZeroU64::MIN),
            latest: None,
            active: None,
            status: PublicationStatus::Unrequested,
        }
    }
    pub fn status(&self) -> PublicationStatus {
        self.status
    }
    pub fn active_revision(&self) -> Option<ResourceRevision> {
        self.active
    }
    pub fn latest_request(&self) -> Option<RequestTicket<Key>> {
        self.latest
    }
    /// Start an already-authorized request. Exhaustion changes no state.
    pub fn request(&mut self) -> Result<RequestTicket<Key>, PublicationError> {
        let generation = self.next_request.ok_or(PublicationError::Requests)?;
        self.next_request = next(generation, self.limits.requests);
        let ticket = RequestTicket {
            key: self.key,
            generation,
        };
        self.latest = Some(ticket);
        self.status = PublicationStatus::Loading;
        Ok(ticket)
    }
    /// Current pending ticket only. This neither reserves admission nor promises
    /// candidate validity or revision capacity. Recheck after asynchronous work.
    pub fn is_current(&self, ticket: RequestTicket<Key>) -> bool {
        self.status == PublicationStatus::Loading && self.latest == Some(ticket)
    }
    /// Admit a complete, validated candidate or record its failure. Wrong-resource,
    /// obsolete and duplicate results are inert. The serialized host must move the
    /// prepared candidate into its active slot after Publish without fallible work
    /// between these steps; this metadata gate alone is not a host transaction.
    pub fn finish(
        &mut self,
        ticket: RequestTicket<Key>,
        success: bool,
    ) -> Result<PublicationDecision, PublicationError> {
        if !self.is_current(ticket) {
            return Ok(PublicationDecision::Ignored);
        }
        if !success {
            self.status = PublicationStatus::Failed;
            return Ok(PublicationDecision::Failed);
        }
        let Some(revision) = self.next_revision else {
            self.status = PublicationStatus::Failed;
            return Err(PublicationError::Revisions);
        };
        self.next_revision = next(revision, self.limits.revisions);
        let revision = ResourceRevision(revision);
        self.active = Some(revision);
        self.status = PublicationStatus::Ready;
        Ok(PublicationDecision::Publish(revision))
    }
}
fn next(current: NonZeroU64, limit: NonZeroU64) -> Option<NonZeroU64> {
    if current == limit {
        None
    } else {
        current.get().checked_add(1).and_then(NonZeroU64::new)
    }
}
