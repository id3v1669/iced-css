//! Progress timing demand; native and image scheduling remain host obligations.
use super::clock::{TimingPhase, TimingWindow};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressWakeup {
    Idle,
    NextFrame,
    At(Instant),
}

pub(crate) fn progress_wakeup(
    now: Instant,
    windows: impl Iterator<Item = TimingWindow>,
) -> ProgressWakeup {
    let mut earliest: Option<Instant> = None;
    for window in windows {
        match window.phase_at(now) {
            TimingPhase::Active { .. } => return ProgressWakeup::NextFrame,
            TimingPhase::Delayed => {
                let start = window.starts_at();
                earliest = Some(earliest.map_or(start, |previous| previous.min(start)));
            }
            TimingPhase::Complete => {}
        }
    }
    earliest.map_or(ProgressWakeup::Idle, ProgressWakeup::At)
}
