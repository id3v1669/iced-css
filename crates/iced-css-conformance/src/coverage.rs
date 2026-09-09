//! Source integrity and prerequisite closure, not an automatic semantic audit.
use crate::{Status, aggregate, fixture::contained_path};
use serde::Deserialize;
mod provenance;
use provenance::OracleEvidence;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirement {
    id: String,
    children: Vec<String>,
    interactions: Vec<String>,
    cases: Vec<String>,
    sources: Vec<String>,
    audit_complete: bool,
    claimed_complete: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    requirements: Vec<String>,
    sources: Vec<String>,
    status: Status,
    source_hashes: BTreeMap<String, String>,
    oracle: OracleEvidence,
    target: String,
    backend: String,
    mode: String,
    execution: Option<crate::report::ExecutionContext>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    id: String,
    path: String,
    sha256: String,
}
#[derive(Debug)]
pub struct CoverageSummary {
    pub complete: bool,
    pub closed: Vec<String>,
    pub pending: Vec<String>,
}

fn index<T>(items: &[T], id: impl Fn(&T) -> &str) -> Result<BTreeMap<&str, &T>, String> {
    let mut map = BTreeMap::new();
    for item in items {
        let key = id(item);
        if key.trim().is_empty() || map.insert(key, item).is_some() {
            return Err(format!("empty or duplicate ID: {key}"));
        }
    }
    Ok(map)
}
fn unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

/// Validate source bytes and all named edges, then close only fully evidenced DAGs.
/// `audit_complete` is an explicit external review assertion, never inferred here.
pub fn evaluate(
    requirements: &Value,
    cases: &Value,
    sources: &Value,
    root: &Path,
) -> Result<CoverageSummary, String> {
    let requirements: Vec<Requirement> =
        serde_json::from_value(requirements.clone()).map_err(|e| e.to_string())?;
    let cases: Vec<Case> = serde_json::from_value(cases.clone()).map_err(|e| e.to_string())?;
    let sources: Vec<Source> =
        serde_json::from_value(sources.clone()).map_err(|e| e.to_string())?;
    let reqs = index(&requirements, |r| &r.id)?;
    let case_index = index(&cases, |c| &c.id)?;
    let source_index = index(&sources, |s| &s.id)?;
    for source in &sources {
        let path = contained_path(root, Path::new(&source.path))?;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let actual = format!("{:x}", Sha256::digest(bytes));
        if actual != source.sha256 {
            return Err(format!("source hash mismatch: {}", source.id));
        }
    }
    for req in &requirements {
        if !unique(&req.children)
            || !unique(&req.interactions)
            || !unique(&req.cases)
            || !unique(&req.sources)
        {
            return Err(format!("duplicate requirement edge: {}", req.id));
        }
        for dependency in req.children.iter().chain(&req.interactions) {
            if !reqs.contains_key(dependency.as_str()) {
                return Err(format!("unknown prerequisite: {dependency}"));
            }
        }
        if req.sources.is_empty()
            || req
                .sources
                .iter()
                .any(|s| !source_index.contains_key(s.as_str()))
        {
            return Err(format!("missing requirement source: {}", req.id));
        }
        for id in &req.cases {
            let case = case_index
                .get(id.as_str())
                .ok_or_else(|| format!("unknown case: {id}"))?;
            if !case.requirements.contains(&req.id) {
                return Err(format!("case lacks reverse requirement edge: {id}"));
            }
        }
    }
    for case in &cases {
        aggregate(std::slice::from_ref(&case.status))?;
        provenance::validate(case, &source_index, root)?;
        if case.requirements.is_empty()
            || !unique(&case.requirements)
            || case.sources.is_empty()
            || !unique(&case.sources)
            || case
                .sources
                .iter()
                .any(|s| !source_index.contains_key(s.as_str()))
        {
            return Err(format!("invalid case ownership/source: {}", case.id));
        }
        for id in &case.requirements {
            let req = reqs
                .get(id.as_str())
                .ok_or_else(|| format!("unknown case requirement: {id}"))?;
            if !req.cases.contains(&case.id) {
                return Err(format!("requirement lacks reverse case edge: {id}"));
            }
            if req.sources.iter().any(|s| !case.sources.contains(s)) {
                return Err(format!("case omits requirement source: {}", case.id));
            }
        }
    }
    // Iterative topological elimination avoids recursion on arbitrarily deep ledgers.
    let mut remaining: BTreeSet<&str> = reqs.keys().copied().collect();
    let mut results: BTreeMap<&str, bool> = BTreeMap::new();
    while !remaining.is_empty() {
        let ready: Vec<&str> = remaining
            .iter()
            .copied()
            .filter(|id| {
                let r = reqs[id];
                r.children
                    .iter()
                    .chain(&r.interactions)
                    .all(|d| results.contains_key(d.as_str()))
            })
            .collect();
        if ready.is_empty() {
            return Err("cyclic prerequisite graph".into());
        }
        for id in ready {
            let req = reqs[id];
            let closed = req.audit_complete
                && !req.cases.is_empty()
                && req
                    .cases
                    .iter()
                    .all(|c| case_index[c.as_str()].status == Status::Pass)
                && req
                    .children
                    .iter()
                    .chain(&req.interactions)
                    .all(|d| results[d.as_str()]);
            if req.claimed_complete && !closed {
                return Err(format!("unsupported completion claim: {id}"));
            }
            results.insert(id, closed);
            remaining.remove(id);
        }
    }
    let mut closed = Vec::new();
    let mut pending = Vec::new();
    for (id, complete) in results {
        if complete {
            closed.push(id.to_owned());
        } else {
            pending.push(id.to_owned());
        }
    }
    Ok(CoverageSummary {
        complete: !closed.is_empty() && pending.is_empty(),
        closed,
        pending,
    })
}

/// Validate the project's docs-relative source manifest, including actual bytes.
/// Success means the evidence is structurally valid, not that coverage is complete.
pub fn validate(requirements: &Value, cases: &Value, sources: &Value) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    evaluate(requirements, cases, sources, &root).map(|_| ())
}
