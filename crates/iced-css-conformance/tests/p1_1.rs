use serde_json::{Value, json};

fn run(ancestor: Value, child: Value, document: Value) -> Value {
    iced_css_conformance::registry::lookup("p1_1").unwrap()(&json!({
        "scenario":"language_inheritance",
        "ancestor_lang":ancestor,
        "child_lang":child,
        "document_lang":document
    }))
    .unwrap()
}

#[test]
fn contract_p1_1() {
    iced_css_conformance::assert_case("P1.1");
}

#[test]
fn language_values_and_provenance_come_from_changed_inputs() {
    for (ancestor, child, document, language, from_document) in [
        (json!("en"), json!(""), json!("fr"), Value::Null, false),
        (json!("en"), Value::Null, json!("fr"), json!("en"), false),
        (Value::Null, Value::Null, json!("fr"), json!("fr"), true),
        (json!("fr"), Value::Null, json!("fr"), json!("fr"), false),
        (Value::Null, json!("fr"), json!("fr"), json!("fr"), false),
        (json!(""), Value::Null, json!("fr"), Value::Null, false),
        (json!(""), json!("de"), json!("fr"), json!("de"), false),
        (Value::Null, Value::Null, Value::Null, Value::Null, false),
    ] {
        assert_eq!(
            run(ancestor, child, document),
            json!({
                "child_language":language,
                "inherits_document_language":from_document
            })
        );
    }
}

#[test]
fn adapter_rejects_unknown_fields_scenarios_and_unqualified_document_input() {
    let runner = iced_css_conformance::registry::lookup("p1_1").unwrap();
    let input = json!({"scenario":"language_inheritance", "document_lang":"fr"});
    assert!(runner(&input).is_ok());
    for (key, value) in [
        ("extra", json!(true)),
        ("scenario", json!("unknown")),
        ("document_lang", json!("")),
        ("child_lang", json!(42)),
    ] {
        let mut changed = input.clone();
        changed[key] = value;
        assert!(runner(&changed).is_err());
    }
}
