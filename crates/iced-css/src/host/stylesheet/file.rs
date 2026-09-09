//! Owned completion transport for an already-authorized local stylesheet request.
use super::RuntimeStylesheet;
use crate::resources::{
    stylesheet::{
        BackgroundStylesheetError, StylesheetEncoding, StylesheetReadLimits, StylesheetSubmitError,
    },
    worker::FileReader,
};
use iced_css_core::{
    resources::publication::PublicationError,
    stylesheet::{
        RuleContext, UnicodeStylesheetCandidate,
        publication::{SheetCompletion, StylesheetTicket},
    },
};
use std::{future::Future, path::PathBuf};

#[derive(Debug)]
pub enum FileStylesheetError {
    Submit(StylesheetSubmitError),
    Background(BackgroundStylesheetError),
}
impl std::fmt::Display for FileStylesheetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Submit(error) => error.fmt(f),
            Self::Background(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for FileStylesheetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Submit(error) => error,
            Self::Background(error) => error,
        })
    }
}
/// Carries its private owner-scoped ticket together with the owned result.
/// Deliver to the owner's update path; receiving alone does not publish styles.
/// Once delivered by the future, its storage belongs to the application's queue.
pub struct FileStylesheetCompletion {
    ticket: StylesheetTicket,
    candidate: Result<UnicodeStylesheetCandidate, FileStylesheetError>,
}
impl std::fmt::Debug for FileStylesheetCompletion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileStylesheetCompletion")
            .field("ticket", &self.ticket)
            .field("error", &self.candidate.as_ref().err())
            .finish_non_exhaustive()
    }
}
impl RuntimeStylesheet {
    /// Allocate a request and submit bounded local intake synchronously. The
    /// returned future fits `iced::Task::perform` and borrows neither owner nor
    /// reader. Submission errors travel through the same completion path as read
    /// errors. Request exhaustion returns immediately and submits no worker.
    ///
    /// The caller authorizes the path and delivers the result to `finish_file`.
    /// Dropping the future does not restore an older request generation or publish
    /// a failure. This starts no automatic retry, watcher or import acquisition.
    pub fn request_file(
        &mut self,
        reader: &FileReader,
        path: PathBuf,
        encoding: StylesheetEncoding,
        context: RuleContext,
        limits: StylesheetReadLimits,
    ) -> Result<
        impl Future<Output = FileStylesheetCompletion> + Send + 'static + use<>,
        PublicationError,
    > {
        let ticket = self.request()?;
        let work = reader.read_stylesheet_file(path, encoding, context, limits);
        Ok(async move {
            let candidate = match work {
                Ok(work) => work.await.map_err(FileStylesheetError::Background),
                Err(error) => Err(FileStylesheetError::Submit(error)),
            };
            FileStylesheetCompletion { ticket, candidate }
        })
    }
    /// Consume a completion using the existing last-valid and recovery-log policy.
    /// Foreign and obsolete completions are ignored by the owner-scoped gate.
    pub fn finish_file(
        &mut self,
        completion: FileStylesheetCompletion,
    ) -> Result<SheetCompletion<FileStylesheetError>, PublicationError> {
        self.finish(completion.ticket, completion.candidate)
    }
}
