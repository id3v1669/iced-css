//! Background file operations for iced's existing executor. Admission is bounded;
//! the resource host owns request authorization, pending dispatch and publication.
use super::budget::OperationBudget;
use super::local::{ReadBodyError, ReadBodyLimits, read_file_blocking};
use iced::futures::channel::oneshot;
use iced_css_core::resources::body::CompletedBody;
use std::{
    future::Future,
    io,
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
};

#[derive(Debug, Clone, Copy)]
pub struct BackgroundReadLimits {
    pub read: ReadBodyLimits,
    /// Capacity of the owned native PathBuf, not a Unicode/URI spelling limit.
    pub path_bytes: NonZeroUsize,
}
#[derive(Debug)]
pub enum SubmitError {
    Limit,
    PathBytes,
    Spawn(io::Error),
}
impl std::fmt::Display for SubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "background submission: {self:?}")
    }
}
impl std::error::Error for SubmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error) => Some(error),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerFailure {
    Panicked,
    Stopped,
}
impl std::fmt::Display for WorkerFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "background worker: {self:?}")
    }
}
impl std::error::Error for WorkerFailure {}
#[derive(Debug)]
pub enum BackgroundReadError {
    Worker(WorkerFailure),
    Read(ReadBodyError),
}
impl std::fmt::Display for BackgroundReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "background file read: {self:?}")
    }
}
impl std::error::Error for BackgroundReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Worker(error) => Some(error),
            Self::Read(error) => Some(error),
        }
    }
}
/// Clones share one operation/completion limit. Fixed Arc/channel metadata and
/// native thread stacks are not covered by the per-body retained-byte limit.
#[derive(Debug, Clone)]
pub struct FileReader {
    budget: OperationBudget,
}
impl FileReader {
    pub fn new(maximum: NonZeroUsize) -> Self {
        Self {
            budget: OperationBudget::new(maximum),
        }
    }
    /// Includes unfinished operations and completion futures not consumed/dropped.
    pub fn in_flight(&self) -> usize {
        self.budget.active()
    }
    /// Start an already-authorized read on a native thread. Awaiting the returned
    /// Send future requires no application Tokio runtime. Dropping it cancels work
    /// observed before start, but cannot forcibly interrupt a blocking filesystem
    /// call. The host must reject late results using its resource/lifetime ticket.
    pub fn read_file(
        &self,
        path: PathBuf,
        limits: BackgroundReadLimits,
    ) -> Result<
        impl Future<Output = Result<CompletedBody, BackgroundReadError>> + Send + 'static + use<>,
        SubmitError,
    > {
        if path.capacity() > limits.path_bytes.get() {
            return Err(SubmitError::PathBytes);
        }
        let work = self.submit(move || read_file_blocking(&path, limits.read))?;
        Ok(async move {
            work.await
                .map_err(BackgroundReadError::Worker)?
                .map_err(BackgroundReadError::Read)
        })
    }
    pub(super) fn submit<T: Send + 'static, F: FnOnce() -> T + Send + 'static>(
        &self,
        operation: F,
    ) -> Result<
        impl Future<Output = Result<T, WorkerFailure>> + Send + 'static + use<T, F>,
        SubmitError,
    > {
        let lease = self.budget.acquire().ok_or(SubmitError::Limit)?;
        let worker_lease = lease.clone();
        let (sender, receiver) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("iced-css-resource".into())
            .spawn(move || {
                if sender.is_canceled() {
                    return;
                }
                let result =
                    catch_unwind(AssertUnwindSafe(operation)).map_err(|_| WorkerFailure::Panicked);
                // The operation has returned. An unread future still holds its own
                // lease, so receiving its result can release capacity deterministically.
                drop(worker_lease);
                let _ = sender.send(result);
            })
            .map_err(SubmitError::Spawn)?;
        // Completion travels through the future; no join blocks the caller.
        drop(thread);
        Ok(async move {
            let result = receiver.await.unwrap_or(Err(WorkerFailure::Stopped));
            drop(lease);
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_in_flight_work_retains_its_lease_until_the_operation_returns() {
        use std::{
            sync::mpsc,
            time::{Duration, Instant},
        };
        let reader = FileReader::new(NonZeroUsize::MIN);
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let work = reader
            .submit(move || {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                7u8
            })
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(work);
        assert_eq!(reader.in_flight(), 1);
        assert!(matches!(reader.submit(|| 9u8), Err(SubmitError::Limit)));
        release_tx.send(()).unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        while reader.in_flight() != 0 && Instant::now() < end {
            std::thread::yield_now();
        }
        assert_eq!(reader.in_flight(), 0);
    }
    #[test]
    fn a_worker_panic_is_reported_and_does_not_leak_capacity() {
        let reader = FileReader::new(NonZeroUsize::MIN);
        let work = reader
            .submit(|| -> u8 { panic!("controlled worker panic") })
            .unwrap();
        assert_eq!(
            iced::futures::executor::block_on(work),
            Err(WorkerFailure::Panicked)
        );
        assert_eq!(reader.in_flight(), 0);
        assert_eq!(
            iced::futures::executor::block_on(reader.submit(|| 7u8).unwrap()),
            Ok(7)
        );
    }
}
