use iced_css_core::{
    cascade::{winner_table::*, *},
    syntax::*,
    values::{
        NumericLimits, border_width::BorderFontContext, color::ColorLimits,
        computed_border_width::*, property::*, registry::PropertyId,
    },
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

fn id(name: &str) -> PropertyId {
    PropertyId::lookup(name).unwrap()
}
fn limits() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1000000).unwrap(),
    }
}
fn compute(
    side: &str,
    width: Option<&str>,
    style: &str,
    font: f32,
    parent: Option<ComputedBorderWidth>,
) -> ComputedBorderWidth {
    let values: Vec<_> = width
        .map(|v| parsed(&format!("border-{side}-width:{v}")))
        .into_iter()
        .collect();
    compute_cascaded_border_width(
        &table(&values),
        id(&format!("border-{side}-width")),
        &parsed(&format!("border-{side}-style:{style}")),
        BorderFontContext {
            size: font,
            x_height: font / 2.0,
        },
        parent,
        limits(),
    )
    .unwrap()
}
#[test]
fn inherited_width_is_parent_computed_and_obeys_child_suppression() {
    for side in ["top", "right", "bottom", "left"] {
        let parent = compute(side, Some("2em"), "solid", 10.0, None);
        assert_eq!(parent.sample().value, 20.0);
        let child = compute(side, Some("inherit"), "solid", 100.0, Some(parent));
        assert_eq!(child.sample().value, 20.0);
        assert_eq!(child.property(), parent.property());
        for style in ["none", "hidden"] {
            assert_eq!(
                compute(side, Some("inherit"), style, 100.0, Some(parent))
                    .sample()
                    .value,
                0.0
            );
            let zero = compute(side, Some("2em"), style, 10.0, None);
            assert_eq!(
                compute(side, Some("inherit"), "solid", 100.0, Some(zero))
                    .sample()
                    .value,
                0.0
            );
        }
        assert_eq!(
            compute(side, None, "solid", 100.0, Some(parent))
                .sample()
                .value,
            3.0
        );
        assert_eq!(
            compute(side, Some("inherit"), "solid", 100.0, None)
                .sample()
                .value,
            3.0
        );
        assert_eq!(
            compute(side, Some("inherit"), "none", 100.0, None)
                .sample()
                .value,
            0.0
        );
    }
}
#[test]
fn target_comparison_is_numeric_not_epsilon_or_source_identity() {
    let first = compute("top", Some("1px"), "solid", 10.0, None);
    let same = compute("top", Some("0.1em"), "solid", 10.0, None);
    assert!(!first.resolved_target_changed(same).unwrap());
    let adjacent = compute(
        "top",
        Some("1.00000011920928955078125px"),
        "solid",
        10.0,
        None,
    );
    assert!(first.resolved_target_changed(adjacent).unwrap());
    let zero = compute("top", Some("0px"), "solid", 10.0, None);
    assert!(
        !zero
            .resolved_target_changed(compute("top", Some("-0px"), "solid", 10.0, None))
            .unwrap()
    );
    assert!(
        !zero
            .resolved_target_changed(compute("top", Some("20px"), "hidden", 10.0, None))
            .unwrap()
    );
    assert!(
        first
            .resolved_target_changed(compute("left", Some("1px"), "solid", 10.0, None))
            .is_err()
    );
}
#[test]
fn inherited_conversion_does_not_repeat_parent_event_or_consume_child_font() {
    let huge = format!("{}px", "9".repeat(200));
    let parent = compute("top", Some(&huge), "solid", 10.0, None);
    assert_eq!(parent.sample().value, f32::MAX);
    assert!(parent.sample().event.is_some());
    let child = compute("top", Some("inherit"), "solid", f32::NAN, Some(parent));
    assert_eq!(child.sample().value, f32::MAX);
    assert_eq!(child.sample().event, None);
}
#[test]
fn wrong_property_unresolved_style_and_conversion_failures_are_errors() {
    let vals = [parsed("border-top-width:inherit")];
    let winners = table(&vals);
    let font = BorderFontContext {
        size: 10.0,
        x_height: 5.0,
    };
    let style = parsed("border-top-style:solid");
    let wrong = compute("left", Some("2px"), "solid", 10.0, None);
    assert!(
        compute_cascaded_border_width(
            &winners,
            id("border-top-width"),
            &style,
            font,
            Some(wrong),
            limits()
        )
        .is_err()
    );
    for bad in [
        parsed("border-left-style:solid"),
        parsed("border-top-style:inherit"),
        parsed("outline-style:solid"),
    ] {
        assert!(
            compute_cascaded_border_width(
                &winners,
                id("border-top-width"),
                &bad,
                font,
                None,
                limits()
            )
            .is_err()
        );
    }
    assert!(
        compute_cascaded_border_width(&winners, id("outline-width"), &style, font, None, limits())
            .is_err()
    );
    let vals = [parsed("border-top-width:12345px")];
    let tiny = NumericLimits {
        digits: NonZeroUsize::new(1).unwrap(),
        work: NonZeroUsize::new(1).unwrap(),
    };
    assert!(
        compute_cascaded_border_width(
            &table(&vals),
            id("border-top-width"),
            &style,
            font,
            None,
            tiny
        )
        .is_err()
    );
}
