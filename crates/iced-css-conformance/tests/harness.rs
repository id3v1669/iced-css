use iced_css_conformance::{Harness, Registry, Status, aggregate, compare};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "iced-css-harness-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, value: &Value) {
        std::fs::write(self.0.join(name), serde_json::to_vec(value).unwrap()).unwrap();
    }

    fn harness(&self, files: &[&str]) -> Result<Harness, String> {
        Harness::load(&self.0, Registry::new(&[("echo", echo)])?, files)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn echo(input: &Value) -> Result<Value, String> {
    Ok(input.clone())
}

fn fixture(id: &str) -> Value {
    json!({
        "id": id,
        "runner": "echo",
        "spec": "architecture-72-conformance-harness.md",
        "classification": "policy",
        "input": {"value": 1},
        "expected": {"value": 1}
    })
}

#[test]
fn valid_case_runs_and_records_actual_output() {
    let dir = Directory::new();
    dir.write("case.json", &fixture("one"));
    let report = dir.harness(&["case.json"]).unwrap().run(&["one"]).unwrap();
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].status, Status::Pass);
    assert_eq!(report[0].actual, Some(json!({"value": 1})));
}

#[test]
fn expected_output_is_not_supplied_to_runner() {
    let dir = Directory::new();
    let mut case = fixture("mismatch");
    case["expected"] = json!({"value": 2});
    dir.write("case.json", &case);
    let report = dir
        .harness(&["case.json"])
        .unwrap()
        .run(&["mismatch"])
        .unwrap();
    assert!(matches!(report[0].status, Status::Fail { .. }));
    assert_eq!(report[0].actual, Some(json!({"value": 1})));
    assert!(compare(&json!({"value": 1}), &json!({"value": 2})).is_err());
}

#[test]
fn empty_and_unknown_selection_cannot_pass() {
    let dir = Directory::new();
    dir.write("case.json", &fixture("one"));
    let harness = dir.harness(&["case.json"]).unwrap();
    assert!(harness.run(&[]).is_err());
    assert!(harness.run(&["missing"]).is_err());
    assert!(harness.run(&["one", "one"]).is_err());
}

#[test]
fn duplicate_fixture_ids_are_rejected_before_execution() {
    let dir = Directory::new();
    dir.write("a.json", &fixture("same"));
    dir.write("b.json", &fixture("same"));
    assert!(dir.harness(&["a.json", "b.json"]).is_err());
}

#[test]
fn unregistered_runner_is_not_a_successful_empty_implementation() {
    assert!(iced_css_conformance::registry::lookup("unregistered").is_err());
    let dir = Directory::new();
    let mut case = fixture("one");
    case["runner"] = json!("unregistered");
    dir.write("case.json", &case);
    assert!(dir.harness(&["case.json"]).is_err());
}

#[test]
fn conflicting_runner_names_are_rejected() {
    assert!(Registry::new(&[("echo", echo), ("echo", echo)]).is_err());
}

#[test]
fn missing_empty_or_unknown_provenance_is_rejected() {
    let dir = Directory::new();
    for key in ["id", "runner", "spec", "classification"] {
        let mut case = fixture("one");
        case.as_object_mut().unwrap().remove(key);
        dir.write("case.json", &case);
        assert!(dir.harness(&["case.json"]).is_err(), "missing {key}");
        case = fixture("one");
        case[key] = json!("   ");
        dir.write("case.json", &case);
        assert!(dir.harness(&["case.json"]).is_err(), "empty {key}");
    }
    let mut case = fixture("one");
    case["classification"] = json!("probably_normative");
    dir.write("case.json", &case);
    assert!(dir.harness(&["case.json"]).is_err());
}

#[test]
fn malformed_unknown_and_duplicate_fields_are_rejected() {
    let dir = Directory::new();
    std::fs::write(dir.0.join("case.json"), b"{").unwrap();
    assert!(dir.harness(&["case.json"]).is_err());
    let mut case = fixture("one");
    case["exepcted"] = json!({});
    dir.write("case.json", &case);
    assert!(dir.harness(&["case.json"]).is_err());
    let duplicate = serde_json::to_string(&fixture("one")).unwrap();
    std::fs::write(
        dir.0.join("case.json"),
        duplicate.replacen('{', "{\"id\":\"two\",", 1),
    )
    .unwrap();
    assert!(dir.harness(&["case.json"]).is_err());
}

