use iced_css_core::{
    animation::{combine::Combination, cursor::*},
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

use iced_css_core::values::computed_cursor::*;
fn limits() -> CursorComputationLimits {
    CursorComputationLimits {
        uris: NonZeroUsize::new(32).unwrap(),
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
        work: NonZeroUsize::new(100000).unwrap(),
    }
}
#[test]
fn ordered_absolute_urls_and_fallbacks_select_as_whole_cursors() {
    let property = parsed("cursor:url(../a.cur#one),url(b.cur),pointer");
    let start = compute_cursor(&property, "https://example.test/css/theme.css", limits()).unwrap();
    let end = compute_cursor(
        &parsed("cursor:url(c.cur),help"),
        "https://example.test/css/theme.css",
        limits(),
    )
    .unwrap();
    let keyword = compute_cursor(&parsed("cursor:wait"), "unused", limits()).unwrap();
    assert_eq!(
        start.uris(),
        [
            "https://example.test/a.cur#one",
            "https://example.test/css/b.cur"
        ]
    );
    assert_eq!(start.fallback().as_str(), "pointer");
    assert!(keyword.uris().is_empty());
    assert_eq!(keyword.retained_bytes(), 0);
    assert!(start.retained_bytes() <= limits().retained_bytes.get());
    let changed = compute_cursor(
        &property,
        "https://example.test/other/css/theme.css",
        limits(),
    )
    .unwrap();
    assert_ne!(start.uris(), changed.uris());
    for (a, b) in [
        (&start, &end),
        (&start, &keyword),
        (&keyword, &end),
        (&start, &changed),
        (&keyword, &keyword),
    ] {
        for p in [
            -f32::MAX,
            0.0,
            f32::from_bits(0.5_f32.to_bits() - 1),
            0.5,
            1.0,
            f32::MAX,
        ] {
            assert!(std::ptr::eq(
                combine_cursor(a, b, Combination::Interpolate(p)).unwrap(),
                if p < 0.5 { a } else { b }
            ));
        }
        for op in [Combination::Add, Combination::Accumulate] {
            assert!(std::ptr::eq(combine_cursor(a, b, op).unwrap(), a));
        }
        for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(combine_cursor(a, b, Combination::Interpolate(p)).is_err());
        }
    }
}
#[test]
fn fallback_admission_reuses_css22_keyword_set() {
    for name in [
        "auto",
        "crosshair",
        "default",
        "pointer",
        "move",
        "e-resize",
        "ne-resize",
        "nw-resize",
        "n-resize",
        "se-resize",
        "sw-resize",
        "s-resize",
        "w-resize",
        "text",
        "wait",
        "help",
        "progress",
    ] {
        let cursor =
            compute_cursor(&parsed(&format!("cursor:{name}")), "unused", limits()).unwrap();
        assert_eq!(cursor.fallback().as_str(), name);
    }
    for css in ["cursor:inherit", "overflow:visible"] {
        assert!(compute_cursor(&parsed(css), "unused", limits()).is_err());
    }
    let mut forged = parsed("cursor:pointer");
    let SpecifiedValue::Keyword(invalid) = parsed("overflow:visible").value else {
        panic!()
    };
    let SpecifiedValue::Cursor(cursor) = &mut forged.value else {
        panic!()
    };
    cursor.fallback = invalid;
    assert!(compute_cursor(&forged, "unused", limits()).is_err());
}
#[test]
fn list_limits_are_cumulative_and_failed_computation_preserves_source() {
    let one = parsed("cursor:url(b),pointer");
    let two = parsed("cursor:url(b),url(c),pointer");
    let original = two.clone();
    let mut budget = limits();
    budget.work = NonZeroUsize::new(250).unwrap();
    assert!(compute_cursor(&one, "x:a", budget).is_ok());
    assert!(compute_cursor(&two, "x:a", budget).is_err());
    budget = limits();
    budget.uris = NonZeroUsize::MIN;
    assert!(compute_cursor(&two, "x:a", budget).is_err());
    budget = limits();
    budget.retained_bytes = NonZeroUsize::new(2 * size_of::<String>()).unwrap();
    assert!(compute_cursor(&two, "x:a", budget).is_err());
    assert_eq!(two, original);
    assert!(compute_cursor(&one, "relative", limits()).is_err());
    assert!(compute_cursor(&parsed("cursor:url(café.cur),pointer"), "x:a", limits()).is_err());
}
