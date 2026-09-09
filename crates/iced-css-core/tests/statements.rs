use iced_css_core::syntax::{RecoveryLimits, StatementContext, StatementKind, Statements};
use std::num::NonZeroUsize;

fn limits() -> RecoveryLimits {
    RecoveryLimits {
        work: NonZeroUsize::new(100_000).unwrap(),
        nesting: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
    }
}

#[test]
fn top_level_statements_preserve_at_rule_and_qualified_boundaries() {
    let input = "<!-- /**/ @import 'a.css' screen; @media screen {.a{color:red}} .b{width:1px} -->";
    let values = Statements::new(input, StatementContext::Stylesheet, limits())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(values.len(), 3);
    assert_eq!(values[0].kind, StatementKind::AtRule { keyword: "@import" });
    assert_eq!(values[0].prelude, " 'a.css' screen");
    assert_eq!(values[0].body, None);
    assert_eq!(values[1].prelude, " screen ");
    assert_eq!(values[1].body, Some(".a{color:red}"));
    assert_eq!(values[2].kind, StatementKind::QualifiedRule);
    assert_eq!(values[2].prelude, ".b");
    for value in values {
        assert_eq!(&input[value.span.clone()], value.raw);
        assert_eq!(&input[value.prelude_span.clone()], value.prelude);
        assert_eq!(value.body_span.map(|span| &input[span]), value.body);
    }
}

#[test]
fn unknown_at_rules_and_bad_selectors_cannot_expose_nested_rules() {
    let input =
        "@unknown f(';'){.hidden{color:red}} .bad; .also_hidden{color:red} .visible{color:blue}";
    let values = Statements::new(input, StatementContext::Stylesheet, limits())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(values.len(), 3);
    assert_eq!(values[0].body, Some(".hidden{color:red}"));
    assert_eq!(values[1].prelude, ".bad; .also_hidden");
    assert_eq!(values[2].prelude, ".visible");
}

#[test]
fn block_context_does_not_skip_sgml_tokens_or_consume_containing_brace() {
    let input = "<!-- .a{} } .after{}";
    let mut statements = Statements::new(input, StatementContext::Block, limits());
    assert_eq!(statements.next().unwrap().unwrap().prelude, "<!-- .a");
    assert!(statements.next().is_none());
    assert_eq!(statements.remainder(), "} .after{}");
    assert!(statements.next().is_none());
    let mut top = Statements::new("}} .a{} .b{}", StatementContext::Stylesheet, limits());
    assert_eq!(top.next().unwrap().unwrap().prelude, "}} .a");
    assert_eq!(top.next().unwrap().unwrap().prelude, ".b");
}

#[test]
fn eof_records_closure_without_promoting_incomplete_rules() {
    let mut statements = Statements::new(
        "@media screen {.a{content:'x",
        StatementContext::Stylesheet,
        limits(),
    );
    let value = statements.next().unwrap().unwrap();
    assert!(value.implicit_close);
    assert_eq!(value.body, Some(".a{content:'x"));
    assert!(statements.next().is_none());
    let value = Statements::new(".incomplete", StatementContext::Stylesheet, limits())
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(value.body, None);
    assert_eq!(value.prelude, ".incomplete");
}

#[test]
fn iterator_has_cumulative_work_and_fuses_after_resource_failure() {
    let mut bounded = limits();
    bounded.work = NonZeroUsize::new(100).unwrap();
    let input = ".a{} ".repeat(100);
    let mut values = Statements::new(&input, StatementContext::Stylesheet, bounded);
    let mut successful = 0;
    while values.next().expect("must exhaust work").is_ok() {
        successful += 1;
    }
    assert!(successful > 0 && successful < 100);
    assert!(values.next().is_none());
    assert!(values.next().is_none());
}

#[test]
fn short_malformed_inputs_preserve_progress_and_valid_borrowed_spans() {
    let alphabet = ["@x", "a", "{", "}", "(", ")", ";", "'", "\\", " ", "é"];
    for mut index in 0..alphabet.len().pow(4) {
        let mut input = String::new();
        for _ in 0..4 {
            input.push_str(alphabet[index % alphabet.len()]);
            index /= alphabet.len();
        }
        for context in [StatementContext::Stylesheet, StatementContext::Block] {
            let mut statements = Statements::new(&input, context, limits());
            let mut end = 0;
            let mut count = 0;
            for value in statements.by_ref() {
                let value = value.unwrap();
                assert!(
                    value.span.end > value.span.start && value.span.start >= end,
                    "{input:?}"
                );
                end = value.span.end;
                assert_eq!(&input[value.span], value.raw);
                assert_eq!(&input[value.prelude_span], value.prelude);
                assert_eq!(value.body_span.map(|span| &input[span]), value.body);
                count += 1;
                assert!(count <= input.len());
            }
            assert!(statements.next().is_none());
        }
    }
}
