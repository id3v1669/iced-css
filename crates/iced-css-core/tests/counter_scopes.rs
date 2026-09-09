use iced_css_core::{
    formatting::counters::*,
    values::{CounterStyle, Integer, NumericLimits, property::CounterDirective},
};
use std::num::NonZeroUsize;
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn limits() -> CounterTraversalLimits {
    CounterTraversalLimits {
        participants: nz(100),
        depth: nz(20),
        scopes: nz(100),
        results: nz(100),
        retained_bytes: nz(100000),
        numeric: NumericLimits {
            digits: nz(100),
            work: nz(100000),
        },
    }
}
fn directive(name: &str, value: &str) -> CounterDirective {
    CounterDirective {
        name: name.into(),
        value: Integer::parse(value, limits().numeric).unwrap(),
    }
}
fn read<'a>(name: &'a str, separator: Option<&'a str>) -> CounterRead<'a> {
    CounterRead {
        name,
        separator,
        style: CounterStyle::Decimal,
    }
}
fn node<'a>(
    parent: Option<usize>,
    resets: &'a [CounterDirective],
    increments: &'a [CounterDirective],
    reads: &'a [CounterRead<'a>],
) -> CounterParticipant<'a> {
    CounterParticipant {
        parent,
        participates: true,
        resets,
        increments,
        reads,
    }
}
#[test]
fn nested_instances_continue_through_siblings_but_do_not_escape_their_parent() {
    let root = [directive("n", "1")];
    let nested = [directive("n", "10")];
    let one = [directive("n", "1")];
    let queries = [read("n", Some(".")), read("n", None)];
    let nodes = [
        node(None, &root, &[], &queries),
        node(Some(0), &nested, &one, &queries),
        node(Some(1), &[], &one, &queries),
        node(Some(0), &[], &one, &queries),
        node(Some(0), &[], &[], &queries),
    ];
    let texts = evaluate_counters(&nodes, limits()).unwrap();
    let text: Vec<_> = texts.iter().map(|v| v.text.as_str()).collect();
    assert_eq!(
        text,
        [
            "1", "1", "1.11", "11", "1.12", "12", "1.13", "13", "1.13", "13"
        ]
    );
    assert_eq!(texts[6].participant, 3);
    assert_eq!(texts[6].read, 0);
    // A nested scope under the first child ends before a sibling of that child.
    let nodes = [
        node(None, &root, &[], &[]),
        node(Some(0), &[], &[], &[]),
        node(Some(1), &nested, &[], &queries),
        node(Some(0), &[], &one, &queries),
    ];
    let texts = evaluate_counters(&nodes, limits()).unwrap();
    assert_eq!(texts[0].text, "1.10");
    assert_eq!(texts[2].text, "2");
}
#[test]
fn repeated_resets_replace_at_same_depth_and_precede_all_increments() {
    let reset = [directive("n", "2"), directive("n", "0")];
    let increments = [directive("n", "1"), directive("n", "2")];
    let next = [directive("n", "-4")];
    let queries = [read("n", Some("."))];
    let nodes = [
        node(None, &[], &[], &[]),
        node(Some(0), &reset, &increments, &queries),
        node(Some(0), &next, &[], &queries),
    ];
    let texts = evaluate_counters(&nodes, limits()).unwrap();
    assert_eq!(texts[0].text, "3");
    assert_eq!(texts[1].text, "-4");
}
#[test]
fn missing_reads_instantiate_zero_and_names_are_case_sensitive() {
    let queries = [read("n", Some(".")), read("N", None)];
    let plus = [directive("n", "999999999999999999999999999999")];
    let nodes = [
        node(None, &[], &[], &queries),
        node(Some(0), &[], &plus, &queries),
    ];
    let texts = evaluate_counters(&nodes, limits()).unwrap();
    assert_eq!(
        texts.iter().map(|v| v.text.as_str()).collect::<Vec<_>>(),
        ["0", "0", "999999999999999999999999999999", "0"]
    );
    let nodes = [node(None, &[], &plus, &queries)];
    assert_eq!(
        evaluate_counters(&nodes, limits()).unwrap()[0].text,
        "999999999999999999999999999999"
    );
}
#[test]
fn suppressed_subtrees_do_not_reset_increment_or_read_but_hidden_participants_do() {
    let reset = [directive("n", "0")];
    let plus = [directive("n", "1")];
    let queries = [read("n", None)];
    let mut nodes = [
        node(None, &reset, &[], &[]),
        node(Some(0), &[], &plus, &queries),
        node(Some(1), &[], &plus, &queries),
        node(Some(0), &[], &plus, &queries),
    ];
    nodes[1].participates = false;
    let texts = evaluate_counters(&nodes, limits()).unwrap();
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0].participant, 3);
    assert_eq!(texts[0].text, "1");
}
#[test]
fn malformed_traversal_and_each_budget_reject_the_batch_without_mutating_inputs() {
    let queries = [read("n", None)];
    let directives = [directive("n", "99"), directive("n", "1")];
    let nodes = [
        node(None, &[], &directives, &queries),
        node(Some(0), &[], &[], &queries),
    ];
    let original = directives.clone();
    for mut bounded in [limits(); 6].into_iter().enumerate() {
        match bounded.0 {
            0 => bounded.1.participants = nz(1),
            1 => bounded.1.depth = nz(1),
            2 => bounded.1.results = nz(1),
            3 => bounded.1.retained_bytes = nz(1),
            4 => bounded.1.numeric.work = nz(1),
            _ => bounded.1.numeric.digits = nz(2),
        }
        assert!(evaluate_counters(&nodes, bounded.1).is_err());
        assert_eq!(directives, original);
    }
    let queries = [read("n", None), read("other", None)];
    let nodes = [node(None, &[], &[], &queries)];
    let mut bounded = limits();
    bounded.scopes = nz(1);
    assert!(evaluate_counters(&nodes, bounded).is_err());
    for nodes in [
        vec![node(Some(0), &[], &[], &[])],
        vec![node(None, &[], &[], &[]), node(None, &[], &[], &[])],
        vec![
            node(None, &[], &[], &[]),
            node(Some(0), &[], &[], &[]),
            node(Some(0), &[], &[], &[]),
            node(Some(1), &[], &[], &[]),
        ],
    ] {
        assert_eq!(
            evaluate_counters(&nodes, limits()),
            Err(CounterTraversalError::Order)
        );
    }
}
