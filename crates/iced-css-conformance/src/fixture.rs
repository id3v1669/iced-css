//! Strict structural admission. Source qualification remains a separate gate.
use serde::Deserialize;
use serde_json::Value;
use std::path::{Component, Path, PathBuf};
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Classification {
    Requirement,
    Policy,
    Mixed,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub id: String,
    pub runner: String,
    pub spec: String,
    pub classification: Classification,
    pub input: Value,
    pub expected: Value,
}
pub fn load(path: &Path) -> Result<Fixture, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let fixture: Fixture =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    for (name, value) in [
        ("id", &fixture.id),
        ("runner", &fixture.runner),
        ("spec", &fixture.spec),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{}: empty {name}", path.display()));
        }
    }
    Ok(fixture)
}
pub(crate) fn contained_path(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!(
            "invalid fixture-relative path: {}",
            relative.display()
        ));
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(format!(
            "fixture outside root or not a file: {}",
            path.display()
        ));
    }
    Ok(path)
}
