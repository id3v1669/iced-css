use iced_css_core::{
    formatting::line_geometry::{
        InlineBounds, LineGeometryLimits, LineParticipant, VerticalAlignContext,
        VerticalAlignError, VerticalMetric, layout_line_geometry, resolve_vertical_align,
    },
    syntax::*,
    values::{NumericLimits, color::ColorLimits, property::*},
};
use std::num::NonZeroUsize;
fn numeric() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn value(input: &str) -> SpecifiedValue {
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
    value.value
}

fn context() -> VerticalAlignContext {
    VerticalAlignContext {
        font_size: Some(20.0),
        x_height: Some(9.0),
        line_height: Some(30.0),
        parent_x_height: Some(6.0),
        parent_content: Some([-8.0, 2.0]),
        subscript_lowering: Some(3.0),
        superscript_raise: Some(4.0),
    }
}
fn run(css: &str, bounds: InlineBounds, context: VerticalAlignContext) -> (f32, f32, f32, f32) {
    let resolved = resolve_vertical_align(&value(css), bounds, context, numeric()).unwrap();
    assert!(!resolved.rounded_to_zero);
    let nz = |n| NonZeroUsize::new(n).unwrap();
    let line = layout_line_geometry(
        InlineBounds::font(8.0, 2.0, 10.0).unwrap(),
        &[LineParticipant {
            bounds,
            alignment: resolved.alignment,
        }],
        LineGeometryLimits {
            participants: nz(10),
            retained_bytes: nz(10000),
            work: nz(10000),
        },
    )
    .unwrap();
    let p = line.placements[0];
    (line.height, line.baseline, p.top, p.bottom)
}
#[test]
fn parent_relative_keywords_use_the_supplied_content_and_font_metrics() {
    let bounds = InlineBounds::edges(-4.0, 2.0).unwrap();
    for (css, expected) in [
        ("vertical-align:baseline", (10.0, 8.0, 4.0, 10.0)),
        ("vertical-align:middle", (10.0, 8.0, 2.0, 8.0)),
        ("vertical-align:text-top", (10.0, 8.0, 0.0, 6.0)),
        ("vertical-align:text-bottom", (10.0, 8.0, 4.0, 10.0)),
        ("vertical-align:sub", (13.0, 8.0, 7.0, 13.0)),
        ("vertical-align:super", (10.0, 8.0, 0.0, 6.0)),
        ("vertical-align:top", (10.0, 8.0, 0.0, 6.0)),
        ("vertical-align:bottom", (10.0, 8.0, 4.0, 10.0)),
    ] {
        assert_eq!(run(css, bounds, context()), expected, "{css}");
    }
}
#[test]
fn length_and_percentage_raises_use_element_bases_and_signs() {
    let bounds = InlineBounds::edges(-4.0, 2.0).unwrap();
    for (css, expected) in [
        ("vertical-align:10%", (10.0, 8.0, 1.0, 7.0)),
        ("vertical-align:-10%", (13.0, 8.0, 7.0, 13.0)),
        ("vertical-align:0.5em", (16.0, 14.0, 0.0, 6.0)),
        ("vertical-align:1ex", (15.0, 13.0, 0.0, 6.0)),
        ("vertical-align:3px", (10.0, 8.0, 1.0, 7.0)),
    ] {
        assert_eq!(run(css, bounds, context()), expected, "{css}");
    }
}
#[test]
fn missing_context_is_not_silently_guessed_and_unused_context_is_ignored() {
    let bounds = InlineBounds::edges(-4.0, 2.0).unwrap();
    let empty = VerticalAlignContext::default();
    for (css, metric) in [
        ("vertical-align:middle", VerticalMetric::ParentXHeight),
        ("vertical-align:text-top", VerticalMetric::ParentContent),
        ("vertical-align:sub", VerticalMetric::SubscriptLowering),
        ("vertical-align:super", VerticalMetric::SuperscriptRaise),
        ("vertical-align:10%", VerticalMetric::LineHeight),
    ] {
        assert!(
            matches!(resolve_vertical_align(&value(css),bounds,empty,numeric()),Err(VerticalAlignError::Missing(m)) if m==metric)
        );
    }
    assert_eq!(
        run("vertical-align:baseline", bounds, empty),
        (10.0, 8.0, 4.0, 10.0)
    );
    let mut invalid = context();
    invalid.parent_x_height = Some(f32::NAN);
    assert_eq!(
        run("vertical-align:3px", bounds, invalid),
        (10.0, 8.0, 1.0, 7.0)
    );
    assert!(matches!(
        resolve_vertical_align(&value("vertical-align:middle"), bounds, invalid, numeric()),
        Err(VerticalAlignError::Invalid(VerticalMetric::ParentXHeight))
    ));
    assert!(matches!(
        resolve_vertical_align(&value("vertical-align:inherit"), bounds, empty, numeric()),
        Err(VerticalAlignError::Value)
    ));
}

