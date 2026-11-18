use iced_css_core::{
    selectors::*,
    syntax::{SyntaxFault, SyntaxResource},
};
use std::num::NonZeroUsize;
fn limits() -> SelectorLimits {
    SelectorLimits {
        work: NonZeroUsize::new(100000).unwrap(),
        retained_bytes: NonZeroUsize::new(65536).unwrap(),
        entries: NonZeroUsize::new(1000).unwrap(),
    }
}
#[test]
fn css22_selector_chain_preserves_relations_values_and_specificity() {
    let values = parse_selector_list(
        r"row.main > text#Title[lang|=en]:hover + .x:before",
        limits(),
    )
    .unwrap();
    assert_eq!(values.len(), 1);
    let value = &values[0];
    assert_eq!(
        value.combinators,
        vec![Combinator::Child, Combinator::Adjacent]
    );
    assert_eq!(value.compounds.len(), 3);
    assert_eq!(value.pseudo, Some(PseudoElement::Before));
    assert_eq!(value.specificity.components(), [0, 1, 4, 3]);
    assert_eq!(
        value.compounds[0].tests,
        vec![
            SimpleSelector::Type("row".into()),
            SimpleSelector::Class("main".into())
        ]
    );
}
#[test]
fn grouping_is_atomic_and_forms_past_level3_are_not_admitted() {
    assert_eq!(parse_selector_list("text, .x", limits()).unwrap().len(), 2);
    for input in [
        "",
        "text,",
        ",text",
        "text, :unknown",
        "text::hover",
        "text::placeholder",
        "text: :before",
        "text:: before",
        "text:::before",
        "text::lang(en)",
        ":nth-child(2 of)",
        "text||.x",
        "[a=x i]",
        ".x:before:hover",
        ".x:before > text",
        "[x=12]",
        "div/**/text",
        ":lang()",
        ":lang(en,fr)",
        "#123",
        "#1st",
        "#-",
        "#-1",
        "#123, text",
    ] {
        assert!(parse_selector_list(input, limits()).is_err(), "{input:?}");
    }
}
#[test]
fn two_colons_name_the_css2_pseudo_elements() {
    for (two, one) in [
        ("text::before", "text:before"),
        ("text::AFTER", "text:after"),
        (".x::first-line", ".x:first-line"),
        ("::first-letter", ":first-letter"),
    ] {
        assert_eq!(
            parse_selector_list(two, limits()).unwrap(),
            parse_selector_list(one, limits()).unwrap(),
            "{two:?}"
        );
    }
}
#[test]
fn whitespace_and_comments_do_not_have_the_same_combinator_meaning() {
    let descendant = parse_selector_list(".a /**/ .b", limits()).unwrap();
    assert_eq!(descendant[0].combinators, vec![Combinator::Descendant]);
    let compound = parse_selector_list(".a/**/.b", limits()).unwrap();
    assert!(compound[0].combinators.is_empty());
    assert_eq!(compound[0].compounds[0].tests.len(), 2);
}
#[test]
fn escapes_strings_and_language_keep_semantic_identity() {
    let value = parse_selector_list(r"\74 ext.\31 23[a='x;y']:lang(EN-gb)", limits()).unwrap();
    assert!(
        value[0].compounds[0]
            .tests
            .contains(&SimpleSelector::Class("123".into()))
    );
    assert!(
        value[0].compounds[0]
            .tests
            .contains(&SimpleSelector::Attribute {
                name: "a".into(),
                operator: AttributeOperator::Equal,
                value: "x;y".into()
            })
    );
    assert!(
        value[0].compounds[0]
            .tests
            .contains(&SimpleSelector::Lang("EN-gb".into()))
    );
    assert!(
        parse_selector_list(r"#\31 23", limits()).unwrap()[0].compounds[0]
            .tests
            .contains(&SimpleSelector::Id("123".into()))
    );
    for input in [r".\0", r"[x='\d800']"] {
        assert!(parse_selector_list(input, limits()).is_err());
    }
}
#[test]
fn limits_fail_without_returning_a_partial_selector_group() {
    let mut budget = limits();
    budget.entries = NonZeroUsize::MIN;
    assert!(parse_selector_list(".a,.b", budget).is_err());
    budget = limits();
    budget.retained_bytes = NonZeroUsize::MIN;
    assert!(parse_selector_list(".a", budget).is_err());
    budget = limits();
    budget.work = NonZeroUsize::MIN;
    let error = parse_selector_list(".a", budget).unwrap_err();
    assert!(matches!(
        error.fault,
        SyntaxFault::Resource(SyntaxResource::Work)
    ));
}

