//! RFC9111 delta-seconds and Age extraction. This does not compute freshness
//! or authorize reuse/refresh of an active application resource.
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct AgeLimits {
    pub bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgeError {
    Invalid { offset: usize },
    Bytes,
    Work,
}
impl std::fmt::Display for AgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP age: {self:?}")
    }
}
impl std::error::Error for AgeError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeltaSeconds(u64);
impl DeltaSeconds {
    pub const fn from_seconds(seconds: u64) -> Self {
        Self(seconds)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
    /// RFC9111 1.2.2: overflow uses the greatest conveniently represented value.
    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
    pub const fn elapsed_since(self, earlier: Self) -> Self {
        Self(self.0.saturating_sub(earlier.0))
    }
}
fn admit(input: &[u8], limits: AgeLimits, multiplier: usize) -> Result<(), AgeError> {
    if input.len() > limits.bytes.get() {
        return Err(AgeError::Bytes);
    }
    let work = input
        .len()
        .checked_mul(multiplier)
        .and_then(|n| n.checked_add(1))
        .ok_or(AgeError::Work)?;
    if work > limits.work.get() {
        return Err(AgeError::Work);
    }
    Ok(())
}
fn digits(input: &[u8]) -> Result<DeltaSeconds, AgeError> {
    if input.is_empty() {
        return Err(AgeError::Invalid { offset: 0 });
    }
    let mut value = 0u64;
    for (offset, byte) in input.iter().copied().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(AgeError::Invalid { offset });
        }
        value = value
            .saturating_mul(10)
            .saturating_add(u64::from(byte - b'0'));
    }
    Ok(DeltaSeconds(value))
}
pub fn parse_delta_seconds(input: &[u8], limits: AgeLimits) -> Result<DeltaSeconds, AgeError> {
    admit(input, limits, 1)?;
    digits(input)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgeField {
    Absent,
    Seconds {
        value: DeltaSeconds,
        discarded: bool,
    },
    /// Invalid Age is ignored under RFC9111 5.1, not converted to valid zero.
    Ignored {
        offset: usize,
        discarded: bool,
    },
}
/// Only the first list member is considered, per RFC9111 5.1. Later members
/// and lines are not fallback values. Bounds apply to the first full field value;
/// later field slices are not scanned or copied by this operation.
pub fn parse_age_fields(fields: &[&[u8]], limits: AgeLimits) -> Result<AgeField, AgeError> {
    let Some(input) = fields.first() else {
        return Ok(AgeField::Absent);
    };
    admit(input, limits, 4)?;
    let comma = input.iter().position(|b| *b == b',');
    let discarded = fields.len() > 1 || comma.is_some();
    let mut start = 0;
    let mut end = comma.unwrap_or(input.len());
    while start < end && matches!(input[start], b' ' | b'\t') {
        start += 1;
    }
    while end > start && matches!(input[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    match digits(&input[start..end]) {
        Ok(value) => Ok(AgeField::Seconds { value, discarded }),
        Err(AgeError::Invalid { offset }) => Ok(AgeField::Ignored {
            offset: start + offset,
            discarded,
        }),
        Err(error) => Err(error),
    }
}
