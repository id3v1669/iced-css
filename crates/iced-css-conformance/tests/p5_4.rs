use serde_json::json;
#[test]
fn contract_p5_4() {
    iced_css_conformance::assert_case("P5.4");
}
fn input() -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(include_str!("../fixtures/p5_4.json")).unwrap()["input"].clone()
}
#[test]
fn changed_inputs_reach_exact_product_counter_formatting() {
    let run = iced_css_conformance::registry::lookup("p5_4").unwrap();
    for (style, value, expected) in [
        ("upper-roman", "49", "XLIX"),
        ("lower-alpha", "27", "27"),
        (
            "decimal",
            "999999999999999999999999999",
            "999999999999999999999999999",
        ),
        ("none", "-1", ""),
    ] {
        let mut changed = input();
        changed["style"] = style.into();
        changed["value"] = value.into();
        assert_eq!(run(&changed).unwrap(), json!({"text":expected}));
    }
}
#[test]
fn unknown_syntax_and_exhausted_limits_are_errors() {
    let run = iced_css_conformance::registry::lookup("p5_4").unwrap();
    for (field, value) in [
        ("style", json!("custom-style")),
        ("value", json!("1.0")),
        ("scenario", json!("counter_scope")),
        ("unexpected", json!(true)),
        ("work", json!(1)),
        ("digits", json!(0)),
        ("retained_bytes", json!(1)),
    ] {
        let mut changed = input();
        changed[field] = value;
        assert!(run(&changed).is_err(), "{field}");
    }
}
