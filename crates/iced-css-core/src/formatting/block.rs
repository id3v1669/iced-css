//! CSS22 10.3.3/10.4 for normal-flow non-replaced block horizontal sizing.
//! Inputs are already resolved computed f32 values; other sizing categories differ.
use super::arithmetic::Exact;
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AutoLength {
    Auto,
    Value(f32),
}
impl AutoLength {
    fn fixed_or_zero(self) -> Exact {
        match self {
            Self::Auto => Exact::ZERO,
            Self::Value(value) => Exact::from_f32(value),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Ltr,
    Rtl,
}
#[derive(Debug, Clone, Copy)]
pub struct BlockWidthInput {
    pub containing_width: f32,
    /// Direction of the containing block, not necessarily this element.
    pub direction: Direction,
    pub width: AutoLength,
    pub margin_left: AutoLength,
    pub margin_right: AutoLength,
    pub border_left: f32,
    pub border_right: f32,
    pub padding_left: f32,
    pub padding_right: f32,
    pub min_width: f32,
    pub max_width: Option<f32>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockWidthResult {
    pub width: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    /// Initial calculation plus any max/min category reruns (at most three).
    pub passes: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockWidthError {
    InvalidInput(&'static str),
    Unrepresentable,
}
impl std::fmt::Display for BlockWidthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "normal block width: {self:?}")
    }
}
impl std::error::Error for BlockWidthError {}
fn nonnegative(value: f32, field: &'static str) -> Result<(), BlockWidthError> {
    if !value.is_finite() || value < 0.0 {
        Err(BlockWidthError::InvalidInput(field))
    } else {
        Ok(())
    }
}
fn validate(input: &BlockWidthInput) -> Result<(), BlockWidthError> {
    for (value, field) in [
        (input.containing_width, "containing_width"),
        (input.border_left, "border_left"),
        (input.border_right, "border_right"),
        (input.padding_left, "padding_left"),
        (input.padding_right, "padding_right"),
        (input.min_width, "min_width"),
    ] {
        nonnegative(value, field)?;
    }
    if let Some(maximum) = input.max_width {
        nonnegative(maximum, "max_width")?;
    }
    if let AutoLength::Value(width) = input.width {
        nonnegative(width, "width")?;
    }
    for (margin, field) in [
        (input.margin_left, "margin_left"),
        (input.margin_right, "margin_right"),
    ] {
        if let AutoLength::Value(value) = margin
            && !value.is_finite()
        {
            return Err(BlockWidthError::InvalidInput(field));
        }
    }
    Ok(())
}
struct Tentative {
    width: Exact,
    left: Exact,
    right: Exact,
}
fn calculate(input: &BlockWidthInput, width: AutoLength) -> Tentative {
    let available = Exact::from_f32(input.containing_width)
        .sub(Exact::from_f32(input.border_left))
        .sub(Exact::from_f32(input.padding_left))
        .sub(Exact::from_f32(input.padding_right))
        .sub(Exact::from_f32(input.border_right));
    let mut left = input.margin_left;
    let mut right = input.margin_right;
    let AutoLength::Value(width) = width else {
        let left = left.fixed_or_zero();
        let right = right.fixed_or_zero();
        return Tentative {
            width: available.sub(left).sub(right),
            left,
            right,
        };
    };
    let width = Exact::from_f32(width);
    let remainder = available.sub(width);
    if remainder < left.fixed_or_zero().add(right.fixed_or_zero()) {
        if left == AutoLength::Auto {
            left = AutoLength::Value(0.0);
        }
        if right == AutoLength::Auto {
            right = AutoLength::Value(0.0);
        }
    }
    let (left, right) = match (left, right) {
        (AutoLength::Auto, AutoLength::Auto) => (remainder.half(), remainder.half()),
        (AutoLength::Auto, right) => {
            let right = right.fixed_or_zero();
            (remainder.sub(right), right)
        }
        (left, AutoLength::Auto) => {
            let left = left.fixed_or_zero();
            (left, remainder.sub(left))
        }
        (left, right) => match input.direction {
            Direction::Ltr => {
                let left = left.fixed_or_zero();
                (left, remainder.sub(left))
            }
            Direction::Rtl => {
                let right = right.fixed_or_zero();
                (remainder.sub(right), right)
            }
        },
    };
    Tentative { width, left, right }
}
/// Exact intermediate equations and comparisons, then nearest/ties-even finite
/// f32 used values. Min/max reruns never overwrite the supplied computed values.
pub fn solve_block_width(input: BlockWidthInput) -> Result<BlockWidthResult, BlockWidthError> {
    validate(&input)?;
    let mut result = calculate(&input, input.width);
    let mut passes = 1;
    if let Some(maximum) = input.max_width
        && result.width > Exact::from_f32(maximum)
    {
        result = calculate(&input, AutoLength::Value(maximum));
        passes += 1;
    }
    if result.width < Exact::from_f32(input.min_width) {
        result = calculate(&input, AutoLength::Value(input.min_width));
        passes += 1;
    }
    Ok(BlockWidthResult {
        width: result.width.to_f32()?,
        margin_left: result.left.to_f32()?,
        margin_right: result.right.to_f32()?,
        passes,
    })
}
