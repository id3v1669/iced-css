use iced_css_core::{
    formatting::display::*,
    syntax::*,
    values::{NumericLimits, color::ColorLimits, property::*},
};
use std::num::NonZeroUsize;
fn parsed(css: &str) -> ParsedProperty {
    let n = NonZeroUsize::new(100000).unwrap();
    let DeclarationItem::Declaration(d) = Declarations::new(
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
    let PropertyResult::Parsed(p) = parse_property(
        &d,
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
    p
}
#[test]
fn css_display_table_and_ordered_triggers_cover_all_keyword_combinations() {
    use Display::*;
    let table = [
        ("inline", Inline, Block),
        ("block", Block, Block),
        ("list-item", ListItem, ListItem),
        ("inline-block", InlineBlock, Block),
        ("table", Table, Table),
        ("inline-table", InlineTable, Table),
        ("table-row-group", TableRowGroup, Block),
        ("table-header-group", TableHeaderGroup, Block),
        ("table-footer-group", TableFooterGroup, Block),
        ("table-row", TableRow, Block),
        ("table-column-group", TableColumnGroup, Block),
        ("table-column", TableColumn, Block),
        ("table-cell", TableCell, Block),
        ("table-caption", TableCaption, Block),
        ("none", None, None),
    ];
    let mut cases = 0;
    for (keyword, display, converted) in table {
        assert_eq!(
            Display::try_from(&parsed(&format!("display:{keyword}"))).unwrap(),
            display
        );
        for position in [
            Position::Static,
            Position::Relative,
            Position::Absolute,
            Position::Fixed,
        ] {
            for float in [Float::None, Float::Left, Float::Right] {
                for root in [false, true] {
                    let result = resolve_display(display, position, float, root);
                    let absolute = matches!(position, Position::Absolute | Position::Fixed);
                    let triggered = absolute || float != Float::None || root;
                    assert_eq!(result.display, if triggered { converted } else { display });
                    assert_eq!(result.position, position);
                    assert_eq!(
                        result.float,
                        if display != None && absolute {
                            Float::None
                        } else {
                            float
                        }
                    );
                    let flow = if display == None {
                        Flow::Suppressed
                    } else if absolute {
                        Flow::AbsolutelyPositioned
                    } else if float != Float::None {
                        Flow::Floating
                    } else {
                        Flow::Normal
                    };
                    assert_eq!(result.flow, flow);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 360);
}
#[test]
fn computed_input_conversion_checks_property_ownership_and_resolution() {
    for (text, expected) in [
        ("static", Position::Static),
        ("relative", Position::Relative),
        ("absolute", Position::Absolute),
        ("fixed", Position::Fixed),
    ] {
        assert_eq!(
            Position::try_from(&parsed(&format!("position:{text}"))).unwrap(),
            expected
        );
    }
    for (text, expected) in [
        ("none", Float::None),
        ("left", Float::Left),
        ("right", Float::Right),
    ] {
        assert_eq!(
            Float::try_from(&parsed(&format!("float:{text}"))).unwrap(),
            expected
        );
    }
    assert!(Display::try_from(&parsed("display:inherit")).is_err());
    assert!(Display::try_from(&parsed("float:none")).is_err());
    assert!(Float::try_from(&parsed("float:inherit")).is_err());
    assert!(Position::try_from(&parsed("background-attachment:fixed")).is_err());
}
