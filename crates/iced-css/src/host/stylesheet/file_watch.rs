//! Completion-coupled watch preparation. Automatic retry and Message routing are
//! separate from this explicit, fallible preparation/publication boundary.
use super::file_graph::{FileGraphCompletion, FileGraphLoadError, RuntimeFileStylesheetGraph};
use crate::resources::{
    watch::{WatchConfig, WatchControl},
    watch_paths::{WatchPathError, WatchPathLimits, WatchPathRegistry},
    watch_sessions::{
        PreparedWatchSession, WatchMappingLimits, WatchSessionCommit, WatchSessionError,
        WatchSessionHost, prepare_watch_session_registered,
    },
};
use iced_css_core::{
    resources::{
        publication::PublicationError,
        watch_ownership::{WatchCommitFailure, WatchOwnerId, WatchOwnershipError},
    },
    stylesheet::publication::SheetCompletion,
};
use std::future::Future;
#[derive(Debug, Clone, Copy)]
pub struct FileGraphWatchConfig {
    /// Keep this ID stable for this graph within the same watch host/registry.
    pub owner: WatchOwnerId,
    pub paths: WatchPathLimits,
    pub mapping: WatchMappingLimits,
    pub watch: WatchConfig,
}
#[derive(Debug)]
pub enum FileGraphWatchError {
    Obsolete,
    MissingReport,
    Paths(WatchPathError),
    Ownership(WatchOwnershipError),
    Session(WatchSessionError),
    Commit(WatchCommitFailure),
    Work(crate::resources::watch_work::WatchWorkError),
}
impl std::fmt::Display for FileGraphWatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file graph watch preparation: {self:?}")
    }
}
impl std::error::Error for FileGraphWatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Work(e) => Some(e),
            Self::Paths(e) => Some(e),
            Self::Ownership(e) => Some(e),
            Self::Session(e) => Some(e),
            _ => None,
        }
    }
}
/// No graph or watch state was committed. The caller retains the exact input for
/// explicit retry/error handling; no automatic loop or application logging occurs.
#[derive(Debug)]
pub struct RejectedFileGraphWatch {
    pub reason: FileGraphWatchError,
    pub completion: FileGraphCompletion,
}
impl std::fmt::Display for RejectedFileGraphWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.reason.fmt(f)
    }
}
impl std::error::Error for RejectedFileGraphWatch {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.reason)
    }
}
#[derive(Debug)]
pub struct PreparedFileGraphWatch {
    completion: FileGraphCompletion,
    watch: PreparedWatchSession,
}
impl PreparedFileGraphWatch {
    pub(super) fn completion(&self) -> &FileGraphCompletion {
        &self.completion
    }
    pub fn candidate_control(&self) -> Option<&WatchControl> {
        self.watch.candidate_control()
    }
}
#[derive(Debug)]
pub struct FileGraphWatchCommit {
    pub sheet: Result<SheetCompletion<FileGraphLoadError>, PublicationError>,
    /// None for an obsolete graph. Dispose retired coverage after commit.
    pub watch: Option<WatchSessionCommit>,
}
impl RuntimeFileStylesheetGraph {
    /// Borrow only during preparation of bounded directory/ownership metadata.
    /// The resulting future owns both the graph completion and watch preparation.
    /// Report/mapping errors return the input without publication or registration.
    /// A missing report cannot establish replacement coverage.
    pub fn prepare_watch(
        &self,
        completion: FileGraphCompletion,
        paths: &mut WatchPathRegistry,
        watches: &WatchSessionHost,
        config: FileGraphWatchConfig,
    ) -> impl Future<Output = Result<PreparedFileGraphWatch, RejectedFileGraphWatch>>
    + Send
    + 'static
    + use<> {
        self.prepare_watch_with_registration(completion, paths, watches, config, None)
    }
    pub(super) fn prepare_watch_registered(
        &self,
        completion: FileGraphCompletion,
        paths: &mut WatchPathRegistry,
        watches: &WatchSessionHost,
        config: FileGraphWatchConfig,
        registration: crate::resources::watch_work::WatchWorkRegistrar,
    ) -> impl Future<Output = Result<PreparedFileGraphWatch, RejectedFileGraphWatch>>
    + Send
    + 'static
    + use<> {
        self.prepare_watch_with_registration(completion, paths, watches, config, Some(registration))
    }
    fn prepare_watch_with_registration(
        &self,
        completion: FileGraphCompletion,
        paths: &mut WatchPathRegistry,
        watches: &WatchSessionHost,
        config: FileGraphWatchConfig,
        registration: Option<crate::resources::watch_work::WatchWorkRegistrar>,
    ) -> impl Future<Output = Result<PreparedFileGraphWatch, RejectedFileGraphWatch>>
    + Send
    + 'static
    + use<> {
        let pending = if self.is_current(&completion) {
            self.pending_dependencies(&completion)
                .ok_or(FileGraphWatchError::MissingReport)
        } else {
            Err(FileGraphWatchError::Obsolete)
        };
        let work = prepare_report_watch(
            self.active_dependencies(),
            pending,
            paths,
            watches,
            config,
            registration,
        );
        async move {
            let result = work.await;
            match result {
                Ok(watch) => Ok(PreparedFileGraphWatch { completion, watch }),
                Err(reason) => Err(RejectedFileGraphWatch { reason, completion }),
            }
        }
    }
    /// Recheck the graph, then commit compatible coverage before ordinary graph
    /// publication. The coverage union retains all previous active paths even if
    /// publication fails. Successful publication may retain previous paths until
    /// the next preparation. No caller callbacks occur between these admissions.
    #[allow(clippy::result_large_err)] // Preserve owned retry input without allocation.
    pub fn finish_watched(
        &mut self,
        prepared: PreparedFileGraphWatch,
        watches: &mut WatchSessionHost,
    ) -> Result<FileGraphWatchCommit, RejectedFileGraphWatch> {
        let PreparedFileGraphWatch { completion, watch } = prepared;
        if !self.is_current(&completion) {
            return Ok(FileGraphWatchCommit {
                sheet: Ok(SheetCompletion::Ignored),
                watch: None,
            });
        }
        match watches.commit(watch) {
            Err(rejected) => Err(RejectedFileGraphWatch {
                reason: FileGraphWatchError::Commit(rejected.reason),
                completion,
            }),
            Ok(watch) => Ok(FileGraphWatchCommit {
                sheet: self.finish(completion),
                watch: Some(watch),
            }),
        }
    }
}

