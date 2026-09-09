use serde_json::json;
#[test]
fn contract_p5_1() {
    iced_css_conformance::assert_case("P5.1");
}
#[test]
fn changed_groups_drive_product_collapse_arithmetic() {
    let run = iced_css_conformance::registry::lookup("p5_1").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p5_1.json")).unwrap();
    for (values, expected) in [
        (json!([10, -20]), "c1200000"),
        (json!([-10, -20]), "c1a00000"),
        (json!([]), "00000000"),
    ] {
        let mut input = fixture["input"].clone();
        input["adjoining_margins"] = values;
        assert_eq!(
            run(&input).unwrap(),
            json!({"collapsed_margin_bits":expected})
        );
    }
}
#[test]
fn adjacency_barriers_and_invalid_control_inputs_are_explicitly_outside_this_adapter() {
    let run = iced_css_conformance::registry::lookup("p5_1").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p5_1.json")).unwrap();
    let input = fixture["input"].clone();
    assert!(run(&input).is_ok());
    for (field, value) in [
        ("barriers", json!(["clearance"])),
        ("scenario", json!("unknown")),
        ("unexpected", json!(true)),
    ] {
        let mut changed = input.clone();
        changed[field] = value;
        assert!(run(&changed).is_err());
    }
    let mut changed = input;
    changed["work"] = 1.into();
    assert!(run(&changed).is_err());
}
