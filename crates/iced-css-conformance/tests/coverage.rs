use iced_css_conformance::coverage;
use serde_json::{Value, json};
use std::path::Path;

fn requirement(id: &str) -> Value {
    json!({"id":id,"children":[],"interactions":[],"cases":["case"],"sources":["source"],"audit_complete":true,"claimed_complete":false})
}
fn cases() -> Value {
    // Synthetic structural-validator evidence only. This does not assert an
    // actual command, independent CSS oracle, or platform execution took place.
    let hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    json!([{
        "id":"case", "requirements":["parent"], "sources":["source"],
        "source_hashes":{"source":hash},
        "oracle":{"kind":"requirement","path":"empty.txt","sha256":hash},
        "target":"synthetic", "backend":"synthetic", "mode":"synthetic",
        "execution":{
            "command":["synthetic-validator-fixture"],
            "target":"synthetic", "backend":"synthetic", "mode":"synthetic",
            "tools":{"fixture":"synthetic"}, "artifacts":{"empty.txt":hash}
        },
        "status":{"status":"pass"}
    }])
}

fn sources() -> Value {
    // SHA256 of the independently known empty byte sequence.
    json!([{"id":"source","path":"empty.txt","sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}])
}
fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/coverage"))
}

#[test]
fn verified_case_can_close_an_audited_leaf() {
    let summary = coverage::evaluate(
        &json!([requirement("parent")]),
        &cases(),
        &sources(),
        root(),
    )
    .unwrap();
    assert!(summary.complete);
    assert_eq!(summary.closed, vec!["parent"]);
}
#[test]
fn open_child_and_interaction_prevent_parent_completion() {
    for edge in ["children", "interactions"] {
        let mut parent = requirement("parent");
        parent[edge] = json!(["child"]);
        let mut child = requirement("child");
        child["cases"] = json!([]);
        let req = json!([parent, child]);
        let summary = coverage::evaluate(&req, &cases(), &sources(), root()).unwrap();
        assert!(!summary.complete);
        assert!(summary.closed.is_empty());
        let mut claim = req;
        claim[0]["claimed_complete"] = json!(true);
        assert!(coverage::evaluate(&claim, &cases(), &sources(), root()).is_err());
    }
}
#[test]
fn skip_not_run_and_unreviewed_oracle_do_not_close_requirements() {
    for status in ["skip", "not-run", "fail"] {
        let mut case = cases();
        case[0]["status"] = json!({"status":status,"reason":"unqualified"});
        assert!(
            !coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root())
                .unwrap()
                .complete
        );
    }
    let mut req = requirement("parent");
    req["audit_complete"] = json!(false);
    assert!(
        !coverage::evaluate(&json!([req]), &cases(), &sources(), root())
            .unwrap()
            .complete
    );
    assert!(
        !coverage::evaluate(&json!([]), &json!([]), &json!([]), root())
            .unwrap()
            .complete
    );
}
#[test]
fn broken_bidirectional_ownership_and_unknown_dependencies_are_rejected() {
    let mut case = cases();
    case[0]["requirements"] = json!(["unknown"]);
    assert!(
        coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
    );
    let mut parent = requirement("parent");
    parent["children"] = json!(["unknown"]);
    assert!(coverage::evaluate(&json!([parent]), &cases(), &sources(), root()).is_err());
    let mut case = cases();
    case[0]["requirements"] = json!([]);
    assert!(
        coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
    );
}
#[test]
fn duplicate_ids_missing_sources_and_changed_bytes_are_rejected() {
    assert!(
        coverage::evaluate(
            &json!([requirement("parent"), requirement("parent")]),
            &cases(),
            &sources(),
            root()
        )
        .is_err()
    );
    let mut source = sources();
    source[0]["path"] = json!("missing.txt");
    assert!(
        coverage::evaluate(&json!([requirement("parent")]), &cases(), &source, root()).is_err()
    );
    let mut source = sources();
    source[0]["sha256"] = json!("00".repeat(32));
    assert!(
        coverage::evaluate(&json!([requirement("parent")]), &cases(), &source, root()).is_err()
    );
}
#[test]
fn cyclic_dependency_without_leaf_evidence_cannot_self_certify() {
    let mut parent = requirement("parent");
    parent["children"] = json!(["parent"]);
    assert!(coverage::evaluate(&json!([parent]), &cases(), &sources(), root()).is_err());
}

#[test]
fn passing_cases_require_oracle_and_execution_provenance() {
    let mut case = cases();
    case[0].as_object_mut().unwrap().remove("oracle");
    assert!(
        coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
    );
    let mut case = cases();
    case[0]["execution"] = Value::Null;
    assert!(
        coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
    );
}
#[test]
fn oracle_and_execution_artifact_bytes_and_paths_are_verified() {
    for (field, value) in [
        ("sha256", json!("00".repeat(32))),
        ("path", json!("missing.txt")),
        ("path", json!("../empty.txt")),
        ("kind", json!("browser-screenshot-is-truth")),
    ] {
        let mut case = cases();
        case[0]["oracle"][field] = value;
        assert!(
            coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
        );
    }
    for artifacts in [
        json!({"empty.txt":"00".repeat(32)}),
        json!({"missing.txt":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}),
        json!({"../empty.txt":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}),
    ] {
        let mut case = cases();
        case[0]["execution"]["artifacts"] = artifacts;
        assert!(
            coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
        );
    }
}
#[test]
fn per_case_source_hashes_and_environment_labels_must_agree() {
    for hashes in [
        json!({}),
        json!({"source":"00".repeat(32)}),
        json!({"source":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","extra":"00".repeat(32)}),
    ] {
        let mut case = cases();
        case[0]["source_hashes"] = hashes;
        assert!(
            coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
        );
    }
    for field in ["target", "backend", "mode"] {
        let mut case = cases();
        case[0]["execution"][field] = json!("different");
        assert!(
            coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
        );
        let mut case = cases();
        case[0][field] = json!(" ");
        assert!(
            coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
        );
    }
}
#[test]
fn not_run_needs_an_oracle_but_not_fabricated_execution() {
    let mut case = cases();
    case[0]["status"] = json!({"status":"not-run","reason":"no actual run"});
    case[0]["execution"] = Value::Null;
    assert!(
        !coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root())
            .unwrap()
            .complete
    );
    case[0].as_object_mut().unwrap().remove("oracle");
    assert!(
        coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
    );
}

#[test]
fn oracle_kinds_and_closed_metadata_schema_do_not_infer_execution() {
    for kind in ["requirement", "policy", "mixed", "dependency-observation"] {
        let mut case = cases();
        case[0]["oracle"]["kind"] = json!(kind);
        assert!(
            coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root())
                .unwrap()
                .complete
        );
    }
    for field in ["source_hashes", "oracle", "target", "backend", "mode"] {
        let mut case = cases();
        case[0].as_object_mut().unwrap().remove(field);
        assert!(
            coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
        );
    }
    for field in ["oracle", "execution"] {
        let mut case = cases();
        case[0][field]["unrecognized"] = json!(true);
        assert!(
            coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
        );
    }
    let mut case = cases();
    case[0]["status"] = json!({"status":"fail","reason":"observed failure"});
    case[0]["execution"] = Value::Null;
    assert!(
        coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root()).is_err()
    );
    case[0]["status"] = json!({"status":"skip","reason":"environment unavailable"});
    assert!(
        !coverage::evaluate(&json!([requirement("parent")]), &case, &sources(), root())
            .unwrap()
            .complete
    );
}
