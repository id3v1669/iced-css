//! Bounded evidence of local paths requested by one file import collection.
use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Copy)]
pub struct FileDependencyLimits {
    pub entries: NonZeroUsize,
    /// Combined record, URI and native-path capacities; temporary growth excluded.
    pub retained_bytes: NonZeroUsize,
    /// Shared across all recording and duplicate checks in this report.
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDependencyError {
    Entries,
    Bytes,
    Work,
    Allocation,
}
impl std::fmt::Display for FileDependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file dependency report: {self:?}")
    }
}
impl std::error::Error for FileDependencyError {}
#[derive(Debug)]
pub struct FileDependency {
    uri: String,
    path: PathBuf,
}
impl FileDependency {
    pub fn acquisition_uri(&self) -> &str {
        &self.uri
    }
    pub fn native_path(&self) -> &Path {
        &self.path
    }
}
/// Exact URI-keyed attempted-input evidence, not canonical filesystem identity or
/// a native watch acknowledgement. A partial rejected report remains inspectable.
#[derive(Debug)]
pub struct FileDependencyReport {
    entries: Vec<FileDependency>,
    payload_bytes: usize,
    work: usize,
    limits: FileDependencyLimits,
    rejected: Option<FileDependencyError>,
}
impl FileDependencyReport {
    pub(crate) fn new(limits: FileDependencyLimits) -> Self {
        Self {
            entries: Vec::new(),
            payload_bytes: 0,
            work: limits.work.get(),
            limits,
            rejected: None,
        }
    }
    pub fn entries(&self) -> &[FileDependency] {
        &self.entries
    }
    pub fn rejection(&self) -> Option<FileDependencyError> {
        self.rejected
    }
    pub fn retained_bytes(&self) -> usize {
        self.payload_bytes + self.entries.capacity() * size_of::<FileDependency>()
    }
    pub(super) fn record(
        &mut self,
        uri: &str,
        path: PathBuf,
    ) -> Result<usize, FileDependencyError> {
        if let Some(error) = self.rejected {
            return Err(error);
        }
        let result = self.record_inner(uri, path);
        if let Err(error) = result {
            self.rejected = Some(error);
        }
        result
    }
    fn spend(&mut self, amount: usize) -> Result<(), FileDependencyError> {
        self.work = self
            .work
            .checked_sub(amount)
            .ok_or(FileDependencyError::Work)?;
        Ok(())
    }
    fn record_inner(&mut self, uri: &str, path: PathBuf) -> Result<usize, FileDependencyError> {
        let limit = self.limits.retained_bytes.get();
        if limit > isize::MAX as usize {
            return Err(FileDependencyError::Bytes);
        }
        for index in 0..self.entries.len() {
            self.spend(1)?;
            if self.entries[index].uri.len() == uri.len() {
                self.spend(uri.len())?;
                if self.entries[index].uri == uri {
                    return Ok(index);
                }
            }
        }
        if self.entries.len() >= self.limits.entries.get() {
            return Err(FileDependencyError::Entries);
        }
        let native = self
            .payload_bytes
            .checked_add(path.capacity())
            .filter(|n| *n <= limit)
            .ok_or(FileDependencyError::Bytes)?;
        if uri.len() > limit - native {
            return Err(FileDependencyError::Bytes);
        }
        self.spend(uri.len())?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(uri.len())
            .map_err(|_| FileDependencyError::Allocation)?;
        let payload = native
            .checked_add(owned.capacity())
            .filter(|n| *n <= limit)
            .ok_or(FileDependencyError::Bytes)?;
        owned.push_str(uri);
        let maximum =
            ((limit - payload) / size_of::<FileDependency>()).min(self.limits.entries.get());
        if self.entries.capacity() > maximum || self.entries.len() >= maximum {
            return Err(FileDependencyError::Bytes);
        }
        if self.entries.len() == self.entries.capacity() {
            let next = self
                .entries
                .capacity()
                .saturating_mul(2)
                .max(1)
                .min(maximum);
            self.spend(
                self.entries
                    .len()
                    .checked_mul(size_of::<FileDependency>())
                    .ok_or(FileDependencyError::Work)?,
            )?;
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(next)
                .map_err(|_| FileDependencyError::Allocation)?;
            if entries.capacity() > maximum {
                return Err(FileDependencyError::Bytes);
            }
            entries.append(&mut self.entries);
            self.entries = entries;
        }
        let index = self.entries.len();
        self.entries.push(FileDependency { uri: owned, path });
        self.payload_bytes = payload;
        Ok(index)
    }
}
