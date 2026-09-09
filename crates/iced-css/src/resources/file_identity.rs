//! Unix file identity retained with the descriptor used to acquire body bytes.
//! Blocking operations belong on the resource worker, never a native UI tick.
use super::{
    budget::{Lease, OperationBudget},
    local::{ReadBodyError, ReadBodyLimits, read_body_blocking},
};
use iced_css_core::resources::body::CompletedBody;
use std::{fs::File, io, num::NonZeroUsize, os::unix::fs::MetadataExt, path::Path, sync::Arc};

#[derive(Debug)]
pub enum IdentityReadError {
    Handles,
    Io(io::Error),
    NotRegular,
    Read(ReadBodyError),
}
impl std::fmt::Display for IdentityReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "identified file read: {self:?}")
    }
}
impl std::error::Error for IdentityReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Read(error) => Some(error),
            _ => None,
        }
    }
}
/// Clones share admission for descriptors, including completed retained assets.
/// This budget is separate from worker operations and retained body bytes.
#[derive(Debug, Clone)]
pub struct FileIdentityPool {
    budget: OperationBudget,
}
impl FileIdentityPool {
    pub fn new(maximum: NonZeroUsize) -> Self {
        Self {
            budget: OperationBudget::new(maximum),
        }
    }
    pub fn retained_handles(&self) -> usize {
        self.budget.active()
    }
    /// Open a regular file and obtain identity/length from that same descriptor.
    ///
    /// # Errors
    /// Rejects exhausted admission, open/metadata errors and non-regular files.
    /// File::open can block even when the eventual object is rejected.
    pub fn open(&self, path: &Path) -> Result<OpenedFile, IdentityReadError> {
        let lease = self.budget.acquire().ok_or(IdentityReadError::Handles)?;
        let file = File::open(path).map_err(IdentityReadError::Io)?;
        let metadata = file.metadata().map_err(IdentityReadError::Io)?;
        if !metadata.is_file() {
            return Err(IdentityReadError::NotRegular);
        }
        Ok(OpenedFile {
            identity: FileIdentity {
                file,
                device: metadata.dev(),
                inode: metadata.ino(),
                _lease: lease,
            },
            expected_length: metadata.len(),
        })
    }
}
/// Equality describes simultaneously retained Unix file objects, not content
/// revisions or persistent names. No raw key or clone escapes the open lifetime.
/// Filesystem detach/remount recovery belongs to the resource host.
pub struct FileIdentity {
    // Declaration order closes the descriptor before releasing admission.
    file: File,
    device: u64,
    inode: u64,
    _lease: Arc<Lease>,
}
impl std::fmt::Debug for FileIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileIdentity").finish_non_exhaustive()
    }
}
impl PartialEq for FileIdentity {
    fn eq(&self, other: &Self) -> bool {
        (self.device, self.inode) == (other.device, other.inode)
    }
}
impl Eq for FileIdentity {}
#[derive(Debug)]
pub struct OpenedFile {
    identity: FileIdentity,
    expected_length: u64,
}
impl OpenedFile {
    /// Read the original descriptor through EOF. Concurrent writes can still
    /// change bytes; equality is not evidence of an atomic contents snapshot.
    ///
    /// # Errors
    /// Returns the bounded collector/read error, releasing descriptor admission.
    pub fn read(mut self, limits: ReadBodyLimits) -> Result<IdentifiedFileBody, IdentityReadError> {
        let body = read_body_blocking(&mut self.identity.file, Some(self.expected_length), limits)
            .map_err(IdentityReadError::Read)?;
        Ok(IdentifiedFileBody {
            body,
            identity: self.identity,
        })
    }
}
#[derive(Debug)]
pub struct IdentifiedFileBody {
    body: CompletedBody,
    identity: FileIdentity,
}
impl IdentifiedFileBody {
    pub fn body(&self) -> &CompletedBody {
        &self.body
    }
    pub fn identity(&self) -> &FileIdentity {
        &self.identity
    }
    pub fn into_parts(self) -> (CompletedBody, FileIdentity) {
        (self.body, self.identity)
    }
}
