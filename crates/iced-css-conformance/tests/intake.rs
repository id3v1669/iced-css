use iced_css_conformance::coverage;
use serde_json::Value;
use std::path::Path;

#[test]
fn retained_intake_has_verified_bytes_but_cannot_claim_implementation() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let load = |name: &str| -> Value {
        serde_json::from_slice(&std::fs::read(fixtures.join(name)).unwrap()).unwrap()
    };
    let sources = load("sources.json");
    let requirements = load("requirements.json");
    let cases = load("cases.json");
    let root = fixtures
        .join("../../..")
        .join("docs")
        .canonicalize()
        .unwrap();
    let report = coverage::evaluate(&requirements, &cases, &sources, &root).unwrap();
    assert!(!report.complete);
    assert!(report.closed.is_empty());
    assert!(!report.pending.is_empty());
}

#[test]
fn default_validation_uses_the_retained_document_root() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let load = |name: &str| -> Value {
        serde_json::from_slice(&std::fs::read(fixtures.join(name)).unwrap()).unwrap()
    };
    coverage::validate(
        &load("requirements.json"),
        &load("cases.json"),
        &load("sources.json"),
    )
    .unwrap();
}
