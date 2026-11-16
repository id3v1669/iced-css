//! Case evidence integrity. Semantic truth/independence remain externally audited.
use super::{Case, Source, Status, contained_path};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum OracleKind {
    Requirement,
    Policy,
    Mixed,
    DependencyObservation,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OracleEvidence {
    kind: OracleKind,
    path: String,
    sha256: String,
}
fn verify_artifact(root: &Path, path: &str, expected: &str, label: &str) -> Result<(), String> {
    let path = contained_path(root, Path::new(path)).map_err(|e| format!("{label}: {e}"))?;
    let bytes = std::fs::read(path).map_err(|e| format!("{label}: {e}"))?;
    if Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
        != expected
    {
        return Err(format!("{label}: artifact hash mismatch"));
    }
    Ok(())
}
pub(super) fn validate(
    case: &Case,
    sources: &BTreeMap<&str, &Source>,
    root: &Path,
) -> Result<(), String> {
    if case.source_hashes.len() != case.sources.len()
        || case.sources.iter().any(|id| {
            sources
                .get(id.as_str())
                .is_none_or(|source| case.source_hashes.get(id) != Some(&source.sha256))
        })
    {
        return Err(format!(
            "case source hashes do not match membership/manifest: {}",
            case.id
        ));
    }
    if [&case.target, &case.backend, &case.mode]
        .iter()
        .any(|s| s.trim().is_empty())
    {
        return Err(format!("missing case environment: {}", case.id));
    }
    verify_artifact(
        root,
        &case.oracle.path,
        &case.oracle.sha256,
        &format!("case {} {:?} oracle", case.id, case.oracle.kind),
    )?;
    let Some(execution) = &case.execution else {
        if matches!(case.status, Status::Pass | Status::Fail { .. }) {
            return Err(format!(
                "executed case lacks execution provenance: {}",
                case.id
            ));
        }
        return Ok(());
    };
    execution.validate()?;
    if execution.target != case.target
        || execution.backend != case.backend
        || execution.mode != case.mode
    {
        return Err(format!("case/execution environment mismatch: {}", case.id));
    }
    for (path, hash) in &execution.artifacts {
        verify_artifact(root, path, hash, &format!("case {} execution", case.id))?;
    }
    Ok(())
}
