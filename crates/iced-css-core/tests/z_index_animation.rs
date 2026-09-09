use iced_css_core::{
    animation::{combine::Combination, z_index::*},
    syntax::*,
    values::{
        Integer, IntegerInterpolationError, NumericLimits, color::ColorLimits, property::*,
        z_index::*,
    },
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

fn computed(css: &str) -> ComputedZIndex {
    compute_z_index(parsed(css)).unwrap()
}
fn integer(value: &str) -> ComputedZIndex {
    ComputedZIndex::Integer(Integer::parse(value, numeric()).unwrap())
}
#[test]
fn computed_z_index_preserves_auto_and_exact_integer_values() {
    assert_eq!(computed("z-index:auto"), ComputedZIndex::Auto);
    assert_eq!(computed("z-index:0"), integer("0"));
    assert_ne!(computed("z-index:auto"), computed("z-index:0"));
    let wide = format!("-{}", "9".repeat(120));
    assert_eq!(computed(&format!("z-index:{wide}")), integer(&wide));
    for css in ["z-index:inherit", "widows:2"] {
        assert_eq!(
            compute_z_index(parsed(css)).unwrap_err(),
            ZIndexError::Unresolved
        );
    }
}
#[test]
fn z_index_numeric_pairs_round_exactly_without_a_machine_integer_limit() {
    let a = computed("z-index:-2");
    let b = computed("z-index:-1");
    assert_eq!(
        combine_z_index(&a, &b, Combination::Interpolate(0.5), numeric())
            .unwrap()
            .computed(),
        &integer("-1")
    );
    let a = computed(&format!("z-index:{}", "9".repeat(120)));
    let b = computed("z-index:1");
    for op in [Combination::Add, Combination::Accumulate] {
        let result = combine_z_index(&a, &b, op, numeric()).unwrap();
        assert_eq!(
            result.computed(),
            &integer(&format!("1{}", "0".repeat(120)))
        );
    }
}
#[test]
fn auto_pairs_use_discrete_rules_and_reject_nonfinite_progress() {
    let auto = computed("z-index:auto");
    let number = computed("z-index:7");
    for (op, expected) in [
        (Combination::Interpolate(0.25), &auto),
        (Combination::Interpolate(0.5), &number),
        (Combination::Add, &auto),
        (Combination::Accumulate, &auto),
    ] {
        let sample = combine_z_index(&auto, &number, op, numeric()).unwrap();
        assert_eq!(sample.computed(), expected);
        assert!(std::ptr::eq(sample.computed(), expected));
    }
    assert_eq!(
        combine_z_index(&number, &auto, Combination::Add, numeric())
            .unwrap()
            .computed(),
        &number
    );
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_z_index(&auto, &auto, Combination::Interpolate(p), numeric()).unwrap_err(),
            IntegerInterpolationError::NonfiniteProgress
        );
    }
    let mut tiny = numeric();
    tiny.work = NonZeroUsize::MIN;
    assert!(matches!(
        combine_z_index(&number, &number, Combination::Add, tiny),
        Err(IntegerInterpolationError::Numeric(_))
    ));
}
