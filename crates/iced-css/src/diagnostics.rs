//! Host-owned diagnostic delivery through unmodified iced's Subscription API.
use iced::{Subscription, futures::Stream};
use iced_css_core::diagnostics::{Admission, Delivery, Diagnostic, QueueLimits};
use std::{
    num::NonZeroU64,
    pin::Pin,
    task::{Context, Poll},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticHostError {
    Faulted,
}
impl std::fmt::Display for DiagnosticHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("diagnostic host faulted")
    }
}
impl std::error::Error for DiagnosticHostError {}
use iced_css_core::diagnostics::DiagnosticQueue;
use std::{
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, Weak},
    task::Waker,
};

struct State {
    queue: DiagnosticQueue,
    alive: bool,
    receiver: Weak<()>,
    waker: Option<Waker>,
}
struct Shared {
    state: Mutex<State>,
}
/// Owned by the CSS host, independently of subscriptions. Dropping it permanently
/// closes production; subscriptions/producers hold weak references only.
pub struct DiagnosticHost {
    shared: Arc<Shared>,
}
#[derive(Clone)]
pub struct DiagnosticProducer {
    shared: Weak<Shared>,
}
pub struct DiagnosticStream {
    shared: Weak<Shared>,
    token: Arc<()>,
}
#[derive(Clone)]
struct SubscriptionData {
    shared: Weak<Shared>,
}
impl Hash for SubscriptionData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.shared.as_ptr().hash(state);
    }
}
impl DiagnosticHost {
    /// `id` is the enclosing host's immutable diagnostic identity, assigned by its
    /// checked allocator. Subscription identity additionally uses this live owner.
    pub fn new(id: NonZeroU64, limits: QueueLimits) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    queue: DiagnosticQueue::new(id, limits),
                    alive: true,
                    receiver: Weak::new(),
                    waker: None,
                }),
            }),
        }
    }
    pub fn producer(&self) -> DiagnosticProducer {
        DiagnosticProducer {
            shared: Arc::downgrade(&self.shared),
        }
    }
    /// Include this subscription while the host is active and map its Delivery into
    /// the application's Message. Recreating this value keeps the same recipe identity.
    pub fn subscription(&self) -> Subscription<Delivery> {
        Subscription::run_with(
            SubscriptionData {
                shared: Arc::downgrade(&self.shared),
            },
            connect,
        )
    }
    /// Fatal state remains queryable without an attached diagnostic subscription.
    pub fn health(&self) -> Result<(), DiagnosticHostError> {
        self.shared
            .state
            .lock()
            .map(|_| ())
            .map_err(|_| DiagnosticHostError::Faulted)
    }
}
impl Drop for DiagnosticHost {
    fn drop(&mut self) {
        let waker = {
            // Poison recovery is only for terminal disposal, never normal continuation.
            let mut state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.alive = false;
            state.queue.close();
            state.receiver = Weak::new();
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}
impl DiagnosticProducer {
    pub fn report(&self, diagnostic: Diagnostic) -> Result<Admission, DiagnosticHostError> {
        let Some(shared) = self.shared.upgrade() else {
            return Ok(Admission::Closed);
        };
        let (admission, waker) = {
            let mut state = shared
                .state
                .lock()
                .map_err(|_| DiagnosticHostError::Faulted)?;
            if !state.alive {
                return Ok(Admission::Closed);
            }
            let admission = state.queue.admit(diagnostic);
            (admission, state.waker.take())
        };
        // Never invoke executor wake callbacks while holding the admission lock.
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(admission)
    }
}
fn connect(data: &SubscriptionData) -> DiagnosticStream {
    let token = Arc::new(());
    if let Some(shared) = data.shared.upgrade() {
        let waker = if let Ok(mut state) = shared.state.lock() {
            state.receiver = Arc::downgrade(&token);
            state.waker.take()
        } else {
            None
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    DiagnosticStream {
        shared: data.shared.clone(),
        token,
    }
}
fn is_receiver(state: &State, token: &Arc<()>) -> bool {
    state
        .receiver
        .upgrade()
        .is_some_and(|current| Arc::ptr_eq(&current, token))
}
impl Stream for DiagnosticStream {
    type Item = Delivery;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Some(shared) = self.shared.upgrade() else {
            return Poll::Ready(None);
        };
        let next_waker = cx.waker().clone();
        let (result, old_waker) = {
            let Ok(mut state) = shared.state.lock() else {
                return Poll::Ready(None);
            };
            if !state.alive || !is_receiver(&state, &self.token) {
                return Poll::Ready(None);
            }
            match state.queue.handoff() {
                Some(delivery) => (Poll::Ready(Some(delivery)), state.waker.take()),
                None => (Poll::Pending, state.waker.replace(next_waker)),
            }
        };
        drop(old_waker);
        // No await/callback occurs between taking a queue item and yielding it.
        result
    }
}
impl Drop for DiagnosticStream {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.upgrade() {
            let waker = if let Ok(mut state) = shared.state.lock() {
                if is_receiver(&state, &self.token) {
                    state.receiver = Weak::new();
                    state.waker.take()
                } else {
                    None
                }
            } else {
                None
            };
            drop(waker);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn poisoned_host_error_is_direct_even_without_a_subscription() {
        let host = DiagnosticHost::new(NonZeroU64::MIN, QueueLimits::new(1, 1).unwrap());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = host.shared.state.lock().unwrap();
            panic!("controlled host fault");
        }));
        assert!(result.is_err());
        assert_eq!(host.health(), Err(DiagnosticHostError::Faulted));
        // Drop uses terminal disposal of poisoned state, without normal continuation.
        drop(host);
    }
}
