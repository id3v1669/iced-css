//! Pair animation preserving computed normal, exact number and absolute length.
use super::{
    combine::{Combination, Endpoint, discrete_choice},
    number::{Clamp, CombinedNumber, NumberError, combine_number},
};
use crate::values::{
    Decimal, DecimalArithmeticError, NumericError, NumericLimits,
    line_height::{ComputedLineHeight, LineHeightError, LineHeightKindRef},
};
#[derive(Debug, PartialEq, Eq)]
pub enum LineHeightAnimationError {
    NonfiniteProgress,
    Numeric(NumericError),
    Decimal(DecimalArithmeticError),
    Number(NumberError),
    Computed(LineHeightError),
}
impl std::fmt::Display for LineHeightAnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line-height animation: {self:?}")
    }
}
impl std::error::Error for LineHeightAnimationError {}
#[derive(Debug)]
enum Pair<'a> {
    Borrowed(&'a ComputedLineHeight),
    Number(Decimal),
    Absolute(CombinedNumber),
}
#[derive(Debug)]
pub struct CombinedLineHeight<'a>(Pair<'a>);
#[derive(Debug)]
enum Resolved<'a> {
    Borrowed(&'a ComputedLineHeight),
    Owned(ComputedLineHeight),
}
/// A final computed value; exact factors still resolve against the using font.
#[derive(Debug)]
pub struct LineHeightSample<'a> {
    value: Resolved<'a>,
    pub clamped: Option<Clamp>,
}
impl LineHeightSample<'_> {
    pub fn computed(&self) -> &ComputedLineHeight {
        match &self.value {
            Resolved::Borrowed(value) => value,
            Resolved::Owned(value) => value,
        }
    }
}
impl<'a> CombinedLineHeight<'a> {
    /// Final numeric factor clamp is nonnegative, with no length maximum applied
    /// to dimensionless values. Absolute results use the caller's finite maximum.
    /// Limits govern constructing exact zero if a factor needs a lower clamp.
    pub fn resolve(
        self,
        absolute_maximum: f32,
        limits: NumericLimits,
    ) -> Result<LineHeightSample<'a>, LineHeightAnimationError> {
        if !absolute_maximum.is_finite() || absolute_maximum < 0.0 {
            return Err(LineHeightAnimationError::Number(NumberError::InvalidRange));
        }
        let (value, clamped) = match self.0 {
            Pair::Borrowed(value) => {
                if let LineHeightKindRef::Absolute(pixels) = value.animation_kind()
                    && pixels > absolute_maximum
                {
                    (
                        Resolved::Owned(
                            ComputedLineHeight::from_animation_absolute(absolute_maximum)
                                .map_err(LineHeightAnimationError::Computed)?,
                        ),
                        Some(Clamp::Upper),
                    )
                } else {
                    (Resolved::Borrowed(value), None)
                }
            }
            Pair::Number(value) => {
                let clamped = value.is_negative();
                let value = if clamped {
                    Decimal::parse("0", limits).map_err(LineHeightAnimationError::Numeric)?
                } else {
                    value
                };
                (
                    Resolved::Owned(
                        ComputedLineHeight::from_animation_number(value)
                            .map_err(LineHeightAnimationError::Computed)?,
                    ),
                    clamped.then_some(Clamp::Lower),
                )
            }
            Pair::Absolute(value) => {
                let sample = value
                    .resolve(0.0, absolute_maximum)
                    .map_err(LineHeightAnimationError::Number)?;
                (
                    Resolved::Owned(
                        ComputedLineHeight::from_animation_absolute(sample.value)
                            .map_err(LineHeightAnimationError::Computed)?,
                    ),
                    sample.clamped,
                )
            }
        };
        Ok(LineHeightSample { value, clamped })
    }
}
/// Combine compatible numeric types without early endpoint conversion. Incompatible
/// computed types use the whole-value discrete rule and borrow its chosen endpoint.
///
/// ```compile_fail
/// # use iced_css_core::{animation::{line_height::*,combine::Combination},values::{NumericLimits,line_height::ComputedLineHeight}};
/// fn escape(a:&ComputedLineHeight,b:&ComputedLineHeight,l:NumericLimits)->LineHeightSample<'static> {
///     combine_line_height(a,b,Combination::Add,l).unwrap().resolve(100.0,l).unwrap()
/// }
/// ```
///
/// # Errors
/// Rejects nonfinite progress or exhausted exact-decimal arithmetic limits.
pub fn combine_line_height<'a>(
    start: &'a ComputedLineHeight,
    end: &'a ComputedLineHeight,
    operation: Combination,
    limits: NumericLimits,
) -> Result<CombinedLineHeight<'a>, LineHeightAnimationError> {
    if let Combination::Interpolate(p) = operation
        && !p.is_finite()
    {
        return Err(LineHeightAnimationError::NonfiniteProgress);
    }
    let value = match (start.animation_kind(), end.animation_kind()) {
        (LineHeightKindRef::Number(a), LineHeightKindRef::Number(b)) => {
            Pair::Number(match operation {
                Combination::Interpolate(p) => a
                    .checked_interpolate(b, p, limits)
                    .map_err(LineHeightAnimationError::Decimal)?,
                Combination::Add | Combination::Accumulate => a
                    .checked_add(b, limits)
                    .map_err(LineHeightAnimationError::Numeric)?,
            })
        }
        (LineHeightKindRef::Absolute(a), LineHeightKindRef::Absolute(b)) => Pair::Absolute(
            combine_number(a, b, operation).map_err(LineHeightAnimationError::Number)?,
        ),
        _ => Pair::Borrowed(
            match discrete_choice(operation).expect("progress was validated") {
                Endpoint::Start => start,
                Endpoint::End => end,
            },
        ),
    };
    Ok(CombinedLineHeight(value))
}
