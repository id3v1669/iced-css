#[test]
fn contract_p2_2() {
    iced_css_conformance::assert_case("P2.2");
}
#[test]
fn changing_importance_changes_the_real_winner() {
    let runner = iced_css_conformance::registry::lookup("p2_2").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p2_2.json")).unwrap();
    let mut input = fixture["input"].clone();
    input["user"] = "color: blue".into();
    assert_eq!(
        runner(&input).unwrap(),
        serde_json::json!({"winner_origin":"author","color":"red"})
    );
    input["author"] = "color: orange".into();
    assert_eq!(
        runner(&input).unwrap(),
        serde_json::json!({"winner_origin":"author","color":"orange"})
    );
    input["author"] = "color: invalid".into();
    assert!(runner(&input).is_err());
    input["author"] = "height:1px".into();
    assert!(runner(&input).is_err());
}

#[test]
fn origin_fixture_cannot_compare_different_color_properties() {
    let runner = iced_css_conformance::registry::lookup("p2_2").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p2_2.json")).unwrap();
    let mut input = fixture["input"].clone();
    input["author"] = "background-color:red !important".into();
    assert!(runner(&input).is_err());
}
