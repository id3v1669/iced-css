//! Sections 17/65 runtime request authorization. Compile acquisition and
//! actual Message routing remain outside this local-file/web/embedded gate.
use super::publication::{
    PublicationDecision, PublicationError, PublicationGate, PublicationLimits, PublicationStatus,
    RequestTicket, ResourceRevision,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Manual,
    Auto,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    LocalFile,
    Web,
    Embedded,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestReason {
    FirstAcquisition,
    ExplicitReload,
    LocalChange,
    /// An explicit resolver notification Message, not a passive callback poll.
    EmbeddedNotification,
    Reevaluation,
    CacheExpiry,
    CacheEviction,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestError {
    Unauthorized,
    Exhausted(PublicationError),
}
impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "resource request: {self:?}")
    }
}
impl std::error::Error for RequestError {}

/// Retain this gate for the acquisition identity across view rebuilds and cache
/// eviction. Key namespace/uniqueness is the same owner contract as PublicationGate.
/// Authorization returns a ticket; this type itself starts no transport or task.
pub struct RuntimeResourceGate<Key> {
    publication: PublicationGate<Key>,
    mode: RuntimeMode,
    kind: ResourceKind,
}
impl<Key: Copy + Eq> RuntimeResourceGate<Key> {
    pub fn new(key: Key, mode: RuntimeMode, kind: ResourceKind, limits: PublicationLimits) -> Self {
        Self {
            publication: PublicationGate::new(key, limits),
            mode,
            kind,
        }
    }
    pub fn status(&self) -> PublicationStatus {
        self.publication.status()
    }
    pub fn active_revision(&self) -> Option<ResourceRevision> {
        self.publication.active_revision()
    }
    pub fn request(&mut self, reason: RequestReason) -> Result<RequestTicket<Key>, RequestError> {
        let allowed = request_allowed(
            self.mode,
            self.kind,
            reason,
            self.publication.latest_request().is_none(),
        );
        if !allowed {
            return Err(RequestError::Unauthorized);
        }
        self.publication.request().map_err(RequestError::Exhausted)
    }
    pub fn finish(
        &mut self,
        ticket: RequestTicket<Key>,
        success: bool,
    ) -> Result<PublicationDecision, PublicationError> {
        self.publication.finish(ticket, success)
    }
}

/// Shared section17/65 routing predicate. The owner supplies whether no request
/// has yet been accepted; a reason is a routing assertion, not proof of a Message.
/// This creates no ticket, publication state, task or transport operation.
pub fn request_allowed(
    mode: RuntimeMode,
    kind: ResourceKind,
    reason: RequestReason,
    first_available: bool,
) -> bool {
    match reason {
        RequestReason::FirstAcquisition => first_available,
        RequestReason::ExplicitReload => true,
        RequestReason::LocalChange => mode == RuntimeMode::Auto && kind == ResourceKind::LocalFile,
        RequestReason::EmbeddedNotification => kind == ResourceKind::Embedded,
        RequestReason::Reevaluation | RequestReason::CacheExpiry | RequestReason::CacheEviction => {
            false
        }
    }
}
