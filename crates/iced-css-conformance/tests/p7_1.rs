use serde_json::json;
#[test]
fn contract_p7_1() {
    iced_css_conformance::assert_case("P7.1");
}
#[test]
fn selection_results_come_from_the_actual_property_registry() {
    let runner = iced_css_conformance::registry::lookup("p7_1").unwrap();
    for (property, eligible) in [
        ("display", false),
        ("DISPLAY", false),
        ("font-variant", true),
        ("color", true),
        ("margin", true),
    ] {
        let actual = runner(
            &json!({"scenario":"display_request","property":property,"start":"block","end":"none"}),
        )
        .unwrap();
        assert_eq!(
            actual,
            json!({"eligible":eligible,"fallback_discrete":false})
        );
    }
}
#[test]
fn adapter_rejects_unqualified_input_and_does_not_parse_ineligible_endpoints() {
    let runner = iced_css_conformance::registry::lookup("p7_1").unwrap();
    let input = json!({"scenario":"display_request","property":"display","start":"invalid CSS","end":"also invalid"});
    assert_eq!(
        runner(&input).unwrap(),
        json!({"eligible":false,"fallback_discrete":false})
    );
    for (key, value) in [
        ("scenario", json!("unknown")),
        ("property", json!("not-a-property")),
        ("property", json!("page-break-after")),
        ("extra", json!(true)),
        ("start", json!(42)),
    ] {
        let mut changed = input.clone();
        changed[key] = value;
        assert!(runner(&changed).is_err());
    }
}

#[test]
fn color_harness_calls_the_actual_computed_pair_pipeline() {
    let runner = iced_css_conformance::registry::lookup("p7_1").unwrap();
    let mut input = json!({"scenario":"color_pair","property":"background-color","start":"red","end":"transparent","operation":{"kind":"interpolate","progress_bits":0.5_f32.to_bits()}});
    assert_eq!(
        runner(&input).unwrap(),
        json!({"property":"background-color","kind":"numeric","rgba":[1.0,0.0,0.0,0.5],"rounded_to_zero":[false,false,false,false],"fallback_discrete":false})
    );
    input["end"] = json!("blue");
    assert_eq!(runner(&input).unwrap()["rgba"], json!([0.5, 0.0, 0.5, 1.0]));
    input["operation"] = json!({"kind":"add"});
    assert_eq!(runner(&input).unwrap()["rgba"], json!([0.5, 0.0, 0.5, 2.0]));
    input["operation"] = json!({"kind":"accumulate"});
    assert_eq!(runner(&input).unwrap()["rgba"], json!([0.5, 0.0, 0.5, 2.0]));
    input["property"] = json!("outline-color");
    input["start"] = json!("invert");
    input["operation"] = json!({"kind":"interpolate","progress_bits":0.25_f32.to_bits()});
    assert_eq!(
        runner(&input).unwrap(),
        json!({"property":"outline-color","kind":"invert","fallback_discrete":true})
    );
}
#[test]
fn color_harness_rejects_extra_fields_and_unresolved_or_invalid_input() {
    let runner = iced_css_conformance::registry::lookup("p7_1").unwrap();
    let input = json!({"scenario":"color_pair","property":"color","start":"red","end":"blue","operation":{"kind":"interpolate","progress_bits":0.5_f32.to_bits()}});
    for (key, value) in [
        ("extra", json!(true)),
        ("property", json!("display")),
        ("property", json!("margin")),
        ("start", json!("inherit")),
        ("start", json!("red;color:blue")),
        ("start", json!("ButtonFace")),
        ("operation", json!({"kind":"unknown"})),
        (
            "operation",
            json!({"kind":"interpolate","progress_bits":f32::NAN.to_bits()}),
        ),
        (
            "operation",
            json!({"kind":"interpolate","progress_bits":0,"extra":true}),
        ),
    ] {
        let mut changed = input.clone();
        changed[key] = value;
        assert!(runner(&changed).is_err(), "{changed}");
    }
}
#[test]
fn color_harness_regression_seeds_are_registered() {
    for id in ["P7.1-COLOR", "P7.1-INVERT", "P7.1-COLOR-WIDE"] {
        iced_css_conformance::assert_case(id);
    }
}
