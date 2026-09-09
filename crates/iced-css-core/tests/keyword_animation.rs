use iced_css_core::{
    animation::{combine::Combination, keyword::*},
    syntax::*,
    values::{NumericLimits, color::ColorLimits, keyword::*, property::*},
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

// Generated from retained definitions.json, independently of runtime keyword tables.
const CASES: &[(&str, &[&str])] = &[
    ("background-attachment", &["scroll", "fixed"]),
    (
        "background-repeat",
        &["repeat", "repeat-x", "repeat-y", "no-repeat"],
    ),
    ("border-collapse", &["collapse", "separate"]),
    ("caption-side", &["top", "bottom"]),
    ("clear", &["none", "left", "right", "both"]),
    ("direction", &["ltr", "rtl"]),
    ("empty-cells", &["show", "hide"]),
    ("font-variant", &["normal", "small-caps"]),
    ("list-style-position", &["inside", "outside"]),
    (
        "list-style-type",
        &[
            "disc",
            "circle",
            "square",
            "decimal",
            "decimal-leading-zero",
            "lower-roman",
            "upper-roman",
            "lower-greek",
            "lower-latin",
            "upper-latin",
            "armenian",
            "georgian",
            "lower-alpha",
            "upper-alpha",
            "none",
        ],
    ),
    ("overflow", &["visible", "hidden", "scroll", "auto"]),
    ("position", &["static", "relative", "absolute", "fixed"]),
    ("table-layout", &["auto", "fixed"]),
    (
        "text-transform",
        &["capitalize", "uppercase", "lowercase", "none"],
    ),
    ("unicode-bidi", &["normal", "embed", "bidi-override"]),
    (
        "white-space",
        &["normal", "pre", "nowrap", "pre-wrap", "pre-line"],
    ),
];

fn computed(css: &str) -> ComputedKeyword {
    ComputedKeyword::try_from(&parsed(css)).unwrap()
}
#[test]
fn every_admitted_keyword_pair_obeys_discrete_combination() {
    let mut pairs = 0;
    for &(name, keywords) in CASES {
        for &a in keywords {
            for &b in keywords {
                let start = computed(&format!("{name}:{a}"));
                let end = computed(&format!("{name}:{b}"));
                assert_eq!(start.property().descriptor().name, name);
                assert_eq!(start.keyword().as_str(), a);
                for (op, expected) in [
                    (Combination::Interpolate(-f32::MAX), start),
                    (Combination::Interpolate(0.0), start),
                    (
                        Combination::Interpolate(f32::from_bits(0.5_f32.to_bits() - 1)),
                        start,
                    ),
                    (Combination::Interpolate(0.5), end),
                    (Combination::Interpolate(1.0), end),
                    (Combination::Interpolate(f32::MAX), end),
                    (Combination::Add, start),
                    (Combination::Accumulate, start),
                ] {
                    assert_eq!(
                        combine_keyword(start, end, op).unwrap(),
                        expected,
                        "{name}: {a}/{b}, {op:?}"
                    );
                }
                for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                    assert_eq!(
                        combine_keyword(start, end, Combination::Interpolate(p)),
                        Err(KeywordAnimationError::NonfiniteProgress)
                    );
                }
                pairs += 1;
            }
        }
    }
    assert_eq!(pairs, 371);
}
#[test]
fn computed_keywords_reject_unresolved_special_and_forged_inputs() {
    for css in [
        "overflow:inherit",
        "display:block",
        "float:left",
        "font-style:normal",
        "visibility:visible",
        "text-align:left",
        "z-index:auto",
        "page-break-after:auto",
    ] {
        assert!(ComputedKeyword::try_from(&parsed(css)).is_err(), "{css}");
    }
    let mut forged = parsed("overflow:visible");
    forged.value = parsed("direction:rtl").value;
    assert_eq!(
        ComputedKeyword::try_from(&forged),
        Err(ComputedKeywordError::InvalidKeyword)
    );
    for value in [
        SpecifiedValue::Initial,
        SpecifiedValue::Inherit,
        parsed("width:1px").value,
    ] {
        forged.value = value;
        assert_eq!(
            ComputedKeyword::try_from(&forged),
            Err(ComputedKeywordError::Unresolved)
        );
    }
}
#[test]
fn pair_identity_is_preserved_even_for_shared_keyword_spellings() {
    let start = computed("overflow:scroll");
    let end = computed("background-attachment:scroll");
    for op in [
        Combination::Add,
        Combination::Accumulate,
        Combination::Interpolate(0.5),
    ] {
        assert_eq!(
            combine_keyword(start, end, op),
            Err(KeywordAnimationError::PropertyMismatch)
        );
    }
}
