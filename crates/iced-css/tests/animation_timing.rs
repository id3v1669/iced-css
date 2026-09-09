use iced_css::animation::clock::*;
use std::time::{Duration, Instant};
#[test]
fn delay_and_zero_duration_have_exact_boundary_semantics() {
    let now = Instant::now();
    let timing = AnimationTiming::new(Duration::ZERO, Duration::from_secs(1));
    let window = timing.accept(now).unwrap();
    assert_eq!(window.phase_at(now), TimingPhase::Delayed);
    assert_eq!(
        window.phase_at(now + Duration::from_millis(999)),
        TimingPhase::Delayed
    );
    assert_eq!(
        window.phase_at(now + Duration::from_secs(1)),
        TimingPhase::Complete
    );
    assert_eq!(
        window.phase_at(now + Duration::from_secs(5)),
        TimingPhase::Complete
    );
    assert_eq!(
        AnimationTiming::new(Duration::ZERO, Duration::ZERO)
            .accept(now)
            .unwrap()
            .phase_at(now),
        TimingPhase::Complete
    );
}
#[test]
fn duration_window_preserves_sub_epsilon_intervals_without_float_conversion() {
    let now = Instant::now();
    let window = AnimationTiming::new(Duration::from_nanos(10), Duration::from_nanos(5))
        .accept(now)
        .unwrap();
    assert_eq!(
        window.phase_at(now + Duration::from_nanos(5)),
        TimingPhase::Active {
            elapsed: Duration::ZERO,
            duration: Duration::from_nanos(10)
        }
    );
    assert_eq!(
        window.phase_at(now + Duration::from_nanos(9)),
        TimingPhase::Active {
            elapsed: Duration::from_nanos(4),
            duration: Duration::from_nanos(10)
        }
    );
    assert_eq!(
        window.phase_at(now + Duration::from_nanos(15)),
        TimingPhase::Complete
    );
    assert_eq!(window.starts_at(), now + Duration::from_nanos(5));
    assert_eq!(window.ends_at(), now + Duration::from_nanos(15));
}
#[test]
fn floating_configuration_and_timestamp_overflow_fail_explicitly() {
    for invalid in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
        assert_eq!(
            AnimationTiming::from_seconds(invalid, 0.0),
            Err(TimingError::Duration)
        );
        assert_eq!(
            AnimationTiming::from_seconds(0.0, invalid),
            Err(TimingError::Delay)
        );
    }
    let timing = AnimationTiming::from_seconds(0.5, -0.0).unwrap();
    assert_eq!(timing.duration(), Duration::from_millis(500));
    assert_eq!(timing.delay(), Duration::ZERO);
    let now = Instant::now();
    let huge = AnimationTiming::new(Duration::MAX, Duration::MAX);
    assert_eq!(huge.accept(now).unwrap_err(), TimingError::Timestamp);
}
#[test]
fn monotonic_batch_clock_reuses_equal_or_older_instants() {
    let now = Instant::now();
    let mut clock = AnimationClock::default();
    assert_eq!(clock.last(), None);
    assert_eq!(clock.accept(now), now);
    let later = now + Duration::from_secs(1);
    assert_eq!(clock.accept(later), later);
    assert_eq!(clock.accept(now), later);
    assert_eq!(clock.accept(later), later);
    assert_eq!(clock.last(), Some(later));
}