#[test]
fn retained_specificity_examples_are_computed_from_actual_selectors() {
    for (input, expected) in [
        ("*", [0, 0, 0, 0]),
        ("li", [0, 0, 0, 1]),
        ("li:first-line", [0, 0, 0, 2]),
        ("ul li", [0, 0, 0, 2]),
        ("ul ol+li", [0, 0, 0, 3]),
        ("h1 + *[rel=up]", [0, 0, 1, 1]),
        ("ul ol li.red", [0, 0, 1, 3]),
        ("li.red.level", [0, 0, 2, 1]),
        ("#x34y", [0, 1, 0, 0]),
        ("[id=x34y]", [0, 0, 1, 0]),
    ] {
        assert_eq!(
            parse_selector_list(input, limits()).unwrap()[0]
                .specificity
                .components(),
            expected,
            "{input:?}"
        );
    }
}

fn retained_bytes(selectors: &Vec<Selector>) -> usize {
    selectors.capacity() * size_of::<Selector>()
        + selectors
            .iter()
            .map(|selector| {
                selector.compounds.capacity() * size_of::<Compound>()
                    + selector.combinators.capacity() * size_of::<Combinator>()
                    + selector
                        .compounds
                        .iter()
                        .map(|compound| {
                            compound.tests.capacity() * size_of::<SimpleSelector>()
                                + compound
                                    .tests
                                    .iter()
                                    .map(|test| match test {
                                        SimpleSelector::Type(value)
                                        | SimpleSelector::Id(value)
                                        | SimpleSelector::Class(value)
                                        | SimpleSelector::Lang(value) => value.capacity(),
                                        SimpleSelector::Attribute { name, value, .. } => {
                                            name.capacity() + value.capacity()
                                        }
                                        _ => 0,
                                    })
                                    .sum::<usize>()
                        })
                        .sum::<usize>()
            })
            .sum::<usize>()
}
#[test]
fn actual_retained_ast_capacities_obey_all_small_budgets() {
    for input in ["*", ".a,#b", "a[x='abc']:lang(en) > .b:before"] {
        for bytes in 1..1024 {
            let budget = SelectorLimits {
                retained_bytes: NonZeroUsize::new(bytes).unwrap(),
                ..limits()
            };
            match parse_selector_list(input, budget) {
                Ok(value) => assert!(retained_bytes(&value) <= bytes),
                Err(error) => assert!(matches!(
                    error.fault,
                    SyntaxFault::Resource(SyntaxResource::Bytes)
                )),
            }
        }
    }
    let budget = SelectorLimits {
        retained_bytes: NonZeroUsize::new(usize::MAX).unwrap(),
        ..limits()
    };
    assert!(matches!(
        parse_selector_list("*", budget).unwrap_err().fault,
        SyntaxFault::Resource(SyntaxResource::Bytes)
    ));
}

