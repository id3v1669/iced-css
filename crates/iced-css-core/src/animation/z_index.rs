//! Whole-value z-index animation; stacking-context placement remains separate.
use super::{
    combine::{Combination, Endpoint, discrete_choice},
    integer::combine_integer,
};
use crate::values::{IntegerInterpolationError, NumericLimits, z_index::ComputedZIndex};
#[derive(Debug)]
enum Value<'a> {
    Borrowed(&'a ComputedZIndex),
    Owned(ComputedZIndex),
}
#[derive(Debug)]
pub struct ZIndexSample<'a>(Value<'a>);
impl ZIndexSample<'_> {
    pub fn computed(&self) -> &ComputedZIndex {
        match &self.0 {
            Value::Borrowed(value) => value,
            Value::Owned(value) => value,
        }
    }
}
/// Integer pairs combine exactly; auto/integer pairs select a whole endpoint.
/// No machine-sized integer range or float conversion is introduced.
///
/// ```compile_fail
/// # use iced_css_core::{animation::{combine::Combination,z_index::*},values::{NumericLimits,z_index::ComputedZIndex}};
/// fn escape(a:&ComputedZIndex,b:&ComputedZIndex,l:NumericLimits)->ZIndexSample<'static> {
///     combine_z_index(a,b,Combination::Add,l).unwrap()
/// }
/// ```
///
/// # Errors
/// Rejects nonfinite progress and exact integer arithmetic resource failures.
pub fn combine_z_index<'a>(
    start: &'a ComputedZIndex,
    end: &'a ComputedZIndex,
    operation: Combination,
    limits: NumericLimits,
) -> Result<ZIndexSample<'a>, IntegerInterpolationError> {
    if let Combination::Interpolate(p) = operation
        && !p.is_finite()
    {
        return Err(IntegerInterpolationError::NonfiniteProgress);
    }
    let value = match (start, end) {
        (ComputedZIndex::Integer(a), ComputedZIndex::Integer(b)) => Value::Owned(
            ComputedZIndex::Integer(combine_integer(a, b, operation, limits)?),
        ),
        _ => Value::Borrowed(
            match discrete_choice(operation).expect("progress was validated") {
                Endpoint::Start => start,
                Endpoint::End => end,
            },
        ),
    };
    Ok(ZIndexSample(value))
}
