use iced_css_core::{
    animation::{combine::Combination, float::*},
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

use iced_css_core::formatting::display::{Display, Float, Flow, Position, resolve_display};
#[test]
fn every_float_pair_obeys_discrete_combination() {
    let floats = [Float::None, Float::Left, Float::Right];
    for a in floats {
        for b in floats {
            for p in [
                -f32::MAX,
                0.0,
                f32::from_bits(0.5_f32.to_bits() - 1),
                0.5,
                1.0,
                f32::MAX,
            ] {
                assert_eq!(
                    combine_float(a, b, Combination::Interpolate(p)).unwrap(),
                    if p < 0.5 { a } else { b }
                );
            }
            for op in [Combination::Add, Combination::Accumulate] {
                assert_eq!(combine_float(a, b, op).unwrap(), a);
            }
            for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert!(combine_float(a, b, Combination::Interpolate(p)).is_err());
            }
        }
    }
}
#[test]
fn sampled_float_flows_through_position_suppression_and_display_transformation() {
    let authored = Float::try_from(&parsed("float:left")).unwrap();
    let endpoint = resolve_display(Display::Inline, Position::Static, authored, false).float;
    let sample = combine_float(Float::None, endpoint, Combination::Interpolate(0.5)).unwrap();
    for position in [Position::Static, Position::Relative] {
        let value = resolve_display(Display::Inline, position, sample, false);
        assert_eq!(
            (value.display, value.float, value.flow),
            (Display::Block, Float::Left, Flow::Floating)
        );
        let value = resolve_display(Display::InlineTable, position, sample, false);
        assert_eq!(value.display, Display::Table);
    }
    for position in [Position::Absolute, Position::Fixed] {
        let value = resolve_display(Display::Inline, position, sample, false);
        assert_eq!(
            (value.display, value.float, value.flow),
            (Display::Block, Float::None, Flow::AbsolutelyPositioned)
        );
        // Computation also suppresses the authored target before pair admission.
        let computed = resolve_display(Display::Inline, position, authored, false).float;
        assert_eq!(
            combine_float(Float::None, computed, Combination::Interpolate(1.0)).unwrap(),
            Float::None
        );
        let hidden = resolve_display(Display::None, position, sample, false);
        assert_eq!(
            (hidden.display, hidden.flow),
            (Display::None, Flow::Suppressed)
        );
    }
    let before = combine_float(Float::None, endpoint, Combination::Interpolate(0.25)).unwrap();
    assert_eq!(
        resolve_display(Display::Inline, Position::Static, before, false).display,
        Display::Inline
    );
    assert!(Float::try_from(&parsed("float:inherit")).is_err());
    assert!(Float::try_from(&parsed("text-align:left")).is_err());
}
