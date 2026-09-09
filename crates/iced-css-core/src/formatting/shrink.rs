//! CSS22 10.3.5/10.3.9 and 10.4: non-replaced floats and normal-flow inline-blocks.
//! Intrinsic measurements are inputs; exclusion placement and text shaping are separate.
use super::{arithmetic::Exact, block::AutoLength};

#[derive(Debug, Clone, Copy)]
pub struct ShrinkWidthInput {
    pub containing_width: f32,
    pub width: AutoLength,
    pub margin_left: AutoLength,
    pub margin_right: AutoLength,
    pub border_left: f32,
    pub border_right: f32,
    pub padding_left: f32,
    pub padding_right: f32,
    /// Total relevant horizontal space occupied by scrollbars.
    pub scrollbar_width: f32,
    pub preferred_min_width: f32,
    pub preferred_width: f32,
    pub min_width: f32,
    pub max_width: Option<f32>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShrinkWidthResult {
    pub width: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub passes: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrinkWidthError {
    InvalidInput(&'static str),
    Unrepresentable,
}
impl std::fmt::Display for ShrinkWidthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "float/inline-block non-replaced width: {self:?}")
    }
}
impl std::error::Error for ShrinkWidthError {}
fn nonnegative(value: f32, field: &'static str) -> Result<(), ShrinkWidthError> {
    if !value.is_finite() || value < 0.0 {
        Err(ShrinkWidthError::InvalidInput(field))
    } else {
        Ok(())
    }
}
fn margin(value: AutoLength, field: &'static str) -> Result<f32, ShrinkWidthError> {
    match value {
        AutoLength::Auto => Ok(0.0),
        AutoLength::Value(value) if value.is_finite() => Ok(if value == 0.0 { 0.0 } else { value }),
        _ => Err(ShrinkWidthError::InvalidInput(field)),
    }
}
/// Exact available-width arithmetic followed by the category's max/min reruns.
/// Neither fixed nor auto margins absorb leftover containing-block space here.
pub fn solve_shrink_width(input: ShrinkWidthInput) -> Result<ShrinkWidthResult, ShrinkWidthError> {
    for (value, field) in [
        (input.containing_width, "containing_width"),
        (input.border_left, "border_left"),
        (input.border_right, "border_right"),
        (input.padding_left, "padding_left"),
        (input.padding_right, "padding_right"),
        (input.scrollbar_width, "scrollbar_width"),
        (input.preferred_min_width, "preferred_min_width"),
        (input.preferred_width, "preferred_width"),
        (input.min_width, "min_width"),
    ] {
        nonnegative(value, field)?;
    }
    if input.preferred_min_width > input.preferred_width {
        return Err(ShrinkWidthError::InvalidInput("intrinsic_width_order"));
    }
    if let AutoLength::Value(width) = input.width {
        nonnegative(width, "width")?;
    }
    if let Some(maximum) = input.max_width {
        nonnegative(maximum, "max_width")?;
    }
    let left = margin(input.margin_left, "margin_left")?;
    let right = margin(input.margin_right, "margin_right")?;
    let mut width = match input.width {
        AutoLength::Value(width) => Exact::from_f32(width),
        AutoLength::Auto => {
            let available = Exact::from_f32(input.containing_width)
                .sub(Exact::from_f32(left))
                .sub(Exact::from_f32(right))
                .sub(Exact::from_f32(input.border_left))
                .sub(Exact::from_f32(input.border_right))
                .sub(Exact::from_f32(input.padding_left))
                .sub(Exact::from_f32(input.padding_right))
                .sub(Exact::from_f32(input.scrollbar_width));
            available
                .max(Exact::from_f32(input.preferred_min_width))
                .min(Exact::from_f32(input.preferred_width))
        }
    };
    let mut passes = 1;
    // A rerun with a definite substituted width simply uses that width under
    // 10.3.5/10.3.9; margins stay fixed/zero and do not balance a box equation.
    if let Some(maximum) = input.max_width
        && width > Exact::from_f32(maximum)
    {
        width = Exact::from_f32(maximum);
        passes += 1;
    }
    if width < Exact::from_f32(input.min_width) {
        width = Exact::from_f32(input.min_width);
        passes += 1;
    }
    Ok(ShrinkWidthResult {
        width: width
            .to_f32()
            .map_err(|_| ShrinkWidthError::Unrepresentable)?,
        margin_left: left,
        margin_right: right,
        passes,
    })
}
