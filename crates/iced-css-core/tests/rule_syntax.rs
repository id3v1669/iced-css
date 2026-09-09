use iced_css_core::syntax::*;
use std::num::NonZeroUsize;
fn limits() -> RecoveryLimits {
    RecoveryLimits {
        work: NonZeroUsize::new(100000).unwrap(),
        nesting: NonZeroUsize::new(100).unwrap(),
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
    }
}
#[test]
fn qualified_rule_retains_prelude_body_and_following_source() {
    let input = " /*x*/ .a { width:f(a;b);color:green; } .b{}";
    let rule = qualified_rule_prefix(input, limits()).unwrap();
    assert_eq!(rule.prelude, ".a");
    assert_eq!(rule.declarations, " width:f(a;b);color:green; ");
    assert_eq!(&input[rule.consumed..], " .b{}");
    assert_eq!(&input[rule.prelude_span], rule.prelude);
    assert_eq!(&input[rule.declaration_span], rule.declarations);
    assert!(!rule.implicit_close);
}
#[test]
fn quoted_and_nested_braces_do_not_split_the_rule_prelude() {
    for input in ["a[title='{'] {color:green}", "a:f({x}) {color:green}"] {
        let rule = qualified_rule_prefix(input, limits()).unwrap();
        assert_eq!(rule.declarations, "color:green");
    }
}
#[test]
fn missing_closing_brace_is_recorded_without_repairing_bad_declarations() {
    let rule = qualified_rule_prefix(".a {content:'bad\n;color:green", limits()).unwrap();
    assert!(rule.implicit_close);
    assert_eq!(rule.declarations, "content:'bad\n;color:green");
}
#[test]
fn non_rules_and_resource_failures_are_distinct() {
    for input in ["", " /**/", "{}", "@foo{}", ".a;", ".a"] {
        assert_eq!(
            qualified_rule_prefix(input, limits()),
            Err(RuleError::Invalid),
            "{input:?}"
        );
    }
    let budget = RecoveryLimits {
        work: NonZeroUsize::MIN,
        ..limits()
    };
    assert!(matches!(
        qualified_rule_prefix(".a{}", budget),
        Err(RuleError::Syntax(_))
    ));
}

#[test]
fn malformed_selector_tokens_stay_attached_to_the_following_block() {
    for prelude in [".bad; .good", "}} .good", ") ( {} ) .good"] {
        let input = format!("{prelude} {{color:red}} .after{{color:blue}}");
        let rule = qualified_rule_prefix(&input, limits()).unwrap();
        assert_eq!(rule.prelude, prelude);
        assert_eq!(rule.declarations, "color:red");
        assert_eq!(&input[rule.consumed..], " .after{color:blue}");
        let selector_limits = iced_css_core::selectors::SelectorLimits {
            work: NonZeroUsize::new(100000).unwrap(),
            retained_bytes: NonZeroUsize::new(4096).unwrap(),
            entries: NonZeroUsize::new(100).unwrap(),
        };
        assert!(
            iced_css_core::selectors::parse_selector_list(rule.prelude, selector_limits).is_err()
        );
    }
}
