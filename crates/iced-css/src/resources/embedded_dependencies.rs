//! Bounded contextual attempted-input and successful-version evidence.
use iced_css_core::syntax::imports::ResourceId;
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct EmbeddedDependencyLimits {
    pub entries: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedDependencyError {
    Entries,
    Bytes,
    Work,
    Allocation,
    Target,
}
impl std::fmt::Display for EmbeddedDependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedded dependency report: {self:?}")
    }
}
impl std::error::Error for EmbeddedDependencyError {}
#[derive(Debug)]
pub struct EmbeddedDependency {
    source: ResourceId,
    uri: String,
    version: Option<Vec<u8>>,
}
impl EmbeddedDependency {
    pub fn source(&self) -> ResourceId {
        self.source
    }
    pub fn acquisition_uri(&self) -> &str {
        &self.uri
    }
    pub fn version(&self) -> Option<&[u8]> {
        self.version.as_deref()
    }
}
#[derive(Debug)]
pub struct EmbeddedDependencyReport {
    entries: Vec<EmbeddedDependency>,
    payload_bytes: usize,
    work: usize,
    limits: EmbeddedDependencyLimits,
    rejected: Option<EmbeddedDependencyError>,
}
impl EmbeddedDependencyReport {
    pub(super) fn new(limits: EmbeddedDependencyLimits) -> Self {
        Self {
            entries: Vec::new(),
            payload_bytes: 0,
            work: limits.work.get(),
            limits,
            rejected: None,
        }
    }
    pub fn entries(&self) -> &[EmbeddedDependency] {
        &self.entries
    }
    pub fn rejection(&self) -> Option<EmbeddedDependencyError> {
        self.rejected
    }
    pub fn retained_bytes(&self) -> usize {
        self.payload_bytes + self.entries.capacity() * size_of::<EmbeddedDependency>()
    }
    fn spend(&mut self, work: usize) -> Result<(), EmbeddedDependencyError> {
        self.work = self
            .work
            .checked_sub(work)
            .ok_or(EmbeddedDependencyError::Work)?;
        Ok(())
    }
    pub(super) fn record(
        &mut self,
        source: ResourceId,
        uri: &str,
    ) -> Result<usize, EmbeddedDependencyError> {
        if let Some(error) = self.rejected {
            return Err(error);
        }
        let result = self.record_inner(source, uri);
        if let Err(error) = result {
            self.rejected = Some(error);
        }
        result
    }
    fn record_inner(
        &mut self,
        source: ResourceId,
        uri: &str,
    ) -> Result<usize, EmbeddedDependencyError> {
        let limit = self.limits.retained_bytes.get();
        if limit > isize::MAX as usize {
            return Err(EmbeddedDependencyError::Bytes);
        }
        for index in 0..self.entries.len() {
            self.spend(1)?;
            if self.entries[index].source == source {
                self.spend(uri.len())?;
                return if self.entries[index].uri == uri {
                    Ok(index)
                } else {
                    Err(EmbeddedDependencyError::Target)
                };
            }
        }
        if self.entries.len() >= self.limits.entries.get() {
            return Err(EmbeddedDependencyError::Entries);
        }
        if uri.len() > limit - self.payload_bytes {
            return Err(EmbeddedDependencyError::Bytes);
        }
        self.spend(uri.len())?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(uri.len())
            .map_err(|_| EmbeddedDependencyError::Allocation)?;
        let payload = self
            .payload_bytes
            .checked_add(owned.capacity())
            .filter(|n| *n <= limit)
            .ok_or(EmbeddedDependencyError::Bytes)?;
        owned.push_str(uri);
        let maximum =
            ((limit - payload) / size_of::<EmbeddedDependency>()).min(self.limits.entries.get());
        if self.entries.capacity() > maximum || self.entries.len() >= maximum {
            return Err(EmbeddedDependencyError::Bytes);
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
                    .checked_mul(size_of::<EmbeddedDependency>())
                    .ok_or(EmbeddedDependencyError::Work)?,
            )?;
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(next)
                .map_err(|_| EmbeddedDependencyError::Allocation)?;
            if entries.capacity() > maximum {
                return Err(EmbeddedDependencyError::Bytes);
            }
            entries.append(&mut self.entries);
            self.entries = entries;
        }
        let index = self.entries.len();
        self.entries.push(EmbeddedDependency {
            source,
            uri: owned,
            version: None,
        });
        self.payload_bytes = payload;
        Ok(index)
    }
    pub(super) fn set_version(
        &mut self,
        index: usize,
        version: Vec<u8>,
    ) -> Result<(), EmbeddedDependencyError> {
        if let Some(error) = self.rejected {
            return Err(error);
        }
        let result = (|| {
            self.spend(1)?;
            let bytes = self
                .retained_bytes()
                .checked_add(version.capacity())
                .filter(|n| *n <= self.limits.retained_bytes.get())
                .ok_or(EmbeddedDependencyError::Bytes)?;
            let payload = bytes - self.entries.capacity() * size_of::<EmbeddedDependency>();
            let entry = self
                .entries
                .get_mut(index)
                .filter(|entry| entry.version.is_none())
                .ok_or(EmbeddedDependencyError::Target)?;
            self.payload_bytes = payload;
            entry.version = Some(version);
            Ok(())
        })();
        if let Err(error) = result {
            self.rejected = Some(error);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;
    fn n(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }
    fn id(value: u64) -> ResourceId {
        ResourceId::new(NonZeroU64::new(value).unwrap())
    }
    fn limits() -> EmbeddedDependencyLimits {
        EmbeddedDependencyLimits {
            entries: n(10),
            retained_bytes: n(10000),
            work: n(10000),
        }
    }
    #[test]
    fn contextual_records_keep_absent_and_empty_versions_distinct() {
        let mut report = EmbeddedDependencyReport::new(limits());
        assert_eq!(report.record(id(1), "app:/root.css").unwrap(), 0);
        assert_eq!(report.record(id(1), "app:/root.css").unwrap(), 0);
        assert_eq!(report.record(id(2), "app:/root.css").unwrap(), 1);
        report.set_version(0, vec![]).unwrap();
        assert_eq!(report.entries()[0].source(), id(1));
        assert_eq!(report.entries()[0].version(), Some([].as_slice()));
        assert_eq!(report.entries()[1].version(), None);
        assert_eq!(
            report.set_version(0, vec![1]),
            Err(EmbeddedDependencyError::Target)
        );
        assert_eq!(report.rejection(), Some(EmbeddedDependencyError::Target));
        assert_eq!(
            report.record(id(3), "app:/later.css"),
            Err(EmbeddedDependencyError::Target)
        );
        assert_eq!(report.entries().len(), 2);
        assert_eq!(report.entries()[0].version(), Some([].as_slice()));
    }
    #[test]
    fn version_capacity_failure_preserves_the_attempt_without_a_partial_version() {
        let mut report = EmbeddedDependencyReport::new(limits());
        let index = report.record(id(1), "app:/root.css").unwrap();
        let retained = report.retained_bytes();
        let version = vec![0; 37];
        report.limits.retained_bytes = n(retained + version.capacity() - 1);
        assert_eq!(
            report.set_version(index, version),
            Err(EmbeddedDependencyError::Bytes)
        );
        assert_eq!(report.rejection(), Some(EmbeddedDependencyError::Bytes));
        assert_eq!(report.retained_bytes(), retained);
        assert_eq!(report.entries()[0].version(), None);
    }
    #[test]
    fn rejected_storage_work_and_conflicting_identity_cannot_certify_a_report() {
        for (settings, expected) in [
            (
                EmbeddedDependencyLimits {
                    retained_bytes: n(1),
                    ..limits()
                },
                EmbeddedDependencyError::Bytes,
            ),
            (
                EmbeddedDependencyLimits {
                    retained_bytes: n(usize::MAX),
                    ..limits()
                },
                EmbeddedDependencyError::Bytes,
            ),
            (
                EmbeddedDependencyLimits {
                    work: n(1),
                    ..limits()
                },
                EmbeddedDependencyError::Work,
            ),
        ] {
            let mut report = EmbeddedDependencyReport::new(settings);
            assert_eq!(report.record(id(1), "app:/root.css"), Err(expected));
            assert_eq!(report.rejection(), Some(expected));
            assert!(report.entries().is_empty());
        }
        let mut report = EmbeddedDependencyReport::new(limits());
        report.record(id(1), "app:/root.css").unwrap();
        assert_eq!(
            report.record(id(1), "app:/other.css"),
            Err(EmbeddedDependencyError::Target)
        );
        assert_eq!(report.entries()[0].acquisition_uri(), "app:/root.css");
        assert!(report.retained_bytes() <= limits().retained_bytes.get());
    }
}
