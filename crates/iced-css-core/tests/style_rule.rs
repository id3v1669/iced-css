use iced_css_core::{
    selectors::SelectorLimits,
    stylesheet::*,
    syntax::{RecoveryLimits, StringLimits, SyntaxFault},
    values::{NumericLimits, color::ColorLimits, property::SpecifiedValue},
};
use std::num::{NonZeroU64, NonZeroUsize};
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100000).unwrap()
}
fn limits() -> RuleAssemblyLimits {
    RuleAssemblyLimits {
        recovery: RecoveryLimits {
            work: n(),
            retained_bytes: n(),
            nesting: n(),
        },
        selectors: SelectorLimits {
            work: n(),
            retained_bytes: n(),
            entries: n(),
        },
        values: ColorLimits {
            syntax: StringLimits {
                work: n(),
                retained_bytes: n(),
            },
            numeric: NumericLimits {
                work: n(),
                digits: n(),
            },
        },
        entries: n(),
        record_bytes: n(),
        work: n(),
        name_bytes: n(),
    }
}
fn context() -> RuleContext {
    RuleContext {
        first_order: 10,
        first_declaration: NonZeroU64::new(20).unwrap(),
    }
}
#[test]
fn recovered_rule_keeps_declaration_order_priority_shorthands_and_original_locations() {
    let css = "  .a { width:f(a;b); unknown:1; color:red!important; margin:1px 2px; broken; @unknown x; height:3px } .b{}";
    let RuleAssembly::Accepted(rule) = parse_style_rule_prefix(css, context(), limits()).unwrap()
    else {
        panic!()
    };
    assert_eq!(&css[rule.consumed..], " .b{}");
    assert_eq!(rule.declarations.len(), 3);
    assert_eq!(rule.rejected.len(), 4);
    assert_eq!(
        rule.declarations
            .iter()
            .map(|d| d.parsed.property.descriptor().name)
            .collect::<Vec<_>>(),
        ["color", "margin", "height"]
    );
    assert_eq!(
        rule.declarations
            .iter()
            .map(|d| (d.order, d.declaration.get()))
            .collect::<Vec<_>>(),
        [(12, 22), (13, 23), (16, 26)]
    );
    assert!(rule.declarations[0].parsed.important);
    assert!(matches!(
        rule.declarations[1].parsed.value,
        SpecifiedValue::Expanded(_)
    ));
    assert_eq!(&css[rule.locations[0].value.clone()], "red");
    assert_eq!(&css[rule.locations[2].declaration.clone()], "height:3px ");
    assert_eq!(
        rule.rejected.iter().map(|d| d.reason).collect::<Vec<_>>(),
        [
            RejectionReason::InvalidValue,
            RejectionReason::UnknownProperty,
            RejectionReason::MalformedDeclaration,
            RejectionReason::AtRule
        ]
    );
    assert_eq!(rule.rejected[0].name.as_deref(), Some("width"));
    let next = rule.next_context.unwrap();
    assert_eq!(next.first_order, 17);
    assert_eq!(next.first_declaration.get(), 27);
    assert_eq!(
        rule.offer(iced_css_core::cascade::rule::RuleOrigin::User)
            .declarations
            .len(),
        3
    );
}
#[test]
fn invalid_selector_discards_the_whole_rule_before_unsupported_values_are_parsed() {
    let css = ".a, :unknown {font-weight:+100} .next{}";
    let RuleAssembly::Discarded {
        consumed,
        diagnostic,
    } = parse_style_rule_prefix(css, context(), limits()).unwrap()
    else {
        panic!()
    };
    assert_eq!(&css[consumed..], " .next{}");
    assert_eq!(diagnostic.fault, SyntaxFault::InvalidSelector);
}
#[test]
fn resource_and_unimplemented_grammar_failures_are_not_recovered_as_invalid_css() {
    assert!(matches!(
        parse_style_rule_prefix(".a{font-weight:+100}", context(), limits()),
        Err(RuleAssemblyError::Property { .. })
    ));
    let mut bounded = limits();
    bounded.selectors.work = NonZeroUsize::MIN;
    assert!(matches!(
        parse_style_rule_prefix(".a{color:red}", context(), bounded),
        Err(RuleAssemblyError::Syntax(_))
    ));
    bounded = limits();
    bounded.entries = NonZeroUsize::MIN;
    assert!(matches!(
        parse_style_rule_prefix(".a{color:red;width:1px}", context(), bounded),
        Err(RuleAssemblyError::Entries)
    ));
    bounded = limits();
    bounded.record_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        parse_style_rule_prefix(".a{color:red}", context(), bounded),
        Err(RuleAssemblyError::Bytes)
    ));
}
#[test]
fn end_of_input_and_final_identity_are_retained_without_wrapping() {
    let maximum = RuleContext {
        first_order: u64::MAX,
        first_declaration: NonZeroU64::new(u64::MAX).unwrap(),
    };
    let RuleAssembly::Accepted(rule) =
        parse_style_rule_prefix(".a{color:red", maximum, limits()).unwrap()
    else {
        panic!()
    };
    assert!(rule.implicit_close);
    assert!(rule.next_context.is_none());
    assert_eq!(rule.declarations[0].order, u64::MAX);
    assert!(matches!(
        parse_style_rule_prefix(".a{color:red;color:blue}", maximum, limits()),
        Err(RuleAssemblyError::Identity)
    ));
}
