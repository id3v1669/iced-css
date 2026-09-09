//! CSS22 10.3.7/10.4 horizontal sizing for absolutely positioned non-replaced boxes.
//! Intrinsic widths and static positions are supplied by their owning algorithms.
use super::{
    arithmetic::Exact,
    block::{AutoLength, Direction},
};

#[derive(Debug, Clone, Copy)]
pub struct AbsoluteWidthInput {
    pub containing_width: f32,
    pub containing_direction: Direction,
    /// Direction of the element establishing the static-position containing block.
    pub static_direction: Direction,
    pub static_left: f32,
    pub static_right: f32,
    pub left: AutoLength,
    pub width: AutoLength,
    pub right: AutoLength,
    pub margin_left: AutoLength,
    pub margin_right: AutoLength,
    pub border_left: f32,
    pub padding_left: f32,
    pub padding_right: f32,
    pub border_right: f32,
    pub preferred_min_width: f32,
    pub preferred_width: f32,
    pub min_width: f32,
    pub max_width: Option<f32>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsoluteWidthResult {
    pub left: f32,
    pub width: f32,
    pub right: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub passes: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsoluteWidthError {
    InvalidInput(&'static str),
    Unrepresentable,
}
impl std::fmt::Display for AbsoluteWidthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "absolute width: {self:?}")
    }
}
impl std::error::Error for AbsoluteWidthError {}

fn validate(input: &AbsoluteWidthInput) -> Result<(), AbsoluteWidthError> {
    for (value, name) in [
        (input.containing_width, "containing_width"),
        (input.border_left, "border_left"),
        (input.border_right, "border_right"),
        (input.padding_left, "padding_left"),
        (input.padding_right, "padding_right"),
        (input.preferred_min_width, "preferred_min_width"),
        (input.preferred_width, "preferred_width"),
        (input.min_width, "min_width"),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(AbsoluteWidthError::InvalidInput(name));
        }
    }
    if input.preferred_min_width > input.preferred_width {
        return Err(AbsoluteWidthError::InvalidInput("intrinsic_width_order"));
    }
    if let Some(value) = input.max_width
        && (!value.is_finite() || value < 0.0)
    {
        return Err(AbsoluteWidthError::InvalidInput("max_width"));
    }
    if let AutoLength::Value(value) = input.width
        && (!value.is_finite() || value < 0.0)
    {
        return Err(AbsoluteWidthError::InvalidInput("width"));
    }
    for (value, name) in [
        (input.left, "left"),
        (input.right, "right"),
        (input.margin_left, "margin_left"),
        (input.margin_right, "margin_right"),
        (AutoLength::Value(input.static_left), "static_left"),
        (AutoLength::Value(input.static_right), "static_right"),
    ] {
        if let AutoLength::Value(value) = value
            && !value.is_finite()
        {
            return Err(AbsoluteWidthError::InvalidInput(name));
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
    left: Exact,
    width: Exact,
    right: Exact,
    margin_left: Exact,
    margin_right: Exact,
}
fn calculate(input: &AbsoluteWidthInput, width: AutoLength) -> Tentative {
    let available = Exact::from_f32(input.containing_width)
        .sub(Exact::from_f32(input.border_left))
        .sub(Exact::from_f32(input.padding_left))
        .sub(Exact::from_f32(input.padding_right))
        .sub(Exact::from_f32(input.border_right));
    let (mut left, width, mut right) = (exact(input.left), exact(width), exact(input.right));
    let (margin_left, margin_right) = (exact(input.margin_left), exact(input.margin_right));
    if left.is_none() && width.is_none() && right.is_none() {
        match input.static_direction {
            Direction::Ltr => left = Some(Exact::from_f32(input.static_left)),
            Direction::Rtl => right = Some(Exact::from_f32(input.static_right)),
        }
    }
    if let (Some(mut left), Some(width), Some(mut right)) = (left, width, right) {
        let remainder = available.sub(left).sub(width).sub(right);
        let (margin_left, margin_right) = match (margin_left, margin_right) {
            (None, None) if remainder >= Exact::ZERO => {
                let half = remainder.half();
                (half, half)
            }
            (None, None) => match input.containing_direction {
                Direction::Ltr => (Exact::ZERO, remainder),
                Direction::Rtl => (remainder, Exact::ZERO),
            },
            (None, Some(right)) => (remainder.sub(right), right),
            (Some(left), None) => (left, remainder.sub(left)),
            (Some(margin_left), Some(margin_right)) => {
                let remaining = available.sub(width).sub(margin_left).sub(margin_right);
                match input.containing_direction {
                    Direction::Ltr => right = remaining.sub(left),
                    Direction::Rtl => left = remaining.sub(right),
                }
                (margin_left, margin_right)
            }
        };
        return Tentative {
            left,
            width,
            right,
            margin_left,
            margin_right,
        };
    }
    let (margin_left, margin_right) = (
        margin_left.unwrap_or(Exact::ZERO),
        margin_right.unwrap_or(Exact::ZERO),
    );
    let available = available.sub(margin_left).sub(margin_right);
    let shrink = |space: Exact| {
        space
            .max(Exact::from_f32(input.preferred_min_width))
            .min(Exact::from_f32(input.preferred_width))
    };
    let (left, width, right) = match (left, width, right) {
        (None, None, Some(right)) => {
            let width = shrink(available.sub(right));
            (available.sub(width).sub(right), width, right)
        }
        (None, Some(width), None) => match input.static_direction {
            Direction::Ltr => {
                let left = Exact::from_f32(input.static_left);
                (left, width, available.sub(left).sub(width))
            }
            Direction::Rtl => {
                let right = Exact::from_f32(input.static_right);
                (available.sub(width).sub(right), width, right)
            }
        },
        (Some(left), None, None) => {
            let width = shrink(available.sub(left));
            (left, width, available.sub(left).sub(width))
        }
        (None, Some(width), Some(right)) => (available.sub(width).sub(right), width, right),
        (Some(left), None, Some(right)) => (left, available.sub(left).sub(right), right),
        (Some(left), Some(width), None) => (left, width, available.sub(left).sub(width)),
        _ => unreachable!("all-auto and no-auto triples were handled before partial-auto cases"),
    };
    Tentative {
        left,
        width,
        right,
        margin_left,
        margin_right,
    }
}

pub fn absolute_width(
    input: AbsoluteWidthInput,
) -> Result<AbsoluteWidthResult, AbsoluteWidthError> {
    validate(&input)?;
    let mut value = calculate(&input, input.width);
    let mut passes = 1;
    if let Some(maximum) = input.max_width
        && value.width > Exact::from_f32(maximum)
    {
        value = calculate(&input, AutoLength::Value(maximum));
        passes += 1;
    }
    if value.width < Exact::from_f32(input.min_width) {
        value = calculate(&input, AutoLength::Value(input.min_width));
        passes += 1;
    }
    let rounded = |value: Exact| {
        value
            .to_f32()
            .map_err(|_| AbsoluteWidthError::Unrepresentable)
    };
    Ok(AbsoluteWidthResult {
        left: rounded(value.left)?,
        width: rounded(value.width)?,
        right: rounded(value.right)?,
        margin_left: rounded(value.margin_left)?,
        margin_right: rounded(value.margin_right)?,
        passes,
    })
}
