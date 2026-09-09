use iced_css_core::formatting::line_geometry::{
    InlineBounds, LineAlignment, LineGeometryError, LineGeometryLimits, LineParticipant,
    layout_line_geometry,
};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> LineGeometryLimits {
    LineGeometryLimits {
        participants: nz(100),
        retained_bytes: nz(10000),
        work: nz(10000),
    }
}
fn strut() -> InlineBounds {
    InlineBounds::font(8.0, 2.0, 10.0).unwrap()
}
fn normal(bounds: InlineBounds, raise: f32) -> LineParticipant {
    LineParticipant {
        bounds,
        alignment: LineAlignment::Baseline { raise },
    }
}
#[test]
fn parent_strut_and_negative_leading_determine_the_line_not_ink_height() {
    let empty = layout_line_geometry(strut(), &[], limits()).unwrap();
    assert_eq!((empty.height, empty.baseline), (10.0, 8.0));
    assert!(empty.placements.is_empty());
    let large_font = normal(InlineBounds::font(15.0, 5.0, 10.0).unwrap(), 0.0);
    let line = layout_line_geometry(strut(), &[large_font], limits()).unwrap();
    assert_eq!((line.height, line.baseline), (12.0, 10.0));
    let p = line.placements[0];
    assert_eq!((p.top, p.bottom, p.origin), (0.0, 10.0, 10.0));
}
#[test]
fn positive_raise_moves_up_and_negative_raise_moves_down() {
    let entries = [normal(strut(), 3.0), normal(strut(), -4.0)];
    let line = layout_line_geometry(strut(), &entries, limits()).unwrap();
    assert_eq!((line.height, line.baseline), (17.0, 11.0));
    let positions: Vec<_> = line
        .placements
        .iter()
        .map(|p| (p.top, p.bottom, p.origin))
        .collect();
    assert_eq!(positions, [(0.0, 10.0, 8.0), (7.0, 17.0, 15.0)]);
}
#[test]
fn tall_top_and_bottom_subtrees_preserve_the_normal_baseline_policy() {
    let entries = [
        LineParticipant {
            bounds: InlineBounds::edges(-6.0, 14.0).unwrap(),
            alignment: LineAlignment::Top,
        },
        LineParticipant {
            bounds: InlineBounds::edges(-3.0, 2.0).unwrap(),
            alignment: LineAlignment::Bottom,
        },
        normal(strut(), 0.0),
    ];
    let line = layout_line_geometry(strut(), &entries, limits()).unwrap();
    assert_eq!((line.height, line.baseline), (20.0, 8.0));
    let positions: Vec<_> = line
        .placements
        .iter()
        .map(|p| (p.top, p.bottom, p.origin))
        .collect();
    assert_eq!(
        positions,
        [(0.0, 20.0, 6.0), (15.0, 20.0, 18.0), (0.0, 10.0, 8.0)]
    );
}
#[test]
fn zero_line_height_and_exact_subnormal_half_leading_are_preserved() {
    let zero = InlineBounds::font(8.0, 2.0, 0.0).unwrap();
    let line = layout_line_geometry(zero, &[normal(zero, 0.0)], limits()).unwrap();
    assert_eq!((line.height, line.baseline), (0.0, 3.0));
    let p = line.placements[0];
    assert_eq!((p.top, p.bottom, p.origin), (0.0, 0.0, 3.0));
    // Each half is below the smallest positive f32. Preserve them until their
    // difference, otherwise an early rounding would erase this line's height.
    let tiny = f32::from_bits(1);
    let bounds = InlineBounds::font(0.0, 0.0, tiny).unwrap();
    let line = layout_line_geometry(bounds, &[normal(bounds, 0.0)], limits()).unwrap();
    assert_eq!(line.height.to_bits(), 1);
    assert_eq!(line.baseline.to_bits(), 0);
    assert_eq!(line.placements[0].bottom.to_bits(), 1);
}
#[test]
fn checked_extremes_allow_cancellation_but_reject_unrepresentable_geometry() {
    let max = f32::MAX;
    let bounds = InlineBounds::font(max, max, max).unwrap();
    let line = layout_line_geometry(bounds, &[normal(bounds, 0.0)], limits()).unwrap();
    assert_eq!(line.height, max); // A+D would overflow a naive f32 intermediate.
    assert_eq!(line.baseline, max / 2.0);
    let wide = InlineBounds::edges(-max, max).unwrap();
    assert!(matches!(
        layout_line_geometry(wide, &[], limits()),
        Err(LineGeometryError::Unrepresentable)
    ));
}
#[test]
fn invalid_inputs_and_limits_reject_the_whole_line() {
    for invalid in [-1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            InlineBounds::font(invalid, 0.0, 1.0),
            Err(LineGeometryError::Metrics)
        );
        assert_eq!(
            InlineBounds::font(1.0, invalid, 1.0),
            Err(LineGeometryError::Metrics)
        );
        assert_eq!(
            InlineBounds::font(1.0, 0.0, invalid),
            Err(LineGeometryError::Metrics)
        );
    }
    assert_eq!(
        InlineBounds::edges(2.0, 1.0),
        Err(LineGeometryError::Bounds)
    );
    assert_eq!(
        InlineBounds::edges(f32::NEG_INFINITY, 1.0),
        Err(LineGeometryError::Bounds)
    );
    assert!(matches!(
        layout_line_geometry(strut(), &[normal(strut(), f32::NAN)], limits()),
        Err(LineGeometryError::Alignment)
    ));
    let entries = [normal(strut(), 0.0); 2];
    let mut bound = limits();
    bound.participants = nz(1);
    assert!(matches!(
        layout_line_geometry(strut(), &entries, bound),
        Err(LineGeometryError::Participants)
    ));
    bound = limits();
    bound.retained_bytes = nz(1);
    assert!(matches!(
        layout_line_geometry(strut(), &entries, bound),
        Err(LineGeometryError::Bytes)
    ));
    bound = limits();
    bound.work = nz(1);
    assert!(matches!(
        layout_line_geometry(strut(), &entries, bound),
        Err(LineGeometryError::Work)
    ));
}