// Selectors Level 3.
fn one(input: &str) -> Selector {
    let mut list = parse_selector_list(input, limits()).unwrap();
    assert_eq!(list.len(), 1, "{input}");
    list.remove(0)
}
fn nth(input: &str) -> (i32, i32) {
    match one(&format!(":nth-child({input})")).compounds[0].tests[0] {
        SimpleSelector::Nth { a, b, .. } => (a, b),
        ref other => panic!("{input}: {other:?}"),
    }
}
#[test]
fn an_plus_b_takes_the_level3_forms() {
    for (input, expected) in [
        ("odd", (2, 1)),
        ("EVEN", (2, 0)),
        ("3", (0, 3)),
        ("-3", (0, -3)),
        ("+3", (0, 3)),
        ("n", (1, 0)),
        ("+n", (1, 0)),
        ("-n", (-1, 0)),
        ("2n", (2, 0)),
        ("2N+1", (2, 1)),
        ("2n-1", (2, -1)),
        ("2n + 1", (2, 1)),
        ("2n - 1", (2, -1)),
        (" 2n+1 ", (2, 1)),
        ("-n+3", (-1, 3)),
        ("-2n + 3", (-2, 3)),
        ("n-2", (1, -2)),
        ("0n+5", (0, 5)),
    ] {
        assert_eq!(nth(input), expected, "{input}");
    }
    for input in [
        "", "2 n", "n 1", "+ 2n", "2n +", "2n + -1", "n+", "foo", "2n+1.5", "1.5", "2n1", "n n",
        "--n", "2e1",
    ] {
        assert!(
            parse_selector_list(&format!(":nth-child({input})"), limits()).is_err(),
            "{input}"
        );
    }
}
#[test]
fn level3_pseudo_classes_and_their_specificity() {
    let value = one("text:nth-last-of-type(2n):not(.x):root:empty:only-of-type");
    assert_eq!(
        value.compounds[0].tests,
        vec![
            SimpleSelector::Type("text".into()),
            SimpleSelector::Nth {
                a: 2,
                b: 0,
                last: true,
                of_type: true
            },
            SimpleSelector::Not(Box::new(SimpleSelector::Class("x".into()))),
            SimpleSelector::Root,
            SimpleSelector::Empty,
            SimpleSelector::Only { of_type: true },
        ]
    );
    // One type and five class-level tests.
    assert_eq!(value.specificity.components(), [0, 0, 5, 1]);
    // A negation counts as its argument: an id, a type, nothing for `*`.
    assert_eq!(one(":not(#a)").specificity.components(), [0, 1, 0, 0]);
    assert_eq!(one(":not(text)").specificity.components(), [0, 0, 0, 1]);
    assert_eq!(one(":not(*)").specificity.components(), [0, 0, 0, 0]);
    assert_eq!(one(":not([a^=b])").specificity.components(), [0, 0, 1, 0]);
}
#[test]
fn negation_admits_no_pseudo_element() {
    // Selectors Level 4 lets combinators in.
    for input in [":not(a b)", ":not(a > b)"] {
        assert!(parse_selector_list(input, limits()).is_ok(), "{input}");
    }
    for input in [
        ":not()",
        ":not(:before)",
        ":not(::after)",
        ":not(a",
        "::not(a)",
        "::nth-child(1)",
    ] {
        assert!(parse_selector_list(input, limits()).is_err(), "{input}");
    }
}
#[test]
fn level3_combinator_and_attribute_operators() {
    let value = one("a ~ b[x^=y][x$='z'][x*=w]");
    assert_eq!(value.combinators, vec![Combinator::Sibling]);
    let operators: Vec<_> = value.compounds[1]
        .tests
        .iter()
        .filter_map(|test| match test {
            SimpleSelector::Attribute { operator, .. } => Some(*operator),
            _ => None,
        })
        .collect();
    assert_eq!(
        operators,
        [
            AttributeOperator::Prefix,
            AttributeOperator::Suffix,
            AttributeOperator::Substring
        ]
    );
    // `~=` is still the includes match, and the two delimiters admit nothing between.
    assert!(matches!(
        one("[x~=y]").compounds[0].tests[0],
        SimpleSelector::Attribute {
            operator: AttributeOperator::Includes,
            ..
        }
    ));
    for input in ["[x^ =y]", "[x^y]", "[x*]", "a ~", "~ a"] {
        assert!(parse_selector_list(input, limits()).is_err(), "{input}");
    }
}

