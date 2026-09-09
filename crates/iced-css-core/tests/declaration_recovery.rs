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
fn nested_semicolons_are_recovery_content_not_declaration_boundaries() {
    for input in [
        "width:f(a;b);color:green",
        "width:([a;b]);color:green",
        "color{;color:maroon};color:green",
    ] {
        let value = recover_prefix(input, RecoveryMode::Declaration, limits()).unwrap();
        assert_eq!(value.boundary, Boundary::Semicolon);
        assert_eq!(&input[value.consumed..], "color:green");
        assert!(value.recovery_only);
    }
}
#[test]
fn string_uri_and_escaped_semicolons_are_opaque_tokens() {
    for input in [
        "background-image:url(a;b);color:green",
        "content:\"a;b\";color:green",
        r"content:a\;b;color:green",
        r"background-image:url(a\\);color:green",
    ] {
        let value = recover_prefix(input, RecoveryMode::Declaration, limits()).unwrap();
        assert_eq!(&input[value.consumed..], "color:green");
        assert!(!value.recovery_only);
        assert!(!value.invalid_tokens);
    }
}
#[test]
fn bad_string_and_escape_invalidity_survive_recovery() {
    for input in [
        "content:'bad\n;color:green",
        r"content:'\0';color:green",
        r"background-image:url(a b);color:green",
    ] {
        let value = recover_prefix(input, RecoveryMode::Declaration, limits()).unwrap();
        assert!(value.invalid_tokens);
        assert_eq!(&input[value.consumed..], "color:green");
    }
}
#[test]
fn unknown_at_rule_stops_after_its_block_or_semicolon() {
    for input in ["@foo {bar:baz}color:green", "@foo f(a;b);color:green"] {
        let value = recover_prefix(input, RecoveryMode::AtRule, limits()).unwrap();
        assert_eq!(&input[value.consumed..], "color:green");
    }
    let value = recover_prefix("@foo }rest", RecoveryMode::AtRule, limits()).unwrap();
    assert_eq!(value.boundary, Boundary::RuleClose);
    assert_eq!(value.consumed, 5);
}
#[test]
fn logical_eof_retains_unclosed_nesting_and_existing_invalidity() {
    let value = recover_prefix("width:f([a", RecoveryMode::Declaration, limits()).unwrap();
    assert_eq!(value.unclosed_delimiters, 2);
    assert!(value.implicit_close);
    assert_eq!(value.boundary, Boundary::EndOfInput);
    let bad = recover_prefix("content:'a\n", RecoveryMode::Declaration, limits()).unwrap();
    assert!(bad.invalid_tokens);
    let closed = recover_prefix("color:green}rest", RecoveryMode::Declaration, limits()).unwrap();
    assert_eq!(closed.boundary, Boundary::RuleClose);
    assert_eq!(closed.raw, "color:green");
}
#[test]
fn resource_failure_does_not_publish_truncated_recovery() {
    let mut budget = limits();
    budget.nesting = NonZeroUsize::MIN;
    assert!(recover_prefix("f((x));color:red", RecoveryMode::Declaration, budget).is_err());
    budget = limits();
    budget.work = NonZeroUsize::MIN;
    assert!(matches!(
        recover_prefix("color:red", RecoveryMode::Declaration, budget),
        Err(SyntaxError {
            fault: SyntaxFault::Resource(SyntaxResource::Work),
            ..
        })
    ));
}

#[test]
fn bad_uri_prefix_keeps_its_open_parenthesis_during_contextual_recovery() {
    for input in [
        "background-image:url(a b;c);color:green",
        "background-image:url(a b{;x});color:green",
        "background-image:url(\"a\n;b);color:green",
    ] {
        let value = recover_prefix(input, RecoveryMode::Declaration, limits()).unwrap();
        assert!(value.invalid_tokens);
        assert_eq!(&input[value.consumed..], "color:green", "{input:?}");
    }
    let input = "@foo url(a b;c) {x:y}color:green";
    let value = recover_prefix(input, RecoveryMode::AtRule, limits()).unwrap();
    assert_eq!(&input[value.consumed..], "color:green");
}

#[test]
fn block_boundary_metadata_excludes_nested_function_blocks() {
    let input = "a:f({x}) { color:green }after";
    let recovered = recover_prefix(input, RecoveryMode::AtRule, limits()).unwrap();
    assert_eq!(recovered.block_start, Some(9));
    assert_eq!(&input[recovered.consumed..], "after");
}
