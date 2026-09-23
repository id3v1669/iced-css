use iced_css_core::formatting::{
    block::AutoLength,
    clearance::{ClearanceState, solve_clearance},
    floats::{Band, FloatRequest, FloatSide, PlacedFloat, exclusion_band, place_float},
    shrink::{ShrinkWidthInput, solve_shrink_width},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::NonZeroUsize;

#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "snake_case", deny_unknown_fields)]
enum Scenario {
    ShrinkToFit(ShrinkToFit),
    ExclusionBand(Exclusion),
    PlaceFloat(Placement),
    Clearance(ClearanceInput),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClearanceInput {
    hypothetical_top: f32,
    state: ClearancePresence,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ClearancePresence {
    Absent {},
    Required {
        zero_clearance_border_top: f32,
        lowest_float_bottom: f32,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShrinkToFit {
    preferred_minimum: f32,
    available: f32,
    preferred: f32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Float {
    side: String,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Placement {
    containing_left: f32,
    containing_right: f32,
    containing_top: f32,
    floats: Vec<Float>,
    side: String,
    width: f32,
    height: f32,
    hypothetical_top: f32,
    work: NonZeroUsize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Exclusion {
    containing_left: f32,
    containing_right: f32,
    floats: Vec<Float>,
    y: f32,
}
fn side(value: &str) -> Result<FloatSide, String> {
    match value {
        "left" => Ok(FloatSide::Left),
        "right" => Ok(FloatSide::Right),
        other => Err(format!("unknown float side: {other}")),
    }
}
fn floats(values: &[Float]) -> Result<Vec<PlacedFloat>, String> {
    values
        .iter()
        .map(|float| {
            Ok(PlacedFloat {
                side: side(&float.side)?,
                left: float.left,
                right: float.right,
                top: float.top,
                bottom: float.bottom,
            })
        })
        .collect()
}
/// Float placement, exclusion bands, shrink-to-fit width and explicit clearance
/// amounts. Every scenario calls product code. Outputs serialize the resulting f32
/// values as ordinary JSON numbers; core tests cover bit-sensitive rounding cases.
pub fn run(input: &Value) -> crate::CaseResult {
    match serde_json::from_value(input.clone()).map_err(|e| e.to_string())? {
        Scenario::Clearance(input) => {
            let state = match input.state {
                ClearancePresence::Absent {} => ClearanceState::Absent,
                ClearancePresence::Required {
                    zero_clearance_border_top,
                    lowest_float_bottom,
                } => ClearanceState::Required {
                    zero_clearance_border_top,
                    lowest_float_bottom,
                },
            };
            let result =
                solve_clearance(input.hypothetical_top, state).map_err(|e| e.to_string())?;
            Ok(json!({"border_top": result.border_top, "clearance": result.clearance}))
        }
        Scenario::ShrinkToFit(input) => {
            // The float category's auto width is exactly the containing-block
            // arithmetic of CSS 2.2 §10.3.5 with no margins, borders or padding.
            let result = solve_shrink_width(ShrinkWidthInput {
                containing_width: input.available,
                width: AutoLength::Auto,
                margin_left: AutoLength::Value(0.0),
                margin_right: AutoLength::Value(0.0),
                border_left: 0.0,
                border_right: 0.0,
                padding_left: 0.0,
                padding_right: 0.0,
                scrollbar_width: 0.0,
                preferred_min_width: input.preferred_minimum,
                preferred_width: input.preferred,
                min_width: 0.0,
                max_width: None,
            })
            .map_err(|e| e.to_string())?;
            Ok(json!({ "used_width": result.width }))
        }
        Scenario::ExclusionBand(input) => {
            let band = exclusion_band(
                &floats(&input.floats)?,
                Band {
                    left: input.containing_left,
                    right: input.containing_right,
                },
                input.y,
            )
            .map_err(|e| e.to_string())?;
            Ok(json!({"left": band.left, "right": band.right, "width": band.width()}))
        }
        Scenario::PlaceFloat(input) => {
            let placement = place_float(
                &floats(&input.floats)?,
                Band {
                    left: input.containing_left,
                    right: input.containing_right,
                },
                FloatRequest {
                    side: side(&input.side)?,
                    width: input.width,
                    height: input.height,
                    containing_top: input.containing_top,
                    hypothetical_top: input.hypothetical_top,
                },
                input.work,
            )
            .map_err(|e| e.to_string())?;
            Ok(json!({
                "left": placement.placed.left,
                "right": placement.placed.right,
                "top": placement.placed.top,
                "bottom": placement.placed.bottom,
                "bands": placement.bands,
                "overflows": placement.overflows,
            }))
        }
    }
}
