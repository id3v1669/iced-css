use iced_css_core::{
    syntax::StringLimits,
    values::{
        LengthTarget, NumericLimits,
        color::{Color, ColorLimits},
        initial::*,
        property::SpecifiedValue,
        registry::PropertyId,
    },
};
use std::num::NonZeroUsize;
fn limits() -> ColorLimits {
    ColorLimits {
        syntax: StringLimits {
            work: NonZeroUsize::new(100000).unwrap(),
            retained_bytes: NonZeroUsize::new(4096).unwrap(),
        },
        numeric: NumericLimits {
            work: NonZeroUsize::new(100000).unwrap(),
            digits: NonZeroUsize::new(100).unwrap(),
        },
    }
}
fn initial(name: &str) -> SpecifiedValue {
    initial_value(PropertyId::lookup(name).unwrap(), limits()).unwrap()
}
#[test]
fn css_initial_values_are_typed_without_host_overrides() {
    let SpecifiedValue::Keyword(display) = initial("display") else {
        panic!()
    };
    assert_eq!(display.as_str(), "inline");
    let SpecifiedValue::Keyword(weight) = initial("font-weight") else {
        panic!()
    };
    assert_eq!(weight.as_str(), "normal");
    assert_eq!(
        initial("margin-top"),
        SpecifiedValue::Length(LengthTarget::parse("0", limits().numeric).unwrap())
    );
    assert_eq!(
        initial("background-color"),
        SpecifiedValue::Color(Color::Transparent)
    );
    assert_eq!(
        initial("outline-color"),
        SpecifiedValue::Color(Color::Invert)
    );
}
#[test]
fn shorthand_has_no_independent_initial_value() {
    let margin = PropertyId::lookup("margin").unwrap();
    assert_eq!(
        initial_value(margin, limits()),
        Err(InitialError::Shorthand(margin))
    );
}

#[test]
fn context_dependent_defaults_remain_explicit_references() {
    for (name, reference) in [
        ("color", InitialReference::UserAgentColor),
        ("font-family", InitialReference::UserAgentFontFamily),
        ("quotes", InitialReference::UserAgentQuotes),
        ("text-align", InitialReference::DirectionalTextAlign),
        ("border-left-color", InitialReference::CurrentColor),
    ] {
        assert_eq!(initial(name), SpecifiedValue::InitialReference(reference));
    }
}
