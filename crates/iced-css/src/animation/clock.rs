//! Timing validation and exact boundary gates, separate from luminate sampling.
use std::time::{Duration, Instant};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingError {
    Duration,
    Delay,
    Timestamp,
}
impl std::fmt::Display for TimingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "animation timing: {self:?}")
    }
}
impl std::error::Error for TimingError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationTiming {
    duration: Duration,
    delay: Duration,
}
impl AnimationTiming {
    pub const fn new(duration: Duration, delay: Duration) -> Self {
        Self { duration, delay }
    }
    /// Convert finite nonnegative seconds using the standard Duration conversion.
    /// Values outside Duration's representable domain are errors.
    pub fn from_seconds(duration: f32, delay: f32) -> Result<Self, TimingError> {
        let duration = Duration::try_from_secs_f32(duration).map_err(|_| TimingError::Duration)?;
        let delay = Duration::try_from_secs_f32(delay).map_err(|_| TimingError::Delay)?;
        Ok(Self::new(duration, delay))
    }
    pub const fn duration(self) -> Duration {
        self.duration
    }
    pub const fn delay(self) -> Duration {
        self.delay
    }
    pub fn accept(self, timestamp: Instant) -> Result<TimingWindow, TimingError> {
        let start = timestamp
            .checked_add(self.delay)
            .ok_or(TimingError::Timestamp)?;
        let end = start
            .checked_add(self.duration)
            .ok_or(TimingError::Timestamp)?;
        Ok(TimingWindow {
            start,
            end,
            duration: self.duration,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingPhase {
    Delayed,
    Active {
        elapsed: Duration,
        duration: Duration,
    },
    Complete,
}
#[derive(Debug, Clone, Copy)]
pub struct TimingWindow {
    start: Instant,
    end: Instant,
    duration: Duration,
}
impl TimingWindow {
    pub fn starts_at(self) -> Instant {
        self.start
    }
    pub fn ends_at(self) -> Instant {
        self.end
    }
    pub fn phase_at(self, timestamp: Instant) -> TimingPhase {
        if timestamp < self.start {
            TimingPhase::Delayed
        } else if timestamp >= self.end {
            TimingPhase::Complete
        } else {
            TimingPhase::Active {
                elapsed: timestamp.duration_since(self.start),
                duration: self.duration,
            }
        }
    }
}
/// Owner calls once per accepted ordered Message batch, before ticking luminate.
#[derive(Debug, Default)]
pub struct AnimationClock {
    last: Option<Instant>,
}
impl AnimationClock {
    pub fn last(&self) -> Option<Instant> {
        self.last
    }
    pub fn accept(&mut self, timestamp: Instant) -> Instant {
        let accepted = self
            .last
            .map_or(timestamp, |previous| previous.max(timestamp));
        self.last = Some(accepted);
        accepted
    }
}
