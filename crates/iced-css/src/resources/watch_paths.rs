//! Exact native directory registration keys derived from file dependency reports.
use super::{
    file_dependencies::{FileDependencyError, FileDependencyReport},
    watch_sessions::WatchDirectoryBinding,
};
use iced_css_core::resources::watch_ownership::WatchDependencyId;
use std::{
    ffi::OsString,
    num::{NonZeroU64, NonZeroUsize},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Copy)]
pub struct WatchPathRegistryLimits {
    pub directories: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
}
#[derive(Debug, Clone, Copy)]
pub struct WatchPathLimits {
    /// Used by map_reports; bindings accepts an already-prepared dependency list.
    pub reports: NonZeroUsize,
    pub dependencies: NonZeroUsize,
    /// Output ID capacity, or combined binding/path capacities for bindings.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchPathError {
    RegistryDirectories,
    RegistryBytes,
    OutputBytes,
    Dependencies,
    Reports,
    Work,
    Allocation,
    Identity,
    RejectedReport {
        report: usize,
        reason: FileDependencyError,
    },
    Path {
        report: usize,
        entry: usize,
    },
    UnknownDependency(WatchDependencyId),
    DuplicateDependency(WatchDependencyId),
}
impl std::fmt::Display for WatchPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "watch path mapping: {self:?}")
    }
}
impl std::error::Error for WatchPathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RejectedReport { reason, .. } => Some(reason),
            _ => None,
        }
    }
}
struct Work(usize);
impl Work {
    fn spend(&mut self, n: usize) -> Result<(), WatchPathError> {
        self.0 = self.0.checked_sub(n).ok_or(WatchPathError::Work)?;
        Ok(())
    }
}
struct Directory {
    id: WatchDependencyId,
    path: PathBuf,
}
/// Scope one registry to its watch identity owner. IDs never change or get reused;
/// IDs from another registry must not be mixed into that owner. Native spellings
/// are exact keys, not canonical filesystem identities. Mapping failure can retain
/// newly interned metadata but never changes WatchOwnership or starts a watcher.
pub struct WatchPathRegistry {
    directories: Vec<Directory>,
    path_bytes: usize,
    limits: WatchPathRegistryLimits,
}
impl WatchPathRegistry {
    pub fn new(limits: WatchPathRegistryLimits) -> Result<Self, WatchPathError> {
        if limits.retained_bytes.get() > isize::MAX as usize {
            return Err(WatchPathError::RegistryBytes);
        }
        Ok(Self {
            directories: Vec::new(),
            path_bytes: 0,
            limits,
        })
    }
    pub fn retained_bytes(&self) -> usize {
        self.path_bytes + self.directories.capacity() * size_of::<Directory>()
    }
    /// Unique registration dependencies for these reports. Paths must have an
    /// absolute lexical parent; rejected reports cannot supply complete mappings.
    pub fn map_reports(
        &mut self,
        reports: &[&FileDependencyReport],
        limits: WatchPathLimits,
    ) -> Result<Vec<WatchDependencyId>, WatchPathError> {
        if reports.len() > limits.reports.get() {
            return Err(WatchPathError::Reports);
        }
        if limits.retained_bytes.get() > isize::MAX as usize {
            return Err(WatchPathError::OutputBytes);
        }
        let mut work = Work(limits.work.get());
        for (report, input) in reports.iter().enumerate() {
            work.spend(1)?;
            if let Some(reason) = input.rejection() {
                return Err(WatchPathError::RejectedReport { report, reason });
            }
        }
        let mut dependencies = Vec::new();
        for (report, input) in reports.iter().enumerate() {
            work.spend(1)?;
            for (entry, input) in input.entries().iter().enumerate() {
                let path = input.native_path();
                work.spend(
                    path.as_os_str()
                        .len()
                        .checked_add(1)
                        .ok_or(WatchPathError::Work)?,
                )?;
                let parent = path
                    .parent()
                    .filter(|p| p.is_absolute())
                    .ok_or(WatchPathError::Path { report, entry })?;
                let id = self.intern(parent, &mut work)?;
                let mut seen = false;
                for existing in &dependencies {
                    work.spend(1)?;
                    if *existing == id {
                        seen = true;
                        break;
                    }
                }
                if seen {
                    continue;
                }
                if dependencies.len() >= limits.dependencies.get() {
                    return Err(WatchPathError::Dependencies);
                }
                reserve_output(
                    &mut dependencies,
                    limits.dependencies.get(),
                    limits.retained_bytes.get(),
                    &mut work,
                )?;
                dependencies.push(id);
            }
        }
        Ok(dependencies)
    }
    /// Bind every ID in the prepared ownership union, including older active
    /// owners. Borrowed registry storage is separate from output capacity.
    pub fn bindings(
        &self,
        dependencies: &[WatchDependencyId],
        limits: WatchPathLimits,
    ) -> Result<Vec<WatchDirectoryBinding>, WatchPathError> {
        if dependencies.len() > limits.dependencies.get() {
            return Err(WatchPathError::Dependencies);
        }
        if limits.retained_bytes.get() > isize::MAX as usize {
            return Err(WatchPathError::OutputBytes);
        }
        let mut work = Work(limits.work.get());
        let mut output = Vec::new();
        let mut path_bytes = 0usize;
        for (index, id) in dependencies.iter().enumerate() {
            work.spend(1)?;
            for previous in &dependencies[..index] {
                work.spend(1)?;
                if previous == id {
                    return Err(WatchPathError::DuplicateDependency(*id));
                }
            }
            let mut found = None;
            for entry in &self.directories {
                work.spend(1)?;
                if entry.id == *id {
                    found = Some(&entry.path);
                    break;
                }
            }
            let path = found.ok_or(WatchPathError::UnknownDependency(*id))?;
            let directory = copy_path(
                path,
                limits.retained_bytes.get() - path_bytes,
                &mut work,
                WatchPathError::OutputBytes,
            )?;
            path_bytes = path_bytes
                .checked_add(directory.capacity())
                .filter(|n| *n <= limits.retained_bytes.get())
                .ok_or(WatchPathError::OutputBytes)?;
            reserve_output(
                &mut output,
                limits.dependencies.get(),
                limits.retained_bytes.get() - path_bytes,
                &mut work,
            )?;
            output.push(WatchDirectoryBinding {
                dependency: *id,
                directory,
            });
        }
        Ok(output)
    }
    fn intern(
        &mut self,
        path: &Path,
        work: &mut Work,
    ) -> Result<WatchDependencyId, WatchPathError> {
        for entry in &self.directories {
            work.spend(
                entry
                    .path
                    .as_os_str()
                    .len()
                    .checked_add(path.as_os_str().len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(WatchPathError::Work)?,
            )?;
            if entry.path.as_os_str() == path.as_os_str() {
                return Ok(entry.id);
            }
        }
        if self.directories.len() >= self.limits.directories.get() {
            return Err(WatchPathError::RegistryDirectories);
        }
        let number = self
            .directories
            .len()
            .checked_add(1)
            .and_then(|n| u64::try_from(n).ok())
            .and_then(NonZeroU64::new)
            .ok_or(WatchPathError::Identity)?;
        let id = WatchDependencyId::new(number);
        let limit = self.limits.retained_bytes.get();
        let path = copy_path(
            path,
            limit - self.path_bytes,
            work,
            WatchPathError::RegistryBytes,
        )?;
        let payload = self
            .path_bytes
            .checked_add(path.capacity())
            .filter(|n| *n <= limit)
            .ok_or(WatchPathError::RegistryBytes)?;
        let maximum =
            ((limit - payload) / size_of::<Directory>()).min(self.limits.directories.get());
        if self.directories.capacity() > maximum || self.directories.len() >= maximum {
            return Err(WatchPathError::RegistryBytes);
        }
        if self.directories.len() == self.directories.capacity() {
            let next = self
                .directories
                .capacity()
                .saturating_mul(2)
                .max(1)
                .min(maximum);
            work.spend(
                self.directories
                    .len()
                    .checked_mul(size_of::<Directory>())
                    .ok_or(WatchPathError::Work)?,
            )?;
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(next)
                .map_err(|_| WatchPathError::Allocation)?;
            if entries.capacity() > maximum {
                return Err(WatchPathError::RegistryBytes);
            }
            entries.append(&mut self.directories);
            self.directories = entries;
        }
        self.directories.push(Directory { id, path });
        self.path_bytes = payload;
        Ok(id)
    }
}
fn copy_path(
    path: &Path,
    available: usize,
    work: &mut Work,
    limit_error: WatchPathError,
) -> Result<PathBuf, WatchPathError> {
    let bytes = path.as_os_str().as_bytes();
    if bytes.len() > available {
        return Err(limit_error);
    }
    work.spend(bytes.len())?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| WatchPathError::Allocation)?;
    if owned.capacity() > available {
        return Err(limit_error);
    }
    owned.extend_from_slice(bytes);
    let path = PathBuf::from(OsString::from_vec(owned));
    if path.capacity() > available {
        return Err(limit_error);
    }
    Ok(path)
}
fn reserve_output<T>(
    values: &mut Vec<T>,
    entries: usize,
    bytes: usize,
    work: &mut Work,
) -> Result<(), WatchPathError> {
    let maximum = (bytes / size_of::<T>()).min(entries);
    if values.capacity() > maximum || values.len() >= maximum {
        return Err(WatchPathError::OutputBytes);
    }
    if values.len() == values.capacity() {
        let next = values.capacity().saturating_mul(2).max(1).min(maximum);
        work.spend(
            values
                .len()
                .checked_mul(size_of::<T>())
                .ok_or(WatchPathError::Work)?,
        )?;
        values
            .try_reserve_exact(next - values.len())
            .map_err(|_| WatchPathError::Allocation)?;
        if values.capacity() > maximum {
            return Err(WatchPathError::OutputBytes);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::file_dependencies::FileDependencyLimits;
    #[test]
    fn empty_sets_and_root_paths_do_not_invent_registration_bindings() {
        let n = NonZeroUsize::new(10000).unwrap();
        let mut registry = WatchPathRegistry::new(WatchPathRegistryLimits {
            directories: n,
            retained_bytes: n,
        })
        .unwrap();
        let limits = WatchPathLimits {
            reports: n,
            dependencies: n,
            retained_bytes: n,
            work: n,
        };
        assert!(registry.map_reports(&[], limits).unwrap().is_empty());
        assert!(registry.bindings(&[], limits).unwrap().is_empty());
        let mut root = FileDependencyReport::new(FileDependencyLimits {
            entries: n,
            retained_bytes: n,
            work: n,
        });
        root.record("file:///", PathBuf::from("/")).unwrap();
        assert_eq!(
            registry.map_reports(&[&root], limits).unwrap_err(),
            WatchPathError::Path {
                report: 0,
                entry: 0
            }
        );
        assert_eq!(registry.retained_bytes(), 0);
        let invalid = WatchPathLimits {
            retained_bytes: NonZeroUsize::new(usize::MAX).unwrap(),
            ..limits
        };
        assert_eq!(
            registry.map_reports(&[], invalid).unwrap_err(),
            WatchPathError::OutputBytes
        );
        assert!(matches!(
            registry.bindings(&[], invalid),
            Err(WatchPathError::OutputBytes)
        ));
    }
}
