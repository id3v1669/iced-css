//! Exact specified-value primitives; property validation and f32 evaluation are separate.
pub mod bidi;
mod exact;
pub use exact::{
    BorderLengthView, CalcLength, CompactDecimal, ConversionError, CounterError, CounterLimits,
    CounterStyle, Decimal, DecimalArithmeticError, DecimalScalingError, ExactLength, FiniteF32,
    Integer, IntegerInterpolationError, LengthTarget, NonnegativeF32, RangeEvent, RelativeUnit,
    format_counter, format_counters,
};
mod calc;
pub mod custom;
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy)]
pub struct NumericLimits {
    pub digits: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericError {
    InvalidNumber,
    UnsupportedUnit,
    DigitLimit,
    WorkLimit,
    Allocation,
}
impl std::fmt::Display for NumericError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidNumber => "invalid CSS decimal",
            Self::UnsupportedUnit => "unsupported length unit or missing nonzero unit",
            Self::DigitLimit => "numeric digit budget exceeded",
            Self::WorkLimit => "numeric work budget exceeded",
            Self::Allocation => "numeric allocation failed",
        })
    }
}
impl std::error::Error for NumericError {}
struct Budget {
    remaining: usize,
}
impl Budget {
    fn new(limits: NumericLimits) -> Self {
        Self {
            remaining: limits.work.get(),
        }
    }
    fn spend(&mut self, amount: usize) -> Result<(), NumericError> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or(NumericError::WorkLimit)?;
        Ok(())
    }
}
pub mod border_width;
pub mod color;
pub mod content_computation;
pub mod display;
pub mod font_size;
pub mod geometry;
pub mod initial;
pub mod line_height;
pub mod property;
pub mod registry;

pub(crate) use exact::counter_scope;

pub(crate) mod float_ratio;
pub mod font_metrics;

pub use exact::font_weight;

pub mod border_spacing;

pub mod text_spacing;

pub mod z_index;

pub mod keyword;

pub mod computed_clip;

pub mod background_position;

pub mod box_length;

pub mod vertical_align;

pub mod text_align;

pub mod outline;

pub mod image;

pub mod computed_cursor;

pub mod computed_border_width;
pub mod computed_color;

pub mod text_keywords;
