//! Explicit embedded notifications and bounded owned iced Message work.
use super::embedded_graph::{
    EmbeddedGraphConfiguration, EmbeddedGraphLoadError, EmbeddedRuntimeCompletion,
    EmbeddedRuntimeLimits, RuntimeEmbeddedStylesheetGraph,
};
use crate::resources::{
    embedded::{EmbeddedDependencyReport, EmbeddedGraphSource, EmbeddedReader, EmbeddedResolver},
    watch_work::{
        WatchWorkError, WatchWorkLease, WatchWorkLimits, WatchWorkOrigin, WatchWorkPool,
        WatchWorkRegistrar,
    },
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
pub struct EmbeddedStylesheetHostConfig {
    pub work: WatchWorkLimits,
    pub mode: RuntimeMode,
    pub context: RuleContext,
    pub publication: PublicationLimits,
    pub diagnostic_work: NonZeroUsize,
    pub identities: IdentityLimits,
    pub runtime: EmbeddedRuntimeLimits,
}
#[derive(Debug)]
pub enum EmbeddedStylesheetHostError {
    Work(WatchWorkError),
    Identity(IdentityError),
}
impl std::fmt::Display for EmbeddedStylesheetHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedded stylesheet host: {self:?}")
    }
}
impl std::error::Error for EmbeddedStylesheetHostError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Work(e) => e,
            Self::Identity(e) => e,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedStylesheetRequestError {
    Work(WatchWorkError),
    Request(RequestError),
}
impl std::fmt::Display for EmbeddedStylesheetRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedded stylesheet request: {self:?}")
    }
}
impl std::error::Error for EmbeddedStylesheetRequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Work(e) => e,
            Self::Request(e) => e,
        })
    }
}
#[allow(clippy::large_enum_variant)] // Keep admitted graph buffers owned without another allocation.
enum Payload<E> {
    Completed {
        completion: EmbeddedRuntimeCompletion<E>,
        lease: WatchWorkLease,
    },
    Notification,
    Closed,
}
/// Clones share one consumed-once payload; neither the error nor graph needs Clone.
pub struct EmbeddedStylesheetMessage<E> {
    origin: WatchWorkOrigin,
    payload: Arc<Mutex<Option<Payload<E>>>>,
}
impl<E> Clone for EmbeddedStylesheetMessage<E> {
    fn clone(&self) -> Self {
        Self {
            origin: self.origin.clone(),
            payload: Arc::clone(&self.payload),
        }
    }
}
impl<E> std::fmt::Debug for EmbeddedStylesheetMessage<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self.payload.lock() {
            Ok(payload) => match payload.as_ref() {
                Some(Payload::Completed { .. }) => "completed",
                Some(Payload::Notification) => "notification",
                Some(Payload::Closed) => "closed",
                None => "consumed",
            },
            Err(_) => "faulted",
        };
        f.debug_struct("EmbeddedStylesheetMessage")
            .field("kind", &kind)
            .finish_non_exhaustive()
    }
}
impl<E> EmbeddedStylesheetMessage<E> {
    fn new(origin: WatchWorkOrigin, payload: Payload<E>) -> Self {
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
pub struct EmbeddedStylesheetWork<E>(
    Pin<Box<dyn Future<Output = EmbeddedStylesheetMessage<E>> + Send + 'static>>,
);
impl<E> std::fmt::Debug for EmbeddedStylesheetWork<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddedStylesheetWork")
            .finish_non_exhaustive()
    }
}
impl<E> Future for EmbeddedStylesheetWork<E> {
    type Output = EmbeddedStylesheetMessage<E>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.as_mut().poll(cx)
    }
}
impl<E: Send + 'static> EmbeddedStylesheetWork<E> {
    pub fn into_task(self) -> Task<EmbeddedStylesheetMessage<E>> {
        Task::perform(self, std::convert::identity)
    }
    fn acquired(
        work: impl Future<Output = EmbeddedRuntimeCompletion<E>> + Send + 'static,
        lease: WatchWorkLease,
        origin: WatchWorkOrigin,
    ) -> Self {
        Self(Box::pin(async move {
            if !lease.is_live() {
                return EmbeddedStylesheetMessage::new(origin, Payload::Closed);
            }
            let completion = work.await;
            if !lease.is_live() {
                return EmbeddedStylesheetMessage::new(origin, Payload::Closed);
            }
            EmbeddedStylesheetMessage::new(origin, Payload::Completed { completion, lease })
        }))
    }
}
pub type EmbeddedStylesheetOutcome<E> =
    Result<SheetCompletion<EmbeddedGraphLoadError<E>>, PublicationError>;