#[test]
fn inherited_line_height_semantics_feed_the_same_geometry_kernel() {
    use iced_css_core::values::{
        Decimal, LengthTarget, NumericLimits,
        line_height::{LineHeightFont, compute_line_height, used_line_height},
        property::SpecifiedValue,
    };
    let numeric = NumericLimits {
        digits: nz(1000),
        work: nz(1000000),
    };
    let parent_font = LineHeightFont {
        size: 20.0,
        x_height: 9.0,
    };
    let factor = compute_line_height(
        SpecifiedValue::Number(Decimal::parse("1.5", numeric).unwrap()),
        parent_font,
        numeric,
    )
    .unwrap();
    let percent = compute_line_height(
        SpecifiedValue::Length(LengthTarget::parse("150%", numeric).unwrap()),
        parent_font,
        numeric,
    )
    .unwrap();
    for (computed, height, baseline) in [(&factor.value, 15.0, 10.5), (&percent.value, 30.0, 18.0)]
    {
        let used = used_line_height(computed, 10.0, f32::NAN, numeric).unwrap();
        let bounds = InlineBounds::font(8.0, 2.0, used.value).unwrap();
        let line = layout_line_geometry(bounds, &[], limits()).unwrap();
        assert_eq!((line.height, line.baseline), (height, baseline));
    }
}

#[test]
fn unrepresentable_participant_origin_rejects_without_partial_placements() {
    let bounds = InlineBounds::edges(-f32::MAX, -f32::MAX).unwrap();
    let entries = [normal(bounds, 0.0), normal(bounds, -f32::MAX)];
    assert!(matches!(
        layout_line_geometry(bounds, &entries, limits()),
        Err(LineGeometryError::Unrepresentable)
    ));
}
