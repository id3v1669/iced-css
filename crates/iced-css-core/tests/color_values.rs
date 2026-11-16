use iced_css_core::syntax::StringLimits;
use iced_css_core::values::{Decimal, Integer, NumericLimits, color::*};
use std::num::NonZeroUsize;
fn limits() -> ColorLimits {
    ColorLimits {
        syntax: StringLimits {
            retained_bytes: NonZeroUsize::new(4096).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
        numeric: NumericLimits {
            digits: NonZeroUsize::new(100).unwrap(),
            work: NonZeroUsize::new(100000).unwrap(),
        },
    }
}
#[test]
fn css22_named_and_hex_colors_use_retained_srgb_values() {
    for (input, expected) in [
        ("green", [0, 128, 0]),
        ("orange", [255, 165, 0]),
        ("#fb0", [255, 187, 0]),
        ("#ffBB00", [255, 187, 0]),
        (r"\52 ED", [255, 0, 0]),
        (r"#\66 00", [255, 0, 0]),
        (" /*x*/ navy /**/", [0, 0, 128]),
    ] {
        assert_eq!(
            parse_color(input, limits()).unwrap(),
            Color::Srgb(expected),
            "{input:?}"
        );
    }
}
#[test]
fn system_colors_remain_symbolic_and_case_insensitive() {
    for input in ["WindowText", "windowtext", r"\57 indowText"] {
        let Color::System(actual) = parse_color(input, limits()).unwrap() else {
            panic!()
        };
        assert_eq!(actual.name(), "WindowText");
    }
}
#[test]
fn rgb_forms_retain_exact_signed_operands_before_device_gamut_mapping() {
    let expected = ["300", "-10", "0"].map(|v| Integer::parse(v, limits().numeric).unwrap());
    assert_eq!(
        parse_color("rgb(300,-10,0)", limits()).unwrap(),
        Color::IntegerRgb(expected)
    );
    let expected = ["110", "0.125", "-0.5"].map(|v| Decimal::parse(v, limits().numeric).unwrap());
    assert_eq!(
        parse_color(r"\72 gb(110%, /**/ .125%, -.5%)", limits()).unwrap(),
        Color::PercentageRgb(expected)
    );
}
#[test]
fn invalid_css_and_color_syntax_past_level3_are_rejected() {
    for input in [
        "grey",
        "rebeccapurple",
        "inherit",
        "#ff",
        "#fffff",
        "#fffffff",
        "#fffffffff",
        "rgb(1,2%,3)",
        "rgb(1.0,2,3)",
        "rgb(1e2,0,0)",
        "rgb(1e2%,0%,0%)",
        "rgb(1 2 3)",
        "rgb(1,2,3,4,5)",
        "hsl(0deg,0%,0%)",
        "color(srgb 1 0 0)",
        "lab(50% 0 0)",
        "rgb(1,2)",
        "red blue",
        "rgb/**/(1,2,3)",
        r"\0",
        "rgb(1,2,3)!important",
    ] {
        assert_eq!(
            parse_color(input, limits()),
            Err(ColorError::Invalid),
            "{input:?}"
        );
    }
}
#[test]
fn eof_closes_only_complete_rgb_operands_and_resources_are_explicit() {
    assert!(parse_color("rgb(1,2,3", limits()).is_ok());
    assert_eq!(parse_color("rgb(1,2,", limits()), Err(ColorError::Invalid));
    let mut budget = limits();
    budget.syntax.work = NonZeroUsize::MIN;
    assert!(matches!(
        parse_color("red", budget),
        Err(ColorError::Syntax(_))
    ));
    budget = limits();
    budget.numeric.digits = NonZeroUsize::MIN;
    assert!(matches!(
        parse_color("rgb(123,0,0)", budget),
        Err(ColorError::Numeric(_))
    ));
}
