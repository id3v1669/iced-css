//! Owned Unix file acquisition on FileReader's bounded background workers.
use super::{
    file_identity::{FileIdentity, FileIdentityPool, IdentityReadError},
    file_path::{FilePathError, FilePathLimits, unix_path},
    local::{ReadBodyError, ReadBodyLimits, read_file_blocking},
    worker::{FileReader, SubmitError, WorkerFailure},
};
use iced_css_core::resources::{
    body::CompletedBody,
    file_uri::{FileUri, FileUriError},
    uri::UriLimits,
};
use std::{
    future::Future,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy)]
pub struct FileUriReadLimits {
    /// Owned input URI capacity and generic/file syntax work.
    pub uri: UriLimits,
    /// Separate native name conversion capacity and work.
    pub path: FilePathLimits,
    pub read: ReadBodyLimits,
}
#[derive(Debug)]
pub enum FileUriSubmitError {
    UriBytes,
    Worker(SubmitError),
}
impl std::fmt::Display for FileUriSubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file URI submission: {self:?}")
    }
}
impl std::error::Error for FileUriSubmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Worker(error) => Some(error),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub enum FileUriReadError {
    Worker(WorkerFailure),
    Uri(FileUriError),
    Path(FilePathError),
    Read(ReadBodyError),
    Identity(IdentityReadError),
}
impl std::fmt::Display for FileUriReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file URI acquisition: {self:?}")
    }
}
impl std::error::Error for FileUriReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Worker(error) => Some(error),
            Self::Uri(error) => Some(error),
            Self::Path(error) => Some(error),
            Self::Read(error) => Some(error),
            Self::Identity(error) => Some(error),
        }
    }
}
/// Completed bytes and exact request metadata, not an atomic filesystem snapshot
/// or a canonical watch/dedup identity. The host owns generation admission.
#[derive(Debug)]
pub struct FileUriAsset {
    acquisition_uri: String,
    native_path: PathBuf,
    body: CompletedBody,
}
impl FileUriAsset {
    pub fn acquisition_uri(&self) -> &str {
        &self.acquisition_uri
    }
    pub fn native_path(&self) -> &Path {
        &self.native_path
    }
    pub fn body(&self) -> &CompletedBody {
        &self.body
    }
    pub fn into_body(self) -> CompletedBody {
        self.body
    }
}
impl FileReader {
    /// Read an already-authorized absolute acquisition URI on Unix. Validation,
    /// name decoding and filesystem I/O run on the worker. The owned Send future
    /// holds admission until consumed/dropped, just like `read_file`.
    pub fn read_file_uri(
        &self,
        uri: String,
        limits: FileUriReadLimits,
    ) -> Result<
        impl Future<Output = Result<FileUriAsset, FileUriReadError>> + Send + 'static + use<>,
        FileUriSubmitError,
    > {
        if limits.uri.bytes.get() > isize::MAX as usize || uri.capacity() > limits.uri.bytes.get() {
            return Err(FileUriSubmitError::UriBytes);
        }
        let work = self
            .submit(move || {
                let parsed = FileUri::parse(&uri, limits.uri).map_err(FileUriReadError::Uri)?;
                let native_path = unix_path(parsed, limits.path).map_err(FileUriReadError::Path)?;
                let body = read_file_blocking(&native_path, limits.read)
                    .map_err(FileUriReadError::Read)?;
                Ok(FileUriAsset {
                    acquisition_uri: uri,
                    native_path,
                    body,
                })
            })
            .map_err(FileUriSubmitError::Worker)?;
        Ok(async move { work.await.map_err(FileUriReadError::Worker)? })
    }
}

/// Completed URI acquisition retaining the actual opened Unix file identity.
/// The identity handle has its own admission, independent of the read worker.
#[derive(Debug)]
pub struct IdentifiedFileUriAsset {
    asset: FileUriAsset,
    identity: FileIdentity,
}
impl IdentifiedFileUriAsset {
    pub fn asset(&self) -> &FileUriAsset {
        &self.asset
    }
    pub fn identity(&self) -> &FileIdentity {
        &self.identity
    }
    pub fn into_parts(self) -> (FileUriAsset, FileIdentity) {
        (self.asset, self.identity)
    }
}
impl FileReader {
    /// Read an authorized URI using a descriptor retained with the result.
    /// Parsing, native opening, identity lookup and body collection run together
    /// on the worker. The host still owns request generation and alias watches.
    ///
    /// # Errors
    /// Submission rejects oversized URI storage or unavailable worker admission.
    /// The future reports syntax, native mapping, handle admission or read errors.
    pub fn read_identified_file_uri(
        &self,
        uri: String,
        limits: FileUriReadLimits,
        identities: FileIdentityPool,
    ) -> Result<
        impl Future<Output = Result<IdentifiedFileUriAsset, FileUriReadError>> + Send + 'static + use<>,
        FileUriSubmitError,
    > {
        if limits.uri.bytes.get() > isize::MAX as usize || uri.capacity() > limits.uri.bytes.get() {
            return Err(FileUriSubmitError::UriBytes);
        }
        let work = self
            .submit(move || {
                let parsed = FileUri::parse(&uri, limits.uri).map_err(FileUriReadError::Uri)?;
                let native_path = unix_path(parsed, limits.path).map_err(FileUriReadError::Path)?;
                let identified = identities
                    .open(&native_path)
                    .and_then(|opened| opened.read(limits.read))
                    .map_err(FileUriReadError::Identity)?;
                let (body, identity) = identified.into_parts();
                Ok(IdentifiedFileUriAsset {
                    asset: FileUriAsset {
                        acquisition_uri: uri,
                        native_path,
                        body,
                    },
                    identity,
                })
            })
            .map_err(FileUriSubmitError::Worker)?;
        Ok(async move { work.await.map_err(FileUriReadError::Worker)? })
    }
}
