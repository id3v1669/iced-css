use crate::{
    identity::InstanceId,
    values::{NonnegativeF32, RangeEvent, registry::PropertyId},
};
use std::{
    collections::VecDeque,
    num::{NonZeroU64, NonZeroUsize},
    ops::Range,
    sync::Arc,
};

#[derive(Debug)]
pub struct Provenance {
    pub logical_name: String,
    pub revision: NonZeroU64,
}
#[derive(Debug, Clone)]
pub struct SourceSpan {
    pub bytes: Range<usize>,
    pub line: NonZeroUsize,
    pub column: NonZeroUsize,
}
#[derive(Debug, Clone)]
pub enum Origin {
    Authored {
        source: Arc<Provenance>,
        span: SourceSpan,
    },
    Synthetic(String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    Range(RangeEvent),
    Config,
    Capture,
    CssRecovery,
    Resource,
    Provider,
    Representation,
    Request,
    HostFault,
}
#[derive(Debug)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub origin: Origin,
    pub related: Vec<Origin>,
    pub instance: Option<InstanceId>,
    pub property: Option<PropertyId>,
    pub result: Option<f32>,
    pub reason: String,
}
impl Diagnostic {
    /// Capture an observed conversion event. Ordinary precision rounding is silent.
    pub fn range(
        value: NonnegativeF32,
        origin: Origin,
        instance: Option<InstanceId>,
        property: PropertyId,
    ) -> Option<Self> {
        Some(Self {
            kind: DiagnosticKind::Range(value.event?),
            origin,
            related: Vec::new(),
            instance,
            property: Some(property),
            result: Some(value.value),
            reason: String::new(),
        })
    }
}
#[derive(Debug)]
pub struct Report {
    pub host: NonZeroU64,
    pub diagnostic: Diagnostic,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Omissions {
    pub count: u64,
    pub overflow: bool,
}
#[derive(Debug)]
pub enum Delivery {
    Report(Report),
    Omissions {
        host: NonZeroU64,
        summary: Omissions,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueConfigError {
    InvalidCount,
    InvalidBytes,
}
impl std::fmt::Display for QueueConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidCount => "invalid pending-report count limit",
            Self::InvalidBytes => "invalid pending-report byte limit",
        })
    }
}
impl std::error::Error for QueueConfigError {}
#[derive(Debug, Clone, Copy)]
pub struct QueueLimits {
    count: usize,
    bytes: usize,
}
impl QueueLimits {
    pub fn new(count: usize, bytes: usize) -> Result<Self, QueueConfigError> {
        if count == 0 || count > isize::MAX as usize {
            return Err(QueueConfigError::InvalidCount);
        }
        if bytes == 0 || bytes > isize::MAX as usize {
            return Err(QueueConfigError::InvalidBytes);
        }
        Ok(Self { count, bytes })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmissionReason {
    Count,
    Bytes,
    Allocation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Queued,
    Omitted(OmissionReason),
    Closed,
}
/// One host-owned admission/delivery state. A runtime bridge must synchronize this
/// state and call handoff only at the stream's successful yield boundary.
pub struct DiagnosticQueue {
    host: NonZeroU64,
    limits: QueueLimits,
    pending: VecDeque<Report>,
    omissions: Omissions,
    closed: bool,
    prefer_summary: bool,
    bytes: usize,
}
impl DiagnosticQueue {
    pub fn new(host: NonZeroU64, limits: QueueLimits) -> Self {
        Self {
            host,
            limits,
            pending: VecDeque::new(),
            omissions: Omissions::default(),
            closed: false,
            prefer_summary: true,
            bytes: 0,
        }
    }
    pub fn admit(&mut self, diagnostic: Diagnostic) -> Admission {
        if self.closed {
            return Admission::Closed;
        }
        if self.pending.len() >= self.limits.count {
            return self.omit(OmissionReason::Count);
        }
        let Some(needed) = self.pending.len().checked_add(1) else {
            return self.omit(OmissionReason::Count);
        };
        let report = Report {
            host: self.host,
            diagnostic,
        };
        let slots = needed.max(self.pending.capacity());
        let Some(mut bytes) = self.retained_for(Some(&report), slots) else {
            return self.omit(OmissionReason::Bytes);
        };
        if bytes > self.limits.bytes {
            return self.omit(OmissionReason::Bytes);
        }
        if needed > self.pending.capacity() {
            let mut candidate = VecDeque::new();
            if candidate.try_reserve_exact(needed).is_err() {
                return self.omit(OmissionReason::Allocation);
            }
            let Some(actual) = self.retained_for(Some(&report), candidate.capacity()) else {
                return self.omit(OmissionReason::Bytes);
            };
            if actual > self.limits.bytes {
                return self.omit(OmissionReason::Bytes);
            }
            bytes = actual;
            // Only transfer old reports after every candidate admission check.
            candidate.append(&mut self.pending);
            candidate.push_back(report);
            self.pending = candidate;
        } else {
            self.pending.push_back(report);
        }
        self.bytes = bytes;
        Admission::Queued
    }
    fn omit(&mut self, reason: OmissionReason) -> Admission {
        self.omissions.increment();
        Admission::Omitted(reason)
    }
    fn retained_for(&self, extra: Option<&Report>, slots: usize) -> Option<usize> {
        let reports = self.pending.iter().chain(extra);
        let mut bytes = slots.checked_mul(std::mem::size_of::<Report>())?;
        for report in reports.clone() {
            let d = &report.diagnostic;
            bytes = bytes.checked_add(d.reason.capacity())?.checked_add(
                d.related
                    .capacity()
                    .checked_mul(std::mem::size_of::<Origin>())?,
            )?;
        }
        for (index, origin) in reports.clone().flat_map(origins).enumerate() {
            match origin {
                Origin::Synthetic(label) => bytes = bytes.checked_add(label.capacity())?,
                Origin::Authored { source, .. } => {
                    let seen = reports
                        .clone()
                        .flat_map(origins)
                        .take(index)
                        .any(|old| match old {
                            Origin::Authored {
                                source: previous, ..
                            } => Arc::ptr_eq(source, previous),
                            Origin::Synthetic(_) => false,
                        });
                    if !seen {
                        bytes = bytes
                            .checked_add(std::mem::size_of::<ChargedArcProvenance>())?
                            .checked_add(source.logical_name.capacity())?;
                    }
                }
            }
        }
        Some(bytes)
    }
    /// Take ownership only when the runtime stream is ready to yield. Preparation
    /// or canceled polls must not call this. No application acknowledgement follows.
    pub fn handoff(&mut self) -> Option<Delivery> {
        if self.closed {
            return None;
        }
        // Alternate when both exist: neither continuing reports nor omissions can starve the other.
        if (self.omissions.count != 0 || self.omissions.overflow)
            && (self.prefer_summary || self.pending.is_empty())
        {
            self.prefer_summary = false;
            let summary = std::mem::take(&mut self.omissions);
            return Some(Delivery::Omissions {
                host: self.host,
                summary,
            });
        }
        let report = self.pending.pop_front()?;
        self.prefer_summary = true;
        if self.pending.is_empty() {
            self.pending = VecDeque::new();
            self.bytes = 0;
        } else {
            // Remaining immutable reports are a subset of admitted storage.
            self.bytes = self
                .retained_for(None, self.pending.capacity())
                .expect("admitted subset cannot overflow retained accounting");
        }
        Some(Delivery::Report(report))
    }
    pub fn close(&mut self) {
        self.closed = true;
        self.pending = VecDeque::new();
        self.omissions = Omissions::default();
        self.bytes = 0;
    }
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    pub fn retained_bytes(&self) -> usize {
        self.bytes
    }
    pub fn omissions(&self) -> Omissions {
        self.omissions
    }
}

fn origins(report: &Report) -> impl Iterator<Item = &Origin> {
    std::iter::once(&report.diagnostic.origin).chain(report.diagnostic.related.iter())
}
// Payload/control-block layout from retained Rust1.97.1 alloc/src/sync.rs.
// This accounting model must be requalified on toolchain changes. Allocator
// bookkeeping/rounding outside the requested layout is not a process-memory cap.
#[repr(C, align(2))]
struct ChargedArcProvenance {
    _strong: std::sync::atomic::AtomicUsize,
    _weak: std::sync::atomic::AtomicUsize,
    _data: Provenance,
}
impl Omissions {
    fn increment(&mut self) {
        match self.count.checked_add(1) {
            Some(count) => self.count = count,
            None => self.overflow = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn omission_count_saturates_and_preserves_overflow_until_handoff() {
        let mut queue = DiagnosticQueue::new(NonZeroU64::MIN, QueueLimits::new(1, 1).unwrap());
        queue.omissions = Omissions {
            count: u64::MAX - 1,
            overflow: false,
        };
        queue.omit(OmissionReason::Bytes);
        assert_eq!(
            queue.omissions,
            Omissions {
                count: u64::MAX,
                overflow: false
            }
        );
        queue.omit(OmissionReason::Allocation);
        assert_eq!(
            queue.omissions,
            Omissions {
                count: u64::MAX,
                overflow: true
            }
        );
        let Some(Delivery::Omissions { summary, .. }) = queue.handoff() else {
            panic!()
        };
        assert_eq!(
            summary,
            Omissions {
                count: u64::MAX,
                overflow: true
            }
        );
        assert_eq!(queue.omissions, Omissions::default());
    }
}
