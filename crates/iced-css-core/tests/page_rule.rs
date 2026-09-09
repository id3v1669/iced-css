use iced_css_core::{
    selectors::SelectorLimits,
    stylesheet::*,
    syntax::{RecoveryLimits, StringLimits},
    values::{NumericLimits, color::ColorLimits},
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
fn page_context_keeps_only_applicable_valid_margins_with_locations_and_order() {
    let css = "  @PAGE :LEFT {color:red; margin:1cm 2%; margin-top:0em; margin:1px 2ex; margin-left:auto!important; nonsense:1; font-weight:+100; @x{y:z}; margin-bottom:inherit} x{}";
    let PageAssembly::Accepted(rule) = parse_page_rule_prefix(css, context(), limits()).unwrap()
    else {
        panic!()
    };
    assert_eq!(rule.selector, PageSelector::Left);
    assert_eq!(&css[rule.consumed..], " x{}");
    assert_eq!(
        rule.declarations
            .iter()
            .map(|d| (d.parsed.property.descriptor().name, d.order))
            .collect::<Vec<_>>(),
        [("margin", 11), ("margin-left", 14), ("margin-bottom", 18)]
    );
    assert!(rule.declarations[1].parsed.important);
    assert_eq!(&css[rule.locations[1].value.clone()], "auto");
    assert_eq!(
        rule.rejected.iter().map(|d| d.reason).collect::<Vec<_>>(),
        [
            RejectionReason::InapplicableProperty,
            RejectionReason::InvalidValue,
            RejectionReason::InvalidValue,
            RejectionReason::UnknownProperty,
            RejectionReason::InapplicableProperty,
            RejectionReason::AtRule
        ]
    );
    assert_eq!(rule.next_context.unwrap().first_order, 19);
}
#[test]
fn page_selector_is_a_complete_css22_pseudo_page_not_an_element_selector() {
    for (prelude, expected) in [
        ("", PageSelector::All),
        (":first", PageSelector::First),
        (":right", PageSelector::Right),
        (r":l\65 ft", PageSelector::Left),
        (":/**/LEFT", PageSelector::Left),
    ] {
        let css = format!("@p\\61 ge {prelude} {{margin:0}}");
        let PageAssembly::Accepted(rule) =
            parse_page_rule_prefix(&css, context(), limits()).unwrap()
        else {
            panic!("{css}")
        };
        assert_eq!(rule.selector, expected);
    }
    for prelude in [
        "foo",
        ":unknown",
        ":left:right",
        ":left,:right",
        ": left",
        ":",
    ] {
        let css = format!("@page {prelude} {{font-weight:+100}} x{{}}");
        let PageAssembly::Discarded { consumed, .. } =
            parse_page_rule_prefix(&css, context(), limits()).unwrap()
        else {
            panic!("{css}")
        };
        assert_eq!(&css[consumed..], " x{}");
    }
}
#[test]
fn page_eof_and_resource_errors_remain_explicit() {
    let PageAssembly::Accepted(rule) =
        parse_page_rule_prefix("@page{margin:1px", context(), limits()).unwrap()
    else {
        panic!()
    };
    assert!(rule.implicit_close);
    let mut bounded = limits();
    bounded.record_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        parse_page_rule_prefix("@page{margin:0}", context(), bounded),
        Err(RuleAssemblyError::Bytes)
    ));
    bounded = limits();
    bounded.values.syntax.work = NonZeroUsize::MIN;
    assert!(matches!(
        parse_page_rule_prefix("@page:left{margin:0}", context(), bounded),
        Err(RuleAssemblyError::Syntax(_))
    ));
    assert!(matches!(
        parse_page_rule_prefix("@media screen{}", context(), limits()),
        Err(RuleAssemblyError::Boundary(_))
    ));
    assert!(matches!(
        parse_page_rule_prefix("@page;", context(), limits()),
        Ok(PageAssembly::Discarded { .. })
    ));
}
