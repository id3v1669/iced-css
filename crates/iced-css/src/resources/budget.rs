use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
#[derive(Debug)]
struct Counter {
    maximum: usize,
    active: AtomicUsize,
}
pub(super) struct Lease {
    counter: Arc<Counter>,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.counter.active.fetch_sub(1, Ordering::AcqRel);
    }
}
#[derive(Debug, Clone)]
pub(super) struct OperationBudget {
    counter: Arc<Counter>,
}
impl OperationBudget {
    pub(super) fn new(maximum: NonZeroUsize) -> Self {
        Self {
            counter: Arc::new(Counter {
                maximum: maximum.get(),
                active: AtomicUsize::new(0),
            }),
        }
    }
    pub(super) fn active(&self) -> usize {
        self.counter.active.load(Ordering::Acquire)
    }
    pub(super) fn acquire(&self) -> Option<Arc<Lease>> {
        self.counter
            .active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.counter.maximum).then(|| active + 1)
            })
            .ok()?;
        Some(Arc::new(Lease {
            counter: self.counter.clone(),
        }))
    }
}
