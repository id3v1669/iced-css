#[test]
fn contract_p1_3() {
    iced_css_conformance::assert_case("P1.3");
}

#[test]
fn adapter_requires_real_range_events_and_explicit_valid_inputs() {
    let runner = iced_css_conformance::registry::lookup("p1_3").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p1_3.json")).unwrap();
    let mut input = fixture["input"].clone();
    input["coefficient"] = "0".into();
    assert!(runner(&input).is_err());
    input = fixture["input"].clone();
    input["events"][0] = "unrecognized".into();
    assert!(runner(&input).is_err());
    input = fixture["input"].clone();
    input["limits"]["reports"] = 0.into();
    assert!(runner(&input).is_err());
    input = fixture["input"].clone();
    input["limits"]["reports"] = 1.into();
    assert_eq!(
        runner(&input).unwrap(),
        serde_json::json!({"admitted_reports":1})
    );
}
