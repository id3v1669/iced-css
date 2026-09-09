//! Owner-scoped whole-candidate publication, defaulting to one parsed stylesheet.
use super::UnicodeStylesheetCandidate;
use crate::resources::publication::{
    PublicationDecision, PublicationError, PublicationGate, PublicationLimits, PublicationStatus,
    RequestTicket, ResourceRevision,
};
use std::sync::Arc;

/// Scoped request capability. Cloning permits worker transport without making a
/// duplicate completion eligible. The marker retains no stylesheet or host.
#[derive(Clone, Debug)]
pub struct StylesheetTicket {
    owner: Arc<()>,
    request: RequestTicket<()>,
}
impl StylesheetTicket {
    pub fn generation(&self) -> u64 {
        self.request.generation()
    }
}
#[derive(Debug)]
pub enum SheetCompletion<E> {
    Ignored,
    /// The current request failed. Caller routes this original error to logs.
    Failed(E),
    Published(ResourceRevision),
}
/// Concrete single-stylesheet API; request-only callers need no type annotation.
pub type StylesheetPublication = CandidatePublication<UnicodeStylesheetCandidate>;

/// Owns the last accepted candidate together with its publication metadata.
/// Candidate construction supplies validation and storage bounds; this slot does
/// not inspect or clone it. The default is one parsed stylesheet; an owned import
/// graph can use the same publication policy. Host integration, semantic
/// equivalence and diagnostic deduplication remain separate.
pub struct CandidatePublication<Candidate> {
    marker: Arc<()>,
    gate: PublicationGate<()>,
    active: Option<Candidate>,
}
impl<Candidate> CandidatePublication<Candidate> {
    pub fn new(limits: PublicationLimits) -> Self {
        Self {
            marker: Arc::new(()),
            gate: PublicationGate::new((), limits),
            active: None,
        }
    }
    /// Begin an already-authorized request. No I/O or parsing is performed here.
    pub fn request(&mut self) -> Result<StylesheetTicket, PublicationError> {
        let request = self.gate.request()?;
        Ok(StylesheetTicket {
            owner: self.marker.clone(),
            request,
        })
    }
    pub fn active(&self) -> Option<&Candidate> {
        self.active.as_ref()
    }
    pub fn active_revision(&self) -> Option<ResourceRevision> {
        self.gate.active_revision()
    }
    pub fn status(&self) -> PublicationStatus {
        self.gate.status()
    }
    /// Read-only owner/generation/pending check. It reserves nothing and is not
    /// a promise that a successful candidate can obtain another revision.
    pub fn is_current(&self, ticket: &StylesheetTicket) -> bool {
        Arc::ptr_eq(&self.marker, &ticket.owner) && self.gate.is_current(ticket.request)
    }
    /// Publish a complete usable candidate, including successful CSS recovery or
    /// an empty stylesheet. A failed newest request retains active content; first
    /// failure leaves it absent. Stale/foreign/duplicate completions are inert.
    /// No callback or fallible operation separates gate admission and replacement.
    pub fn finish<E>(
        &mut self,
        ticket: StylesheetTicket,
        candidate: Result<Candidate, E>,
    ) -> Result<SheetCompletion<E>, PublicationError> {
        self.finish_with_previous(ticket, candidate)
            .map(|(completion, _)| completion)
    }
    /// As finish, returning the replaced candidate for post-publication diagnostics.
    /// No candidate is cloned; old and new storage temporarily coexist.
    pub fn finish_with_previous<E>(
        &mut self,
        ticket: StylesheetTicket,
        candidate: Result<Candidate, E>,
    ) -> Result<(SheetCompletion<E>, Option<Candidate>), PublicationError> {
        if !self.is_current(&ticket) {
            return Ok((SheetCompletion::Ignored, None));
        }
        let decision = self.gate.finish(ticket.request, candidate.is_ok())?;
        match (decision, candidate) {
            (PublicationDecision::Publish(revision), Ok(candidate)) => {
                let previous = self.active.replace(candidate);
                Ok((SheetCompletion::Published(revision), previous))
            }
            (PublicationDecision::Failed, Err(error)) => Ok((SheetCompletion::Failed(error), None)),
            _ => Ok((SheetCompletion::Ignored, None)),
        }
    }
}
