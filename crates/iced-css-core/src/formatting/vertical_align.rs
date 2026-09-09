use super::{Exact, InlineBounds, LineAlignment, ResolvedRaise};
use crate::values::{
    LengthTarget, NumericLimits,
    geometry::{LengthContext, LengthEvaluation, LengthEvaluationError, evaluate_length},
    property::SpecifiedValue,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct VerticalAlignContext {
    pub font_size: Option<f32>,
    pub x_height: Option<f32>,
    /// The using element's used line-height, not its parent's line-height.
    pub line_height: Option<f32>,
    pub parent_x_height: Option<f32>,
    pub parent_content: Option<[f32; 2]>,
    /// Font-owner supplied proper lowering/raise; no font-size change is made.
    pub subscript_lowering: Option<f32>,
    pub superscript_raise: Option<f32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalMetric {
    LineHeight,
    ParentXHeight,
    ParentContent,
    SubscriptLowering,
    SuperscriptRaise,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignError {
    Value,
    Missing(VerticalMetric),
    Invalid(VerticalMetric),
    Length(LengthEvaluationError),
}
impl std::fmt::Display for VerticalAlignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "inline vertical-align: {self:?}")
    }
}
impl std::error::Error for VerticalAlignError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Length(error) => Some(error),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct ResolvedVerticalAlign {
    pub alignment: LineAlignment,
    /// Forwarded signed-length conversion notice for the diagnostic owner.
    pub rounded_to_zero: bool,
}
fn metric(value: Option<f32>, kind: VerticalMetric) -> Result<f32, VerticalAlignError> {
    let value = value.ok_or(VerticalAlignError::Missing(kind))?;
    if !value.is_finite() || value < 0.0 {
        return Err(VerticalAlignError::Invalid(kind));
    }
    Ok(value)
}
fn content(value: Option<[f32; 2]>) -> Result<[Exact; 2], VerticalAlignError> {
    let [top, bottom] = value.ok_or(VerticalAlignError::Missing(VerticalMetric::ParentContent))?;
    if !top.is_finite() || !bottom.is_finite() || top > bottom {
        return Err(VerticalAlignError::Invalid(VerticalMetric::ParentContent));
    }
    Ok([Exact::from_f32(top), Exact::from_f32(bottom)])
}
fn raised(value: Exact) -> LineAlignment {
    LineAlignment::Resolved(ResolvedRaise(value))
}
/// Resolve one inline's local vertical alignment against supplied parent metrics.
/// Tree flattening must separately compose parent coordinate transforms.
///
/// # Errors
/// Rejects unresolved values and missing/invalid context actually used by this
/// property. Numeric conversions retain their typed error and zero-rounding flag.
pub fn resolve_vertical_align(
    value: &SpecifiedValue,
    bounds: InlineBounds,
    context: VerticalAlignContext,
    limits: NumericLimits,
) -> Result<ResolvedVerticalAlign, VerticalAlignError> {
    if let SpecifiedValue::Length(length) = value {
        let percentage_basis = if matches!(length, LengthTarget::Percentage(_)) {
            Some(metric(context.line_height, VerticalMetric::LineHeight)?)
        } else {
            None
        };
        let evaluated = evaluate_length(
            length,
            LengthContext {
                font_size: context.font_size,
                x_height: context.x_height,
                percentage_basis,
            },
            limits,
        )
        .map_err(VerticalAlignError::Length)?;
        let LengthEvaluation::Value(evaluated) = evaluated else {
            return Err(VerticalAlignError::Value);
        };
        return Ok(ResolvedVerticalAlign {
            alignment: LineAlignment::Baseline {
                raise: evaluated.value,
            },
            rounded_to_zero: evaluated.rounded_to_zero,
        });
    }
    let SpecifiedValue::Keyword(keyword) = value else {
        return Err(VerticalAlignError::Value);
    };
    resolve_keyword(keyword.as_str(), bounds, context)
}
fn resolve_keyword(
    keyword: &str,
    bounds: InlineBounds,
    context: VerticalAlignContext,
) -> Result<ResolvedVerticalAlign, VerticalAlignError> {
    let alignment = match keyword {
        "baseline" => LineAlignment::Baseline { raise: 0.0 },
        "top" => LineAlignment::Top,
        "bottom" => LineAlignment::Bottom,
        "sub" => raised(Exact::ZERO.sub(Exact::from_f32(metric(
            context.subscript_lowering,
            VerticalMetric::SubscriptLowering,
        )?))),
        "super" => raised(Exact::from_f32(metric(
            context.superscript_raise,
            VerticalMetric::SuperscriptRaise,
        )?)),
        "middle" => {
            let x = Exact::from_f32(metric(
                context.parent_x_height,
                VerticalMetric::ParentXHeight,
            )?);
            // Font bounds sum to D-A; explicit edges are original f32 values.
            // Their sum always has an even quantum, even with subnormal leading.
            raised(bounds.top.add(bounds.bottom).half().add(x.half()))
        }
        "text-top" => raised(bounds.top.sub(content(context.parent_content)?[0])),
        "text-bottom" => raised(bounds.bottom.sub(content(context.parent_content)?[1])),
        _ => return Err(VerticalAlignError::Value),
    };
    Ok(ResolvedVerticalAlign {
        alignment,
        rounded_to_zero: false,
    })
}

/// Already-computed pixel raises are not re-scaled against later font/line metrics.
/// This stage makes no new conversion notice; carry earlier notices separately.
pub fn resolve_computed_vertical_align(
    value: crate::values::vertical_align::ComputedVerticalAlign,
    bounds: InlineBounds,
    context: VerticalAlignContext,
) -> Result<ResolvedVerticalAlign, VerticalAlignError> {
    use crate::values::vertical_align::VerticalAlignComponent;
    match value.component() {
        VerticalAlignComponent::Pixels(raise) => Ok(ResolvedVerticalAlign {
            alignment: LineAlignment::Baseline { raise },
            rounded_to_zero: false,
        }),
        VerticalAlignComponent::Keyword(k) => resolve_keyword(k.as_str(), bounds, context),
    }
}
