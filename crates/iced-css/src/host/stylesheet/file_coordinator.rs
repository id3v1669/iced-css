//! Owned local stylesheet Message scheduling for Manual and Auto modes.
use super::{
    file_graph::{
        FileGraphAcquisitionError, FileGraphCompletion, FileGraphConfiguration, FileGraphLimits,
        FileGraphLoadError, FileGraphRequest, FileGraphSource, RuntimeFileStylesheetGraph,
    },
    file_watch::{
        FileGraphWatchConfig, FileGraphWatchError, PreparedFileGraphWatch, RejectedFileGraphWatch,
        prepare_report_watch,
    },
};
use crate::resources::{
    file_dependencies::FileDependencyReport,
    file_imports::root_file_dependencies,
    watch::{WatchControl, WatchDelivery, WatchNotice},
    watch_paths::{WatchPathError, WatchPathRegistry, WatchPathRegistryLimits},
    watch_sessions::{PreparedWatchSession, WatchSessionHost},
    watch_work::{
        WatchWorkError, WatchWorkLease, WatchWorkLimits, WatchWorkOrigin, WatchWorkPool,
        WatchWorkRegistrar,
    },
    worker::FileReader,
};
use iced::{Subscription, Task};
use iced_css_core::{
    resources::{
        publication::{PublicationError, PublicationLimits, PublicationStatus, ResourceRevision},
        triggers::{RequestError, RequestReason, RuntimeMode},
        watch_ownership::{WatchOwnershipError, WatchOwnershipLimits},
    },
    stylesheet::{
        RuleContext,
        identities::{IdentityError, IdentityLimits},
        located_graph::LocatedStylesheetGraph,
        publication::SheetCompletion,
    },
};
use std::{
    future::Future,
    num::NonZeroUsize,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
#[derive(Debug, Clone, Copy)]
pub struct FileStylesheetHostConfig {
    pub work: WatchWorkLimits,
    pub mode: RuntimeMode,
    pub context: RuleContext,
    pub publication: PublicationLimits,
    pub diagnostic_work: NonZeroUsize,
    pub identities: IdentityLimits,
    pub graph: FileGraphLimits,
    pub paths: WatchPathRegistryLimits,
    pub ownership: WatchOwnershipLimits,
    pub watch: FileGraphWatchConfig,
}
#[derive(Debug)]
pub enum FileStylesheetHostError {
    Work(WatchWorkError),
    Identity(IdentityError),
    Paths(WatchPathError),
    Ownership(WatchOwnershipError),
}
impl std::fmt::Display for FileStylesheetHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file stylesheet host: {self:?}")
    }
}
impl std::error::Error for FileStylesheetHostError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Work(e) => e,
            Self::Identity(e) => e,
            Self::Paths(e) => e,
            Self::Ownership(e) => e,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStylesheetRequestError {
    Work(WatchWorkError),
    Request(RequestError),
}
impl std::fmt::Display for FileStylesheetRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file stylesheet request: {self:?}")
    }
}
impl std::error::Error for FileStylesheetRequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Work(e) => e,
            Self::Request(e) => e,
        })
    }
}
#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // Keep admitted graph/session buffers owned without another allocation.
enum Payload {
    RootPrepared {
        request: FileGraphRequest,
        result: Result<PreparedWatchSession, FileGraphAcquisitionError>,
        report: FileDependencyReport,
        lease: WatchWorkLease,
    },
    Acquired {
        completion: FileGraphCompletion,
        lease: WatchWorkLease,
    },
    Prepared {
        result: Result<PreparedFileGraphWatch, RejectedFileGraphWatch>,
        lease: WatchWorkLease,
    },
    Watch(WatchDelivery),
    Closed,
}
/// Cloneable, single-consumption owner/payload/lease envelope. Clones share the
/// same admitted buffers and pending slot; only the first delivery takes payload.
#[derive(Clone)]
pub struct FileStylesheetMessage {
    origin: WatchWorkOrigin,
    payload: Arc<Mutex<Option<Payload>>>,
}
impl std::fmt::Debug for FileStylesheetMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Extract a fixed label, then format outside the envelope lock.
        let kind = match self.payload.lock() {
            Ok(payload) => match payload.as_ref() {
                Some(Payload::RootPrepared { .. }) => "root prepared",
                Some(Payload::Acquired { .. }) => "acquired",
                Some(Payload::Prepared { .. }) => "prepared",
                Some(Payload::Watch(_)) => "watch",
                Some(Payload::Closed) => "closed",
                None => "consumed",
            },
            Err(_) => "faulted",
        };
        f.debug_struct("FileStylesheetMessage")
            .field("kind", &kind)
            .finish_non_exhaustive()
    }
}
impl FileStylesheetMessage {
    fn new(origin: WatchWorkOrigin, payload: Payload) -> Self {
        Self {
            origin,
            payload: Arc::new(Mutex::new(Some(payload))),
        }
    }
    /// Controller lifetime only; graph-generation eligibility is checked by update.
    pub fn is_live(&self) -> bool {
        self.origin.is_live()
    }
    pub fn watch_notice(&self) -> Option<WatchNotice> {
        self.payload
            .lock()
            .ok()
            .and_then(|payload| match payload.as_ref() {
                Some(Payload::Watch(delivery)) => Some(delivery.notice()),
                _ => None,
            })
    }
    /// Read-only preparation control for lifecycle diagnostics. A clone cannot
    /// prevent controller disposal from requesting cancellation of this candidate.
    pub fn candidate_control(&self) -> Option<WatchControl> {
        self.payload
            .lock()
            .ok()
            .and_then(|payload| match payload.as_ref() {
                Some(Payload::RootPrepared {
                    result: Ok(prepared),
                    ..
                }) => prepared.candidate_control().cloned(),
                Some(Payload::Prepared {
                    result: Ok(prepared),
                    ..
                }) => prepared.candidate_control().cloned(),
                _ => None,
            })
    }
    fn closed(origin: WatchWorkOrigin) -> Self {
        Self::new(origin, Payload::Closed)
    }
}
fn watch_message((origin, delivery): (WatchWorkOrigin, WatchDelivery)) -> FileStylesheetMessage {
    FileStylesheetMessage::new(origin, Payload::Watch(delivery))
}
/// Erases the two closed load/preparation future types. This fixed control
/// allocation does not clone stylesheet buffers. Drive it once or map into Task.
pub struct FileStylesheetWork(
    Pin<Box<dyn Future<Output = FileStylesheetMessage> + Send + 'static>>,
);
impl std::fmt::Debug for FileStylesheetWork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileStylesheetWork").finish_non_exhaustive()
    }
}
impl Future for FileStylesheetWork {
    type Output = FileStylesheetMessage;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.as_mut().poll(cx)
    }
}
impl FileStylesheetWork {
    pub fn into_task(self) -> Task<FileStylesheetMessage> {
        Task::perform(self, std::convert::identity)
    }
    fn acquired(
        work: impl Future<Output = FileGraphCompletion> + Send + 'static,
        lease: WatchWorkLease,
        origin: WatchWorkOrigin,
    ) -> Self {
        Self(Box::pin(async move {
            if !lease.is_live() {
                return FileStylesheetMessage::closed(origin);
            }
            let completion = work.await;
            if !lease.is_live() {
                return FileStylesheetMessage::closed(origin);
            }
            FileStylesheetMessage::new(origin, Payload::Acquired { completion, lease })
        }))
    }
    fn prepared(
        work: impl Future<Output = Result<PreparedFileGraphWatch, RejectedFileGraphWatch>>
        + Send
        + 'static,
        lease: WatchWorkLease,
        origin: WatchWorkOrigin,
    ) -> Self {
        Self(Box::pin(async move {
            if !lease.is_live() {
                return FileStylesheetMessage::closed(origin);
            }
            let result = work.await;
            if !lease.is_live() {
                return FileStylesheetMessage::closed(origin);
            }
            FileStylesheetMessage::new(origin, Payload::Prepared { result, lease })
        }))
    }
}
#[derive(Debug, Default)]
pub struct FileStylesheetUpdate {
    /// Terminal current outcome. Ordinary errors have already passed log policy.
    pub outcome: Option<Result<SheetCompletion<FileGraphLoadError>, PublicationError>>,
    pub work: Option<FileStylesheetWork>,
}
/// Component coordinator, not the complete native CssHost. Keep it across view
/// rebuilds and route its owned Messages back here. Only it mutates its graph and
/// watch registry. The scheduler weakly observes its current work phase and coalesces dirty notices;
/// Pending attempts are slot-bounded across futures and Messages. The application
/// still owns its general event queue and must deliver or drop returned work.
pub struct FileStylesheetHost {
    work: WatchWorkPool,
    graph: RuntimeFileStylesheetGraph,
    watches: WatchSessionHost,
    paths: WatchPathRegistry,
    reader: FileReader,
    config: FileStylesheetHostConfig,
    busy: Option<WatchWorkRegistrar>,
    dirty: bool,
}
impl FileStylesheetHost {
    pub fn new(
        reader: FileReader,
        config: FileStylesheetHostConfig,
    ) -> Result<Self, FileStylesheetHostError> {
        Ok(Self {
            work: WatchWorkPool::new(config.work).map_err(FileStylesheetHostError::Work)?,
            graph: RuntimeFileStylesheetGraph::new(
                config.mode,
                config.context,
                config.publication,
                config.diagnostic_work,
                config.identities,
            )
            .map_err(FileStylesheetHostError::Identity)?,
            watches: WatchSessionHost::new(config.ownership)
                .map_err(FileStylesheetHostError::Ownership)?,
            paths: WatchPathRegistry::new(config.paths).map_err(FileStylesheetHostError::Paths)?,
            reader,
            config,
            busy: None,
            dirty: false,
        })
    }
    pub fn active(&self) -> Option<&LocatedStylesheetGraph> {
        self.graph.active()
    }
    pub fn active_revision(&self) -> Option<ResourceRevision> {
        self.graph.active_revision()
    }
    pub fn status(&self) -> PublicationStatus {
        self.graph.status()
    }
    pub fn configuration(&self) -> Option<&FileGraphConfiguration> {
        self.graph.configuration()
    }
    pub fn is_busy(&self) -> bool {
        self.busy.as_ref().is_some_and(WatchWorkRegistrar::is_live)
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn pending_work(&self) -> Result<usize, WatchWorkError> {
        self.work.pending()
    }
    pub fn watch_revision(&self) -> u64 {
        self.watches.ownership().revision()
    }
    pub fn subscription(&self) -> Subscription<FileStylesheetMessage> {
        match self.config.mode {
            RuntimeMode::Auto => self
                .watches
                .subscription()
                .with(self.work.origin())
                .map(watch_message),
            RuntimeMode::Manual => Subscription::none(),
        }
    }
    /// Accept an actual authorized request, superseding old work through the one
    /// graph gate. An accepted fresh read consumes already-known local dirt.
    /// Rejection preserves current busy/dirty state and submits no new job.
    pub fn request(
        &mut self,
        reason: RequestReason,
        source: FileGraphSource,
    ) -> Result<FileStylesheetWork, FileStylesheetRequestError> {
        let lease = self
            .admit_work()
            .map_err(FileStylesheetRequestError::Work)?;
        let request = self
            .graph
            .begin(reason, source, self.config.graph)
            .map_err(FileStylesheetRequestError::Request)?;
        self.busy = Some(lease.registrar());
        self.dirty = false;
        Ok(self.start(request, lease))
    }
    fn start(&mut self, request: FileGraphRequest, lease: WatchWorkLease) -> FileStylesheetWork {
        let origin = self.work.origin();
        let configuration = request
            .configuration()
            .filter(|_| self.config.mode == RuntimeMode::Auto);
        let Some(configuration) = configuration else {
            return FileStylesheetWork::acquired(
                self.graph.collect(&self.reader, request),
                lease,
                origin,
            );
        };
        let (validation, report) =
            root_file_dependencies(&configuration.source().uri, configuration.limits().file);
        let work = match validation {
            Ok(()) => Ok(prepare_report_watch(
                self.graph.active_dependencies(),
                Ok(&report),
                &mut self.paths,
                &self.watches,
                self.config.watch,
                Some(lease.registrar()),
            )),
            Err(error) => Err(FileGraphAcquisitionError::Root(error)),
        };
        FileStylesheetWork(Box::pin(async move {
            if !lease.is_live() {
                return FileStylesheetMessage::closed(origin);
            }
            let result = match work {
                Ok(work) => work.await.map_err(FileGraphAcquisitionError::Watch),
                Err(error) => Err(error),
            };
            if !lease.is_live() {
                return FileStylesheetMessage::closed(origin);
            }
            FileStylesheetMessage::new(
                origin,
                Payload::RootPrepared {
                    request,
                    result,
                    report,
                    lease,
                },
            )
        }))
    }
    pub fn update(&mut self, message: FileStylesheetMessage) -> FileStylesheetUpdate {
        let FileStylesheetMessage { origin, payload } = message;
        if !self.work.owns_origin(&origin) {
            return FileStylesheetUpdate::default();
        }
        // Move owned values out first; publication, logging and destruction must
        // not run under this shared envelope mutex. Duplicate deliveries are inert.
        let incoming = payload
            .lock()
            .map(|mut stored| stored.take())
            .map_err(|_| ());
        let payload = match incoming {
            Ok(Some(payload)) => payload,
            Ok(None) => return FileStylesheetUpdate::default(),
            Err(()) => {
                log::error!(target:"iced_css::stylesheet","file stylesheet Message storage is faulted; payload was not consumed");
                return FileStylesheetUpdate::default();
            }
        };
        let outcome = match payload {
            Payload::Closed => return FileStylesheetUpdate::default(),
            Payload::Watch(delivery) => {
                if self.config.mode == RuntimeMode::Auto
                    && self.watches.accepts(&delivery)
                    && delivery.notice().dirty_all_local
                {
                    self.dirty = true;
                    return FileStylesheetUpdate {
                        outcome: None,
                        work: self.take_dirty(),
                    };
                }
                return FileStylesheetUpdate::default();
            }
            Payload::RootPrepared {
                request,
                result,
                report,
                mut lease,
            } => {
                if !self.graph.is_request_current(&request) {
                    return FileStylesheetUpdate::default();
                }
                let result = result
                    .and_then(|prepared| {
                        self.watches.commit(prepared).map(drop).map_err(|rejected| {
                            FileGraphAcquisitionError::Watch(FileGraphWatchError::Commit(
                                rejected.reason,
                            ))
                        })
                    })
                    .and_then(|()| {
                        lease.advance_registration().map_err(|error| {
                            FileGraphAcquisitionError::Watch(FileGraphWatchError::Work(error))
                        })
                    });
                match result {
                    Ok(()) => {
                        self.busy = Some(lease.registrar());
                        return FileStylesheetUpdate {
                            outcome: None,
                            work: self
                                .graph
                                .acquire_current(&self.reader, request)
                                .map(|work| FileStylesheetWork::acquired(work, lease, origin)),
                        };
                    }
                    Err(error) => {
                        self.busy = None;
                        let outcome = self.graph.fail_request(request, error, Some(report));
                        drop(lease);
                        outcome
                    }
                }
            }
            Payload::Acquired { completion, lease } => {
                if !self.graph.is_current(&completion) {
                    return FileStylesheetUpdate::default();
                }
                // A missing report only comes from an already failed acquisition
                // (rejected input storage or worker failure); finish it directly
                // instead of manufacturing a second coverage failure.
                if self.config.mode == RuntimeMode::Auto
                    && self.graph.pending_dependencies(&completion).is_some()
                {
                    let work = self.graph.prepare_watch_registered(
                        completion,
                        &mut self.paths,
                        &self.watches,
                        self.config.watch,
                        lease.registrar(),
                    );
                    return FileStylesheetUpdate {
                        outcome: None,
                        work: Some(FileStylesheetWork::prepared(work, lease, origin)),
                    };
                }
                self.busy = None;
                let outcome = self.graph.finish(completion);
                drop(lease);
                outcome
            }
            Payload::Prepared { result, lease } => {
                let completion = match &result {
                    Ok(p) => p.completion(),
                    Err(r) => &r.completion,
                };
                if !self.graph.is_current(completion) {
                    return FileStylesheetUpdate::default();
                }
                self.busy = None;
                let outcome = match result {
                    Err(rejected) => self.graph.fail_watch(rejected),
                    Ok(prepared) => {
                        // Replacement coverage registers after the worker read the
                        // files, and the retired session's unread notices are
                        // discarded. Reread once so edits in that window are seen.
                        let replaces = prepared.candidate_control().is_some();
                        match self.graph.finish_watched(prepared, &mut self.watches) {
                            Ok(result) => {
                                if replaces
                                    && matches!(result.sheet, Ok(SheetCompletion::Published(_)))
                                {
                                    self.dirty = true;
                                }
                                result.sheet
                            }
                            Err(rejected) => self.graph.fail_watch(rejected),
                        }
                    }
                };
                drop(lease);
                outcome
            }
        };
        FileStylesheetUpdate {
            outcome: Some(outcome),
            work: self.take_dirty(),
        }
    }
    fn admit_work(&self) -> Result<WatchWorkLease, WatchWorkError> {
        self.work.admit().inspect_err(|error| {
            log::error!(target:"iced_css::stylesheet","file stylesheet work rejected: {error}");
        })
    }
    fn take_dirty(&mut self) -> Option<FileStylesheetWork> {
        if self.is_busy() || !self.dirty || self.config.mode != RuntimeMode::Auto {
            return None;
        }
        let lease = self.admit_work().ok()?;
        match self.graph.begin_retry(RequestReason::LocalChange) {
            Ok(request) => {
                self.busy = Some(lease.registrar());
                self.dirty = false;
                Some(self.start(request, lease))
            }
            Err(_) => None, // Already logged; preserve dirt without a tight retry loop.
        }
    }
}
impl Drop for FileStylesheetHost {
    fn drop(&mut self) {
        self.work.close();
    }
}

#[cfg(test)]
mod message_tests {
    use super::*;
    #[test]
    fn message_debug_formats_only_after_releasing_its_envelope_lock() {
        use std::fmt::Write;
        let pool = WatchWorkPool::new(WatchWorkLimits {
            slots: NonZeroUsize::MIN,
            retained_bytes: NonZeroUsize::new(1000).unwrap(),
        })
        .unwrap();
        let message = FileStylesheetMessage::closed(pool.origin());
        struct Probe<'a>(&'a FileStylesheetMessage);
        impl std::fmt::Write for Probe<'_> {
            fn write_str(&mut self, _: &str) -> std::fmt::Result {
                assert!(self.0.payload.try_lock().is_ok());
                Ok(())
            }
        }
        write!(&mut Probe(&message), "{message:?}").unwrap();
    }
}
