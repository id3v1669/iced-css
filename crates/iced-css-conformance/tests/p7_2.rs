use serde_json::json;
#[test]
fn contract_p7_2() {
    iced_css_conformance::assert_case("P7.2");
}
#[test]
fn explicit_request_mutation_changes_actual_owner_presentation() {
    let runner = iced_css_conformance::registry::lookup("p7_2").unwrap();
    for rebuilds in [0, 1, 3, 1000] {
        for (explicit_requests, expected) in [(0, 0), (1, 1)] {
            let actual = runner(&json!({"scenario":"rebuild_without_request","rebuilds":rebuilds,"explicit_requests":explicit_requests})).unwrap();
            assert_eq!(actual, json!({"new_animations":expected}));
        }
    }
}
#[test]
fn harness_rejects_inputs_outside_its_single_request_contract() {
    let runner = iced_css_conformance::registry::lookup("p7_2").unwrap();
    let input = json!({"scenario":"rebuild_without_request","rebuilds":3,"explicit_requests":0});
    for (key, value) in [
        ("scenario", json!("unknown")),
        ("rebuilds", json!(-1)),
        ("rebuilds", json!(1001)),
        ("explicit_requests", json!(2)),
        ("explicit_requests", json!(-1)),
        ("extra", json!(true)),
    ] {
        let mut changed = input.clone();
        changed[key] = value;
        assert!(runner(&changed).is_err());
    }
}

#[test]
fn capture_restart_seed_is_registered() {
    iced_css_conformance::assert_case("P7.2-CAPTURE");
}
#[test]
fn capture_harness_observes_real_acceptance_delay_and_completion() {
    let runner = iced_css_conformance::registry::lookup("p7_2").unwrap();
    let input = json!({"scenario":"capture_and_restart","outgoing_duration_ms":1000,"accept_after_ms":500,"incoming_duration_ms":1000,"incoming_delay_ms":100,"sample_after_ms":600});
    assert_eq!(
        runner(&input).unwrap(),
        json!({"outgoing_progress":0.5,"incoming_at_acceptance":0.0,"incoming_at_sample":0.5})
    );
    let mut changed = input.clone();
    changed["incoming_delay_ms"] = json!(0);
    assert_eq!(
        runner(&changed).unwrap()["incoming_at_sample"],
        json!(0.6_f32)
    );
    let mut changed = input.clone();
    changed["accept_after_ms"] = json!(1000);
    assert_eq!(runner(&changed).unwrap()["outgoing_progress"], json!(null));
    let mut changed = input.clone();
    changed["sample_after_ms"] = json!(1100);
    assert_eq!(runner(&changed).unwrap()["incoming_at_sample"], json!(null));
    let mut changed = input.clone();
    changed["incoming_duration_ms"] = json!(0);
    changed["sample_after_ms"] = json!(99);
    assert_eq!(runner(&changed).unwrap()["incoming_at_sample"], json!(0.0));
    changed["sample_after_ms"] = json!(100);
    assert_eq!(runner(&changed).unwrap()["incoming_at_sample"], json!(null));
    for key in [
        "outgoing_duration_ms",
        "accept_after_ms",
        "incoming_duration_ms",
        "incoming_delay_ms",
        "sample_after_ms",
    ] {
        for bad in [json!(-1), json!(60001), json!(1.5)] {
            let mut changed = input.clone();
            changed[key] = bad;
            assert!(runner(&changed).is_err());
        }
    }
    let mut changed = input;
    changed["extra"] = json!(true);
    assert!(runner(&changed).is_err());
}
