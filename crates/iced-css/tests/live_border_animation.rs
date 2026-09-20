use iced_animate::{Curve, Easing, Motion, MotionKey};
use iced_css::animation::{clock::*, linear_progress::*, motion_sync::MotionSync};
use iced_css_core::{
    animation::{border::*, combine::Combination},
    formatting::table_borders::BorderStyle,
    syntax::*,
    values::{
        NumericLimits,
        border_width::{BorderFontContext, compute_border_width},
        color::ColorLimits,
        property::*,
    },
};
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};
fn numeric() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn parsed(input: &str) -> ParsedProperty {
    let n = |n| NonZeroUsize::new(n).unwrap();
    let DeclarationItem::Declaration(d) = Declarations::new(
        input,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: n(100000),
                nesting: n(20),
                retained_bytes: n(4096),
            },
            name_bytes: n(4096),
        },
    )
    .next()
    .unwrap()
    .unwrap() else {
        panic!()
    };
    let PropertyResult::Parsed(value) = parse_property(
        &d,
        ColorLimits {
            syntax: StringLimits {
                work: n(100000),
                retained_bytes: n(4096),
            },
            numeric: numeric(),
        },
    )
    .unwrap() else {
        panic!()
    };
    value
}

#[test]
fn actual_style_progress_is_applied_before_the_width_sample() {
    let motion = Motion::new();
    let mut sync = MotionSync::default();
    let start = Instant::now();
    let mut clock = AnimationClock::default();
    let _ = clock.accept(start);
    let duration = Duration::from_secs(1);
    let style = motion.play(
        MotionKey::from_raw(1),
        Curve::ease(Easing::Linear, duration),
        0.0_f32,
        1.0,
    );
    sync.restart(&motion, start);
    let width = motion.play(
        MotionKey::from_raw(2),
        Curve::ease(Easing::Linear, duration),
        0.0_f32,
        1.0,
    );
    sync.restart(&motion, start);
    let timing = AnimationTiming::new(duration, Duration::ZERO)
        .accept(start)
        .unwrap();
    for (millis, expected, suppressed) in [
        (0, 0.0, true),
        (250, 0.0, true),
        (500, 12.0, false),
        (750, 16.0, false),
        (1000, 20.0, false),
    ] {
        let now = clock.accept(start + Duration::from_millis(millis));
        sync.synchronize(&motion, now);
        let style_progress = linear_progress(timing.phase_at(now), style.get()).unwrap();
        let width_progress = linear_progress(timing.phase_at(now), width.get()).unwrap();
        let selected = combine_border_style(
            BorderStyle::Hidden,
            BorderStyle::Solid,
            Combination::Interpolate(style_progress),
        )
        .unwrap();
        let sample = combine_border_width(4.0, 20.0, Combination::Interpolate(width_progress))
            .unwrap()
            .resolve(selected, 100.0)
            .unwrap();
        assert_eq!(sample.value, expected);
        assert_eq!(sample.suppressed_by_style, suppressed);
        assert_eq!(sample.clamped, None);
    }
    assert_eq!(motion.track_count(), 2);
}
#[test]
fn refreshed_relative_target_uses_current_progress_and_an_unchanged_absolute_start() {
    let motion = Motion::new();
    let mut sync = MotionSync::default();
    let start = Instant::now();
    let mut clock = AnimationClock::default();
    let _ = clock.accept(start);
    let duration = Duration::from_secs(1);
    let track = motion.play(
        MotionKey::from_raw(1),
        Curve::ease(Easing::Linear, duration),
        0.0_f32,
        1.0,
    );
    sync.restart(&motion, start);
    let timing = AnimationTiming::new(duration, Duration::ZERO)
        .accept(start)
        .unwrap();
    let target = parsed("border-top-width:2em").value;
    let style = parsed("border-top-style:solid").value;
    let captured_start = 4.0;
    for (millis, font, selected, expected) in [
        (0, 10.0, BorderStyle::Hidden, 0.0),
        (250, 10.0, BorderStyle::None, 0.0),
        (500, 10.0, BorderStyle::Solid, 12.0),
        (750, 20.0, BorderStyle::Solid, 31.0),
        (1000, 20.0, BorderStyle::Solid, 40.0),
    ] {
        let now = clock.accept(start + Duration::from_millis(millis));
        sync.synchronize(&motion, now);
        let progress = linear_progress(timing.phase_at(now), track.get()).unwrap();
        let target_value = compute_border_width(
            &target,
            &style,
            BorderFontContext {
                size: font,
                x_height: font / 2.0,
            },
            numeric(),
        )
        .unwrap();
        let sample = combine_border_width(
            captured_start,
            target_value.value,
            Combination::Interpolate(progress),
        )
        .unwrap()
        .resolve(selected, 100.0)
        .unwrap();
        assert_eq!(sample.value, expected);
        assert_eq!(sample.clamped, None);
    }
    assert_eq!(motion.track_count(), 1);
}

#[test]
fn inherited_target_refresh_uses_parent_computed_width_on_the_running_track() {
    use iced_css_core::{
        cascade::{Origin, Specificity, winner_table::*},
        values::{computed_border_width::*, registry::PropertyId},
    };
    let motion = Motion::new();
    let mut sync = MotionSync::default();
    let start = Instant::now();
    let mut clock = AnimationClock::default();
    let _ = clock.accept(start);
    let duration = Duration::from_secs(1);
    let track = motion.play(
        MotionKey::from_raw(1),
        Curve::ease(Easing::Linear, duration),
        0.0_f32,
        1.0,
    );
    sync.restart(&motion, start);
    let timing = AnimationTiming::new(duration, Duration::ZERO)
        .accept(start)
        .unwrap();
    let target = parsed("border-top-width:inherit");
    let parent_target = parsed("border-top-width:2em");
    let mut child_winners = PropertyWinners::new();
    let mut parent_winners = PropertyWinners::new();
    let admission = || TableAdmission {
        origin: Origin::Author,
        specificity: Specificity::ZERO,
        order: 0,
        declaration: std::num::NonZeroU64::new(1).unwrap(),
    };
    child_winners.offer(&target, admission()).unwrap();
    parent_winners.offer(&parent_target, admission()).unwrap();
    let property = PropertyId::lookup("border-top-width").unwrap();
    let child_style = parsed("border-top-style:solid");
    for (millis, font, parent_style, expected) in [
        (0, 10.0, "solid", 4.0),
        (250, 10.0, "solid", 8.0),
        (500, 20.0, "solid", 22.0),
        (750, 20.0, "hidden", 1.0),
        (1000, 20.0, "solid", 40.0),
    ] {
        let now = clock.accept(start + Duration::from_millis(millis));
        sync.synchronize(&motion, now);
        let progress = linear_progress(timing.phase_at(now), track.get()).unwrap();
        let parent = compute_cascaded_border_width(
            &parent_winners,
            property,
            &parsed(&format!("border-top-style:{parent_style}")),
            BorderFontContext {
                size: font,
                x_height: font / 2.0,
            },
            None,
            numeric(),
        )
        .unwrap();
        let child = compute_cascaded_border_width(
            &child_winners,
            property,
            &child_style,
            BorderFontContext {
                size: 100.0,
                x_height: 50.0,
            },
            Some(parent),
            numeric(),
        )
        .unwrap();
        let sample = combine_border_width(
            4.0,
            child.sample().value,
            Combination::Interpolate(progress),
        )
        .unwrap()
        .resolve(BorderStyle::Solid, 100.0)
        .unwrap();
        assert_eq!(sample.value, expected);
        assert_eq!(sample.clamped, None);
    }
    assert_eq!(motion.track_count(), 1);
}
