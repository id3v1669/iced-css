//! CSS22 10.6.4/10.7 non-replaced absolute vertical constraints.
use super::{arithmetic::Exact, block::AutoLength};

#[derive(Debug, Clone, Copy)]
pub struct AbsoluteHeightInput {
    pub containing_height: f32,
    pub static_top: f32,
    pub top: AutoLength,
    pub height: AutoLength,
    pub bottom: AutoLength,
    pub margin_top: AutoLength,
    pub margin_bottom: AutoLength,
    pub border_top: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
    pub border_bottom: f32,
    /// Supplied by the 10.6.7 content/float extent algorithm, not a trial native size.
    pub content_height: f32,
    pub min_height: f32,
    pub max_height: Option<f32>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsoluteHeightResult {
    pub top: f32,
    pub height: f32,
    pub bottom: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub passes: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsoluteHeightError {
    InvalidInput(&'static str),
    Unrepresentable,
}
impl std::fmt::Display for AbsoluteHeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "absolute height: {self:?}")
    }
}
impl std::error::Error for AbsoluteHeightError {}
fn validate(input: &AbsoluteHeightInput) -> Result<(), AbsoluteHeightError> {
    for (value, name) in [
        (input.containing_height, "containing_height"),
        (input.border_top, "border_top"),
        (input.padding_top, "padding_top"),
        (input.padding_bottom, "padding_bottom"),
        (input.border_bottom, "border_bottom"),
        (input.content_height, "content_height"),
        (input.min_height, "min_height"),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(AbsoluteHeightError::InvalidInput(name));
        }
    }
    if let Some(value) = input.max_height
        && (!value.is_finite() || value < 0.0)
    {
        return Err(AbsoluteHeightError::InvalidInput("max_height"));
    }
    if let AutoLength::Value(value) = input.height
        && (!value.is_finite() || value < 0.0)
    {
        return Err(AbsoluteHeightError::InvalidInput("height"));
    }
    for (value, name) in [
        (input.top, "top"),
        (input.bottom, "bottom"),
        (input.margin_top, "margin_top"),
        (input.margin_bottom, "margin_bottom"),
        (AutoLength::Value(input.static_top), "static_top"),
    ] {
        if let AutoLength::Value(value) = value
            && !value.is_finite()
        {
            return Err(AbsoluteHeightError::InvalidInput(name));
        }
    }
    Ok(())
}
fn exact(value: AutoLength) -> Option<Exact> {
    match value {
        AutoLength::Auto => None,
        AutoLength::Value(value) => Some(Exact::from_f32(value)),
    }
}
struct Tentative {
    top: Exact,
    height: Exact,
    bottom: Exact,
    margin_top: Exact,
    margin_bottom: Exact,
}
fn calculate(input: &AbsoluteHeightInput, height: AutoLength) -> Tentative {
    let available = Exact::from_f32(input.containing_height)
        .sub(Exact::from_f32(input.border_top))
        .sub(Exact::from_f32(input.padding_top))
        .sub(Exact::from_f32(input.padding_bottom))
        .sub(Exact::from_f32(input.border_bottom));
    let (mut top, height, bottom) = (exact(input.top), exact(height), exact(input.bottom));
    let (margin_top, margin_bottom) = (exact(input.margin_top), exact(input.margin_bottom));
    if top.is_none() && height.is_none() && bottom.is_none() {
        top = Some(Exact::from_f32(input.static_top));
    }
    if let (Some(top), Some(height), Some(mut bottom)) = (top, height, bottom) {
        let remainder = available.sub(top).sub(height).sub(bottom);
        let (margin_top, margin_bottom) = match (margin_top, margin_bottom) {
            (None, None) => {
                let half = remainder.half();
                (half, half)
            }
            (None, Some(bottom)) => (remainder.sub(bottom), bottom),
            (Some(top), None) => (top, remainder.sub(top)),
            (Some(margin_top), Some(margin_bottom)) => {
                bottom = available
                    .sub(top)
                    .sub(height)
                    .sub(margin_top)
                    .sub(margin_bottom);
                (margin_top, margin_bottom)
            }
        };
        return Tentative {
            top,
            height,
            bottom,
            margin_top,
            margin_bottom,
        };
    }
    let (margin_top, margin_bottom) = (
        margin_top.unwrap_or(Exact::ZERO),
        margin_bottom.unwrap_or(Exact::ZERO),
    );
    let available = available.sub(margin_top).sub(margin_bottom);
    let content = Exact::from_f32(input.content_height);
    let (top, height, bottom) = match (top, height, bottom) {
        (None, None, Some(bottom)) => (available.sub(content).sub(bottom), content, bottom),
        (None, Some(height), None) => {
            let top = Exact::from_f32(input.static_top);
            (top, height, available.sub(top).sub(height))
        }
        (Some(top), None, None) => (top, content, available.sub(top).sub(content)),
        (None, Some(height), Some(bottom)) => (available.sub(height).sub(bottom), height, bottom),
        (Some(top), None, Some(bottom)) => (top, available.sub(top).sub(bottom), bottom),
        (Some(top), Some(height), None) => (top, height, available.sub(top).sub(height)),
        _ => unreachable!("all-auto and no-auto triples were handled before partial-auto cases"),
    };
    Tentative {
        top,
        height,
        bottom,
        margin_top,
        margin_bottom,
    }
}
pub fn absolute_height(
    input: AbsoluteHeightInput,
) -> Result<AbsoluteHeightResult, AbsoluteHeightError> {
    validate(&input)?;
    let mut value = calculate(&input, input.height);
    let mut passes = 1;
    if let Some(maximum) = input.max_height
        && value.height > Exact::from_f32(maximum)
    {
        value = calculate(&input, AutoLength::Value(maximum));
        passes += 1;
    }
    if value.height < Exact::from_f32(input.min_height) {
        value = calculate(&input, AutoLength::Value(input.min_height));
        passes += 1;
    }
    let rounded = |value: Exact| {
        value
            .to_f32()
            .map_err(|_| AbsoluteHeightError::Unrepresentable)
    };
    Ok(AbsoluteHeightResult {
        top: rounded(value.top)?,
        height: rounded(value.height)?,
        bottom: rounded(value.bottom)?,
        margin_top: rounded(value.margin_top)?,
        margin_bottom: rounded(value.margin_bottom)?,
        passes,
    })
}
