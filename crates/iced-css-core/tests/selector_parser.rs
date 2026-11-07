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
fn grouping_is_atomic_and_newer_selector_forms_are_not_admitted() {
    assert_eq!(parse_selector_list("text, .x", limits()).unwrap().len(), 2);
    for input in [
        "",
        "text,",
        ",text",
        "text, :unknown",
        "text::hover",
        "text::placeholder",
        "text: :before",
        "text:::before",
        "text::lang(en)",
        ":not(.x)",
        "text~.x",
        "[a^=x]",
        "[a=x i]",
        ".x:before:hover",
        ".x:before > text",
        "[x=12]",
        "div/**/text",
        ":lang()",
        ":lang(en,fr)",
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
