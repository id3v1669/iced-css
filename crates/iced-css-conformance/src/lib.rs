mod adapters;
pub mod coverage;
pub mod dependency;
// Test-only fixture admission and execution; not a runtime dependency.
pub mod fixture;
pub mod registry;
pub mod report;
use fixture::Fixture;
pub use registry::Registry;
pub use report::{Observation, Status, Summary, aggregate};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
pub type CaseResult = Result<Value, String>;
pub type CaseRunner = fn(&Value) -> CaseResult;

/// Immutable admitted fixtures and explicit runners. Admission is not oracle proof.
pub struct Harness {
    registry: Registry,
    cases: BTreeMap<String, Fixture>,
}
impl Harness {
    pub fn load(root: &Path, registry: Registry, files: &[&str]) -> Result<Self, String> {
        let mut cases = BTreeMap::new();
        for file in files {
            let path = fixture::contained_path(root, Path::new(file))?;
            let case = fixture::load(&path)?;
            registry.find(&case.runner)?;
            let id = case.id.clone();
            if cases.insert(id.clone(), case).is_some() {
                return Err(format!("duplicate fixture ID: {id}"));
            }
        }
        Ok(Self { registry, cases })
    }
    /// Validate the whole selection before any runner executes.
    pub fn run(&self, ids: &[&str]) -> Result<Vec<Observation>, String> {
        if ids.is_empty() {
            return Err("empty case selection".into());
        }
        let mut selected = Vec::with_capacity(ids.len());
        let mut seen = BTreeSet::new();
        for id in ids {
            if !seen.insert(*id) {
                return Err(format!("duplicate selected case: {id}"));
            }
            let case = self
                .cases
                .get(*id)
                .ok_or_else(|| format!("unknown case: {id}"))?;
            selected.push((case, self.registry.find(&case.runner)?));
        }
        Ok(selected
            .into_iter()
            .map(|(case, runner)| match runner(&case.input) {
                Ok(actual) => {
                    let status = match compare(&actual, &case.expected) {
                        Ok(()) => Status::Pass,
                        Err(reason) => Status::Fail { reason },
                    };
                    Observation {
                        id: case.id.clone(),
                        status,
                        actual: Some(actual),
                    }
                }
                Err(reason) => Observation {
                    id: case.id.clone(),
                    status: Status::Fail {
                        reason: format!("runner failed: {reason}"),
                    },
                    actual: None,
                },
            })
            .collect())
    }
}
pub fn compare(actual: &Value, expected: &Value) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "output mismatch: expected {expected}, got {actual}"
        ))
    }
}
/// Missing fixtures/runners fail loudly; no successful empty selection.
#[track_caller]
pub fn assert_case(id: &str) {
    match run_indexed_case(id) {
        Ok(records) if records.iter().all(|r| r.status == Status::Pass) => {}
        Ok(records) => panic!("case {id} failed: {records:?}"),
        Err(error) => panic!("case {id} not executable: {error}"),
    }
}
fn run_indexed_case(id: &str) -> Result<Vec<Observation>, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let index = std::fs::read(root.join("index.json")).map_err(|e| e.to_string())?;
    let files: Vec<String> = serde_json::from_slice(&index).map_err(|e| e.to_string())?;
    let paths: Vec<&str> = files.iter().map(String::as_str).collect();
    Harness::load(&root, registry::builtins()?, &paths)?.run(&[id])
}
impl Harness {
    pub fn run_recorded(
        &self,
        ids: &[&str],
        context: report::ExecutionContext,
    ) -> Result<report::RunRecord, String> {
        context.validate()?;
        Ok(report::RunRecord {
            context,
            observations: self.run(ids)?,
        })
    }
}
