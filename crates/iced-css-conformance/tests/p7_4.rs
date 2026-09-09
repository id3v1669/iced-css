use serde_json::json;
#[test]
fn contract_p7_4() {
    iced_css_conformance::assert_case("P7.4");
}
#[test]
fn remote_requests_come_from_authorization_not_a_constant() {
    let run = iced_css_conformance::registry::lookup("p7_4").unwrap();
    assert_eq!(run(&json!({"scenario":"remote_refresh_triggers","asset_state":"already_loaded","events":["expiry","eviction","local_watch","view_rebuild"]})).unwrap(),json!({"http_refresh_requests":0}));
    assert_eq!(run(&json!({"scenario":"remote_refresh_triggers","asset_state":"already_loaded","events":["first_acquisition","explicit_message"]})).unwrap(),json!({"http_refresh_requests":1}));
    assert_eq!(run(&json!({"scenario":"remote_refresh_triggers","asset_state":"never_requested","events":["first_acquisition","first_acquisition","explicit_message"]})).unwrap(),json!({"http_refresh_requests":2}));
}
#[test]
fn stylesheet_results_come_from_owned_candidate_publication() {
    let run = iced_css_conformance::registry::lookup("p7_4").unwrap();
    let empty =
        json!({"active_css":null,"active_revision":0,"declarations":0,"recovered_declarations":0});
    assert_eq!(run(&json!({"scenario":"stylesheet_requests","request_count":1,"completions":[{"request":0,"css":null}]})).unwrap(),empty);
    assert_eq!(run(&json!({"scenario":"stylesheet_requests","initial":"a{color:red}","request_count":2,"completions":[{"request":1,"css":null},{"request":0,"css":"a{color:blue}"}]})).unwrap(),json!({"active_css":"a{color:red}","active_revision":1,"declarations":1,"recovered_declarations":0}));
    assert_eq!(run(&json!({"scenario":"stylesheet_requests","request_count":1,"completions":[{"request":0,"css":"a{color:red;width:bogus;height:2px}"}]})).unwrap(),json!({"active_css":"a{color:red;width:bogus;height:2px}","active_revision":1,"declarations":2,"recovered_declarations":1}));
    assert_eq!(run(&json!({"scenario":"stylesheet_requests","initial":"a{color:red}","request_count":1,"completions":[{"request":0,"css":""}]})).unwrap(),json!({"active_css":"","active_revision":2,"declarations":0,"recovered_declarations":0}));
}
#[test]
fn malformed_and_unbounded_inputs_are_rejected() {
    let run = iced_css_conformance::registry::lookup("p7_4").unwrap();
    for input in [
        json!({"scenario":"remote_refresh_triggers","asset_state":"missing","events":[]}),
        json!({"scenario":"remote_refresh_triggers","asset_state":"already_loaded","events":["unknown"]}),
        json!({"scenario":"remote_refresh_triggers","asset_state":"already_loaded","events":vec!["expiry";65]}),
        json!({"scenario":"stylesheet_requests","request_count":65,"completions":[]}),
        json!({"scenario":"stylesheet_requests","request_count":1,"completions":[{"request":1,"css":null}]}),
        json!({"scenario":"stylesheet_requests","request_count":0,"initial":"x".repeat(4097),"completions":[]}),
        json!({"scenario":"stylesheet_requests","request_count":0,"completions":[],"extra":true}),
    ] {
        assert!(run(&input).is_err(), "{input}");
    }
}

#[test]
fn root_import_reload_policy_fixture() {
    iced_css_conformance::assert_case("P7.4-import-reload");
}
#[test]
fn graph_reload_retention_changes_when_import_acquisition_succeeds() {
    let run = iced_css_conformance::registry::lookup("p7_4").unwrap();
    let before = "@import \"child\"; r{color:red}";
    let after = "@import \"child\"; r{color:blue}";
    let mut input = json!({"scenario":"root_import_reload","root_before":before,"import_before":"a{height:2px}","root_after":after,"import_after":null});
    assert_eq!(
        run(&input).unwrap(),
        json!({"active_root":before,"active_import":"a{height:2px}","active_revision":1,"completion":"failed"})
    );
    input["import_after"] = json!("a{height:3px}");
    assert_eq!(
        run(&input).unwrap(),
        json!({"active_root":after,"active_import":"a{height:3px}","active_revision":2,"completion":"published"})
    );
    input["root_after"] = json!("r{color:blue}");
    assert!(run(&input).is_err());
    input["root_after"] = json!(after);
    input["import_after"] = json!("x".repeat(4097));
    assert!(run(&input).is_err());
}
