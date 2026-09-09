use serde_json::json;
#[test]
fn contract_p3_3() {
    iced_css_conformance::assert_case("P3.3");
}
#[test]
fn keyed_state_focus_and_source_recreation_come_from_product_moves() {
    let runner = iced_css_conformance::registry::lookup("p3_3").unwrap();
    let input =
        json!({"scenario":"keyed_reorder","before":["a","b"],"after":["b","a"],"focused":"a"});
    assert_eq!(
        runner(&input).unwrap(),
        json!({"focused":"a","recreated_sources":0})
    );
    let mut changed = input.clone();
    changed["focused"] = json!("b");
    assert_eq!(
        runner(&changed).unwrap(),
        json!({"focused":"b","recreated_sources":0})
    );
    let mut changed = input.clone();
    changed["after"] = json!(["c", "b", "a"]);
    assert_eq!(
        runner(&changed).unwrap(),
        json!({"focused":"a","recreated_sources":1})
    );
    let mut changed = input.clone();
    changed["after"] = json!(["b"]);
    assert_eq!(
        runner(&changed).unwrap(),
        json!({"focused":null,"recreated_sources":0})
    );
    let mut changed = input.clone();
    changed["reset"] = json!(["a"]);
    assert_eq!(
        runner(&changed).unwrap(),
        json!({"focused":null,"recreated_sources":1})
    );
    let mut changed = input;
    changed["reset"] = json!(["b"]);
    assert_eq!(
        runner(&changed).unwrap(),
        json!({"focused":"a","recreated_sources":1})
    );
}
#[test]
fn malformed_or_unbounded_leaf_state_scenarios_are_rejected() {
    let runner = iced_css_conformance::registry::lookup("p3_3").unwrap();
    let input =
        json!({"scenario":"keyed_reorder","before":["a","b"],"after":["b","a"],"focused":"a"});
    for (key, value) in [
        ("before", json!(["a", "a"])),
        ("after", json!(["b", "b"])),
        ("focused", json!("missing")),
        ("reset", json!(["missing"])),
        ("reset", json!(["a", "a"])),
        ("extra", json!(true)),
        ("scenario", json!("unknown")),
        ("before", json!(["x".repeat(4097)])),
        (
            "after",
            json!((0..129).map(|i| i.to_string()).collect::<Vec<_>>()),
        ),
    ] {
        let mut changed = input.clone();
        changed[key] = value;
        assert!(runner(&changed).is_err());
    }
}
