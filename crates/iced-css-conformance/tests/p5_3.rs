use serde_json::json;
#[test]
fn default_newline_case_is_executable() {
    iced_css_conformance::assert_case("P5.3-default-newlines");
}
#[test]
fn changed_text_reaches_the_product_normalizer() {
    let run = iced_css_conformance::registry::lookup("p5_3").unwrap();
    for (text, expected) in [
        ("", ""),
        ("a\rb", "a\nb"),
        ("\r\n\r\n", "\n\n"),
        ("中\u{2028}🦀", "中\u{2028}🦀"),
    ] {
        assert_eq!(run(&json!({"scenario":"default_newlines","text":text,"input_bytes":1000,"retained_bytes":10000,"work":2000})).unwrap(),json!({"normalized":expected}));
    }
}
#[test]
fn unrelated_scenarios_unknown_fields_and_limits_are_rejected() {
    let run = iced_css_conformance::registry::lookup("p5_3").unwrap();
    let input = json!({"scenario":"default_newlines","text":"\r\n","input_bytes":100,"retained_bytes":1000,"work":200});
    for (field, value) in [
        ("scenario", json!("normal_whitespace")),
        ("unknown", json!(true)),
        ("work", json!(1)),
        ("input_bytes", json!(1)),
        ("retained_bytes", json!(1)),
    ] {
        let mut changed = input.clone();
        changed[field] = value;
        assert!(run(&changed).is_err());
    }
}
