use iced_animate::{Curve, Easing, Motion, MotionKey};
use iced_css::animation::{clock::*, linear_progress::*, motion_sync::MotionSync};
use iced_css_core::animation::{
    combine::Combination,
    number::{Clamp, combine_number},
};
use std::time::{Duration, Instant};
#[test]
fn live_linear_progress_drives_css_values_with_final_range_clamps() {
    let motion = Motion::new();
    let mut sync = MotionSync::default();
    let start = Instant::now();
    let mut clock = AnimationClock::default();
    let _ = clock.accept(start);
    let duration = Duration::from_secs(1);
    let delay = Duration::from_secs(1);
    let raw = motion.play(
        MotionKey::from_raw(1),
        Curve::ease(Easing::Linear, duration).delayed(delay),
        0.0f32,
        1.0,
    );
    sync.restart(&motion, start);
    let window = AnimationTiming::new(duration, delay).accept(start).unwrap();
    for (millis, value, clamped) in [
        (0, 0.0, Some(Clamp::Lower)),
        (999, 0.0, Some(Clamp::Lower)),
        (1000, 0.0, Some(Clamp::Lower)),
        (1250, 0.0, None),
        (1500, 10.0, None),
        (1500, 10.0, None),
        (1000, 10.0, None),
        (1750, 20.0, None),
        (2000, 20.0, Some(Clamp::Upper)),
    ] {
        let accepted = clock.accept(start + Duration::from_millis(millis));
        sync.synchronize(&motion, accepted);
        let progress = linear_progress(window.phase_at(accepted), raw.get()).unwrap();
        let sample = combine_number(-10.0, 30.0, Combination::Interpolate(progress))
            .unwrap()
            .resolve(0.0, 20.0)
            .unwrap();
        assert_eq!(sample.value, value);
        assert_eq!(sample.clamped, clamped);
    }
    assert_eq!(motion.track_count(), 1);
}
#[test]
fn live_zero_duration_uses_the_end_value_at_the_delay_boundary() {
    let motion = Motion::new();
    let mut sync = MotionSync::default();
    let start = Instant::now();
    let delay = Duration::from_secs(1);
    let raw = motion.play(
        MotionKey::from_raw(1),
        Curve::ease(Easing::Linear, Duration::ZERO).delayed(delay),
        0.0f32,
        1.0,
    );
    sync.restart(&motion, start);
    let window = AnimationTiming::new(Duration::ZERO, delay)
        .accept(start)
        .unwrap();
    for (offset, expected) in [(Duration::ZERO, -f32::MAX), (delay, f32::MAX)] {
        let now = start + offset;
        sync.synchronize(&motion, now);
        let progress = linear_progress(window.phase_at(now), raw.get()).unwrap();
        let sample = combine_number(-f32::MAX, f32::MAX, Combination::Interpolate(progress))
            .unwrap()
            .resolve(-f32::MAX, f32::MAX)
            .unwrap();
        assert_eq!(sample.value, expected);
        assert_eq!(sample.clamped, None);
    }
}
#[test]
fn live_short_duration_correction_reaches_numeric_combination() {
    let motion = Motion::new();
    let mut sync = MotionSync::default();
    let start = Instant::now();
    let duration = Duration::from_nanos(10);
    let raw = motion.play(
        MotionKey::from_raw(1),
        Curve::ease(Easing::Linear, duration),
        0.0f32,
        1.0,
    );
    sync.restart(&motion, start);
    let window = AnimationTiming::new(duration, Duration::ZERO)
        .accept(start)
        .unwrap();
    let now = start + Duration::from_nanos(5);
    sync.synchronize(&motion, now);
    assert!(raw.get() < 0.1);
    let progress = linear_progress(window.phase_at(now), raw.get()).unwrap();
    let sample = combine_number(10.0, 30.0, Combination::Interpolate(progress))
        .unwrap()
        .resolve(0.0, 100.0)
        .unwrap();
    assert!((sample.value - 20.0).abs() <= 20.0 * f32::EPSILON);
    assert_eq!(sample.clamped, None);
}
