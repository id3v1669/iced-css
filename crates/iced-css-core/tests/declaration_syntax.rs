use iced_css_core::syntax::*;
use std::num::NonZeroUsize;
fn limits() -> DeclarationLimits {
    DeclarationLimits {
        recovery: RecoveryLimits {
            work: NonZeroUsize::new(100000).unwrap(),
            nesting: NonZeroUsize::new(100).unwrap(),
            retained_bytes: NonZeroUsize::new(4096).unwrap(),
        },
        name_bytes: NonZeroUsize::new(4096).unwrap(),
    }
}
#[test]
fn malformed_declarations_and_unknown_at_rules_preserve_valid_following_syntax() {
    let input = "@foo {bar:baz} color:red; color; color:; color:green}";
    let mut items = Declarations::new(input, limits());
    assert!(matches!(
        items.next().unwrap().unwrap(),
        DeclarationItem::AtRule { .. }
    ));
    assert!(matches!(
        items.next().unwrap().unwrap(),
        DeclarationItem::Declaration(Declaration { value: "red", .. })
    ));
    assert!(matches!(
        items.next().unwrap().unwrap(),
        DeclarationItem::Malformed { .. }
    ));
    assert!(matches!(
        items.next().unwrap().unwrap(),
        DeclarationItem::Malformed { .. }
    ));
    assert!(matches!(
        items.next().unwrap().unwrap(),
        DeclarationItem::Declaration(Declaration { value: "green", .. })
    ));
    assert!(items.next().is_none());
    assert_eq!(items.remainder(), "}");
}
#[test]
fn priority_is_decoded_at_the_end_and_not_inside_values() {
    for (input, value, important) in [
        (r"color:red !\69 mportant;", "red", true),
        ("color:red ! /**/ IMPORTANT;", "red", true),
        ("content:'!important';", "'!important'", false),
        ("width:f(!important);", "f(!important)", false),
        ("width:f(!important", "f(!important", false),
        ("color:red !important extra;", "red !important extra", false),
    ] {
        let DeclarationItem::Declaration(actual) =
            Declarations::new(input, limits()).next().unwrap().unwrap()
        else {
            panic!("{input:?}");
        };
        assert_eq!(
            (actual.value, actual.important),
            (value, important),
            "{input:?}"
        );
    }
}
#[test]
fn decoded_names_and_original_value_spans_are_preserved() {
    let input = r"  \63 olor : /* x */ green /**/ !important;";
    let DeclarationItem::Declaration(actual) =
        Declarations::new(input, limits()).next().unwrap().unwrap()
    else {
        panic!()
    };
    assert_eq!(actual.name.text, "color");
    assert_eq!(actual.value, "green");
    assert_eq!(&input[actual.value_span], "green");
    assert_eq!(actual.span.start, 2);
    assert!(actual.important);
}
#[test]
fn recovery_content_is_never_implicitly_admitted_as_property_grammar() {
    let input = "width:f(a;b);color:green";
    let items: Vec<_> = Declarations::new(input, limits())
        .map(Result::unwrap)
        .collect();
    let DeclarationItem::Declaration(first) = &items[0] else {
        panic!()
    };
    assert!(first.recovery_only);
    assert_eq!(first.value, "f(a;b)");
    let DeclarationItem::Declaration(second) = &items[1] else {
        panic!()
    };
    assert_eq!(second.value, "green");
    assert!(matches!(
        Declarations::new("content:'bad\n;color:green", limits())
            .next()
            .unwrap()
            .unwrap(),
        DeclarationItem::Malformed { .. }
    ));
}
#[test]
fn resource_error_terminates_iteration_without_partial_success() {
    let mut budget = limits();
    budget.recovery.work = NonZeroUsize::MIN;
    let mut items = Declarations::new("color:red;color:green", budget);
    assert!(items.next().unwrap().is_err());
    assert!(items.next().is_none());
}
