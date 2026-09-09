//! Boundary and duration-floor correction for a live luminate Linear 0→1 track.
//! The phase and raw sample must belong to the same accepted timestamp/interval.
use super::clock::TimingPhase;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressError {
    InvalidSample,
    InvalidPhase,
}
impl std::fmt::Display for ProgressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "linear animation progress: {self:?}")
    }
}
impl std::error::Error for ProgressError {}
/// This corrects the retained dependency's minimum easing duration only. It must
/// not receive arbitrary easing/spring samples. Property combination and final
/// ranges are separate; the corrected active result is not globally clamped.
pub fn linear_progress(phase: TimingPhase, raw: f32) -> Result<f32, ProgressError> {
    match phase {
        TimingPhase::Delayed => Ok(0.0),
        TimingPhase::Complete => Ok(1.0),
        TimingPhase::Active { elapsed, duration } => {
            if duration.is_zero() || elapsed >= duration {
                return Err(ProgressError::InvalidPhase);
            }
            if !raw.is_finite() || !(0.0..=1.0).contains(&raw) {
                return Err(ProgressError::InvalidSample);
            }
            let requested = duration.as_secs_f32();
            let scale = requested.max(f32::EPSILON) / requested;
            let result = raw * scale;
            Ok(if result == 0.0 { 0.0 } else { result })
        }
    }
}