#[derive(Debug)]
pub struct EmbeddedStylesheetUpdate<E> {
    pub outcome: Option<EmbeddedStylesheetOutcome<E>>,
    pub work: Option<EmbeddedStylesheetWork<E>>,
}
impl<E> Default for EmbeddedStylesheetUpdate<E> {
    fn default() -> Self {
        Self {
            outcome: None,
            work: None,
        }
    }
}
/// Component host with explicit notification Messages; no watcher or polling.
/// Pending attempts are bounded separately from the application's general queue.
pub struct EmbeddedStylesheetHost<R: EmbeddedResolver> {
    work: WatchWorkPool,
    graph: RuntimeEmbeddedStylesheetGraph<R>,
    limits: EmbeddedRuntimeLimits,
    busy: Option<WatchWorkRegistrar>,
    dirty: bool,
}
impl<R: EmbeddedResolver> EmbeddedStylesheetHost<R> {
    pub fn new(
        reader: EmbeddedReader<R>,
        config: EmbeddedStylesheetHostConfig,
    ) -> Result<Self, EmbeddedStylesheetHostError> {
        Ok(Self {
            work: WatchWorkPool::new(config.work).map_err(EmbeddedStylesheetHostError::Work)?,
            graph: RuntimeEmbeddedStylesheetGraph::new(
                reader,
                config.mode,
                config.context,
                config.publication,
                config.diagnostic_work,
                config.identities,
            )
            .map_err(EmbeddedStylesheetHostError::Identity)?,
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
    pub fn configuration(&self) -> Option<&EmbeddedGraphConfiguration> {
        self.graph.configuration()
    }
    pub fn active_dependencies(&self) -> Option<&EmbeddedDependencyReport> {
        self.graph.active_dependencies()
    }
    pub fn candidate_dependencies(&self) -> Option<&EmbeddedDependencyReport> {
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
    /// Route this only for an explicit application resolver notification. It is
    /// an assertion of that cause, not permission to poll the resolver for changes.
    pub fn notification(&self) -> EmbeddedStylesheetMessage<R::Error> {
        EmbeddedStylesheetMessage::new(self.work.origin(), Payload::Notification)
    }
    pub fn request(
        &mut self,
        reason: RequestReason,
        source: EmbeddedGraphSource,
    ) -> Result<EmbeddedStylesheetWork<R::Error>, EmbeddedStylesheetRequestError> {
        let lease = self
            .admit_work()
            .map_err(EmbeddedStylesheetRequestError::Work)?;
        let work = self
            .graph
            .request(reason, source, self.limits)
            .map_err(EmbeddedStylesheetRequestError::Request)?;
        self.busy = Some(lease.registrar());
        self.dirty = false;
        Ok(EmbeddedStylesheetWork::acquired(
            work,
            lease,
            self.work.origin(),
        ))
    }
    pub fn update(
        &mut self,
        message: EmbeddedStylesheetMessage<R::Error>,
    ) -> EmbeddedStylesheetUpdate<R::Error> {
        let EmbeddedStylesheetMessage { origin, payload } = message;
        if !self.work.owns_origin(&origin) {
            return EmbeddedStylesheetUpdate::default();
        }
        let incoming = payload
            .lock()
            .map(|mut stored| stored.take())
            .map_err(|_| ());
        let incoming = match incoming {
            Ok(Some(payload)) => payload,
            Ok(None) => return EmbeddedStylesheetUpdate::default(),
            Err(()) => {
                log::error!(target:"iced_css::stylesheet", "embedded stylesheet Message storage is faulted; payload was not consumed");
                return EmbeddedStylesheetUpdate::default();
            }
        };
        match incoming {
            Payload::Closed => EmbeddedStylesheetUpdate::default(),
            Payload::Notification => {
                self.dirty = true;
                EmbeddedStylesheetUpdate {
                    outcome: None,
                    work: self.take_dirty(),
                }
            }
            Payload::Completed { completion, lease } => {
                if !self.graph.is_current(&completion) {
                    return EmbeddedStylesheetUpdate::default();
                }
                self.busy = None;
                let outcome = self.graph.finish(completion);
                drop(lease);
                EmbeddedStylesheetUpdate {
                    outcome: Some(outcome),
                    work: self.take_dirty(),
                }
            }
        }
    }
    fn admit_work(&self) -> Result<WatchWorkLease, WatchWorkError> {
        self.work.admit().inspect_err(|error| { log::error!(target:"iced_css::stylesheet", "embedded stylesheet work rejected: {error}"); })
    }
    fn take_dirty(&mut self) -> Option<EmbeddedStylesheetWork<R::Error>> {
        if self.is_busy() || !self.dirty {
            return None;
        }
        let lease = self.admit_work().ok()?;
        let work = self.graph.retry(RequestReason::EmbeddedNotification).ok()?;
        self.busy = Some(lease.registrar());
        self.dirty = false;
        Some(EmbeddedStylesheetWork::acquired(
            work,
            lease,
            self.work.origin(),
        ))
    }
}
impl<R: EmbeddedResolver> Drop for EmbeddedStylesheetHost<R> {
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
        let message =
            EmbeddedStylesheetMessage::<std::io::Error>::new(pool.origin(), Payload::Notification);
        struct Probe<'a>(&'a EmbeddedStylesheetMessage<std::io::Error>);
        impl std::fmt::Write for Probe<'_> {
            fn write_str(&mut self, _: &str) -> std::fmt::Result {
                assert!(self.0.payload.try_lock().is_ok());
                Ok(())
            }
        }
        write!(&mut Probe(&message), "{message:?}").unwrap();
    }
}