#[test]
fn level4_is_where_and_not_take_lists_of_complex_selectors() {
    // `:is()` and `:not()` count as their most specific argument; `:where()` as nothing.
    assert_eq!(one(":is(#a, .b, c)").specificity.components(), [0, 1, 0, 0]);
    assert_eq!(
        one("p:where(#a, .b)").specificity.components(),
        [0, 0, 0, 1]
    );
    assert_eq!(one(":not(.a.b, c)").specificity.components(), [0, 0, 2, 0]);
    assert_eq!(one(":not(:not(a))").specificity.components(), [0, 0, 0, 1]);
    // A complex argument counts whole.
    assert_eq!(
        one(":is(#a > b c, .d)").specificity.components(),
        [0, 1, 0, 2]
    );
    for good in [
        ":is(a)",
        ":is(a b)",
        ":is(a > b ~ c, d + e)",
        ":where( a  >  b )",
        ":not(a b)",
        ":is(:has(a))",
        ":not(:has(a))",
        "div :is(.a, [b], :hover) > p",
        ":where( .a , .b )",
        ":matches(a)",
    ] {
        assert!(parse_selector_list(good, limits()).is_ok(), "{good}");
    }
    for bad in [":is(a", ":is(a, (b)", "::is(a)", ":not(a, b >)", ":not()"] {
        assert!(parse_selector_list(bad, limits()).is_err(), "{bad}");
    }
}

#[test]
fn level4_is_and_where_forgive_invalid_arguments() {
    let list = |input: &str| match &one(input).compounds[0].tests[0] {
        SimpleSelector::Matches { list, .. } => list.len(),
        other => panic!("{other:?}"),
    };
    // The invalid ones are dropped; the list may be left empty.
    assert_eq!(list(":is(a >, b, ::before, c d)"), 2);
    assert_eq!(list(":where(1x, :bogus(a, b), [c=])"), 0);
    assert_eq!(list(":is()"), 0);
    assert_eq!(list(":is(a,,b,)"), 2);
    // Brackets inside one are passed whole, a `,` in them included.
    assert_eq!(list(":is(:nth-child(2, 3) a, b)"), 1);
    // What is dropped weighs nothing.
    assert_eq!(one(":is(#a >, .b)").specificity.components(), [0, 0, 1, 0]);
}

#[test]
fn level4_nth_child_takes_an_of_list() {
    // A pseudo-class's weight and the most specific of the list.
    assert_eq!(
        one(":nth-child(2n+1 of #a, .b)").specificity.components(),
        [0, 1, 1, 0]
    );
    for good in [
        ":nth-child(odd of .a)",
        ":nth-last-child(-n+3 of a > b)",
        ":nth-child( 2n + 1  OF .a )",
        ":nth-child(2 of :is(.a, .b))",
    ] {
        assert!(parse_selector_list(good, limits()).is_ok(), "{good}");
    }
    for bad in [
        ":nth-child(of .a)",
        ":nth-child(2n of)",
        ":nth-child(2n of )",
        ":nth-of-type(2n of .a)",
        ":nth-last-of-type(1 of .a)",
        ":nth-child(1 of ::before)",
        ":nth-child(1 .a)",
    ] {
        assert!(parse_selector_list(bad, limits()).is_err(), "{bad}");
    }
}

#[test]
fn level4_has_takes_relative_complex_selectors() {
    assert_eq!(one("a:has(#b, .c)").specificity.components(), [0, 1, 0, 1]);
    for good in [
        ":has(a)",
        ":has(> a.b)",
        ":has(+ a, ~ .b)",
        "div :has( .x ) > p",
        ":has(a b)",
        ":has(> a + b ~ c d)",
        ":has(:is(a b))",
    ] {
        assert!(parse_selector_list(good, limits()).is_ok(), "{good}");
    }
    for bad in [
        ":has()",
        ":has(:has(a))",
        ":has(:is(b :has(a)))",
        ":has(a >)",
        ":has(::before)",
        ":has(>)",
    ] {
        assert!(parse_selector_list(bad, limits()).is_err(), "{bad}");
    }
}
