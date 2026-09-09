use iced_css_core::formatting::table_borders::{
    BorderCandidate, BorderConflictError, BorderConflictLimits, BorderOrder, BorderOrigin,
    BorderStyle, CollapsedBorder, resolve_collapsed_border,
};
use std::num::NonZeroUsize;
fn limits() -> BorderConflictLimits {
    BorderConflictLimits {
        candidates: NonZeroUsize::new(100).unwrap(),
        work: NonZeroUsize::new(1000).unwrap(),
    }
}
fn candidate(
    style: BorderStyle,
    width: f32,
    origin: BorderOrigin,
    inline: usize,
    block: usize,
) -> BorderCandidate {
    BorderCandidate::new(
        style,
        width,
        origin,
        BorderOrder {
            inline_start: inline,
            block_start: block,
        },
    )
    .unwrap()
}
fn winner(candidates: &[BorderCandidate]) -> usize {
    match resolve_collapsed_border(candidates, limits()).unwrap() {
        CollapsedBorder::Winner { index, .. } => index,
        other => panic!("expected winner, got {other:?}"),
    }
}
#[test]
fn width_then_style_precedes_origin_and_spatial_priority() {
    use BorderStyle::*;
    // Independent literal ordering from CSS22 17.6.2.1, strongest first.
    let styles = [Double, Solid, Dashed, Dotted, Ridge, Outset, Groove, Inset];
    for (i, &strong) in styles.iter().enumerate() {
        for &weak in &styles[i + 1..] {
            let a = candidate(strong, 3.0, BorderOrigin::Table, 9, 9);
            let b = candidate(weak, 3.0, BorderOrigin::Cell, 0, 0);
            assert_eq!(winner(&[a, b]), 0, "{strong:?} / {weak:?}");
            assert_eq!(winner(&[b, a]), 1);
            let wider = candidate(weak, 4.0, BorderOrigin::Table, 9, 9);
            assert_eq!(winner(&[a, wider]), 1);
        }
    }
}
#[test]
fn hidden_suppresses_and_none_never_defeats_a_visible_style() {
    use BorderStyle::*;
    let hidden = candidate(Hidden, 0.0, BorderOrigin::Table, 9, 9);
    let none = candidate(None, f32::MAX, BorderOrigin::Cell, 0, 0);
    let solid = candidate(Solid, 0.0, BorderOrigin::Table, 9, 9);
    assert_eq!(
        resolve_collapsed_border(&[], limits()).unwrap(),
        CollapsedBorder::Absent
    );
    assert_eq!(
        resolve_collapsed_border(&[none, none], limits()).unwrap(),
        CollapsedBorder::Absent
    );
    assert_eq!(winner(&[none, solid]), 1);
    for entries in [[hidden, solid, none], [solid, none, hidden]] {
        assert_eq!(
            resolve_collapsed_border(&entries, limits()).unwrap(),
            CollapsedBorder::Suppressed
        );
    }
}
#[test]
fn origin_precedes_supplied_directional_rank_and_exact_ties_are_stable() {
    use BorderOrigin::*;
    let origins = [Cell, Row, RowGroup, Column, ColumnGroup, Table];
    for (i, &strong) in origins.iter().enumerate() {
        for &weak in &origins[i + 1..] {
            let a = candidate(BorderStyle::Solid, 2.0, strong, 9, 9);
            let b = candidate(BorderStyle::Solid, 2.0, weak, 0, 0);
            assert_eq!(winner(&[a, b]), 0);
            assert_eq!(winner(&[b, a]), 1);
        }
    }
    let c = |inline, block| candidate(BorderStyle::Solid, 2.0, Cell, inline, block);
    assert_eq!(winner(&[c(2, 0), c(1, 8)]), 1);
    assert_eq!(winner(&[c(1, 9), c(1, 8)]), 1);
    assert_eq!(winner(&[c(1, 8), c(1, 8)]), 0);
}
#[test]
fn relief_conversion_happens_after_winner_selection() {
    let inset = candidate(BorderStyle::Inset, 5.0, BorderOrigin::Cell, 0, 0);
    let groove = candidate(BorderStyle::Groove, 5.0, BorderOrigin::Table, 9, 9);
    assert_eq!(winner(&[inset, groove]), 1);
    for (input, paint) in [
        (BorderStyle::Inset, BorderStyle::Ridge),
        (BorderStyle::Outset, BorderStyle::Groove),
    ] {
        let c = candidate(input, 2.0, BorderOrigin::Cell, 0, 0);
        assert_eq!(
            resolve_collapsed_border(&[c], limits()).unwrap(),
            CollapsedBorder::Winner {
                index: 0,
                paint_style: paint
            }
        );
    }
}
#[test]
fn invalid_geometry_and_explicit_scan_limits_reject_before_selection() {
    for width in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            BorderCandidate::new(
                BorderStyle::Solid,
                width,
                BorderOrigin::Cell,
                BorderOrder {
                    inline_start: 0,
                    block_start: 0
                }
            ),
            Err(BorderConflictError::Width)
        );
    }
    let c = candidate(BorderStyle::Hidden, 0.0, BorderOrigin::Cell, 0, 0);
    let mut bound = limits();
    bound.candidates = NonZeroUsize::new(1).unwrap();
    assert_eq!(
        resolve_collapsed_border(&[c, c], bound),
        Err(BorderConflictError::Candidates)
    );
    bound = limits();
    bound.work = NonZeroUsize::new(1).unwrap();
    assert_eq!(
        resolve_collapsed_border(&[c, c], bound),
        Err(BorderConflictError::Work)
    );
}

