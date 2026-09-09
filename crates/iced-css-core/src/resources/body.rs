//! Bounded byte-domain body assembly. Framing and content-coding adapters must
//! supply the matching length and report actual complete end versus failure.
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy)]
pub struct BodyLimits {
    /// Retained Vec capacity, excluding incoming chunks and old/new growth overlap.
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyEnd {
    Complete,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyError {
    AdvertisedLength,
    Bytes,
    Work,
    Allocation,
    Overrun,
    Incomplete,
    Transport,
}
impl std::fmt::Display for BodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "representation body: {self:?}")
    }
}
impl std::error::Error for BodyError {}
#[derive(Debug)]
pub struct CompletedBody {
    bytes: Vec<u8>,
}
impl CompletedBody {
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    pub fn retained_bytes(&self) -> usize {
        self.bytes.capacity()
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
#[derive(Debug)]
pub struct BodyCollector {
    bytes: Vec<u8>,
    expected: Option<usize>,
    maximum: usize,
    work: usize,
    rejected: Option<BodyError>,
}
impl BodyCollector {
    pub fn new(expected_length: Option<u64>, limits: BodyLimits) -> Result<Self, BodyError> {
        if limits.retained_bytes.get() > isize::MAX as usize {
            return Err(BodyError::Bytes);
        }
        let expected = expected_length
            .map(|length| {
                usize::try_from(length)
                    .ok()
                    .filter(|length| *length <= limits.retained_bytes.get())
                    .ok_or(BodyError::AdvertisedLength)
            })
            .transpose()?;
        Ok(Self {
            bytes: Vec::new(),
            expected,
            maximum: limits.retained_bytes.get(),
            work: limits.work.get(),
            rejected: None,
        })
    }
    /// Any failed append permanently rejects this candidate. Even empty chunks
    /// cost work, preventing unbounded zero-progress processing under a fixed limit.
    pub fn append(&mut self, chunk: &[u8]) -> Result<(), BodyError> {
        if let Some(error) = self.rejected {
            return Err(error);
        }
        let result = self.append_inner(chunk);
        if let Err(error) = result {
            self.rejected = Some(error);
        }
        result
    }
    fn append_inner(&mut self, chunk: &[u8]) -> Result<(), BodyError> {
        let needed = self
            .bytes
            .len()
            .checked_add(chunk.len())
            .ok_or(BodyError::Bytes)?;
        if self.expected.is_some_and(|expected| needed > expected) {
            return Err(BodyError::Overrun);
        }
        if needed > self.maximum {
            return Err(BodyError::Bytes);
        }
        self.spend(chunk.len().checked_add(1).ok_or(BodyError::Work)?)?;
        if needed > self.bytes.capacity() {
            self.spend(self.bytes.len())?;
            let target = needed.max(self.bytes.capacity().saturating_mul(2).min(self.maximum));
            let mut next = Vec::new();
            next.try_reserve_exact(target)
                .map_err(|_| BodyError::Allocation)?;
            if next.capacity() > self.maximum {
                return Err(BodyError::Bytes);
            }
            next.extend_from_slice(&self.bytes);
            self.bytes = next;
        }
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }
    fn spend(&mut self, amount: usize) -> Result<(), BodyError> {
        self.work = self.work.checked_sub(amount).ok_or(BodyError::Work)?;
        Ok(())
    }
    /// Complete is an explicit assertion by the transport/decoder, not a proof
    /// supplied by this collector. A failed end never creates a completed body.
    pub fn finish(mut self, end: BodyEnd) -> Result<CompletedBody, BodyError> {
        if let Some(error) = self.rejected {
            return Err(error);
        }
        if end == BodyEnd::Failed {
            return Err(BodyError::Transport);
        }
        self.spend(1)?;
        if self
            .expected
            .is_some_and(|expected| expected != self.bytes.len())
        {
            return Err(BodyError::Incomplete);
        }
        Ok(CompletedBody { bytes: self.bytes })
    }
}
