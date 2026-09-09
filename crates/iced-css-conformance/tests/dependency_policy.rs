use iced_css_conformance::dependency::validate_metadata;
use serde_json::Value;
use std::process::Command;
use std::sync::OnceLock;
fn metadata() -> Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        let output = Command::new(env!("CARGO"))
            .args(["metadata", "--offline", "--locked", "--format-version", "1"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    })
    .clone()
}
fn runtime(data: &mut Value) -> &mut Value {
    data["packages"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["name"] == "iced-css")
        .unwrap()
}
fn animation(data: &mut Value) -> &mut Value {
    runtime(data)["dependencies"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|d| d["name"] == "iced_animate")
        .unwrap()
}
#[test]
fn actual_resolved_graph_has_required_unconditional_local_animation() {
    validate_metadata(&metadata()).unwrap();
}
#[test]
fn optional_target_specific_and_dev_only_animation_are_rejected() {
    for (field, value) in [
        ("optional", Value::Bool(true)),
        ("target", Value::String("cfg(windows)".into())),
        ("kind", Value::String("dev".into())),
    ] {
        let mut data = metadata();
        animation(&mut data)[field] = value;
        assert!(validate_metadata(&data).is_err(), "{field}");
    }
}
#[test]
fn wrong_animation_source_cannot_replace_required_local_package() {
    let mut data = metadata();
    animation(&mut data)["path"] = "/some/other/animate".into();
    assert!(validate_metadata(&data).is_err());
}
#[test]
fn missing_resolved_edge_is_not_hidden_by_a_manifest_declaration() {
    let mut data = metadata();
    let id = runtime(&mut data)["id"].clone();
    let node = data["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == id)
        .unwrap();
    node["deps"]
        .as_array_mut()
        .unwrap()
        .retain(|d| d["name"] != "iced_animate");
    assert!(validate_metadata(&data).is_err());
}
#[test]
fn direct_cosmic_and_test_harness_runtime_dependencies_are_rejected() {
    for name in ["cosmic-text", "iced-css-conformance"] {
        let mut data = metadata();
        let mut dep = animation(&mut data).clone();
        dep["name"] = name.into();
        runtime(&mut data)["dependencies"]
            .as_array_mut()
            .unwrap()
            .push(dep);
        assert!(validate_metadata(&data).is_err());
    }
}
#[test]
fn third_party_path_replacement_is_rejected() {
    let mut data = metadata();
    let package = data["packages"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["name"] == "serde")
        .unwrap();
    package["source"] = Value::Null;
    package["manifest_path"] = "/tmp/vendor/serde/Cargo.toml".into();
    assert!(validate_metadata(&data).is_err());
}

#[test]
fn unqualified_registry_cannot_replace_the_retained_source() {
    let mut data = metadata();
    let package = data["packages"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["name"] == "serde")
        .unwrap();
    package["source"] = "registry+https://unqualified.invalid/index".into();
    assert!(validate_metadata(&data).is_err());
}
#[test]
fn iced_release_family_must_remain_014() {
    let mut data = metadata();
    let package = data["packages"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["name"] == "iced")
        .unwrap();
    package["version"] = "0.15.0".into();
    assert!(validate_metadata(&data).is_err());
}
