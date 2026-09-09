//! CSS-owned combination primitives from the retained animation/value references.
use crate::values::property::{ParsedProperty, SpecifiedValue};
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Combination {
    Interpolate(f32),
    Add,
    Accumulate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    Start,
    End,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombinationError {
    NonfiniteProgress,
    UnresolvedVisibility,
    UnresolvedFontStyle,
    InvalidFontStyleAngle,
}
impl std::fmt::Display for CombinationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSS animation combination: {self:?}")
    }
}
impl std::error::Error for CombinationError {}
/// Discrete values are not additive. Accumulation without a separate procedure
/// follows addition, whose non-additive fallback retains the first operand.
pub fn discrete_choice(operation: Combination) -> Result<Endpoint, CombinationError> {
    match operation {
        Combination::Interpolate(progress) => {
            if !progress.is_finite() {
                return Err(CombinationError::NonfiniteProgress);
            }
            Ok(if progress < 0.5 {
                Endpoint::Start
            } else {
                Endpoint::End
            })
        }
        Combination::Add | Combination::Accumulate => Ok(Endpoint::Start),
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}
impl TryFrom<&ParsedProperty> for Visibility {
    type Error = CombinationError;
    fn try_from(parsed: &ParsedProperty) -> Result<Self, Self::Error> {
        if parsed.property.descriptor().name != "visibility" {
            return Err(CombinationError::UnresolvedVisibility);
        }
        let SpecifiedValue::Keyword(keyword) = &parsed.value else {
            return Err(CombinationError::UnresolvedVisibility);
        };
        match keyword.as_str() {
            "visible" => Ok(Self::Visible),
            "hidden" => Ok(Self::Hidden),
            "collapse" => Ok(Self::Collapse),
            _ => Err(CombinationError::UnresolvedVisibility),
        }
    }
}
pub fn combine_visibility(
    start: Visibility,
    end: Visibility,
    operation: Combination,
) -> Result<Visibility, CombinationError> {
    let fallback = discrete_choice(operation)?;
    if let Combination::Interpolate(progress) = operation
        && progress > 0.0
        && progress < 1.0
        && (start == Visibility::Visible || end == Visibility::Visible)
    {
        return Ok(Visibility::Visible);
    }
    Ok(match fallback {
        Endpoint::Start => start,
        Endpoint::End => end,
    })
}
