use iced_animate::{Curve, Easing, Motion, MotionKey};
use iced_css::animation::{clock::*, linear_progress::*, motion_sync::MotionSync};
use std::time::{Duration, Instant};
#[test]
fn live_luminate_zero_duration_is_gated_at_the_exact_delay_boundary() {
    let motion = Motion::new();
    let mut sync = MotionSync::default();
    let now = Instant::now();
    let duration = Duration::ZERO;
    let delay = Duration::from_secs(1);
    let raw = motion.play(
        MotionKey::from_raw(1),
        Curve::ease(Easing::Linear, duration).delayed(delay),
        0.0f32,
        1.0,
    );
    sync.restart(&motion, now);
    let window = AnimationTiming::new(duration, delay).accept(now).unwrap();
    assert_eq!(raw.get(), 0.0);
    assert_eq!(
        linear_progress(window.phase_at(now), raw.get()).unwrap(),
        0.0
    );
    sync.synchronize(&motion, now + delay);
    assert_eq!(
        raw.get(),
        0.0,
        "live dependency still publishes start at exact delay/zero-duration boundary"
    );
    assert_eq!(
        linear_progress(window.phase_at(now + delay), raw.get()).unwrap(),
        1.0
    );
}
#[test]
fn live_sub_epsilon_duration_uses_corrected_linear_progress() {
    let motion = Motion::new();
    let mut sync = MotionSync::default();
    let now = Instant::now();
    let duration = Duration::from_nanos(10);
    let raw = motion.play(
        MotionKey::from_raw(1),
        Curve::ease(Easing::Linear, duration),
        0.0f32,
        1.0,
    );
    sync.restart(&motion, now);
    let window = AnimationTiming::new(duration, Duration::ZERO)
        .accept(now)
        .unwrap();
    let midpoint = now + Duration::from_nanos(5);
    sync.synchronize(&motion, midpoint);
    assert!(raw.get() < 0.1, "dependency duration floor is observable");
    let sample = linear_progress(window.phase_at(midpoint), raw.get()).unwrap();
    assert!((sample - 0.5).abs() <= f32::EPSILON);
    sync.synchronize(&motion, now + duration);
    assert_eq!(
        linear_progress(window.phase_at(now + duration), raw.get()).unwrap(),
        1.0
    );
}
#[test]
fn ordinary_duration_and_repeated_timestamps_do_not_restart_progress() {
    let motion = Motion::new();
    let mut sync = MotionSync::default();
    let now = Instant::now();
    let mut clock = AnimationClock::default();
    let _ = clock.accept(now);
    let duration = Duration::from_secs(1);
    let raw = motion.play(
        MotionKey::from_raw(1),
        Curve::ease(Easing::Linear, duration),
        0.0f32,
        1.0,
    );
    sync.restart(&motion, now);
    let window = AnimationTiming::new(duration, Duration::ZERO)
        .accept(now)
        .unwrap();
    let half = now + Duration::from_millis(500);
    sync.synchronize(&motion, clock.accept(half));
    for timestamp in [half, now, half] {
        let accepted = clock.accept(timestamp);
        sync.synchronize(&motion, accepted);
        assert_eq!(
            linear_progress(window.phase_at(accepted), raw.get()).unwrap(),
            0.5
        );
        motion.end_build();
    }
    assert_eq!(motion.track_count(), 1);
}
#[test]
fn active_progress_rejects_invalid_inputs_and_preserves_explicit_boundaries() {
    let active = TimingPhase::Active {
        elapsed: Duration::ZERO,
        duration: Duration::from_secs(1),
    };
    for invalid in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert_eq!(
            linear_progress(active, invalid),
            Err(ProgressError::InvalidSample)
        );
    }
    assert_eq!(
        linear_progress(
            TimingPhase::Active {
                elapsed: Duration::ZERO,
                duration: Duration::ZERO
            },
            0.0
        ),
        Err(ProgressError::InvalidPhase)
    );
    assert_eq!(
        linear_progress(TimingPhase::Complete, f32::NAN).unwrap(),
        1.0
    );
    assert_eq!(
        linear_progress(TimingPhase::Delayed, f32::NAN).unwrap(),
        0.0
    );
}

#[test]
fn nanosecond_grid_measures_floor_correction_against_elapsed_ratio() {
    let now = Instant::now();
    let mut maximum_error = 0.0f32;
    let mut maximum_ulps = 0u32;
    let mut cases = 0;
    for nanos in 1..=119u64 {
        let motion = Motion::new();
        let mut sync = MotionSync::default();
        let duration = Duration::from_nanos(nanos);
        let raw = motion.play(
            MotionKey::from_raw(1),
            Curve::ease(Easing::Linear, duration),
            0.0f32,
            1.0,
        );
        sync.restart(&motion, now);
        let window = AnimationTiming::new(duration, Duration::ZERO)
            .accept(now)
            .unwrap();
        for elapsed in 0..=nanos {
            let timestamp = now + Duration::from_nanos(elapsed);
            sync.synchronize(&motion, timestamp);
            let actual = linear_progress(window.phase_at(timestamp), raw.get()).unwrap();
            // An independently evaluated elapsed-time ratio; f64 is only the
            // small integer-ratio test oracle, not the product representation.
            let expected = (elapsed as f64 / nanos as f64) as f32;
            let error = (actual - expected).abs();
            maximum_error = maximum_error.max(error);
            maximum_ulps = maximum_ulps.max(actual.to_bits().abs_diff(expected.to_bits()));
            // A conservative finite-grid regression bound, not a general proof
            // or a product's CSS conformance tolerance.
            assert!(error <= (nanos as f32 + 4.0) * f32::EPSILON);
            cases += 1;
        }
    }
    assert_eq!(cases, 7259);
    println!(
        "nanosecond grid: cases={cases} maximum_absolute_error={maximum_error:e} maximum_ulps={maximum_ulps}"
    );
}
