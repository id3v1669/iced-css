use serde_json::json;
#[test]
fn contract_p2_3() {
    iced_css_conformance::assert_case("P2.3");
}
#[test]
fn changed_import_count_and_media_drive_actual_tracking() {
    let run = iced_css_conformance::registry::lookup("p2_3").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p2_3.json")).unwrap();
    for (root, occurrences, acquisitions) in [
        ("@import 'a.css';", 1, 1),
        (
            "@import 'a.css';@import url(a.css);@import 'a.css' SCREEN;",
            3,
            1,
        ),
        ("@import 'a.css' print;", 0, 0),
        ("@import 'unavailable.css' print;", 0, 0),
        ("/**/", 0, 0),
    ] {
        let mut input = fixture["input"].clone();
        input["root"] = root.into();
        assert_eq!(
            run(&input).unwrap(),
            json!({"import_occurrences":occurrences,"unique_acquisition_identities":acquisitions})
        );
    }
}
#[test]
fn controlled_adapter_reports_invalid_or_unimplemented_scope_explicitly() {
    let run = iced_css_conformance::registry::lookup("p2_3").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p2_3.json")).unwrap();
    for root in [
        "@import 'a.css' screen,3D;",
        "@import 'unavailable.css';",
        "@media screen{}",
    ] {
        let mut input = fixture["input"].clone();
        input["root"] = root.into();
        assert!(run(&input).is_err(), "{root}");
    }
    let mut input = fixture["input"].clone();
    input["limits"]["occurrences"] = 1.into();
    assert!(run(&input).is_err());
    input = fixture["input"].clone();
    input["unexpected"] = true.into();
    assert!(run(&input).is_err());
}

#[test]
fn root_import_placement_uses_parsed_statement_dispositions_before_resolution() {
    let run = iced_css_conformance::registry::lookup("p2_3").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/p2_3.json")).unwrap();
    for (root, count) in [
        (".x{} @import 'unavailable.css';", 0),
        (".x:unknown{} @import 'a.css';", 1),
        ("@unknown x{} @import 'a.css';", 1),
        ("@charset \"UTF-8\"; @import 'a.css';", 1),
        ("@import 'a.css';.x{} @import 'unavailable.css';", 1),
    ] {
        let mut input = fixture["input"].clone();
        input["root"] = root.into();
        assert_eq!(
            run(&input).unwrap(),
            json!({"import_occurrences":count,"unique_acquisition_identities":count}),
            "{root}"
        );
    }
}
