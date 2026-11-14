use iced_css_conformance::coverage;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
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

fn fixture_json(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn sha256(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

#[test]
fn ledger_intake_copies_the_pinned_ledgers_and_lists_every_requirement() {
    let pins: BTreeMap<String, String> = fixture_json("sources.json")
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["id"].as_str().unwrap().into(),
                s["sha256"].as_str().unwrap().into(),
            )
        })
        .collect();
    let requirements: BTreeMap<String, Value> = fixture_json("requirements.json")
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["id"].as_str().unwrap().into(), r.clone()))
        .collect();
    let mut listed = BTreeSet::new();
    for (name, ledger) in fixture_json("ledger-intake.json").as_object().unwrap() {
        let source = ledger["source"].as_str().unwrap();
        let text = ledger["complete_document_text"].as_str().unwrap();
        assert_eq!(pins.get(source), Some(&sha256(text)), "{name}: stale copy");
        for row in ledger["rows"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            assert!(listed.insert(id.to_owned()), "{name}: duplicate row {id}");
            assert!(text.contains(row["row"].as_str().unwrap()), "{id}");
            assert!(text.contains(row["header"].as_str().unwrap()), "{id}");
            let sources = &requirements.get(id).unwrap_or_else(|| panic!("{id}"))["sources"];
            assert!(
                sources.as_array().unwrap().iter().any(|s| s == source),
                "{id}"
            );
        }
    }
    assert!(listed.iter().eq(requirements.keys()));
}

#[test]
fn inventory_texts_match_their_recorded_hashes() {
    for (name, inventory) in fixture_json("inventories.json").as_object().unwrap() {
        let text = inventory["text"].as_str().unwrap();
        assert_eq!(
            inventory["sha256"].as_str(),
            Some(sha256(text).as_str()),
            "{name}"
        );
    }
}

#[test]
fn passing_cases_rerun_their_indexed_fixture() {
    let index = fixture_json("index.json");
    let fixtures: BTreeMap<String, Value> = index
        .as_array()
        .unwrap()
        .iter()
        .map(|file| fixture_json(file.as_str().unwrap()))
        .map(|f| (f["id"].as_str().unwrap().into(), f))
        .collect();
    for case in fixture_json("cases.json").as_array().unwrap() {
        if case["status"]["status"] != "pass" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let fixture = fixtures
            .get(id)
            .unwrap_or_else(|| panic!("{id}: no indexed fixture"));
        assert_eq!(fixture["classification"], case["oracle"]["kind"], "{id}");
        iced_css_conformance::assert_case(id);
    }
}