use iced_css_core::{
    syntax::*,
    values::{NumericLimits, color::ColorLimits, property::*},
};
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

#[test]
fn parsed_border_styles_keep_original_priority_and_computed_zero_width() {
    use iced_css_core::values::border_width::{BorderFontContext, compute_border_width};
    let context = BorderFontContext {
        size: 16.0,
        x_height: 8.0,
    };
    for (css, expected) in [
        ("none", BorderStyle::None),
        ("hidden", BorderStyle::Hidden),
        ("inset", BorderStyle::Inset),
        ("groove", BorderStyle::Groove),
        ("outset", BorderStyle::Outset),
        ("ridge", BorderStyle::Ridge),
        ("dotted", BorderStyle::Dotted),
        ("dashed", BorderStyle::Dashed),
        ("solid", BorderStyle::Solid),
        ("double", BorderStyle::Double),
    ] {
        assert_eq!(
            BorderStyle::from_resolved(&value(&format!("border-left-style:{css}"))).unwrap(),
            expected
        );
    }
    let width = value("border-left-width:3px");
    let style = value("border-left-style:outset");
    let computed = compute_border_width(&width, &style, context, numeric()).unwrap();
    let c = BorderCandidate::new(
        BorderStyle::from_resolved(&style).unwrap(),
        computed.value,
        BorderOrigin::Cell,
        BorderOrder {
            inline_start: 0,
            block_start: 0,
        },
    )
    .unwrap();
    assert_eq!(c.style(), BorderStyle::Outset);
    assert_eq!(c.width(), 3.0);
    assert_eq!(
        resolve_collapsed_border(&[c], limits()).unwrap(),
        CollapsedBorder::Winner {
            index: 0,
            paint_style: BorderStyle::Groove
        }
    );
    for css in ["border-left-style:none", "border-left-style:hidden"] {
        let style = BorderStyle::from_resolved(&value(css)).unwrap();
        let c = candidate(style, 3.0, BorderOrigin::Cell, 0, 0);
        assert_eq!(c.width().to_bits(), 0);
    }
    for css in [
        "border-left-style:inherit",
        "border-left-width:thin",
        "border-left-width:3px",
    ] {
        assert_eq!(
            BorderStyle::from_resolved(&value(css)),
            Err(BorderConflictError::Style)
        );
    }
}
