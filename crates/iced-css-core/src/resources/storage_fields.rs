//! Generic header-storage roles for a cache without a proxy identity key.
//! Classifying names does not evaluate raw cache restrictions or remove headers.
use super::{
    connection::{ConnectionError, ConnectionOptions},
    http_syntax::tchar,
};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct StorageFieldLimits {
    pub name_bytes: NonZeroUsize,
    /// Shared across all calls on one classifier, including nomination lookup.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageFieldRole {
    /// These generic rules did not exclude the field; not complete storage admission.
    Candidate,
    Connection,
    ConnectionNominated,
    KnownConnectionSpecific,
    ProxySpecific,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageFieldError {
    InvalidName,
    Bytes,
    Work,
    Options(ConnectionError),
}
impl std::fmt::Display for StorageFieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP storage field: {self:?}")
    }
}
impl std::error::Error for StorageFieldError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Options(error) => Some(error),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub struct StorageFieldClassifier<'connection, 'source> {
    options: &'connection ConnectionOptions<'source>,
    name_bytes: NonZeroUsize,
    work: usize,
}
fn spend(work: &mut usize, count: usize) -> Result<(), StorageFieldError> {
    *work = work.checked_sub(count).ok_or(StorageFieldError::Work)?;
    Ok(())
}
fn one_of(name: &[u8], names: &[&[u8]], work: &mut usize) -> Result<bool, StorageFieldError> {
    for known in names {
        spend(
            work,
            known.len().checked_add(1).ok_or(StorageFieldError::Work)?,
        )?;
        if name.eq_ignore_ascii_case(known) {
            return Ok(true);
        }
    }
    Ok(false)
}
impl<'connection, 'source> StorageFieldClassifier<'connection, 'source> {
    pub fn new(
        options: &'connection ConnectionOptions<'source>,
        limits: StorageFieldLimits,
    ) -> Self {
        Self {
            options,
            name_bytes: limits.name_bytes,
            work: limits.work.get(),
        }
    }
    pub fn remaining_work(&self) -> usize {
        self.work
    }
    /// Before applying these roles to storage/removal, preserve and evaluate raw
    /// restrictions. A forbidden Cache-Control nomination must not erase no-store.
    /// This function allocates nothing and never applies its role to a header block.
    pub fn classify(&mut self, name: &[u8]) -> Result<StorageFieldRole, StorageFieldError> {
        if name.len() > self.name_bytes.get() {
            return Err(StorageFieldError::Bytes);
        }
        spend(
            &mut self.work,
            name.len().checked_add(1).ok_or(StorageFieldError::Work)?,
        )?;
        if name.is_empty() || !name.iter().copied().all(tchar) {
            return Err(StorageFieldError::InvalidName);
        }
        if one_of(name, &[b"Connection"], &mut self.work)? {
            return Ok(StorageFieldRole::Connection);
        }
        if one_of(
            name,
            &[
                b"Proxy-Connection",
                b"Keep-Alive",
                b"TE",
                b"Transfer-Encoding",
                b"Upgrade",
            ],
            &mut self.work,
        )? {
            return Ok(StorageFieldRole::KnownConnectionSpecific);
        }
        if one_of(
            name,
            &[
                b"Proxy-Authenticate",
                b"Proxy-Authentication-Info",
                b"Proxy-Authorization",
            ],
            &mut self.work,
        )? {
            return Ok(StorageFieldRole::ProxySpecific);
        }
        let nominated = self
            .options
            .nominates_with_work(name, &mut self.work)
            .map_err(|error| match error {
                ConnectionError::Work => StorageFieldError::Work,
                other => StorageFieldError::Options(other),
            })?;
        Ok(if nominated {
            StorageFieldRole::ConnectionNominated
        } else {
            StorageFieldRole::Candidate
        })
    }
}

/// Caller-established retained processing, not an inference from response headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredProcessing {
    CodedBytes,
    ContentDecoded,
    /// Includes saved media/charset interpretation, even before CSS tree parsing.
    StylesheetPrepared,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderUpdateAction {
    /// Leave existing metadata untouched; never invent a missing stored value.
    PreserveStored,
    ReplaceFromReceived,
    Exclude(StorageFieldRole),
}
impl StorageFieldClassifier<'_, '_> {
    /// RFC9111 §3.2 action for an incoming update field. No header is modified.
    /// The caller must first qualify the response for update and evaluate raw
    /// cache restrictions; each stored block also needs its own storage filtering.
    pub fn classify_update(
        &mut self,
        name: &[u8],
        processing: StoredProcessing,
    ) -> Result<HeaderUpdateAction, StorageFieldError> {
        let role = self.classify(name)?;
        if role != StorageFieldRole::Candidate {
            return Ok(HeaderUpdateAction::Exclude(role));
        }
        if one_of(name, &[b"Content-Length"], &mut self.work)?
            || (processing != StoredProcessing::CodedBytes
                && one_of(name, &[b"Content-Encoding"], &mut self.work)?)
            || (processing == StoredProcessing::StylesheetPrepared
                && one_of(name, &[b"Content-Type"], &mut self.work)?)
        {
            return Ok(HeaderUpdateAction::PreserveStored);
        }
        Ok(HeaderUpdateAction::ReplaceFromReceived)
    }
}
