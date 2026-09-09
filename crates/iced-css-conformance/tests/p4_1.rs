use serde_json::json;
fn input() -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(include_str!("../fixtures/p4_1.json")).unwrap()["input"].clone()
}
#[test]
fn contract_p4_1() {
    iced_css_conformance::assert_case("P4.1");
}
#[test]
fn changed_revision_and_completion_outcome_use_real_publication_admission() {
    let run = iced_css_conformance::registry::lookup("p4_1").unwrap();
    let mut changed = input();
    changed["active_revision"] = 2.into();
    assert_eq!(
        run(&changed).unwrap(),
        json!({"active_revision":2,"stale_status":true})
    );
    changed["completions"][0]["result"] = "success".into();
    assert_eq!(
        run(&changed).unwrap(),
        json!({"active_revision":3,"stale_status":false})
    );
    changed["completions"] = json!([{"request":"A","result":"error"},{"request":"B","result":"success"},{"request":"B","result":"error"}]);
    assert_eq!(
        run(&changed).unwrap(),
        json!({"active_revision":3,"stale_status":false})
    );
}
#[test]
fn invalid_control_inputs_are_rejected() {
    let run = iced_css_conformance::registry::lookup("p4_1").unwrap();
    for (field, value) in [
        ("unexpected", json!(true)),
        ("scenario", json!("http")),
        ("requests", json!(["A", "A"])),
        ("work", json!(1)),
        ("active_revision", json!(u64::MAX)),
        ("request_limit", json!(1)),
        ("completions", json!([{"request":"C","result":"success"}])),
    ] {
        let mut changed = input();
        changed[field] = value;
        assert!(run(&changed).is_err(), "{field}");
    }
}
