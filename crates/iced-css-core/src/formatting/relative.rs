//! CSS22 9.4.3 relative positioning, after property-specific length resolution.
use super::block::{AutoLength, Direction};

#[derive(Debug, Clone, Copy)]
pub struct RelativeInput {
    pub left: AutoLength,
    pub right: AutoLength,
    pub top: AutoLength,
    pub bottom: AutoLength,
    pub containing_direction: Direction,
}
/// Used offsets; normal-flow placement and size are intentionally not inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativeOffsets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativeError {
    InvalidInput(&'static str),
}
impl std::fmt::Display for RelativeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "relative offsets: {self:?}")
    }
}
impl std::error::Error for RelativeError {}
fn zero(value: f32) -> f32 {
    if value == 0.0 { 0.0 } else { value }
}
fn axis(start: AutoLength, end: AutoLength, start_wins: bool) -> (f32, f32) {
    let start = match (start, end) {
        (AutoLength::Auto, AutoLength::Auto) => 0.0,
        (AutoLength::Auto, AutoLength::Value(end)) => -end,
        (AutoLength::Value(start), AutoLength::Auto) => start,
        (AutoLength::Value(start), AutoLength::Value(end)) => {
            if start_wins {
                start
            } else {
                -end
            }
        }
    };
    (zero(start), zero(-start))
}
pub fn relative_offsets(input: RelativeInput) -> Result<RelativeOffsets, RelativeError> {
    for (value, field) in [
        (input.left, "left"),
        (input.right, "right"),
        (input.top, "top"),
        (input.bottom, "bottom"),
    ] {
        if let AutoLength::Value(value) = value
            && !value.is_finite()
        {
            return Err(RelativeError::InvalidInput(field));
        }
    }
    let (left, right) = axis(
        input.left,
        input.right,
        input.containing_direction == Direction::Ltr,
    );
    let (top, bottom) = axis(input.top, input.bottom, true);
    Ok(RelativeOffsets {
        left,
        right,
        top,
        bottom,
    })
}
