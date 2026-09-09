use iced_css_core::{
    animation::{combine::Combination, counters::*},
    formatting::counters::*,
    syntax::*,
    values::{CounterStyle, Integer, NumericLimits, color::ColorLimits, property::*},
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

fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> CounterCombinationLimits {
    CounterCombinationLimits {
        entries: nz(100),
        retained_bytes: nz(100000),
        comparison_work: nz(100000),
        numeric: numeric(),
    }
}
fn directives(css: &str) -> Vec<CounterDirective> {
    let SpecifiedValue::Counters(v) = parsed(css).value else {
        panic!()
    };
    v
}
fn traversal() -> CounterTraversalLimits {
    CounterTraversalLimits {
        participants: nz(10),
        depth: nz(10),
        scopes: nz(10),
        results: nz(10),
        retained_bytes: nz(100000),
        numeric: numeric(),
    }
}
#[test]
fn compatible_signed_lists_feed_the_existing_counter_traversal() {
    let a = directives("counter-increment:n -2 n 4");
    let b = directives("counter-increment:n -1 n 6");
    let sample =
        combine_counter_directives(&a, &b, Combination::Interpolate(0.5), limits()).unwrap();
    assert_eq!(
        sample.directives(),
        directives("counter-increment:n -1 n 5")
    );
    assert!(sample.retained_bytes() > 0);
    let reads = [CounterRead {
        name: "n",
        separator: None,
        style: CounterStyle::Decimal,
    }];
    let nodes = [CounterParticipant {
        parent: None,
        participates: true,
        resets: &[],
        increments: sample.directives(),
        reads: &reads,
    }];
    assert_eq!(evaluate_counters(&nodes, traversal()).unwrap()[0].text, "4");
    for op in [Combination::Add, Combination::Accumulate] {
        assert_eq!(
            combine_counter_directives(&a, &b, op, limits())
                .unwrap()
                .directives(),
            directives("counter-increment:n -3 n 10")
        );
    }
    let wide = directives(&format!("counter-reset:n {}", "9".repeat(120)));
    let one = directives("counter-reset:n 1");
    assert_eq!(
        combine_counter_directives(&wide, &one, Combination::Add, limits())
            .unwrap()
            .directives(),
        directives(&format!("counter-reset:n 1{}", "0".repeat(120)))
    );
}
#[test]
fn mismatched_lists_select_whole_borrowed_endpoints_before_integer_work() {
    let a = directives("counter-increment:n 7 m 8");
    for b in [
        directives("counter-increment:n 9"),
        directives("counter-increment:N 9 m 10"),
        directives("counter-increment:n 9 z 10"),
        vec![],
    ] {
        for (op, expected) in [
            (Combination::Interpolate(0.25), a.as_slice()),
            (Combination::Interpolate(0.5), b.as_slice()),
            (Combination::Add, a.as_slice()),
            (Combination::Accumulate, a.as_slice()),
        ] {
            let mut l = limits();
            l.numeric.work = NonZeroUsize::MIN;
            l.retained_bytes = NonZeroUsize::MIN;
            let sample = combine_counter_directives(&a, &b, op, l).unwrap();
            assert_eq!(sample.directives(), expected);
            assert_eq!(sample.directives().as_ptr(), expected.as_ptr());
            assert_eq!(sample.retained_bytes(), 0);
        }
    }
    let sample = combine_counter_directives(&[], &[], Combination::Add, limits()).unwrap();
    assert!(sample.directives().is_empty());
    // Escaped spellings compare after parsing/decoding, without reordering names.
    let joined = directives(r"counter-increment:\6e 1");
    assert_eq!(joined, directives("counter-increment:n1 1"));
    let escaped = directives(r"counter-increment:\6e  1");
    assert_eq!(escaped, directives("counter-increment:n 1"));
    let plain = directives("counter-increment:n 3");
    assert_eq!(
        combine_counter_directives(&escaped, &plain, Combination::Interpolate(0.5), limits())
            .unwrap()
            .directives(),
        directives("counter-increment:n 2")
    );
}
#[test]
fn budgets_and_nonfinite_progress_fail_without_mutating_inputs() {
    let a = directives("counter-reset:longname 100 second 2");
    let before = a.clone();
    let mut l = limits();
    l.entries = NonZeroUsize::MIN;
    assert_eq!(
        combine_counter_directives(&a, &a, Combination::Add, l).unwrap_err(),
        CounterCombinationError::Entries
    );
    let mut l = limits();
    l.retained_bytes = NonZeroUsize::MIN;
    assert_eq!(
        combine_counter_directives(&a, &a, Combination::Add, l).unwrap_err(),
        CounterCombinationError::Bytes
    );
    let mut l = limits();
    l.comparison_work = NonZeroUsize::MIN;
    assert_eq!(
        combine_counter_directives(&a, &a, Combination::Add, l).unwrap_err(),
        CounterCombinationError::Work
    );
    let mut l = limits();
    l.numeric.work = NonZeroUsize::MIN;
    assert!(matches!(
        combine_counter_directives(&a, &a, Combination::Add, l),
        Err(CounterCombinationError::Integer(_))
    ));
    for p in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            combine_counter_directives(&[], &[], Combination::Interpolate(p), limits())
                .unwrap_err(),
            CounterCombinationError::NonfiniteProgress
        );
    }
    assert_eq!(a, before);
    let long = vec![CounterDirective {
        name: "x".repeat(200),
        value: Integer::parse("1", numeric()).unwrap(),
    }];
    let mut l = limits();
    l.retained_bytes = nz(size_of::<CounterDirective>() + 50);
    assert_eq!(
        combine_counter_directives(&long, &long, Combination::Add, l).unwrap_err(),
        CounterCombinationError::Bytes
    );
}
