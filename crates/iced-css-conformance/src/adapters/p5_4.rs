use iced_css_core::values::{CounterLimits, CounterStyle, Integer, NumericLimits, format_counter};
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::NonZeroUsize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    scenario: String,
    style: String,
    value: String,
    digits: NonZeroUsize,
    work: NonZeroUsize,
    retained_bytes: NonZeroUsize,
}
/// Q284 counter text only. Scope traversal, markers and fonts have separate tests.
pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    if input.scenario != "builtin_counter" {
        return Err("unknown counter scenario".into());
    }
    let style = CounterStyle::parse(&input.style).ok_or("unknown built-in counter style")?;
    let numeric = NumericLimits {
        digits: input.digits,
        work: input.work,
    };
    let value = Integer::parse(&input.value, numeric).map_err(|e| e.to_string())?;
    let text = format_counter(
        &value,
        style,
        CounterLimits {
            numeric,
            retained_bytes: input.retained_bytes,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(json!({"text":text}))
}
