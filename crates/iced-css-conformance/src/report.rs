//! Execution outcomes; missing evidence cannot complete coverage.
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Status {
    Pass,
    Fail { reason: String },
    Skip { reason: String },
    NotRun { reason: String },
}
#[derive(Debug, Serialize)]
pub struct Observation {
    pub id: String,
    pub status: Status,
    pub actual: Option<Value>,
}
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub not_run: usize,
    pub complete: bool,
}
pub fn aggregate(statuses: &[Status]) -> Result<Summary, String> {
    let mut summary = Summary::default();
    for status in statuses {
        if let Status::Fail { reason } | Status::Skip { reason } | Status::NotRun { reason } =
            status
            && reason.trim().is_empty()
        {
            return Err("nonpassing evidence requires a reason".into());
        }
        match status {
            Status::Pass => summary.passed += 1,
            Status::Fail { .. } => summary.failed += 1,
            Status::Skip { .. } => summary.skipped += 1,
            Status::NotRun { .. } => summary.not_run += 1,
        }
    }
    summary.complete = !statuses.is_empty() && summary.passed == statuses.len();
    Ok(summary)
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionContext {
    pub command: Vec<String>,
    pub target: String,
    pub backend: String,
    pub mode: String,
    pub tools: std::collections::BTreeMap<String, String>,
    pub artifacts: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, serde::Serialize)]
pub struct RunRecord {
    pub context: ExecutionContext,
    pub observations: Vec<Observation>,
}

impl ExecutionContext {
    /// Validate declared execution provenance. Source bytes are checked by coverage;
    /// this cannot establish that a caller's declared tool identity is truthful.
    pub fn validate(&self) -> Result<(), String> {
        if self.command.is_empty()
            || self.command.iter().any(|s| s.trim().is_empty())
            || [&self.target, &self.backend, &self.mode]
                .iter()
                .any(|s| s.trim().is_empty())
            || self.tools.is_empty()
            || self
                .tools
                .iter()
                .any(|(k, v)| k.trim().is_empty() || v.trim().is_empty())
            || self.artifacts.is_empty()
            || self.artifacts.iter().any(|(k, v)| {
                k.trim().is_empty() || v.len() != 64 || !v.bytes().all(|b| b.is_ascii_hexdigit())
            })
        {
            return Err("incomplete execution provenance".into());
        }
        Ok(())
    }
}
