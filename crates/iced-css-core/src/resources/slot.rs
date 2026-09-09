//! Serialized runtime payload publication using the existing authorization gate.
use super::{
    publication::{
        PublicationDecision, PublicationError, PublicationLimits, PublicationStatus, RequestTicket,
        ResourceRevision,
    },
    triggers::{RequestError, RequestReason, ResourceKind, RuntimeMode, RuntimeResourceGate},
};

/// Returned values remain caller-owned, including obsolete successes and errors.
/// Retired payload destruction happens after publication, under the host's disposal
/// policy, rather than inside the metadata/payload transition.
#[derive(Debug)]
pub enum ResourceCompletion<T, E> {
    Published {
        revision: ResourceRevision,
        retired: Option<T>,
    },
    Failed(E),
    Ignored(Result<T, E>),
    Exhausted {
        error: PublicationError,
        candidate: Result<T, E>,
    },
}
pub struct RuntimeResourceSlot<Key, T> {
    gate: RuntimeResourceGate<Key>,
    active: Option<(ResourceRevision, T)>,
}
impl<Key: Copy + Eq, T> RuntimeResourceSlot<Key, T> {
    /// Key uniqueness/host-lifetime requirements are those of PublicationGate.
    /// This slot allocates no acquisition identity and starts no request itself.
    pub fn new(key: Key, mode: RuntimeMode, kind: ResourceKind, limits: PublicationLimits) -> Self {
        Self {
            gate: RuntimeResourceGate::new(key, mode, kind, limits),
            active: None,
        }
    }
    pub fn status(&self) -> PublicationStatus {
        self.gate.status()
    }
    pub fn active(&self) -> Option<(ResourceRevision, &T)> {
        self.active
            .as_ref()
            .map(|(revision, value)| (*revision, value))
    }
    /// Payload of the latest accepted request only when its state is Ready.
    /// Pending/failed replacement may still retain an active display value.
    /// This makes no HTTP freshness assertion and starts no request. Rejected
    /// request errors must still be handled; they do not create a new generation.
    pub fn ready(&self) -> Option<(ResourceRevision, &T)> {
        if self.status() == PublicationStatus::Ready {
            self.active()
        } else {
            None
        }
    }
    pub fn request(&mut self, reason: RequestReason) -> Result<RequestTicket<Key>, RequestError> {
        self.gate.request(reason)
    }
    /// The caller supplies a fully prepared candidate. After successful admission,
    /// only infallible moves update the active slot; no old value is destroyed here.
    /// Any ignored/failed/exhausted completion preserves the active payload.
    pub fn complete<E>(
        &mut self,
        ticket: RequestTicket<Key>,
        candidate: Result<T, E>,
    ) -> ResourceCompletion<T, E> {
        match self.gate.finish(ticket, candidate.is_ok()) {
            Err(error) => ResourceCompletion::Exhausted { error, candidate },
            Ok(PublicationDecision::Ignored) => ResourceCompletion::Ignored(candidate),
            Ok(PublicationDecision::Failed) => {
                let Err(error) = candidate else {
                    unreachable!("gate failure requires failed candidate")
                };
                ResourceCompletion::Failed(error)
            }
            Ok(PublicationDecision::Publish(revision)) => {
                let Ok(value) = candidate else {
                    unreachable!("gate publication requires successful candidate")
                };
                let retired = self
                    .active
                    .replace((revision, value))
                    .map(|(_, value)| value);
                ResourceCompletion::Published { revision, retired }
            }
        }
    }
}
