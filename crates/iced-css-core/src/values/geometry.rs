//! Evaluate a length only when the property owner supplies its required basis.
//! Computed/used staging, percentage inheritance and indefinite-basis policy are
//! not selected here. No property-specific sign restriction is waived.
use super::{ConversionError, FiniteF32, LengthTarget, NumericLimits, RelativeUnit};

#[derive(Debug, Clone, Copy, Default)]
pub struct LengthContext {
    pub font_size: Option<f32>,
    pub x_height: Option<f32>,
    pub percentage_basis: Option<f32>,
    /// The root element's computed font size, for `rem`.
    pub root_font_size: Option<f32>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthEvaluation {
    Auto,
    Value(FiniteF32),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthBasis {
    FontSize,
    XHeight,
    Percentage,
    RootFontSize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthEvaluationError {
    MissingContext(LengthBasis),
    InvalidFontContext,
    Conversion(ConversionError),
}
impl From<ConversionError> for LengthEvaluationError {
    fn from(error: ConversionError) -> Self {
        Self::Conversion(error)
    }
}
impl std::fmt::Display for LengthEvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "geometry length evaluation: {self:?}")
    }
}
impl std::error::Error for LengthEvaluationError {}

pub fn evaluate_length(
    value: &LengthTarget,
    context: LengthContext,
    limits: NumericLimits,
) -> Result<LengthEvaluation, LengthEvaluationError> {
    let evaluated = match value {
        LengthTarget::Auto => return Ok(LengthEvaluation::Auto),
        LengthTarget::Absolute(value) => value.to_finite_f32(limits)?,
        LengthTarget::Percentage(value) => {
            let basis = context
                .percentage_basis
                .ok_or(LengthEvaluationError::MissingContext(
                    LengthBasis::Percentage,
                ))?;
            value.percentage_finite_f32(basis, limits)?
        }
        LengthTarget::Relative(value, unit) => {
            let (basis, kind) = match unit {
                RelativeUnit::Em => (context.font_size, LengthBasis::FontSize),
                RelativeUnit::Ex => (context.x_height, LengthBasis::XHeight),
                RelativeUnit::Rem => (context.root_font_size, LengthBasis::RootFontSize),
                // No font is measured for "0": half the font size.
                RelativeUnit::Ch => (
                    context.font_size.map(|size| size * 0.5),
                    LengthBasis::FontSize,
                ),
            };
            let basis = basis.ok_or(LengthEvaluationError::MissingContext(kind))?;
            if !basis.is_finite() || basis < 0.0 {
                return Err(LengthEvaluationError::InvalidFontContext);
            }
            value.scaled_finite_f32(basis, limits)?
        }
    };
    Ok(LengthEvaluation::Value(evaluated))
}
