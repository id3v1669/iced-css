#[test]
fn absolute_identity_contract() {
    iced_css_conformance::assert_case("P1.2");
}

#[test]
fn registered_runner_rejects_unknown_scenarios_and_incomplete_inputs() {
    let run = iced_css_conformance::registry::lookup("p1_2").unwrap();
    let input = serde_json::json!({"scenario":"absolute_identity","left":"1in","right":"96px","relative":"100%","limits":{"digits":4096,"work":1000000}});
    assert_eq!(
        run(&input).unwrap(),
        serde_json::json!({"absolute_equal":true,"relative_is_absolute":false})
    );
    let mut bad = input.clone();
    bad["scenario"] = "unknown".into();
    assert!(run(&bad).is_err());
    let mut bad = input.clone();
    bad.as_object_mut().unwrap().remove("limits");
    assert!(run(&bad).is_err());
    let mut bad = input.clone();
    bad["limits"]["digits"] = 0.into();
    assert!(run(&bad).is_err());
    let mut bad = input;
    bad["left"] = "auto".into();
    assert!(run(&bad).is_err());
}
