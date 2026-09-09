//! Narrow text-stage adapter; no shaping, wrapping or native execution is implied.
use iced_css_core::text::newlines::{NewlineLimits, normalize_default_newlines};
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::NonZeroUsize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    scenario: String,
    text: String,
    input_bytes: NonZeroUsize,
    retained_bytes: NonZeroUsize,
    work: NonZeroUsize,
}
pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    if input.scenario != "default_newlines" {
        return Err("unknown text scenario".into());
    }
    let normalized = normalize_default_newlines(
        &input.text,
        NewlineLimits {
            input_bytes: input.input_bytes,
            retained_bytes: input.retained_bytes,
            work: input.work,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(json!({"normalized": normalized.text()}))
}
