//! Vertical-align pair semantics after percentage/length computation.
use super::{
    combine::{Combination, Endpoint, discrete_choice},
    number::{Clamp, CombinedNumber, NumberError, combine_number},
};
use crate::values::vertical_align::{
    ComputedVerticalAlign, VerticalAlignComponent, VerticalAlignKeyword,
};
#[derive(Debug, Clone, Copy)]
enum Value {
    Pixels(CombinedNumber),
    Keyword(VerticalAlignKeyword),
}
#[derive(Debug, Clone, Copy)]
pub struct CombinedVerticalAlign(Value);
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VerticalAlignSample {
    pub value: ComputedVerticalAlign,
    pub clamped: Option<Clamp>,
    pub rounded_to_zero: bool,
}
impl CombinedVerticalAlign {
    pub fn resolve(self, minimum: f32, maximum: f32) -> Result<VerticalAlignSample, NumberError> {
        if !minimum.is_finite() || !maximum.is_finite() || minimum > maximum {
            return Err(NumberError::InvalidRange);
        }
        match self.0 {
            Value::Keyword(k) => Ok(VerticalAlignSample {
                value: ComputedVerticalAlign::keyword(k),
                clamped: None,
                rounded_to_zero: false,
            }),
            Value::Pixels(v) => {
                let (sample, rounded_to_zero) = v.resolve_with_underflow(minimum, maximum)?;
                Ok(VerticalAlignSample {
                    value: ComputedVerticalAlign::from_pixels(sample.value)
                        .expect("final numeric result is finite"),
                    clamped: sample.clamped,
                    rounded_to_zero,
                })
            }
        }
    }
}
pub fn combine_vertical_align(
    start: ComputedVerticalAlign,
    end: ComputedVerticalAlign,
    operation: Combination,
) -> Result<CombinedVerticalAlign, NumberError> {
    let selected = discrete_choice(operation).map_err(|_| NumberError::NonfiniteProgress)?;
    let value = match (start.component(), end.component()) {
        (VerticalAlignComponent::Pixels(a), VerticalAlignComponent::Pixels(b)) => {
            Value::Pixels(combine_number(a, b, operation)?)
        }
        _ => match (match selected {
            Endpoint::Start => start,
            Endpoint::End => end,
        })
        .component()
        {
            VerticalAlignComponent::Keyword(k) => Value::Keyword(k),
            VerticalAlignComponent::Pixels(v) => {
                Value::Pixels(combine_number(v, v, Combination::Interpolate(0.0))?)
            }
        },
    };
    Ok(CombinedVerticalAlign(value))
}
