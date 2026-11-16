use iced_css_core::{
    formatting::{
        clip::{ClipError, ClipRegion, resolve_clip},
        display::Position,
    },
    syntax::*,
    values::{NumericLimits, color::ColorLimits, geometry::LengthContext, property::*},
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
fn resolved(css: &str, position: Position) -> Result<ClipRegion, ClipError> {
    resolve_clip(
        &value(css),
        position,
        [100.0, 80.0],
        LengthContext {
            font_size: Some(20.0),
            x_height: Some(9.0),
            percentage_basis: None,
            root_font_size: None,
        },
        numeric(),
    )
}
#[test]
fn auto_property_and_auto_edges_have_different_clipping_effects() {
    assert_eq!(
        resolved("clip:auto", Position::Absolute).unwrap(),
        ClipRegion::Unbounded
    );
    assert_eq!(
        resolved("clip:rect(auto,auto,auto,auto)", Position::Absolute)
            .unwrap()
            .edges(),
        Some([0.0, 0.0, 100.0, 80.0])
    );
    assert_eq!(
        resolved("clip:rect(-1em,auto,3ex,-4px)", Position::Fixed)
            .unwrap()
            .edges(),
        Some([-4.0, -20.0, 100.0, 27.0])
    );
}
#[test]
fn nonabsolute_clip_is_inapplicable_without_evaluating_font_relative_edges() {
    let clip = value("clip:rect(1em,auto,auto,auto)");
    for position in [Position::Static, Position::Relative] {
        assert_eq!(
            resolve_clip(
                &clip,
                position,
                [100.0, 80.0],
                LengthContext::default(),
                numeric()
            )
            .unwrap(),
            ClipRegion::Unbounded
        );
    }
    assert!(matches!(
        resolve_clip(
            &clip,
            Position::Absolute,
            [100.0, 80.0],
            LengthContext::default(),
            numeric()
        ),
        Err(ClipError::Length(_))
    ));
}
#[test]
fn equal_and_reversed_edges_are_empty_without_coordinate_reordering() {
    for css in [
        "clip:rect(0px,5px,20px,5px)",
        "clip:rect(10px,20px,10px,0px)",
        "clip:rect(30px,20px,10px,0px)",
        "clip:rect(0px,-10px,20px,0px)",
    ] {
        assert_eq!(
            resolved(css, Position::Absolute).unwrap(),
            ClipRegion::Empty
        );
    }
}
#[test]
fn common_coordinate_intersections_keep_parent_restrictions_and_large_edges() {
    let parent = ClipRegion::from_edges([0.0, 0.0, 10.0, 10.0]).unwrap();
    let broad = ClipRegion::from_edges([-100.0, -100.0, 100.0, 100.0]).unwrap();
    assert_eq!(parent.intersect(broad), parent);
    let child = ClipRegion::from_edges([5.0, 6.0, 20.0, 30.0]).unwrap();
    assert_eq!(
        parent.intersect(child).edges(),
        Some([5.0, 6.0, 10.0, 10.0])
    );
    assert_eq!(
        parent.intersect(ClipRegion::from_edges([10.0, 0.0, 20.0, 10.0]).unwrap()),
        ClipRegion::Empty
    );
    assert_eq!(parent.intersect(ClipRegion::Unbounded), parent);
    assert_eq!(ClipRegion::Empty.intersect(parent), ClipRegion::Empty);
    let huge = ClipRegion::from_edges([-f32::MAX, -f32::MAX, f32::MAX, f32::MAX]).unwrap();
    assert_eq!(huge.intersect(parent), parent);
    assert_eq!(
        huge.edges(),
        Some([-f32::MAX, -f32::MAX, f32::MAX, f32::MAX])
    );
}
#[test]
fn invalid_geometry_unresolved_values_and_manual_percentage_edges_fail() {
    let clip = value("clip:rect(auto,auto,auto,auto)");
    for size in [[-1.0, 10.0], [10.0, f32::INFINITY]] {
        assert_eq!(
            resolve_clip(
                &clip,
                Position::Absolute,
                size,
                LengthContext::default(),
                numeric()
            ),
            Err(ClipError::BorderSize)
        );
    }
    assert_eq!(
        ClipRegion::from_edges([f32::NAN, 0.0, 10.0, 10.0]),
        Err(ClipError::Coordinate)
    );
    assert_eq!(
        resolve_clip(
            &SpecifiedValue::Inherit,
            Position::Absolute,
            [10.0, 10.0],
            LengthContext::default(),
            numeric()
        ),
        Err(ClipError::Value)
    );
    let SpecifiedValue::Length(percent) = value("width:10%") else {
        panic!()
    };
    let manual = SpecifiedValue::ClipRect(std::array::from_fn(|_| percent.clone()));
    assert_eq!(
        resolve_clip(
            &manual,
            Position::Absolute,
            [10.0, 10.0],
            LengthContext::default(),
            numeric()
        ),
        Err(ClipError::Percentage)
    );
    let zero = ClipRegion::from_edges([-0.0, -0.0, 10.0, 10.0])
        .unwrap()
        .edges()
        .unwrap();
    assert_eq!((zero[0].to_bits(), zero[1].to_bits()), (0, 0));
}
