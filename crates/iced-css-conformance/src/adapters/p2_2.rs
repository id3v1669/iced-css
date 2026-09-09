use iced_css_core::{
    cascade::{Candidate, CascadeKey, Origin, Specificity, Winner},
    syntax::{DeclarationItem, DeclarationLimits, Declarations, RecoveryLimits, StringLimits},
    values::{
        NumericLimits,
        color::ColorLimits,
        property::{ParsedProperty, PropertyResult, SpecifiedValue, parse_property},
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::{NonZeroU64, NonZeroUsize};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    work: NonZeroUsize,
    digits: NonZeroUsize,
    bytes: NonZeroUsize,
    nesting: NonZeroUsize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    scenario: String,
    author: String,
    user: String,
    limits: Limits,
}
#[derive(PartialEq)]
struct ValueWithOrigin<'a> {
    origin: &'static str,
    spelling: &'a str,
    parsed: ParsedProperty,
}
/// Controlled two-color-declaration origin scenario. Selector applicability,
/// membership and default resolution remain independent qualification cases.
pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|error| error.to_string())?;
    if input.scenario != "important_origins" {
        return Err("unknown cascade scenario".into());
    }
    let recovery = RecoveryLimits {
        work: input.limits.work,
        nesting: input.limits.nesting,
        retained_bytes: input.limits.bytes,
    };
    let value_limits = ColorLimits {
        syntax: StringLimits {
            retained_bytes: input.limits.bytes,
            work: input.limits.work,
        },
        numeric: NumericLimits {
            digits: input.limits.digits,
            work: input.limits.work,
        },
    };
    let mut winner = Winner::new();
    for (css, origin, label, id) in [
        (&input.author, Origin::Author, "author", 1u64),
        (&input.user, Origin::User, "user", 2),
    ] {
        let mut declarations = Declarations::new(
            css,
            DeclarationLimits {
                recovery,
                name_bytes: input.limits.bytes,
            },
        );
        let item = declarations
            .next()
            .ok_or("missing declaration")?
            .map_err(|error| error.to_string())?;
        if declarations.next().is_some() || !declarations.remainder().is_empty() {
            return Err("scenario requires one declaration per origin".into());
        }
        let DeclarationItem::Declaration(declaration) = item else {
            return Err("scenario requires declaration syntax".into());
        };
        let PropertyResult::Parsed(parsed) =
            parse_property(&declaration, value_limits).map_err(|error| error.to_string())?
        else {
            return Err("scenario requires valid implemented property values".into());
        };
        if parsed.property.descriptor().name != "color"
            || !matches!(parsed.value, SpecifiedValue::Color(_))
        {
            return Err("scenario requires two color values".into());
        }
        winner
            .offer(Candidate {
                key: CascadeKey {
                    level: origin.level(parsed.important),
                    specificity: Specificity::ZERO,
                    order: id,
                },
                declaration: NonZeroU64::new(id).ok_or("invalid fixture declaration identity")?,
                value: ValueWithOrigin {
                    origin: label,
                    spelling: declaration.value,
                    parsed,
                },
            })
            .map_err(|error| error.to_string())?;
    }
    let selected = &winner.candidate().ok_or("no admitted winner")?.value;
    Ok(json!({"winner_origin":selected.origin,"color":selected.spelling}))
}
