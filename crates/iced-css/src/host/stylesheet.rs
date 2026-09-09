//! Single parsed stylesheet ownership and rejected-request logging.
use iced_css_core::{
    resources::publication::{
        PublicationError, PublicationLimits, PublicationStatus, ResourceRevision,
    },
    stylesheet::{
        UnicodeStylesheetCandidate,
        publication::{SheetCompletion, StylesheetPublication, StylesheetTicket},
    },
};

pub mod file;
pub mod graph;
pub mod graph_recovery;
mod import_diagnostics;
mod recovery;

/// Runtime parsed-resource boundary, not a complete stylesheet/import-set host.
/// The application installs/configures its log sink; this library does not replace
/// a global logger. Recovery reporting has a separate positive work budget.
/// Exact rejected-input matches suppress repeated screen diagnostics.
pub struct RuntimeStylesheet {
    recovery_work: std::num::NonZeroUsize,
    publication: StylesheetPublication,
}
impl RuntimeStylesheet {
    pub fn new(limits: PublicationLimits, recovery_work: std::num::NonZeroUsize) -> Self {
        Self {
            recovery_work,
            publication: StylesheetPublication::new(limits),
        }
    }
    /// Start an already-authorized request; does not schedule I/O or web retries.
    pub fn request(&mut self) -> Result<StylesheetTicket, PublicationError> {
        let result = self.publication.request();
        if let Err(error) = &result {
            log::error!(target: "iced_css::stylesheet", "stylesheet request rejected: {error}");
        }
        result
    }
    pub fn active(&self) -> Option<&UnicodeStylesheetCandidate> {
        self.publication.active()
    }
    pub fn active_revision(&self) -> Option<ResourceRevision> {
        self.publication.active_revision()
    }
    pub fn status(&self) -> PublicationStatus {
        self.publication.status()
    }
    /// Apply publication policy before reporting a current-request failure.
    /// Stale, foreign and duplicate completions produce no log. The original
    /// error remains available in the return value. Application logger callbacks
    /// are outside this boundary; their panics are not caught or replaced.
    pub fn finish<E: std::fmt::Display>(
        &mut self,
        ticket: StylesheetTicket,
        candidate: Result<UnicodeStylesheetCandidate, E>,
    ) -> Result<SheetCompletion<E>, PublicationError> {
        let generation = ticket.generation();
        let result = self.publication.finish_with_previous(ticket, candidate);
        if let Ok((SheetCompletion::Published(_), previous)) = &result
            && let Some(active) = self.publication.active()
            && let Err(error) = recovery::report(active, previous.as_ref(), self.recovery_work)
        {
            log::error!(target: "iced_css::stylesheet", "stylesheet request {generation} recovery diagnostics omitted: {error:?}");
        }
        match &result {
            Ok((SheetCompletion::Failed(error), _)) => {
                log::error!(target: "iced_css::stylesheet", "stylesheet request {generation} failed: {error}");
            }
            Err(error) => {
                log::error!(target: "iced_css::stylesheet", "stylesheet request {generation} publication failed: {error}");
            }
            _ => {}
        }
        result.map(|(completion, _)| completion)
    }
}

#[cfg(unix)]
pub mod file_graph;

#[cfg(unix)]
pub mod file_watch;

#[cfg(unix)]
pub mod file_coordinator;

pub mod embedded_graph;

pub mod embedded_coordinator;

pub mod http_graph;

pub mod http_coordinator;
