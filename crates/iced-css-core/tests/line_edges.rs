use iced_css_core::text::line_edges::*;
use iced_css_core::{
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

fn limits() -> LineEdgeLimits {
    let nz = |v| NonZeroUsize::new(v).unwrap();
    LineEdgeLimits {
        items: nz(1000),
        retained_bytes: nz(10000),
        work: nz(3000),
    }
}
fn scalar(c: char, white_space: WhiteSpace) -> LineEdgeItem {
    LineEdgeItem {
        kind: LineEdgeKind::Scalar(c),
        white_space,
    }
}
fn control() -> LineEdgeItem {
    LineEdgeItem {
        kind: LineEdgeKind::BidiFormatting,
        white_space: WhiteSpace::Normal,
    }
}
#[test]
fn five_modes_have_distinct_css_behavior() {
    for (name, mode, collapse, wrap, newlines) in [
        ("normal", WhiteSpace::Normal, true, true, false),
        ("nowrap", WhiteSpace::Nowrap, true, false, false),
        ("pre", WhiteSpace::Pre, false, false, true),
        ("pre-wrap", WhiteSpace::PreWrap, false, true, true),
        ("pre-line", WhiteSpace::PreLine, true, true, true),
    ] {
        assert_eq!(
            WhiteSpace::from_resolved(&value(&format!("white-space:{name}"))),
            Some(mode)
        );
        assert_eq!(
            (
                mode.collapses_spaces(),
                mode.allows_soft_wrap(),
                mode.preserves_newlines()
            ),
            (collapse, wrap, newlines)
        );
        let actual = trim_line_edges(
            &[scalar(' ', mode), scalar('x', mode), scalar(' ', mode)],
            limits(),
        )
        .unwrap();
        assert_eq!(
            actual,
            if collapse {
                vec![
                    EdgeDisposition::RemovedLeading,
                    EdgeDisposition::Keep,
                    EdgeDisposition::RemovedTrailing,
                ]
            } else {
                vec![EdgeDisposition::Keep; 3]
            }
        );
    }
    assert_eq!(
        WhiteSpace::from_resolved(&value("white-space:inherit")),
        None
    );
    assert_eq!(WhiteSpace::from_resolved(&value("font-weight:bold")), None);
    assert_eq!(
        WhiteSpace::from_resolved(&value("white-space:PRE")),
        Some(WhiteSpace::Pre)
    );
}
#[test]
fn controls_are_transparent_and_retained_while_source_order_is_preserved() {
    let n = WhiteSpace::Normal;
    let p = WhiteSpace::PreLine;
    let items = [
        control(),
        scalar(' ', n),
        control(),
        scalar(' ', p),
        scalar('x', n),
        scalar(' ', n),
        control(),
        scalar(' ', p),
        control(),
    ];
    assert_eq!(
        trim_line_edges(&items, limits()).unwrap(),
        [
            EdgeDisposition::Keep,
            EdgeDisposition::RemovedLeading,
            EdgeDisposition::Keep,
            EdgeDisposition::RemovedLeading,
            EdgeDisposition::Keep,
            EdgeDisposition::RemovedTrailing,
            EdgeDisposition::Keep,
            EdgeDisposition::RemovedTrailing,
            EdgeDisposition::Keep
        ]
    );
}
#[test]
fn atomics_preserved_spaces_and_non_space_scalars_block_edge_removal() {
    let n = WhiteSpace::Normal;
    for barrier in [
        LineEdgeItem {
            kind: LineEdgeKind::Atomic,
            white_space: n,
        },
        scalar(' ', WhiteSpace::Pre),
        scalar(' ', WhiteSpace::PreWrap),
        scalar('\u{a0}', n),
        scalar('\t', WhiteSpace::PreWrap),
    ] {
        let items = [
            barrier,
            scalar(' ', n),
            scalar('x', n),
            scalar(' ', n),
            barrier,
        ];
        assert_eq!(
            trim_line_edges(&items, limits()).unwrap(),
            vec![EdgeDisposition::Keep; 5]
        );
    }
}
#[test]
fn empty_and_only_collapsible_lines_do_not_invent_visible_items() {
    assert!(trim_line_edges(&[], limits()).unwrap().is_empty());
    assert_eq!(
        trim_line_edges(&[control(), control()], limits()).unwrap(),
        vec![EdgeDisposition::Keep; 2]
    );
    assert_eq!(
        trim_line_edges(
            &[
                scalar(' ', WhiteSpace::Normal),
                control(),
                scalar(' ', WhiteSpace::Nowrap)
            ],
            limits()
        )
        .unwrap(),
        [
            EdgeDisposition::RemovedLeading,
            EdgeDisposition::Keep,
            EdgeDisposition::RemovedLeading
        ]
    );
}
#[test]
fn admission_limits_fail_without_partial_results() {
    let items = [
        scalar(' ', WhiteSpace::Normal),
        scalar('x', WhiteSpace::Normal),
    ];
    let mut cap = limits();
    cap.items = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        trim_line_edges(&items, cap),
        Err(LineEdgeError::Items)
    ));
    cap = limits();
    cap.retained_bytes = NonZeroUsize::new(1).unwrap();
    assert!(matches!(
        trim_line_edges(&items, cap),
        Err(LineEdgeError::Bytes)
    ));
    cap = limits();
    cap.work = NonZeroUsize::new(5).unwrap();
    assert!(matches!(
        trim_line_edges(&items, cap),
        Err(LineEdgeError::Work)
    ));
    cap.work = NonZeroUsize::new(6).unwrap();
    assert!(trim_line_edges(&items, cap).is_ok());
}