/// Shared bounded preparation for root metadata and collected import reports.
pub(super) fn prepare_report_watch(
    active: Option<&crate::resources::file_dependencies::FileDependencyReport>,
    pending: Result<
        &crate::resources::file_dependencies::FileDependencyReport,
        FileGraphWatchError,
    >,
    paths: &mut WatchPathRegistry,
    watches: &WatchSessionHost,
    config: FileGraphWatchConfig,
    registration: Option<crate::resources::watch_work::WatchWorkRegistrar>,
) -> impl Future<Output = Result<PreparedWatchSession, FileGraphWatchError>> + Send + 'static + use<>
{
    let input = (|| {
        let pending = pending?;
        let dependencies = match active {
            Some(active) => paths.map_reports(&[active, pending], config.paths),
            None => paths.map_reports(&[pending], config.paths),
        }
        .map_err(FileGraphWatchError::Paths)?;
        let refresh = watches
            .active()
            .is_some_and(|session| session.control().is_shutdown_requested());
        let update = if refresh {
            watches
                .ownership()
                .prepare_refresh_replace(config.owner, &dependencies)
        } else {
            watches
                .ownership()
                .prepare_replace(config.owner, &dependencies)
        }
        .map_err(FileGraphWatchError::Ownership)?;
        let bindings = if !update.refreshes_coverage()
            && update.changes().added().is_empty()
            && update.changes().removed().is_empty()
        {
            Vec::new()
        } else {
            // A physical diff or refresh requires a complete snapshot. Handle absence
            // explicitly rather than panic if that invariant changes.
            let snapshot = update
                .candidate_snapshot()
                .ok_or(FileGraphWatchError::Session(
                    WatchSessionError::UnexpectedBindings,
                ))?;
            paths
                .bindings(snapshot.dependencies(), config.paths)
                .map_err(FileGraphWatchError::Paths)?
        };
        Ok((update, bindings))
    })();
    async move {
        let (update, bindings) = input?;
        prepare_watch_session_registered(
            update,
            bindings,
            config.watch,
            config.mapping,
            registration,
        )
        .await
        .map_err(FileGraphWatchError::Session)
    }
}
