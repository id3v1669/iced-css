//! Guards over Cargo's resolved graph, not a complete license or API-use audit.
use serde_json::Value;
use std::path::Path;
/// The one approved non-registry source: luminate at a pinned revision.
const ANIMATE: &str =
    "git+https://github.com/sonata-ltd/luminate?rev=77a4ae2c7f2cd15bf8a4820ad3ae536bf72a6d15";
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing metadata array: {key}"))
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing metadata string: {key}"))
}
/// Enforce the current selected git/registry sources and dependency boundaries.
/// Re-exported API usage, source-replacement config, codec/backend facilities and
/// target-specific runtime behavior still require their separate qualification.
pub fn validate_metadata(metadata: &Value) -> Result<(), String> {
    let packages = array(metadata, "packages")?;
    let members = array(metadata, "workspace_members")?;
    let root = Path::new(string(metadata, "workspace_root")?);
    let runtime = packages
        .iter()
        .find(|p| p["name"] == "iced-css")
        .ok_or("missing runtime facade")?;
    let animate = packages
        .iter()
        .find(|p| p["name"] == "iced_animate")
        .ok_or("missing resolved animation package")?;
    if animate["source"]
        .as_str()
        .and_then(|source| source.strip_prefix(ANIMATE))
        .is_none_or(|rest| !rest.starts_with('#'))
    {
        return Err("animation must resolve to the selected pinned git revision".into());
    }
    let declarations: Vec<_> = array(runtime, "dependencies")?
        .iter()
        .filter(|d| d["name"] == "iced_animate")
        .collect();
    if declarations.len() != 1 {
        return Err("missing or ambiguous animation declaration".into());
    }
    let declaration = declarations[0];
    if declaration.get("optional") != Some(&Value::Bool(false))
        || declaration.get("kind") != Some(&Value::Null)
        || declaration.get("target") != Some(&Value::Null)
        || declaration["source"] != ANIMATE
        || !declaration["path"].is_null()
    {
        return Err(
            "animation must be a nonoptional, unconditional normal dependency on the selected revision"
                .into(),
        );
    }
    let nodes = array(
        metadata.get("resolve").ok_or("missing resolved graph")?,
        "nodes",
    )?;
    let node = nodes
        .iter()
        .find(|n| n["id"] == runtime["id"])
        .ok_or("missing resolved runtime node")?;
    let mut active = false;
    for dep in array(node, "deps")? {
        if dep["pkg"] == animate["id"]
            && array(dep, "dep_kinds")?.iter().any(|k| {
                k.get("kind") == Some(&Value::Null) && k.get("target") == Some(&Value::Null)
            })
        {
            active = true;
        }
    }
    if !active {
        return Err("runtime has no unconditional resolved animation edge".into());
    }
    for package in packages {
        let name = string(package, "name")?;
        if package["source"].is_null() {
            let owned = members.contains(&package["id"])
                && matches!(
                    name,
                    "iced-css"
                        | "iced-css-core"
                        | "iced-css-macro"
                        | "iced-css-build"
                        | "iced-css-examples"
                        | "iced-css-conformance"
                )
                && Path::new(string(package, "manifest_path")?)
                    == root.join("crates").join(name).join("Cargo.toml");
            if !owned {
                return Err(format!("unapproved path dependency: {name}"));
            }
        } else if string(package, "source")?
            != "registry+https://github.com/rust-lang/crates.io-index"
            && package["id"] != animate["id"]
        {
            return Err(format!("unqualified dependency source: {name}"));
        }
        if matches!(
            name,
            "iced"
                | "iced_core"
                | "iced_futures"
                | "iced_graphics"
                | "iced_renderer"
                | "iced_runtime"
                | "iced_tiny_skia"
                | "iced_wgpu"
                | "iced_winit"
                | "iced_widget"
                | "iced_test"
        ) {
            let mut version = string(package, "version")?.split('.');
            if version.next() != Some("0") || version.next() != Some("14") {
                return Err(format!("unqualified iced release family: {name}"));
            }
        }
        if members.contains(&package["id"]) {
            for dependency in array(package, "dependencies")? {
                let dep_name = string(dependency, "name")?;
                if dep_name == "cosmic-text" || dep_name.starts_with("cosmic-") {
                    return Err(format!("direct cosmic dependency: {name} -> {dep_name}"));
                }
                if name != "iced-css-conformance"
                    && dep_name == "iced-css-conformance"
                    && dependency["kind"] != "dev"
                {
                    return Err("test harness cannot be a product/build dependency".into());
                }
            }
        }
    }
    Ok(())
}
