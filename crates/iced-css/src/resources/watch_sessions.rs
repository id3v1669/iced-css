//! Prepare replacement local coverage before atomically committing ownership.
//! The host still supplies stable dependency/path identity and routes stream notices.
use super::watch::{LocalWatchStream, WatchConfig, WatchControl, WatchDelivery, WatchError};
use iced_css_core::resources::watch_ownership::{
    PreparedWatchUpdate, WatchChanges, WatchCommit, WatchCommitFailure, WatchDependencyId,
    WatchOwnership, WatchOwnershipError, WatchOwnershipLimits,
};
use std::{num::NonZeroUsize, path::PathBuf};
#[derive(Debug)]
pub struct WatchDirectoryBinding {
    pub dependency: WatchDependencyId,
    pub directory: PathBuf,
}
#[derive(Debug, Clone, Copy)]
pub struct WatchMappingLimits {
    pub bindings: NonZeroUsize,
    pub input_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug)]
pub enum WatchSessionError {
    Bindings,
    InputBytes,
    OutputBytes,
    Work,
    Allocation,
    UnexpectedBindings,
    MissingDependency(WatchDependencyId),
    UnknownDependency(WatchDependencyId),
    DuplicateDependency(WatchDependencyId),
    Watch(WatchError),
}
impl std::fmt::Display for WatchSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "watch session preparation: {self:?}")
    }
}
impl std::error::Error for WatchSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Watch(e) => Some(e),
            _ => None,
        }
    }
}
pub struct WatchSession {
    control: WatchControl,
    stream: LocalWatchStream,
}
impl std::fmt::Debug for WatchSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatchSession")
            .field("shutdown_requested", &self.control.is_shutdown_requested())
            .finish_non_exhaustive()
    }
}
impl WatchSession {
    pub fn control(&self) -> &WatchControl {
        &self.control
    }
    pub fn stream_mut(&mut self) -> &mut LocalWatchStream {
        &mut self.stream
    }
}
#[derive(Debug)]
enum CoverageChange {
    Keep,
    Replace(Option<WatchSession>),
}
#[derive(Debug)]
pub struct PreparedWatchSession {
    update: PreparedWatchUpdate,
    coverage: CoverageChange,
}
impl PreparedWatchSession {
    pub fn changes(&self) -> &WatchChanges {
        self.update.changes()
    }
    pub fn candidate_control(&self) -> Option<&WatchControl> {
        match &self.coverage {
            CoverageChange::Replace(Some(session)) => Some(session.control()),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub struct RejectedWatchSession {
    pub reason: WatchCommitFailure,
    pub prepared: PreparedWatchSession,
}
impl std::fmt::Display for RejectedWatchSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "watch session commit rejected: {:?}", self.reason)
    }
}
impl std::error::Error for RejectedWatchSession {}
#[derive(Debug)]
pub struct WatchSessionCommit {
    pub ownership: WatchCommit,
    pub retired: Option<WatchSession>,
}
pub struct WatchSessionHost {
    ownership: WatchOwnership,
    active: Option<WatchSession>,
}
impl WatchSessionHost {
    pub fn new(limits: WatchOwnershipLimits) -> Result<Self, WatchOwnershipError> {
        Ok(Self {
            ownership: WatchOwnership::new(limits)?,
            active: None,
        })
    }
    pub fn ownership(&self) -> &WatchOwnership {
        &self.ownership
    }
    pub fn active(&self) -> Option<&WatchSession> {
        self.active.as_ref()
    }
    /// Map into the application's Message. Recipe identity follows the current
    /// coverage session, and receiver cancellation does not release coverage.
    pub fn subscription(&self) -> iced::Subscription<WatchDelivery> {
        self.active
            .as_ref()
            .map_or_else(iced::Subscription::none, |session| {
                session.stream.subscription()
            })
    }
    /// Reject delayed/foreign deliveries even while retired coverage remains live.
    pub fn accepts(&self, delivery: &WatchDelivery) -> bool {
        self.active
            .as_ref()
            .is_some_and(|session| session.stream.accepts(delivery))
    }
    pub fn active_mut(&mut self) -> Option<&mut WatchSession> {
        self.active.as_mut()
    }
    /// Ownership and session moves are infallible after registry admission.
    /// Return the old stream for cancellation outside this commit boundary.
    #[allow(clippy::result_large_err)] // Return the owned preparation without allocating on rejection.
    pub fn commit(
        &mut self,
        prepared: PreparedWatchSession,
    ) -> Result<WatchSessionCommit, RejectedWatchSession> {
        let PreparedWatchSession { update, coverage } = prepared;
        match self.ownership.commit(update) {
            Err(rejected) => Err(RejectedWatchSession {
                reason: rejected.reason,
                prepared: PreparedWatchSession {
                    update: rejected.prepared,
                    coverage,
                },
            }),
            Ok(ownership) => {
                let retired = match coverage {
                    CoverageChange::Keep => None,
                    CoverageChange::Replace(next) => std::mem::replace(&mut self.active, next),
                };
                Ok(WatchSessionCommit { ownership, retired })
            }
        }
    }
}
fn spend(work: &mut usize, amount: usize) -> Result<(), WatchSessionError> {
    *work = work.checked_sub(amount).ok_or(WatchSessionError::Work)?;
    Ok(())
}
/// An owned async preparation can run independently while other updates arrive.
/// Supply the exact candidate-union mapping only when that union changes; use an
/// empty mapping for no-op/owner-only changes unless refreshes_coverage is true.
/// Explicit refresh requires the full union even when no dependencies change.
/// Dependency directory identities
/// must stay stable: a remapped resource needs a different dependency ID.
///
/// `config` applies only to replacement coverage. Native registration failure
/// still permits the explicit periodic local-rescan fallback; this is not an OS
/// acknowledgement that every directory is natively watched. Outstanding prepared
/// sessions and input/output overlap need the host's separate lifetime/heap budget.
pub async fn prepare_watch_session(
    update: PreparedWatchUpdate,
    mappings: Vec<WatchDirectoryBinding>,
    config: WatchConfig,
    limits: WatchMappingLimits,
) -> Result<PreparedWatchSession, WatchSessionError> {
    prepare_watch_session_registered(update, mappings, config, limits, None).await
}
pub(crate) async fn prepare_watch_session_registered(
    update: PreparedWatchUpdate,
    mappings: Vec<WatchDirectoryBinding>,
    config: WatchConfig,
    limits: WatchMappingLimits,
    registration: Option<super::watch_work::WatchWorkRegistrar>,
) -> Result<PreparedWatchSession, WatchSessionError> {
    if mappings.len() > limits.bindings.get() {
        return Err(WatchSessionError::Bindings);
    }
    let mut work = limits.work.get();
    let mut bytes = mappings
        .capacity()
        .checked_mul(std::mem::size_of::<WatchDirectoryBinding>())
        .ok_or(WatchSessionError::InputBytes)?;
    for binding in &mappings {
        spend(&mut work, 1)?;
        bytes = bytes
            .checked_add(binding.directory.capacity())
            .ok_or(WatchSessionError::InputBytes)?;
    }
    if bytes > limits.input_bytes.get() {
        return Err(WatchSessionError::InputBytes);
    }
    if !update.refreshes_coverage()
        && update.changes().added().is_empty()
        && update.changes().removed().is_empty()
    {
        if !mappings.is_empty() {
            return Err(WatchSessionError::UnexpectedBindings);
        }
        return Ok(PreparedWatchSession {
            update,
            coverage: CoverageChange::Keep,
        });
    }
    // A physical diff or explicit refresh carries a complete core snapshot.
    let dependencies = update
        .candidate_snapshot()
        .ok_or(WatchSessionError::UnexpectedBindings)?
        .dependencies();
    for (index, binding) in mappings.iter().enumerate() {
        let mut known = false;
        for key in dependencies {
            spend(&mut work, 1)?;
            if *key == binding.dependency {
                known = true;
                break;
            }
        }
        if !known {
            return Err(WatchSessionError::UnknownDependency(binding.dependency));
        }
        for prior in &mappings[..index] {
            spend(&mut work, 1)?;
            if prior.dependency == binding.dependency {
                return Err(WatchSessionError::DuplicateDependency(binding.dependency));
            }
        }
    }
    for key in dependencies {
        let mut present = false;
        for binding in &mappings {
            spend(&mut work, 1)?;
            if binding.dependency == *key {
                present = true;
                break;
            }
        }
        if !present {
            return Err(WatchSessionError::MissingDependency(*key));
        }
    }
    if dependencies.is_empty() {
        return Ok(PreparedWatchSession {
            update,
            coverage: CoverageChange::Replace(None),
        });
    }
    let mut directories: Vec<PathBuf> = Vec::new();
    let mut path_bytes = 0usize;
    for binding in mappings {
        let mut duplicate = false;
        for existing in &directories {
            spend(
                &mut work,
                existing
                    .as_os_str()
                    .len()
                    .checked_add(binding.directory.as_os_str().len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(WatchSessionError::Work)?,
            )?;
            if existing.as_os_str() == binding.directory.as_os_str() {
                duplicate = true;
                break;
            }
        }
        if duplicate {
            continue;
        }
        if directories.len() == config.directories.get() {
            return Err(WatchSessionError::Watch(WatchError::Directories));
        }
        path_bytes = path_bytes
            .checked_add(binding.directory.capacity())
            .filter(|bytes| *bytes <= config.retained_path_bytes.get())
            .ok_or(WatchSessionError::OutputBytes)?;
        let size = std::mem::size_of::<PathBuf>();
        let bound =
            ((config.retained_path_bytes.get() - path_bytes) / size).min(config.directories.get());
        if directories.capacity() > bound {
            return Err(WatchSessionError::OutputBytes);
        }
        if directories.len() == directories.capacity() {
            let old = directories.capacity();
            if old >= bound {
                return Err(WatchSessionError::OutputBytes);
            }
            let next = old.saturating_mul(2).max(1).min(bound);
            spend(
                &mut work,
                directories
                    .len()
                    .checked_mul(size)
                    .ok_or(WatchSessionError::Work)?,
            )?;
            directories
                .try_reserve_exact(next - directories.len())
                .map_err(|_| WatchSessionError::Allocation)?;
            if directories.capacity() > bound {
                return Err(WatchSessionError::OutputBytes);
            }
        }
        directories.push(binding.directory);
    }
    let (control, stream) = LocalWatchStream::start_registered(directories, config, registration)
        .await
        .map_err(WatchSessionError::Watch)?;
    Ok(PreparedWatchSession {
        update,
        coverage: CoverageChange::Replace(Some(WatchSession { control, stream })),
    })
}
