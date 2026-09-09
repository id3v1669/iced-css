//! CSS22 10.4 proportional min/max table for tentative replaced both-auto sizes.
//! Intrinsic measurement and subsequent category-specific margin solving are separate.
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintMode {
    Independent,
    /// Caller has established both-auto sizes and a usable intrinsic ratio.
    IntrinsicRatio,
}
#[derive(Debug, Clone, Copy)]
pub struct ReplacedConstraintInput {
    pub width: f32,
    pub height: f32,
    pub min_width: f32,
    pub max_width: Option<f32>,
    pub min_height: f32,
    pub max_height: Option<f32>,
    pub mode: ConstraintMode,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplacedConstraintResult {
    pub width: f32,
    pub height: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacedConstraintError {
    InvalidInput(&'static str),
    Unrepresentable,
}
impl std::fmt::Display for ReplacedConstraintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "replaced size constraints: {self:?}")
    }
}
impl std::error::Error for ReplacedConstraintError {}

use crate::values::float_ratio::Ratio;

#[derive(Clone, Copy)]
enum Size {
    Fixed(f32),
    Proportional(Ratio),
}
impl Size {
    fn at_least(ratio: Ratio, minimum: f32) -> Self {
        if ratio.compare_f32(minimum) == Ordering::Less {
            Self::Fixed(minimum)
        } else {
            Self::Proportional(ratio)
        }
    }
    fn at_most(ratio: Ratio, maximum: Option<f32>) -> Self {
        if let Some(maximum) = maximum
            && ratio.compare_f32(maximum) == Ordering::Greater
        {
            Self::Fixed(maximum)
        } else {
            Self::Proportional(ratio)
        }
    }
    fn round(self) -> Result<f32, ReplacedConstraintError> {
        match self {
            Self::Fixed(value) => Ok(if value == 0.0 { 0.0 } else { value }),
            Self::Proportional(ratio) => ratio
                .round()
                .map_err(|_| ReplacedConstraintError::Unrepresentable),
        }
    }
}
#[derive(Clone, Copy)]
enum Violation {
    Below,
    Within,
    Above(f32),
}
fn violation(value: f32, minimum: f32, maximum: Option<f32>) -> Violation {
    if value < minimum {
        Violation::Below
    } else if let Some(maximum) = maximum
        && value > maximum
    {
        Violation::Above(maximum)
    } else {
        Violation::Within
    }
}
fn independent(value: f32, minimum: f32, maximum: Option<f32>) -> f32 {
    let value = value.max(minimum);
    maximum.map_or(value, |max| value.min(max))
}
/// Apply the ratio table only in IntrinsicRatio mode with nonzero tentative
/// dimensions. The zero-size independent route is the approved project policy.
/// No allocation or input-sized iteration; binary32 rounding takes at most31 steps.
pub fn constrain_replaced_size(
    input: ReplacedConstraintInput,
) -> Result<ReplacedConstraintResult, ReplacedConstraintError> {
    for (value, field) in [
        (Some(input.width), "width"),
        (Some(input.height), "height"),
        (Some(input.min_width), "min_width"),
        (Some(input.min_height), "min_height"),
        (input.max_width, "max_width"),
        (input.max_height, "max_height"),
    ] {
        if value.is_some_and(|value| !value.is_finite() || value < 0.0) {
            return Err(ReplacedConstraintError::InvalidInput(field));
        }
    }
    let max_w = input.max_width.map(|v| v.max(input.min_width));
    let max_h = input.max_height.map(|v| v.max(input.min_height));
    let (w, h) = (input.width, input.height);
    let (width, height) = if input.mode == ConstraintMode::Independent || w == 0.0 || h == 0.0 {
        (
            Size::Fixed(independent(w, input.min_width, max_w)),
            Size::Fixed(independent(h, input.min_height, max_h)),
        )
    } else {
        let above_w = |bound| {
            (
                Size::Fixed(bound),
                Size::at_least(Ratio::product_divided(bound, h, w), input.min_height),
            )
        };
        let below_w = || {
            (
                Size::Fixed(input.min_width),
                Size::at_most(Ratio::product_divided(input.min_width, h, w), max_h),
            )
        };
        let above_h = |bound| {
            (
                Size::at_least(Ratio::product_divided(bound, w, h), input.min_width),
                Size::Fixed(bound),
            )
        };
        let below_h = || {
            (
                Size::at_most(Ratio::product_divided(input.min_height, w, h), max_w),
                Size::Fixed(input.min_height),
            )
        };
        use Violation::{Above, Below, Within};
        match (
            violation(w, input.min_width, max_w),
            violation(h, input.min_height, max_h),
        ) {
            (Within, Within) => (Size::Fixed(w), Size::Fixed(h)),
            (Above(bound), Within) => above_w(bound),
            (Below, Within) => below_w(),
            (Within, Above(bound)) => above_h(bound),
            (Within, Below) => below_h(),
            (Above(bw), Above(bh)) => {
                if Ratio::product_divided(bw, 1.0, w).compare(Ratio::product_divided(bh, 1.0, h))
                    != Ordering::Greater
                {
                    above_w(bw)
                } else {
                    above_h(bh)
                }
            }
            (Below, Below) => {
                if Ratio::product_divided(input.min_width, 1.0, w).compare(Ratio::product_divided(
                    input.min_height,
                    1.0,
                    h,
                )) != Ordering::Greater
                {
                    below_h()
                } else {
                    below_w()
                }
            }
            (Below, Above(bound)) => (Size::Fixed(input.min_width), Size::Fixed(bound)),
            (Above(bound), Below) => (Size::Fixed(bound), Size::Fixed(input.min_height)),
        }
    };
    Ok(ReplacedConstraintResult {
        width: width.round()?,
        height: height.round()?,
    })
}
