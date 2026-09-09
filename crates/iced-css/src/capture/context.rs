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