#[test]
fn missing_file_and_path_escape_are_rejected() {
    let dir = Directory::new();
    assert!(dir.harness(&["missing.json"]).is_err());
    assert!(dir.harness(&["../outside.json"]).is_err());
    assert!(dir.harness(&["/outside.json"]).is_err());
}

#[cfg(unix)]
#[test]
fn symlink_cannot_escape_fixture_root() {
    let dir = Directory::new();
    let outside = Directory::new();
    outside.write("case.json", &fixture("one"));
    std::os::unix::fs::symlink(outside.0.join("case.json"), dir.0.join("link.json")).unwrap();
    assert!(dir.harness(&["link.json"]).is_err());
}

#[test]
fn runner_error_is_a_failed_case_with_no_fabricated_actual() {
    fn failure(_: &Value) -> Result<Value, String> {
        Err("service unavailable".into())
    }
    let dir = Directory::new();
    dir.write("case.json", &fixture("one"));
    let registry = Registry::new(&[("echo", failure)]).unwrap();
    let harness = Harness::load(&dir.0, registry, &["case.json"]).unwrap();
    let report = harness.run(&["one"]).unwrap();
    assert!(matches!(report[0].status, Status::Fail { .. }));
    assert_eq!(report[0].actual, None);
}

#[test]
fn skip_not_run_and_empty_evidence_never_complete_coverage() {
    let summary = aggregate(&[
        Status::Pass,
        Status::Skip {
            reason: "no native session".into(),
        },
        Status::NotRun {
            reason: "not scheduled".into(),
        },
        Status::Fail {
            reason: "mismatch".into(),
        },
    ])
    .unwrap();
    assert_eq!(
        (
            summary.passed,
            summary.failed,
            summary.skipped,
            summary.not_run
        ),
        (1, 1, 1, 1)
    );
    assert!(!summary.complete);
    assert!(!aggregate(&[]).unwrap().complete);
    assert!(aggregate(&[Status::Pass]).unwrap().complete);
    assert!(
        aggregate(&[Status::Skip {
            reason: String::new()
        }])
        .is_err()
    );
}

#[test]
fn recorded_run_requires_configuration_and_tool_provenance() {
    use iced_css_conformance::report::ExecutionContext;
    let dir = Directory::new();
    dir.write("case.json", &fixture("one"));
    let harness = dir.harness(&["case.json"]).unwrap();
    let context = ExecutionContext {
        command: vec!["cargo".into(), "test".into()],
        target: "x86_64-unknown-linux-gnu".into(),
        backend: "harness-only".into(),
        mode: "harness-only".into(),
        tools: std::collections::BTreeMap::from([(
            "rustc".into(),
            "recorded test tool identity".into(),
        )]),
        artifacts: std::collections::BTreeMap::from([("case.json".into(), "0".repeat(64))]),
    };
    assert!(harness.run_recorded(&["one"], context.clone()).is_ok());
    let mut invalid = context.clone();
    invalid.target.clear();
    assert!(harness.run_recorded(&["one"], invalid).is_err());
    let mut invalid = context.clone();
    invalid.tools.clear();
    assert!(harness.run_recorded(&["one"], invalid).is_err());
    let mut invalid = context;
    invalid
        .artifacts
        .insert("case.json".into(), "invalid".into());
    assert!(harness.run_recorded(&["one"], invalid).is_err());
}

#[test]
fn missing_input_or_expected_is_not_implicit_null() {
    let dir = Directory::new();
    for field in ["input", "expected"] {
        let mut case = fixture("one");
        case.as_object_mut().unwrap().remove(field);
        dir.write("case.json", &case);
        assert!(
            dir.harness(&["case.json"]).is_err(),
            "missing {field} must fail admission"
        );
    }
    let mut case = fixture("one");
    case["input"] = Value::Null;
    case["expected"] = Value::Null;
    dir.write("case.json", &case);
    assert_eq!(
        dir.harness(&["case.json"]).unwrap().run(&["one"]).unwrap()[0].status,
        Status::Pass
    );
}
