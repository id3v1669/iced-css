//! Explicit dynamic capture limits, usable independently of a CSS host.
use super::CaptureLimits;
use std::cell::Cell;

thread_local! {
    static CURRENT: Cell<Option<CaptureLimits>> = const { Cell::new(None) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextError {
    Missing,
}
impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("capture requires an explicit CaptureContext or host capture scope")
    }
}
impl std::error::Error for ContextError {}

/// Return the innermost explicit configuration on this thread. No host or
/// process default is inferred. Limits keep their per-operation meaning.
pub fn current_limits() -> Result<CaptureLimits, ContextError> {
    CURRENT.with(Cell::get).ok_or(ContextError::Missing)
}

/// Immutable configuration; constructing it does not activate a scope.
/// Contains no widget, host registry or borrowed application state.
#[derive(Debug, Clone, Copy)]
pub struct CaptureContext {
    limits: CaptureLimits,
}
impl CaptureContext {
    pub fn new(limits: CaptureLimits) -> Self {
        Self { limits }
    }
    /// Run synchronous construction with explicit limits, restoring the previous
    /// scope on return or unwind. Panics propagate unchanged. Nested explicit
    /// scopes override this one only until their closure exits.
    ///
    /// Returned values may borrow the caller. A returned future is *not* scoped
    /// during later polls; do not use this as an asynchronous context mechanism.
    /// Spawned threads do not inherit configuration.
    pub fn run<T>(&self, build: impl FnOnce() -> T) -> T {
        let previous = CURRENT.with(|current| current.replace(Some(self.limits)));
        let _restore = Restore(previous);
        build()
    }
}
struct Restore(Option<CaptureLimits>);
impl Drop for Restore {
    fn drop(&mut self) {
        CURRENT.with(|current| current.set(self.0));
    }
}

impl CaptureContext {
    /// Scope each poll of a caller-pinned future without allocating or activating
    /// configuration between polls. Does not scope creation or destruction of
    /// the borrowed future. The caller retains ownership of its pinned storage.
    pub fn scope_future<'a, F: std::future::Future + ?Sized>(
        &self,
        future: std::pin::Pin<&'a mut F>,
    ) -> ScopedCaptureFuture<'a, F> {
        ScopedCaptureFuture {
            context: *self,
            future,
        }
    }
}

/// Borrows pinned storage and owns only the explicit configuration. The wrapper
/// itself is movable; it never moves the underlying future or stores an active
/// thread-local guard across a suspension point.
#[must_use = "futures do nothing unless polled"]
pub struct ScopedCaptureFuture<'a, F: ?Sized> {
    context: CaptureContext,
    future: std::pin::Pin<&'a mut F>,
}
impl<F: std::future::Future + ?Sized> std::future::Future for ScopedCaptureFuture<'_, F> {
    type Output = F::Output;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        this.context.run(|| this.future.as_mut().poll(cx))
    }
}

/// Entry-time choice for an ordinary public helper. This value does not activate
/// a scope or build a view; branch before consuming the helper's inputs.
#[derive(Debug, Clone, Copy)]
pub enum PublicCaptureEntry {
    Capture(CaptureLimits),
    /// Run the ordinary native view once after reporting the missing setup.
    Unstyled,
}

/// Select capture under the innermost explicit configuration, otherwise report
/// the setup error once and select the ordinary unstyled view. No limits are
/// invented. The reporter owns logging, attribution and cross-entry deduplication;
/// a reporter panic propagates before a view branch is selected.
///
/// This is only the public entry decision: an error from the subsequently selected
/// captured body must not retry through Unstyled. Async helpers select when their
/// bodies run during polling, using the active poll-scoped configuration.
pub fn public_entry(report_missing: impl FnOnce(ContextError)) -> PublicCaptureEntry {
    match current_limits() {
        Ok(limits) => PublicCaptureEntry::Capture(limits),
        Err(error) => {
            report_missing(error);
            PublicCaptureEntry::Unstyled
        }
    }
}

/// Public helper entry using the facade's existing `log` backend. Missing setup
/// emits an Error record under `iced_css::capture`, with the calling location,
/// then selects the ordinary unstyled view. The application owns logger/filter
/// configuration; this neither installs a logger nor creates a capture scope.
#[track_caller]
pub fn public_entry_logged() -> PublicCaptureEntry {
    let caller = std::panic::Location::caller();
    public_entry(|error| {
        log::error!(target: "iced_css::capture", "{error} at {}:{}:{}; returning the ordinary unstyled iced view", caller.file(), caller.line(), caller.column());
    })
}
