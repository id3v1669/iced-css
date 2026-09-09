use iced_css_core::{
    animation::combine::*,
    syntax::*,
    values::{NumericLimits, color::ColorLimits, property::*},
};
use std::num::NonZeroUsize;
#[test]
fn discrete_midpoint_extrapolation_and_nonadditive_operations() {
    for p in [-10.0, -0.0, 0.0, f32::from_bits(0.5f32.to_bits() - 1)] {
        assert_eq!(
            discrete_choice(Combination::Interpolate(p)).unwrap(),
            Endpoint::Start
        );
    }
    for p in [0.5, 1.0, 10.0] {
        assert_eq!(
            discrete_choice(Combination::Interpolate(p)).unwrap(),
            Endpoint::End
        );
    }
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(discrete_choice(op).unwrap(), Endpoint::Start);
    }
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            discrete_choice(Combination::Interpolate(p)),
            Err(CombinationError::NonfiniteProgress)
        );
    }
}
#[test]
fn visibility_interiors_and_endpoints_cover_all_keyword_pairs() {
    use Visibility::*;
    for start in [Visible, Hidden, Collapse] {
        for end in [Visible, Hidden, Collapse] {
            for p in [-1.0, 0.0, 1.0, 2.0] {
                assert_eq!(
                    combine_visibility(start, end, Combination::Interpolate(p)).unwrap(),
                    if p <= 0.0 { start } else { end }
                );
            }
            for p in [
                f32::from_bits(1),
                0.25,
                0.5,
                0.75,
                f32::from_bits(1.0f32.to_bits() - 1),
            ] {
                let expected = if start == Visible || end == Visible {
                    Visible
                } else if p < 0.5 {
                    start
                } else {
                    end
                };
                assert_eq!(
                    combine_visibility(start, end, Combination::Interpolate(p)).unwrap(),
                    expected
                );
            }
            for op in [Combination::Add, Combination::Accumulate] {
                assert_eq!(combine_visibility(start, end, op).unwrap(), start);
            }
            assert_eq!(
                combine_visibility(start, end, Combination::Interpolate(f32::NAN)),
                Err(CombinationError::NonfiniteProgress)
            );
        }
    }
}
fn parse(css: &str) -> ParsedProperty {
    let n = NonZeroUsize::new(100000).unwrap();
    let DeclarationItem::Declaration(declaration) = Declarations::new(
        css,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: n,
                retained_bytes: n,
                nesting: n,
            },
            name_bytes: n,
        },
    )
    .next()
    .unwrap()
    .unwrap() else {
        panic!()
    };
    let PropertyResult::Parsed(parsed) = parse_property(
        &declaration,
        ColorLimits {
            syntax: StringLimits {
                work: n,
                retained_bytes: n,
            },
            numeric: NumericLimits { work: n, digits: n },
        },
    )
    .unwrap() else {
        panic!()
    };
    parsed
}
#[test]
fn only_resolved_visibility_keywords_enter_the_typed_combiner() {
    for (css, expected) in [
        ("visibility:visible", Visibility::Visible),
        ("visibility:hidden", Visibility::Hidden),
        ("visibility:collapse", Visibility::Collapse),
    ] {
        assert_eq!(Visibility::try_from(&parse(css)).unwrap(), expected);
    }
    for css in [
        "visibility:inherit",
        "overflow:hidden",
        "border-collapse:collapse",
    ] {
        assert_eq!(
            Visibility::try_from(&parse(css)),
            Err(CombinationError::UnresolvedVisibility)
        );
    }
}
