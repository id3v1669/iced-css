//! Explicit HTTP reload Messages and bounded owned iced Message work.
use super::http_graph::{
    HttpGraphConfiguration, HttpGraphLoadError, HttpRuntimeCompletion, HttpRuntimeLimits,
    RuntimeHttpStylesheetGraph,
};
use crate::resources::{
    http::DirectHttpTransport,
    http_graph::HttpDependencyReport,
    http_stylesheet::HttpStylesheetSource,
    watch_work::{
        WatchWorkError, WatchWorkLease, WatchWorkLimits, WatchWorkOrigin, WatchWorkPool,
        WatchWorkRegistrar,
    },
    worker::FileReader,
};
use iced::Task;
use iced_css_core::{
    resources::{
        publication::{PublicationError, PublicationLimits, PublicationStatus, ResourceRevision},
        triggers::{RequestError, RequestReason, RuntimeMode},
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
pub struct HttpStylesheetHostConfig {
    pub work: WatchWorkLimits,
    pub mode: RuntimeMode,
    pub context: RuleContext,
    pub publication: PublicationLimits,
    pub diagnostic_work: NonZeroUsize,
    pub identities: IdentityLimits,
    pub runtime: HttpRuntimeLimits,
}
#[derive(Debug)]
pub enum HttpStylesheetHostError {
    Work(WatchWorkError),
    Identity(IdentityError),
}
impl std::fmt::Display for HttpStylesheetHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "http stylesheet host: {self:?}")
    }
}
impl std::error::Error for HttpStylesheetHostError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Work(e) => e,
            Self::Identity(e) => e,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpStylesheetRequestError {
    Work(WatchWorkError),
    Request(RequestError),
}
impl std::fmt::Display for HttpStylesheetRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "http stylesheet request: {self:?}")
    }
}
impl std::error::Error for HttpStylesheetRequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Work(e) => e,
            Self::Request(e) => e,
        })
    }
}
#[allow(clippy::large_enum_variant)] // Keep admitted graph buffers owned without another allocation.
enum Payload {
    Completed {
        completion: HttpRuntimeCompletion,
        lease: WatchWorkLease,
    },
    Reload,
    Closed,
}
/// Clones share one consumed-once payload; neither the error nor graph needs Clone.
pub struct HttpStylesheetMessage {
    origin: WatchWorkOrigin,
    payload: Arc<Mutex<Option<Payload>>>,
}
impl Clone for HttpStylesheetMessage {
    fn clone(&self) -> Self {
        Self {
            origin: self.origin.clone(),
            payload: Arc::clone(&self.payload),
        }
    }
}
impl std::fmt::Debug for HttpStylesheetMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self.payload.lock() {
            Ok(payload) => match payload.as_ref() {
                Some(Payload::Completed { .. }) => "completed",
                Some(Payload::Reload) => "reload_message",
                Some(Payload::Closed) => "closed",
                None => "consumed",
            },
            Err(_) => "faulted",
        };
        f.debug_struct("HttpStylesheetMessage")
            .field("kind", &kind)
            .finish_non_exhaustive()
    }
}
impl HttpStylesheetMessage {
    fn new(origin: WatchWorkOrigin, payload: Payload) -> Self {
        Self {
            origin,
            payload: Arc::new(Mutex::new(Some(payload))),
        }
    }
    /// Controller lifetime only; update checks graph-generation eligibility.
    pub fn is_live(&self) -> bool {
        self.origin.is_live()
    }
}
pub struct HttpStylesheetWork(
    Pin<Box<dyn Future<Output = HttpStylesheetMessage> + Send + 'static>>,
);
impl std::fmt::Debug for HttpStylesheetWork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpStylesheetWork").finish_non_exhaustive()
    }
}
impl Future for HttpStylesheetWork {
    type Output = HttpStylesheetMessage;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.as_mut().poll(cx)
    }
}
impl HttpStylesheetWork {
    pub fn into_task(self) -> Task<HttpStylesheetMessage> {
        Task::perform(self, std::convert::identity)
    }
    fn acquired(
        work: impl Future<Output = HttpRuntimeCompletion> + Send + 'static,
        lease: WatchWorkLease,
        origin: WatchWorkOrigin,
    ) -> Self {
        Self(Box::pin(async move {
            if !lease.is_live() {
                return HttpStylesheetMessage::new(origin, Payload::Closed);
            }
            let completion = work.await;
            if !lease.is_live() {
                return HttpStylesheetMessage::new(origin, Payload::Closed);
            }
            HttpStylesheetMessage::new(origin, Payload::Completed { completion, lease })
        }))
    }
}
pub type HttpStylesheetOutcome = Result<SheetCompletion<HttpGraphLoadError>, PublicationError>;
#[derive(Debug, Default)]
pub struct HttpStylesheetUpdate {
    pub outcome: Option<HttpStylesheetOutcome>,
    pub work: Option<HttpStylesheetWork>,
}
/// Component host with explicit reload Messages; no watcher or polling.
/// Pending attempts are bounded separately from the application's general queue.
pub struct HttpStylesheetHost {
    work: WatchWorkPool,
    graph: RuntimeHttpStylesheetGraph,
    limits: HttpRuntimeLimits,
    busy: Option<WatchWorkRegistrar>,
    dirty: bool,
}
impl HttpStylesheetHost {
    pub fn new(
        reader: FileReader,
        transport: Arc<DirectHttpTransport>,
        config: HttpStylesheetHostConfig,
    ) -> Result<Self, HttpStylesheetHostError> {
        Ok(Self {
            work: WatchWorkPool::new(config.work).map_err(HttpStylesheetHostError::Work)?,
            graph: RuntimeHttpStylesheetGraph::new(
                reader,
                transport,
                config.mode,
                config.context,
                config.publication,
                config.diagnostic_work,
                config.identities,
            )
            .map_err(HttpStylesheetHostError::Identity)?,
            limits: config.runtime,
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
    pub fn configuration(&self) -> Option<&HttpGraphConfiguration> {
        self.graph.configuration()
    }
    pub fn active_dependencies(&self) -> Option<&HttpDependencyReport> {
        self.graph.active_dependencies()
    }
    pub fn candidate_dependencies(&self) -> Option<&HttpDependencyReport> {
        self.graph.candidate_dependencies()
    }
    pub fn pending_work(&self) -> Result<usize, WatchWorkError> {
        self.work.pending()
    }
    pub fn is_busy(&self) -> bool {
        self.busy.as_ref().is_some_and(WatchWorkRegistrar::is_live)
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    /// Route this only for an explicit application reload Message. It asserts
    /// that cause; it does not authorize polling, expiry or local-watch refresh.
    pub fn reload_message(&self) -> HttpStylesheetMessage {
        HttpStylesheetMessage::new(self.work.origin(), Payload::Reload)
    }
    pub fn request(
        &mut self,
        reason: RequestReason,
        source: HttpStylesheetSource,
    ) -> Result<HttpStylesheetWork, HttpStylesheetRequestError> {
        let lease = self
            .admit_work()
            .map_err(HttpStylesheetRequestError::Work)?;
        let work = self
            .graph
            .request(reason, source, self.limits)
            .map_err(HttpStylesheetRequestError::Request)?;
        self.busy = Some(lease.registrar());
        self.dirty = false;
        Ok(HttpStylesheetWork::acquired(
            work,
            lease,
            self.work.origin(),
        ))
    }
    pub fn update(&mut self, message: HttpStylesheetMessage) -> HttpStylesheetUpdate {
        let HttpStylesheetMessage { origin, payload } = message;
        if !self.work.owns_origin(&origin) {
            return HttpStylesheetUpdate::default();
        }
        let incoming = payload
            .lock()
            .map(|mut stored| stored.take())
            .map_err(|_| ());
        let incoming = match incoming {
            Ok(Some(payload)) => payload,
            Ok(None) => return HttpStylesheetUpdate::default(),
            Err(()) => {
                log::error!(target:"iced_css::stylesheet", "http stylesheet Message storage is faulted; payload was not consumed");
                return HttpStylesheetUpdate::default();
            }
        };
        match incoming {
            Payload::Closed => HttpStylesheetUpdate::default(),
            Payload::Reload => {
                self.dirty = true;
                HttpStylesheetUpdate {
                    outcome: None,
                    work: self.take_dirty(),
                }
            }
            Payload::Completed { completion, lease } => {
                if !self.graph.is_current(&completion) {
                    return HttpStylesheetUpdate::default();
                }
                self.busy = None;
                let outcome = self.graph.finish(completion);
                drop(lease);
                HttpStylesheetUpdate {
                    outcome: Some(outcome),
                    work: self.take_dirty(),
                }
            }
        }
    }
    fn admit_work(&self) -> Result<WatchWorkLease, WatchWorkError> {
        self.work.admit().inspect_err(|error| {
            log::error!(target:"iced_css::stylesheet", "http stylesheet work rejected: {error}");
        })
    }
    fn take_dirty(&mut self) -> Option<HttpStylesheetWork> {
        if self.is_busy() || !self.dirty {
            return None;
        }
        let lease = self.admit_work().ok()?;
        let work = self.graph.retry(RequestReason::ExplicitReload).ok()?;
        self.busy = Some(lease.registrar());
        self.dirty = false;
        Some(HttpStylesheetWork::acquired(
            work,
            lease,
            self.work.origin(),
        ))
    }
}
impl Drop for HttpStylesheetHost {
    fn drop(&mut self) {
        self.work.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn message_debug_formats_after_unlocking_its_payload() {
        use std::fmt::Write;
        let pool = WatchWorkPool::new(WatchWorkLimits {
            slots: NonZeroUsize::MIN,
            retained_bytes: NonZeroUsize::new(1000).unwrap(),
        })
        .unwrap();
        let message = HttpStylesheetMessage::new(pool.origin(), Payload::Reload);
        struct Probe<'a>(&'a HttpStylesheetMessage);
        impl std::fmt::Write for Probe<'_> {
            fn write_str(&mut self, _: &str) -> std::fmt::Result {
                assert!(self.0.payload.try_lock().is_ok());
                Ok(())
            }
        }
        write!(&mut Probe(&message), "{message:?}").unwrap();
    }
}
