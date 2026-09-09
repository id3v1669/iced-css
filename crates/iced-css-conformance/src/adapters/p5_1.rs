use iced_css_core::formatting::margins::MarginExtrema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::NonZeroUsize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    scenario: String,
    adjoining_margins: Vec<f32>,
    barriers: Vec<String>,
    work: NonZeroUsize,
}
/// Controlled arithmetic of an already adjoining group. Source adjacency and
/// barriers require the later formatting graph; nonempty barriers are rejected.
pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    if input.scenario != "collapse_group" {
        return Err("unknown margin scenario".into());
    }
    if !input.barriers.is_empty() {
        return Err("source adjacency/barrier scenario is not implemented by this adapter".into());
    }
    let mut margins = MarginExtrema::default();
    margins
        .extend(&input.adjoining_margins, input.work)
        .map_err(|e| e.to_string())?;
    Ok(json!({"collapsed_margin_bits":format!("{:08x}", margins.collapsed().to_bits())}))
}
