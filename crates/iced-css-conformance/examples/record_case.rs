//! Emit actual harness observations. The caller records tool/environment provenance.
use iced_css_conformance::{Harness, Status, registry};
use std::{io, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [root, fixture, id] = args.as_slice() else {
        return Err(io::Error::other("expected evidence-root fixture-file case-id").into());
    };
    let harness = Harness::load(
        Path::new(root),
        registry::builtins().map_err(io::Error::other)?,
        &[fixture],
    )
    .map_err(io::Error::other)?;
    let observations = harness.run(&[id]).map_err(io::Error::other)?;
    println!("{}", serde_json::to_string(&observations)?);
    if observations.iter().any(|o| o.status != Status::Pass) {
        return Err(io::Error::other("selected case did not pass").into());
    }
    Ok(())
}
