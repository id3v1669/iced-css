#[test]
fn contract_p2_1() {
    iced_css_conformance::assert_case("P2.1");
}
#[test]
fn adapter_uses_changed_css_and_reports_missing_grammars_as_errors() {
    let runner = iced_css_conformance::registry::lookup("p2_1").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p2_1.json")).unwrap();
    let mut input = fixture["input"].clone();
    input["css"] = ".a {width:1px;color:invalid}".into();
    assert_eq!(
        runner(&input).unwrap(),
        serde_json::json!({"accepted_properties":["width"],"discarded_properties":["color"]})
    );
    input["css"] = ".a {font-weight:+100}".into();
    assert!(runner(&input).is_err());
    input["css"] = ".a {width:1px} .b{}".into();
    assert!(runner(&input).is_err());
    input["limits"]["work"] = 0.into();
    assert!(runner(&input).is_err());
}

#[test]
fn trailing_trivia_has_one_cumulative_work_allowance() {
    let runner = iced_css_conformance::registry::lookup("p2_1").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p2_1.json")).unwrap();
    let mut input = fixture["input"].clone();
    input["css"] = format!(".a{{}}{}", " /**/".repeat(1000)).into();
    input["limits"]["work"] = 1000.into();
    assert!(runner(&input).is_err());
}

#[test]
fn invalid_selector_member_cannot_expose_its_declarations() {
    let runner = iced_css_conformance::registry::lookup("p2_1").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p2_1.json")).unwrap();
    let mut input = fixture["input"].clone();
    input["css"] = ".a, :unknown {color:green}".into();
    assert!(runner(&input).is_err());
}

#[test]
fn stylesheet_scenario_observes_media_import_placement_and_page_context() {
    let runner = iced_css_conformance::registry::lookup("p2_1").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p2_1.json")).unwrap();
    let mut input = fixture["input"].clone();
    input["scenario"] = "stylesheet".into();
    input["css"]="@unknown{} @import \"base.css\" screen; @media print{a{color:red}} @page{margin:0} @media screen{a{width:1px}} @import \"late.css\";".into();
    assert_eq!(
        runner(&input).unwrap(),
        serde_json::json!({"entry_kinds":["import","media","page","media"],"local_screen_properties":["width"],"imports":[{"location":"base.css","applies_to_screen":true}],"rejections":["unknown_at_rule","late_import"]})
    );
    input["css"] = "@media unknown{a{width:1px}} @media all{a{height:2px}}".into();
    assert_eq!(
        runner(&input).unwrap()["local_screen_properties"],
        serde_json::json!(["height"])
    );
}
#[test]
fn stylesheet_scenario_keeps_resource_and_required_grammar_failures_explicit() {
    let runner = iced_css_conformance::registry::lookup("p2_1").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p2_1.json")).unwrap();
    let mut input = fixture["input"].clone();
    input["scenario"] = "stylesheet".into();
    input["css"] = "a{color:red}".into();
    assert!(runner(&input).is_ok());
    input["css"] = "a{font-weight:+100}".into();
    assert!(runner(&input).is_err());
    input["css"] = "a{color:red}".into();
    input["limits"]["bytes"] = 1.into();
    assert!(runner(&input).is_err());
    input["limits"] = fixture["input"]["limits"].clone();
    input["unexpected"] = true.into();
    assert!(runner(&input).is_err());
}
