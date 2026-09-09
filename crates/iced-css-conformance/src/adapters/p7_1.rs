//! Eligibility and explicit color-pair regressions; final rendering remains separate.
use iced_css_core::{
    animation::{
        color::{ColorAnimationEndpoint, ColorFiniteValue, ColorPairComponent, combine_color},
        combine::Combination,
        registry::{AnimationSelection, SelectionError},
    },
    syntax::{DeclarationItem, DeclarationLimits, Declarations, RecoveryLimits, StringLimits},
    values::{
        NumericLimits,
        color::ColorLimits,
        computed_color::ComputedColor,
        property::{PropertyResult, parse_property},
        registry::PropertyId,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::NonZeroUsize;
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "snake_case", deny_unknown_fields)]
enum Input {
    DisplayRequest {
        property: String,
        // Admission order intentionally leaves these strings uninterpreted.
        #[serde(rename = "start")]
        _start: String,
        #[serde(rename = "end")]
        _end: String,
    },
    ColorPair {
        property: String,
        start: String,
        end: String,
        operation: Operation,
    },
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Interpolate { progress_bits: u32 },
    Add {},
    Accumulate {},
}
fn work() -> NonZeroUsize {
    NonZeroUsize::new(PropertyId::all().len() + 1).expect("single request plus finite registry")
}
fn numeric() -> NumericLimits {
    NumericLimits {
        digits: NonZeroUsize::new(1000).unwrap(),
        work: NonZeroUsize::new(1_000_000).unwrap(),
    }
}
fn color_endpoint(property: PropertyId, value: &str) -> Result<ColorAnimationEndpoint, String> {
    if value.len() > 4096 {
        return Err("color fixture value exceeds adapter input bound".into());
    }
    let source = format!("{}:{value}", property.descriptor().name);
    let n = |v| NonZeroUsize::new(v).unwrap();
    let mut declarations = Declarations::new(
        &source,
        DeclarationLimits {
            recovery: RecoveryLimits {
                work: n(100_000),
                nesting: n(20),
                retained_bytes: n(4096),
            },
            name_bytes: n(4096),
        },
    );
    let declaration = declarations
        .next()
        .ok_or("missing color declaration")?
        .map_err(|e| e.to_string())?;
    let DeclarationItem::Declaration(declaration) = declaration else {
        return Err("expected color declaration".into());
    };
    if declarations.next().is_some() {
        return Err("color fixture must contain one declaration".into());
    }
    let parsed = parse_property(
        &declaration,
        ColorLimits {
            syntax: StringLimits {
                work: n(100_000),
                retained_bytes: n(4096),
            },
            numeric: numeric(),
        },
    )
    .map_err(|e| e.to_string())?;
    let PropertyResult::Parsed(parsed) = parsed else {
        return Err("invalid color value".into());
    };
    let computed = ComputedColor::try_from(&parsed).map_err(|e| e.to_string())?;
    ColorAnimationEndpoint::from_computed(computed, numeric()).map_err(|e| e.to_string())
}
fn color_pair(
    property: PropertyId,
    start: &str,
    end: &str,
    operation: Operation,
) -> crate::CaseResult {
    AnimationSelection::new(&[property], work()).map_err(|e| e.to_string())?;
    let operation = match operation {
        Operation::Interpolate { progress_bits } => {
            Combination::Interpolate(f32::from_bits(progress_bits))
        }
        Operation::Add {} => Combination::Add,
        Operation::Accumulate {} => Combination::Accumulate,
    };
    let start = color_endpoint(property, start)?;
    let end = color_endpoint(property, end)?;
    let pair = combine_color(&start, &end, operation, numeric()).map_err(|e| e.to_string())?;
    let fallback = matches!(pair.component(), ColorPairComponent::Discrete(_));
    let sample = pair
        .to_finite_unpremultiplied(numeric())
        .map_err(|e| e.to_string())?;
    let property = sample.property.descriptor().name;
    match sample.value {
        ColorFiniteValue::Invert => {
            Ok(json!({"property":property,"kind":"invert","fallback_discrete":fallback}))
        }
        ColorFiniteValue::Numeric(values) => Ok(
            json!({"property":property,"kind":"numeric","rgba":values.map(|v|v.value),"rounded_to_zero":values.map(|v|v.rounded_to_zero),"fallback_discrete":fallback}),
        ),
    }
}
pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    match input {
        Input::DisplayRequest { property, .. } => {
            let property = PropertyId::lookup(&property).ok_or("unknown CSS2.2 property")?;
            let eligible = match AnimationSelection::new(&[property], work()) {
                Ok(selection) => !selection.is_empty(),
                Err(SelectionError::NotAnimatable(_)) => false,
                Err(error) => return Err(error.to_string()),
            };
            Ok(json!({"eligible":eligible,"fallback_discrete":false}))
        }
        Input::ColorPair {
            property,
            start,
            end,
            operation,
        } => {
            let property = PropertyId::lookup(&property).ok_or("unknown CSS2.2 property")?;
            color_pair(property, &start, &end, operation)
        }
    }
}
