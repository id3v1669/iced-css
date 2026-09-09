//! Blocking read primitives for a background owner, never for a native UI tick.
use iced_css_core::resources::body::{
    BodyCollector, BodyEnd, BodyError, BodyLimits, CompletedBody,
};
use std::{
    fs::File,
    io::{self, Read},
    num::NonZeroUsize,
    path::Path,
};

#[derive(Debug, Clone, Copy)]
pub struct ReadBodyLimits {
    pub body: BodyLimits,
    /// Includes interrupted attempts and the EOF probe, not only successful data.
    pub reads: NonZeroUsize,
}
#[derive(Debug)]
pub enum ReadBodyError {
    Io(io::Error),
    Body(BodyError),
    Reads,
    InvalidReadCount,
}
impl From<BodyError> for ReadBodyError {
    fn from(error: BodyError) -> Self {
        Self::Body(error)
    }
}
impl From<io::Error> for ReadBodyError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl std::fmt::Display for ReadBodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "blocking resource read: {self:?}")
    }
}
impl std::error::Error for ReadBodyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Body(error) => Some(error),
            _ => None,
        }
    }
}
/// Read through EOF without returning incomplete prefix bytes. A fixed 8 KiB stack
/// buffer is separate from the retained-body byte allowance. Each attempt requests
/// at most the remaining byte allowance plus one byte to detect overflow.
///
/// This function bounds attempts, not the duration of a blocking Read call. Its
/// caller must arrange scheduling/cancellation appropriate to the underlying I/O.
pub fn read_body_blocking(
    reader: &mut impl Read,
    expected_length: Option<u64>,
    limits: ReadBodyLimits,
) -> Result<CompletedBody, ReadBodyError> {
    let mut body = BodyCollector::new(expected_length, limits.body)?;
    let mut remaining_reads = limits.reads.get();
    let mut received = 0usize;
    let mut buffer = [0u8; 8192];
    loop {
        remaining_reads = remaining_reads.checked_sub(1).ok_or(ReadBodyError::Reads)?;
        // BodyCollector admission bounds maximum to isize::MAX, so +1 fits usize.
        let maximum = (limits.body.retained_bytes.get() - received + 1).min(buffer.len());
        let count = match reader.read(&mut buffer[..maximum]) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(ReadBodyError::Io(error)),
        };
        if count > maximum {
            return Err(ReadBodyError::InvalidReadCount);
        }
        if count == 0 {
            return body.finish(BodyEnd::Complete).map_err(ReadBodyError::Body);
        }
        body.append(&buffer[..count])?;
        received = received.checked_add(count).ok_or(BodyError::Bytes)?;
    }
}
/// Open the explicit path and collect its original bytes using the opened file's
/// advertised metadata length. A length mismatch or read error rejects the body.
/// This does not canonicalize URI/file identities or establish an atomic snapshot;
/// same-length concurrent changes require the resource host's reload strategy.
pub fn read_file_blocking(
    path: &Path,
    limits: ReadBodyLimits,
) -> Result<CompletedBody, ReadBodyError> {
    let mut file = File::open(path)?;
    let expected = file.metadata()?.len();
    read_body_blocking(&mut file, Some(expected), limits)
}