#[test]
fn middle_keeps_an_oversized_exact_raise_until_final_placement() {
    let max = f32::MAX;
    let bounds = InlineBounds::edges(max, max).unwrap();
    let ctx = VerticalAlignContext {
        parent_x_height: Some(max),
        ..Default::default()
    };
    let resolved =
        resolve_vertical_align(&value("vertical-align:middle"), bounds, ctx, numeric()).unwrap();
    let nz = |n| NonZeroUsize::new(n).unwrap();
    let line = layout_line_geometry(
        InlineBounds::font(max, max, max).unwrap(),
        &[LineParticipant {
            bounds,
            alignment: resolved.alignment,
        }],
        LineGeometryLimits {
            participants: nz(10),
            retained_bytes: nz(1000),
            work: nz(1000),
        },
    )
    .unwrap();
    assert_eq!(line.height, max);
    assert_eq!(line.baseline, max / 2.0);
    assert_eq!(
        (
            line.placements[0].top,
            line.placements[0].bottom,
            line.placements[0].origin
        ),
        (0.0, 0.0, -max)
    );
}

#[test]
fn signed_underflow_notice_survives_property_resolution() {
    let bounds = InlineBounds::edges(-4.0, 2.0).unwrap();
    for sign in ["", "-"] {
        let css = format!("vertical-align:{sign}0.{}1px", "0".repeat(80));
        let result = resolve_vertical_align(
            &value(&css),
            bounds,
            VerticalAlignContext::default(),
            numeric(),
        )
        .unwrap();
        assert!(result.rounded_to_zero);
    }
}
#[test]
fn invalid_relevant_metrics_are_rejected_without_context_fallback() {
    let bounds = InlineBounds::edges(-4.0, 2.0).unwrap();
    for invalid in [f32::NAN, f32::INFINITY, -1.0] {
        let mut ctx = context();
        ctx.line_height = Some(invalid);
        assert!(matches!(
            resolve_vertical_align(&value("vertical-align:10%"), bounds, ctx, numeric()),
            Err(VerticalAlignError::Invalid(VerticalMetric::LineHeight))
        ));
        ctx = context();
        ctx.subscript_lowering = Some(invalid);
        assert!(matches!(
            resolve_vertical_align(&value("vertical-align:sub"), bounds, ctx, numeric()),
            Err(VerticalAlignError::Invalid(
                VerticalMetric::SubscriptLowering
            ))
        ));
        ctx = context();
        ctx.superscript_raise = Some(invalid);
        assert!(matches!(
            resolve_vertical_align(&value("vertical-align:super"), bounds, ctx, numeric()),
            Err(VerticalAlignError::Invalid(
                VerticalMetric::SuperscriptRaise
            ))
        ));
    }
    for content in [[2.0, 1.0], [f32::NAN, 1.0], [0.0, f32::INFINITY]] {
        let ctx = VerticalAlignContext {
            parent_content: Some(content),
            ..context()
        };
        assert!(matches!(
            resolve_vertical_align(&value("vertical-align:text-bottom"), bounds, ctx, numeric()),
            Err(VerticalAlignError::Invalid(VerticalMetric::ParentContent))
        ));
    }
}
#[test]
fn subnormal_font_midpoint_is_not_rounded_before_alignment() {
    let tiny = f32::from_bits(1);
    let bounds = InlineBounds::font(tiny, 0.0, tiny).unwrap();
    let ctx = VerticalAlignContext {
        parent_x_height: Some(0.0),
        ..Default::default()
    };
    let resolved =
        resolve_vertical_align(&value("vertical-align:middle"), bounds, ctx, numeric()).unwrap();
    let nz = |n| NonZeroUsize::new(n).unwrap();
    let line = layout_line_geometry(
        InlineBounds::font(0.0, 0.0, tiny).unwrap(),
        &[LineParticipant {
            bounds,
            alignment: resolved.alignment,
        }],
        LineGeometryLimits {
            participants: nz(10),
            retained_bytes: nz(1000),
            work: nz(1000),
        },
    )
    .unwrap();
    assert_eq!(line.height.to_bits(), 1);
    assert_eq!(line.placements[0].origin.to_bits(), 1);
    assert_eq!(line.placements[0].top.to_bits(), 0);
    assert_eq!(line.placements[0].bottom.to_bits(), 1);
}
