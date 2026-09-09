use iced_css_core::values::{LengthTarget, NumericLimits};
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::NonZeroUsize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    digits: NonZeroUsize,
    work: NonZeroUsize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    scenario: String,
    left: String,
    right: String,
    relative: String,
    limits: Limits,
}

pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    if input.scenario != "absolute_identity" {
        return Err("unknown exact-length scenario".into());
    }
    let limits = NumericLimits {
        digits: input.limits.digits,
        work: input.limits.work,
    };
    let parse = |text: &str| LengthTarget::parse(text, limits).map_err(|e| e.to_string());
    let left = parse(&input.left)?;
    let right = parse(&input.right)?;
    let relative = parse(&input.relative)?;
    if !matches!(left, LengthTarget::Absolute(_)) || !matches!(right, LengthTarget::Absolute(_)) {
        return Err("absolute comparison requires two absolute operands".into());
    }
    Ok(
        json!({"absolute_equal": left == right, "relative_is_absolute": matches!(relative, LengthTarget::Absolute(_))}),
    )
}
