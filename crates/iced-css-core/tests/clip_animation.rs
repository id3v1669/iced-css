use iced_css_core::{
    animation::{
        clip::*,
        combine::Combination,
        number::{Clamp, NumberError},
    },
    formatting::{
        clip::{ClipRegion, resolve_computed_clip},
        display::Position,
    },
    syntax::*,
    values::{
        NumericLimits,
        color::ColorLimits,
        computed_clip::*,
        geometry::{LengthBasis, LengthContext, LengthEvaluationError},
        property::*,
    },
};
use std::num::NonZeroUsize;
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

fn rect(sides: [Option<f32>; 4]) -> ComputedClip {
    ComputedClip::rectangle(sides).unwrap()
}
fn computed(css: &str) -> ComputedClip {
    compute_clip(&parsed(css), LengthContext::default(), numeric())
        .unwrap()
        .value
}
#[test]
fn clip_computation_keeps_auto_sides_until_border_box_resolution() {
    let c = compute_clip(
        &parsed("clip:rect(-1em,auto,3ex,-0px)"),
        LengthContext {
            font_size: Some(10.0),
            x_height: Some(4.0),
            percentage_basis: None,
            root_font_size: None,
        },
        numeric(),
    )
    .unwrap();
    assert_eq!(c.value, rect([Some(-10.0), None, Some(12.0), Some(0.0)]));
    assert_eq!(c.rounded_to_zero, [false; 4]);
    let all = computed("clip:rect(auto,auto,auto,auto)");
    assert_ne!(all, ComputedClip::Auto);
    for size in [[100.0, 50.0], [200.0, 150.0]] {
        assert_eq!(
            resolve_computed_clip(all, Position::Absolute, size)
                .unwrap()
                .edges(),
            Some([0.0, 0.0, size[0], size[1]])
        );
        assert_eq!(
            resolve_computed_clip(ComputedClip::Auto, Position::Absolute, size).unwrap(),
            ClipRegion::Unbounded
        );
    }
    assert_eq!(
        resolve_computed_clip(c.value, Position::Fixed, [100.0, 50.0])
            .unwrap()
            .edges(),
        Some([0.0, -10.0, 100.0, 12.0])
    );
    assert_eq!(
        resolve_computed_clip(c.value, Position::Relative, [100.0, 50.0]).unwrap(),
        ClipRegion::Unbounded
    );
}
#[test]
fn every_auto_side_mask_pair_uses_whole_value_fallback_on_any_mismatch() {
    for a in 0_u8..16 {
        for b in 0_u8..16 {
            let start = rect(std::array::from_fn(|i| (a & (1 << i) == 0).then_some(10.0)));
            let end = rect(std::array::from_fn(|i| (b & (1 << i) == 0).then_some(30.0)));
            for (op, selected, numeric_value) in [
                (Combination::Interpolate(0.25), start, 15.0),
                (Combination::Interpolate(0.5), end, 20.0),
                (Combination::Add, start, 40.0),
                (Combination::Accumulate, start, 40.0),
            ] {
                let expected = if a == b {
                    rect(std::array::from_fn(|i| {
                        (a & (1 << i) == 0).then_some(numeric_value)
                    }))
                } else {
                    selected
                };
                let actual = combine_clip(start, end, op)
                    .unwrap()
                    .resolve(-100.0, 100.0)
                    .unwrap();
                assert_eq!(actual.value, expected, "mask {a}/{b}, {op:?}");
                assert_eq!(actual.clamped, [None; 4]);
            }
        }
    }
    let r = rect([Some(30.0); 4]);
    for (a, b) in [(ComputedClip::Auto, r), (r, ComputedClip::Auto)] {
        for (op, want) in [
            (Combination::Interpolate(0.25), a),
            (Combination::Interpolate(0.5), b),
            (Combination::Add, a),
            (Combination::Accumulate, a),
        ] {
            assert_eq!(
                combine_clip(a, b, op)
                    .unwrap()
                    .resolve(-100.0, 100.0)
                    .unwrap()
                    .value,
                want
            );
        }
    }
}
#[test]
fn numeric_clip_samples_feed_used_geometry_and_intersections() {
    let a = computed("clip:rect(0px,80px,40px,-20px)");
    let b = computed("clip:rect(10px,100px,60px,0px)");
    let sample = combine_clip(a, b, Combination::Interpolate(0.5))
        .unwrap()
        .resolve(-100.0, 100.0)
        .unwrap();
    let region = resolve_computed_clip(sample.value, Position::Absolute, [200.0, 100.0]).unwrap();
    assert_eq!(region.edges(), Some([-10.0, 5.0, 90.0, 50.0]));
    assert_eq!(
        region
            .intersect(ClipRegion::from_edges([0.0, 0.0, 80.0, 60.0]).unwrap())
            .edges(),
        Some([0.0, 5.0, 80.0, 50.0])
    );
    let sample = combine_clip(a, b, Combination::Interpolate(-2.0))
        .unwrap()
        .resolve(-30.0, 50.0)
        .unwrap();
    assert_eq!(
        sample.value,
        rect([Some(-20.0), Some(40.0), Some(0.0), Some(-30.0)])
    );
    assert_eq!(sample.clamped, [None, None, None, Some(Clamp::Lower)]);
    let empty = rect([Some(10.0), Some(5.0), Some(10.0), Some(0.0)]);
    assert_eq!(
        resolve_computed_clip(empty, Position::Absolute, [100.0, 100.0]).unwrap(),
        ClipRegion::Empty
    );
    let wide = rect([Some(f32::MAX); 4]);
    assert_eq!(
        combine_clip(wide, wide, Combination::Interpolate(f32::MAX))
            .unwrap()
            .resolve(-f32::MAX, f32::MAX)
            .unwrap()
            .value,
        wide
    );
}
#[test]
fn invalid_inputs_and_ranges_fail_without_losing_signed_underflow() {
    for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(ComputedClip::rectangle([Some(v); 4]).is_err());
        for value in [ComputedClip::Auto, rect([None; 4]), rect([Some(1.0); 4])] {
            assert_eq!(
                combine_clip(value, value, Combination::Interpolate(v)).unwrap_err(),
                NumberError::NonfiniteProgress
            );
        }
    }
    assert_eq!(
        compute_clip(&parsed("clip:inherit"), LengthContext::default(), numeric()).unwrap_err(),
        ComputedClipError::Unresolved
    );
    assert_eq!(
        compute_clip(
            &parsed("clip:rect(1em,auto,auto,auto)"),
            LengthContext::default(),
            numeric()
        )
        .unwrap_err(),
        ComputedClipError::Length(LengthEvaluationError::MissingContext(LengthBasis::FontSize))
    );
    let mut forged = parsed("clip:rect(auto,auto,auto,auto)");
    let SpecifiedValue::Length(p) = parsed("width:10%").value else {
        panic!()
    };
    forged.value = SpecifiedValue::ClipRect(std::array::from_fn(|_| p.clone()));
    assert_eq!(
        compute_clip(&forged, LengthContext::default(), numeric()).unwrap_err(),
        ComputedClipError::Percentage
    );
    let tiny = format!("clip:rect(-0.{}1px,auto,auto,auto)", "0".repeat(60));
    let value = compute_clip(&parsed(&tiny), LengthContext::default(), numeric()).unwrap();
    assert_eq!(value.rounded_to_zero, [true, false, false, false]);
    assert_eq!(value.value.sides().unwrap()[0].unwrap().to_bits(), 0);
    for (min, max) in [(1.0, 0.0), (f32::NAN, 1.0), (0.0, f32::INFINITY)] {
        assert_eq!(
            combine_clip(ComputedClip::Auto, ComputedClip::Auto, Combination::Add)
                .unwrap()
                .resolve(min, max)
                .unwrap_err(),
            NumberError::InvalidRange
        );
    }
    assert!(resolve_computed_clip(ComputedClip::Auto, Position::Absolute, [-1.0, 1.0]).is_err());
    // A discrete selected numeric endpoint still obeys final coordinate limits.
    assert_eq!(
        combine_clip(
            ComputedClip::Auto,
            rect([Some(60.0); 4]),
            Combination::Interpolate(0.5)
        )
        .unwrap()
        .resolve(-10.0, 50.0)
        .unwrap()
        .clamped,
        [Some(Clamp::Upper); 4]
    );
}
