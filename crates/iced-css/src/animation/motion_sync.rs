//! Elapsed-time synchronization of the mandatory luminate engine.
//!
//! Upstream `Motion::tick` spends nothing on the first tick after an idle
//! period and at most 1/15 s per tick. CSS timing is wall-clock (section 68),
//! so the owner realizes elapsed time by handing the dependency bounded,
//! strictly increasing timestamps whose deltas never reach that cap. Idle time
//! issues no ticks; a start from idle is followed by one zero-spend restart
//! tick so the next spending tick is charged in full.
use iced_animate::Motion;
use std::time::{Duration, Instant};

/// Sub-step below the dependency's 1/15 s cap. 1/16 s is exact in f32, so the
/// dependency's accumulated ease time equals a single-tick sum up to one
/// rounding of the final remainder.
pub const STEP: Duration = Duration::from_micros(62_500);

/// Private mirror of the dependency's frame state: the last timestamp it was
/// handed and whether that tick left anything unsettled (its `was_moving`).
#[derive(Debug, Default, Clone, Copy)]
pub struct MotionSync {
    synced: Option<Instant>,
    moving: bool,
    ticks: u64,
}
impl MotionSync {
    /// Total ticks handed to the dependency; a diagnostic count, not a time claim.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }
    /// Whether the last tick left an unsettled track.
    pub fn is_moving(&self) -> bool {
        self.moving
    }
    /// The last timestamp handed to the dependency.
    pub fn synced(&self) -> Option<Instant> {
        self.synced
    }
    fn tick(&mut self, motion: &Motion, at: Instant) {
        let status = motion.tick(at);
        self.ticks += 1;
        self.synced = Some(at);
        self.moving = status.animating;
    }
    /// Bring every unsettled track to `accepted` in steps of at most [`STEP`].
    /// Idle engines and equal/older timestamps issue no tick. Stops early once
    /// a tick reports nothing unsettled: the remainder is idle time.
    pub fn synchronize(&mut self, motion: &Motion, accepted: Instant) {
        let Some(mut at) = self.synced else {
            return;
        };
        while self.moving && at < accepted {
            let remaining = accepted.duration_since(at);
            at += remaining.min(STEP);
            self.tick(motion, at);
        }
    }
    /// Call once after starts are installed at `accepted`. While idle the
    /// dependency would spend nothing on its next tick, so issue that zero-spend
    /// tick now. If a spending tick already landed exactly on `accepted`, the
    /// restart uses one nanosecond later; those starts then trail wall-clock by
    /// one nanosecond, the only residual discrepancy of this protocol.
    pub fn restart(&mut self, motion: &Motion, accepted: Instant) {
        if self.moving {
            return;
        }
        let at = match self.synced {
            Some(previous) if previous >= accepted => previous + Duration::from_nanos(1),
            _ => accepted,
        };
        self.tick(motion, at);
    }
}
