//! Read and parse one local stylesheet within the existing bounded worker lease.
use super::{
    local::{ReadBodyError, read_file_blocking},
    worker::{BackgroundReadLimits, FileReader, SubmitError, WorkerFailure},
};
use iced_css_core::{
    stylesheet::{
        RuleContext, StylesheetIntakeError, StylesheetIntakeLimits, UnicodeStylesheetCandidate,
        parse_separate_unicode_stylesheet,
    },
    syntax::encoding_selection::EncodingMetadata,
};
use std::{future::Future, num::NonZeroUsize, path::PathBuf};

/// Owned external encoding labels for worker transport. Default explicitly means
/// that no external label was supplied; CSS intake still performs its detection.
#[derive(Debug, Default)]
pub struct StylesheetEncoding {
    pub transport: Option<Vec<u8>>,
    pub linking: Option<Vec<u8>>,
    pub referring: Option<Vec<u8>>,
}
impl StylesheetEncoding {
    pub(crate) fn metadata(&self) -> EncodingMetadata<'_> {
        EncodingMetadata {
            transport: self.transport.as_deref(),
            linking: self.linking.as_deref(),
            referring: self.referring.as_deref(),
        }
    }
    pub(crate) fn retained_bytes(&self) -> Option<usize> {
        [&self.transport, &self.linking, &self.referring]
            .into_iter()
            .try_fold(0usize, |sum, label| {
                sum.checked_add(label.as_ref().map_or(0, Vec::capacity))
            })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct StylesheetReadLimits {
    pub read: BackgroundReadLimits,
    /// Combined capacities of the three owned encoding-label vectors.
    pub metadata_bytes: NonZeroUsize,
    pub intake: StylesheetIntakeLimits,
}
#[derive(Debug)]
pub enum StylesheetSubmitError {
    MetadataBytes,
    Worker(SubmitError),
}
impl std::fmt::Display for StylesheetSubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stylesheet submission: {self:?}")
    }
}
impl std::error::Error for StylesheetSubmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Worker(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub enum BackgroundStylesheetError {
    Worker(WorkerFailure),
    Read(ReadBodyError),
    Intake(StylesheetIntakeError),
}
impl std::fmt::Display for BackgroundStylesheetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "background stylesheet: {self:?}")
    }
}
impl std::error::Error for BackgroundStylesheetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Worker(e) => e,
            Self::Read(e) => e,
            Self::Intake(e) => e,
        })
    }
}
impl FileReader {
    /// Execute bounded reading and parsing on one resource worker. The original
    /// body and decoded candidate may coexist during intake; their limits are
    /// separate and do not include native thread/channel metadata.
    ///
    /// The caller authorizes the request and publishes with its owner-scoped
    /// ticket. This starts no import acquisition, watcher, web retry or native UI.
    pub fn read_stylesheet_file(
        &self,
        path: PathBuf,
        encoding: StylesheetEncoding,
        context: RuleContext,
        limits: StylesheetReadLimits,
    ) -> Result<
        impl Future<Output = Result<UnicodeStylesheetCandidate, BackgroundStylesheetError>>
        + Send
        + 'static
        + use<>,
        StylesheetSubmitError,
    > {
        if path.capacity() > limits.read.path_bytes.get() {
            return Err(StylesheetSubmitError::Worker(SubmitError::PathBytes));
        }
        if encoding
            .retained_bytes()
            .is_none_or(|bytes| bytes > limits.metadata_bytes.get())
        {
            return Err(StylesheetSubmitError::MetadataBytes);
        }
        let work = self
            .submit(move || {
                let body = read_file_blocking(&path, limits.read.read)
                    .map_err(BackgroundStylesheetError::Read)?;
                parse_separate_unicode_stylesheet(
                    body.as_slice(),
                    encoding.metadata(),
                    context,
                    limits.intake,
                )
                .map_err(BackgroundStylesheetError::Intake)
            })
            .map_err(StylesheetSubmitError::Worker)?;
        Ok(async move { work.await.map_err(BackgroundStylesheetError::Worker)? })
    }
}
