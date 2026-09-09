use iced_css_core::{
    animation::{
        combine::Combination,
        number::{Clamp, NumberError},
        vertical_align::*,
    },
    formatting::line_geometry::{
        InlineBounds, LineAlignment, LineGeometryLimits, LineParticipant, VerticalAlignContext,
        layout_line_geometry, resolve_computed_vertical_align,
    },
    syntax::*,
    values::{NumericLimits, color::ColorLimits, property::*, vertical_align::*},
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

fn computed(css: &str) -> ComputedVerticalAlign {
    compute_vertical_align(
        &parsed(css),
        VerticalAlignComputationContext {
            font_size: Some(10.0),
            x_height: Some(4.0),
            line_height: Some(20.0),
        },
        numeric(),
    )
    .unwrap()
    .value
}
const KEYWORDS: [VerticalAlignKeyword; 8] = [
    VerticalAlignKeyword::Baseline,
    VerticalAlignKeyword::Sub,
    VerticalAlignKeyword::Super,
    VerticalAlignKeyword::Top,
    VerticalAlignKeyword::Bottom,
    VerticalAlignKeyword::Middle,
    VerticalAlignKeyword::TextTop,
    VerticalAlignKeyword::TextBottom,
];
#[test]
fn percentages_compute_to_absolute_raises_before_animation_and_later_layout() {
    let a = computed("vertical-align:50%");
    let b = computed("vertical-align:20px");
    assert_eq!(a.component(), VerticalAlignComponent::Pixels(10.0));
    assert_eq!(
        computed("vertical-align:-2ex").component(),
        VerticalAlignComponent::Pixels(-8.0)
    );
    let sample = combine_vertical_align(a, b, Combination::Interpolate(0.5))
        .unwrap()
        .resolve(-100.0, 100.0)
        .unwrap();
    assert_eq!(
        sample.value.component(),
        VerticalAlignComponent::Pixels(15.0)
    );
    let bounds = InlineBounds::edges(-4.0, 2.0).unwrap();
    let resolved = resolve_computed_vertical_align(
        sample.value,
        bounds,
        VerticalAlignContext {
            line_height: Some(200.0),
            font_size: Some(500.0),
            ..VerticalAlignContext::default()
        },
    )
    .unwrap();
    assert!(matches!(resolved.alignment,LineAlignment::Baseline{raise} if raise==15.0));
    let n = |v| NonZeroUsize::new(v).unwrap();
    let line = layout_line_geometry(
        InlineBounds::font(8.0, 2.0, 10.0).unwrap(),
        &[LineParticipant {
            bounds,
            alignment: resolved.alignment,
        }],
        LineGeometryLimits {
            participants: n(10),
            retained_bytes: n(10000),
            work: n(10000),
        },
    )
    .unwrap();
    assert_eq!(
        (
            line.height,
            line.baseline,
            line.placements[0].top,
            line.placements[0].bottom
        ),
        (21.0, 19.0, 0.0, 6.0)
    );
}
#[test]
fn all_keyword_pairs_and_numeric_incompatibility_select_whole_values() {
    for a in KEYWORDS {
        for b in KEYWORDS {
            let start = computed(&format!("vertical-align:{}", a.as_str()));
            let end = computed(&format!("vertical-align:{}", b.as_str()));
            for (op, want) in [
                (Combination::Interpolate(0.25), start),
                (Combination::Interpolate(0.5), end),
                (Combination::Add, start),
                (Combination::Accumulate, start),
            ] {
                assert_eq!(
                    combine_vertical_align(start, end, op)
                        .unwrap()
                        .resolve(-100.0, 100.0)
                        .unwrap()
                        .value,
                    want
                );
            }
            for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert_eq!(
                    combine_vertical_align(start, end, Combination::Interpolate(p)).unwrap_err(),
                    NumberError::NonfiniteProgress
                );
            }
        }
    }
    let a = computed("vertical-align:baseline");
    let b = computed("vertical-align:20px");
    assert_eq!(
        combine_vertical_align(a, b, Combination::Interpolate(0.25))
            .unwrap()
            .resolve(-100.0, 100.0)
            .unwrap()
            .value,
        a
    );
    assert_eq!(
        combine_vertical_align(a, b, Combination::Interpolate(0.5))
            .unwrap()
            .resolve(-100.0, 100.0)
            .unwrap()
            .value,
        b
    );
    assert_eq!(
        combine_vertical_align(b, a, Combination::Add)
            .unwrap()
            .resolve(-100.0, 100.0)
            .unwrap()
            .value,
        b
    );
}
#[test]
fn signed_animation_underflow_and_final_ranges_are_reported_separately() {
    let zero = ComputedVerticalAlign::from_pixels(0.0).unwrap();
    let tiny = ComputedVerticalAlign::from_pixels(f32::from_bits(1)).unwrap();
    for p in [-0.5, 0.5] {
        let sample = combine_vertical_align(zero, tiny, Combination::Interpolate(p))
            .unwrap()
            .resolve(-1.0, 1.0)
            .unwrap();
        assert_eq!(
            sample.value.component(),
            VerticalAlignComponent::Pixels(0.0)
        );
        assert!(sample.rounded_to_zero);
        assert_eq!(sample.clamped, None);
    }
    let b = computed("vertical-align:20px");
    let sample = combine_vertical_align(zero, b, Combination::Interpolate(10.0))
        .unwrap()
        .resolve(-100.0, 100.0)
        .unwrap();
    assert_eq!(sample.clamped, Some(Clamp::Upper));
    assert!(!sample.rounded_to_zero);
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(
            combine_vertical_align(b, b, op)
                .unwrap()
                .resolve(-100.0, 100.0)
                .unwrap()
                .value
                .component(),
            VerticalAlignComponent::Pixels(40.0)
        );
    }
}
#[test]
fn context_and_property_errors_are_not_hidden_by_unused_defaults() {
    let empty = VerticalAlignComputationContext::default();
    assert_eq!(
        compute_vertical_align(&parsed("vertical-align:10%"), empty, numeric()).unwrap_err(),
        ComputedVerticalAlignError::MissingLineHeight
    );
    assert!(compute_vertical_align(&parsed("vertical-align:2px"), empty, numeric()).is_ok());
    for css in ["vertical-align:inherit", "width:10px"] {
        assert_eq!(
            compute_vertical_align(&parsed(css), empty, numeric()).unwrap_err(),
            ComputedVerticalAlignError::Unresolved
        );
    }
    for v in [f32::NAN, f32::INFINITY, -1.0] {
        assert_eq!(
            compute_vertical_align(
                &parsed("vertical-align:10%"),
                VerticalAlignComputationContext {
                    line_height: Some(v),
                    ..empty
                },
                numeric()
            )
            .unwrap_err(),
            ComputedVerticalAlignError::InvalidLineHeight
        );
    }
    assert!(ComputedVerticalAlign::from_pixels(f32::NAN).is_err());
    let a = computed("vertical-align:top");
    assert_eq!(
        combine_vertical_align(a, a, Combination::Add)
            .unwrap()
            .resolve(1.0, 0.0)
            .unwrap_err(),
        NumberError::InvalidRange
    );
    let bounds = InlineBounds::edges(-4.0, 2.0).unwrap();
    assert!(matches!(
        resolve_computed_vertical_align(a, bounds, VerticalAlignContext::default())
            .unwrap()
            .alignment,
        LineAlignment::Top
    ));
    assert!(
        resolve_computed_vertical_align(
            computed("vertical-align:middle"),
            bounds,
            VerticalAlignContext::default()
        )
        .is_err()
    );
}
