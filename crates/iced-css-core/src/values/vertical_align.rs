//! Computed vertical-align: absolute raises or retained keywords.
use super::{
    LengthTarget, NumericLimits,
    geometry::{LengthContext, LengthEvaluation, LengthEvaluationError, evaluate_length},
    property::{ParsedProperty, SpecifiedValue},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignKeyword {
    Baseline,
    Sub,
    Super,
    Top,
    Bottom,
    Middle,
    TextTop,
    TextBottom,
}
impl VerticalAlignKeyword {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Sub => "sub",
            Self::Super => "super",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Middle => "middle",
            Self::TextTop => "text-top",
            Self::TextBottom => "text-bottom",
        }
    }
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "baseline" => Self::Baseline,
            "sub" => Self::Sub,
            "super" => Self::Super,
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "middle" => Self::Middle,
            "text-top" => Self::TextTop,
            "text-bottom" => Self::TextBottom,
            _ => return None,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalAlignComponent {
    Pixels(f32),
    Keyword(VerticalAlignKeyword),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComputedVerticalAlign(VerticalAlignComponent);
impl ComputedVerticalAlign {
    pub fn from_pixels(value: f32) -> Result<Self, ComputedVerticalAlignError> {
        if !value.is_finite() {
            return Err(ComputedVerticalAlignError::NonfinitePixels);
        }
        Ok(Self(VerticalAlignComponent::Pixels(if value == 0.0 {
            0.0
        } else {
            value
        })))
    }
    pub const fn keyword(value: VerticalAlignKeyword) -> Self {
        Self(VerticalAlignComponent::Keyword(value))
    }
    pub fn component(self) -> VerticalAlignComponent {
        self.0
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct VerticalAlignComputationContext {
    pub font_size: Option<f32>,
    pub x_height: Option<f32>,
    /// Using element's used line-height, required only for percentage computation.
    pub line_height: Option<f32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputedVerticalAlignError {
    Unresolved,
    MissingLineHeight,
    InvalidLineHeight,
    NonfinitePixels,
    Length(LengthEvaluationError),
}
impl std::fmt::Display for ComputedVerticalAlignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "computed vertical-align: {self:?}")
    }
}
impl std::error::Error for ComputedVerticalAlignError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Length(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VerticalAlignComputation {
    pub value: ComputedVerticalAlign,
    pub rounded_to_zero: bool,
}
pub fn compute_vertical_align(
    property: &ParsedProperty,
    context: VerticalAlignComputationContext,
    limits: NumericLimits,
) -> Result<VerticalAlignComputation, ComputedVerticalAlignError> {
    if property.property.descriptor().name != "vertical-align" {
        return Err(ComputedVerticalAlignError::Unresolved);
    }
    if let SpecifiedValue::Keyword(keyword) = property.value {
        let keyword = VerticalAlignKeyword::from_name(keyword.as_str())
            .ok_or(ComputedVerticalAlignError::Unresolved)?;
        return Ok(VerticalAlignComputation {
            value: ComputedVerticalAlign::keyword(keyword),
            rounded_to_zero: false,
        });
    }
    let SpecifiedValue::Length(value) = &property.value else {
        return Err(ComputedVerticalAlignError::Unresolved);
    };
    let percentage_basis = if matches!(value, LengthTarget::Percentage(_)) {
        let height = context
            .line_height
            .ok_or(ComputedVerticalAlignError::MissingLineHeight)?;
        if !height.is_finite() || height < 0.0 {
            return Err(ComputedVerticalAlignError::InvalidLineHeight);
        }
        Some(height)
    } else {
        None
    };
    let LengthEvaluation::Value(value) = evaluate_length(
        value,
        LengthContext {
            font_size: context.font_size,
            x_height: context.x_height,
            percentage_basis,
        },
        limits,
    )
    .map_err(ComputedVerticalAlignError::Length)?
    else {
        return Err(ComputedVerticalAlignError::Unresolved);
    };
    Ok(VerticalAlignComputation {
        value: ComputedVerticalAlign::from_pixels(value.value)?,
        rounded_to_zero: value.rounded_to_zero,
    })
}
