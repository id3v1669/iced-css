use iced_css_core::{
    cascade::{winner_table::*, *},
    formatting::display::*,
    syntax::*,
    values::{NumericLimits, color::ColorLimits, display::compute_display_style, property::*},
};
use std::num::{NonZeroU64, NonZeroUsize};
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
fn table(values: &[ParsedProperty]) -> PropertyWinners<'_> {
    let mut table = PropertyWinners::new();
    for (index, value) in values.iter().enumerate() {
        table
            .offer(
                value,
                TableAdmission {
                    origin: Origin::Author,
                    specificity: Specificity::ZERO,
                    order: index as u64,
                    declaration: NonZeroU64::new(index as u64 + 1).unwrap(),
                },
            )
            .unwrap();
    }
    table
}
#[test]
fn css_initials_and_root_transformation_apply_without_native_hints() {
    let empty = PropertyWinners::new();
    let root = compute_display_style(&empty, None).unwrap();
    assert_eq!(
        (root.display, root.position, root.float, root.flow),
        (Display::Block, Position::Static, Float::None, Flow::Normal)
    );
    let child = compute_display_style(&empty, Some(&root)).unwrap();
    assert_eq!(child.display, Display::Inline);
    let declarations = [parsed("display:list-item")];
    assert_eq!(
        compute_display_style(&table(&declarations), None)
            .unwrap()
            .display,
        Display::ListItem
    );
}
#[test]
fn inheritance_uses_the_parents_computed_display_and_float_values() {
    let p = [
        parsed("display:inline-table"),
        parsed("float:left"),
        parsed("position:absolute"),
    ];
    let parent = compute_display_style(&table(&p), None).unwrap();
    assert_eq!(
        (parent.display, parent.float),
        (Display::Table, Float::None)
    );
    let c = [parsed("display:inherit"), parsed("float:inherit")];
    let child = compute_display_style(&table(&c), Some(&parent)).unwrap();
    assert_eq!(
        (child.display, child.position, child.float),
        (Display::Table, Position::Static, Float::None)
    );
    let c = [parsed("position:inherit")];
    let child = compute_display_style(&table(&c), Some(&parent)).unwrap();
    assert_eq!(
        (child.position, child.display, child.flow),
        (
            Position::Absolute,
            Display::Block,
            Flow::AbsolutelyPositioned
        )
    );
}
#[test]
fn final_cascade_winner_drives_computation_and_root_inherit_uses_initials() {
    let p = [
        parsed("display:none !important"),
        parsed("display:block"),
        parsed("position:fixed"),
        parsed("float:right"),
    ];
    let result = compute_display_style(&table(&p), None).unwrap();
    assert_eq!(
        (result.display, result.float, result.flow),
        (Display::None, Float::Right, Flow::Suppressed)
    );
    let p = [
        parsed("display:inherit"),
        parsed("position:inherit"),
        parsed("float:inherit"),
    ];
    let result = compute_display_style(&table(&p), None).unwrap();
    assert_eq!(
        (result.display, result.position, result.float),
        (Display::Block, Position::Static, Float::None)
    );
}
