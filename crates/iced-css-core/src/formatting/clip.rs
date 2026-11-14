//! CSS22 11.1.2 used clip rectangles and common-coordinate intersections.
//! Ancestor eligibility, transforms, raster rounding and native layers are separate.
//!
//! Not yet integrated: the `iced-css` `BoxTree` widget does not call this module; it
//! resolves and intersects clip rectangles with its own code.
use super::display::Position;
use crate::values::{
    LengthTarget, NumericLimits,
    geometry::{LengthContext, LengthEvaluation, LengthEvaluationError, evaluate_length},
    property::SpecifiedValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipError {
    Value,
    Percentage,
    BorderSize,
    Coordinate,
    Length(LengthEvaluationError),
}
impl std::fmt::Display for ClipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSS clip geometry: {self:?}")
    }
}
impl std::error::Error for ClipError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Length(error) => Some(error),
            _ => None,
        }
    }
}
/// Finite edges in left/top/right/bottom order. Positive width/height are retained
/// as edge relationships; their differences need not be representable as f32.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipRectangle {
    edges: [f32; 4],
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClipRegion {
    Unbounded,
    Empty,
    Rectangle(ClipRectangle),
}
impl ClipRegion {
    pub fn from_edges(edges: [f32; 4]) -> Result<Self, ClipError> {
        if edges.iter().any(|v| !v.is_finite()) {
            return Err(ClipError::Coordinate);
        }
        Ok(Self::finite_edges(edges))
    }
    fn finite_edges(mut edges: [f32; 4]) -> Self {
        if edges[0] >= edges[2] || edges[1] >= edges[3] {
            return Self::Empty;
        }
        for value in &mut edges {
            if *value == 0.0 {
                *value = 0.0;
            }
        }
        Self::Rectangle(ClipRectangle { edges })
    }
    /// None for both unbounded and empty; inspect the variant to distinguish them.
    pub fn edges(self) -> Option<[f32; 4]> {
        match self {
            Self::Rectangle(rect) => Some(rect.edges),
            _ => None,
        }
    }
    /// Both inputs must already use the same coordinate system. This does not
    /// determine whether an ancestor's overflow clip applies to a positioned child.
    pub fn intersect(self, other: Self) -> Self {
        match (self, other) {
            (Self::Empty, _) | (_, Self::Empty) => Self::Empty,
            (Self::Unbounded, region) | (region, Self::Unbounded) => region,
            (Self::Rectangle(a), Self::Rectangle(b)) => Self::finite_edges([
                a.edges[0].max(b.edges[0]),
                a.edges[1].max(b.edges[1]),
                a.edges[2].min(b.edges[2]),
                a.edges[3].min(b.edges[3]),
            ]),
        }
    }
}
/// Resolve a cascaded clip value in local border-box coordinates. `border_size`
/// contains complete used border-box width/height, not content-box dimensions.
/// `length_limits` apply separately to each of the at most four operands.
pub fn resolve_clip(
    value: &SpecifiedValue,
    position: Position,
    border_size: [f32; 2],
    context: LengthContext,
    length_limits: NumericLimits,
) -> Result<ClipRegion, ClipError> {
    if border_size.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(ClipError::BorderSize);
    }
    let operands = match value {
        SpecifiedValue::Keyword(keyword) if keyword.as_str() == "auto" => {
            return Ok(ClipRegion::Unbounded);
        }
        SpecifiedValue::ClipRect(operands) => operands,
        _ => return Err(ClipError::Value),
    };
    if operands
        .iter()
        .any(|v| matches!(v, LengthTarget::Percentage(_)))
    {
        return Err(ClipError::Percentage);
    }
    if !matches!(position, Position::Absolute | Position::Fixed) {
        return Ok(ClipRegion::Unbounded);
    }
    // CSS operand order is top/right/bottom/left, unlike the stored edge order.
    let mut resolved = [0.0, border_size[0], border_size[1], 0.0];
    for (operand, edge) in operands.iter().zip(&mut resolved) {
        if let LengthEvaluation::Value(value) =
            evaluate_length(operand, context, length_limits).map_err(ClipError::Length)?
        {
            *edge = value.value;
        }
    }
    ClipRegion::from_edges([resolved[3], resolved[0], resolved[1], resolved[2]])
}

/// Resolve an already computed or sampled clip. Auto sides use the complete
/// used border-box width/height; whole auto remains unbounded.
pub fn resolve_computed_clip(
    value: crate::values::computed_clip::ComputedClip,
    position: Position,
    border_size: [f32; 2],
) -> Result<ClipRegion, ClipError> {
    if border_size.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(ClipError::BorderSize);
    }
    let Some(sides) = value.sides() else {
        return Ok(ClipRegion::Unbounded);
    };
    if !matches!(position, Position::Absolute | Position::Fixed) {
        return Ok(ClipRegion::Unbounded);
    }
    let defaults = [0.0, border_size[0], border_size[1], 0.0];
    let sides: [f32; 4] = std::array::from_fn(|i| sides[i].unwrap_or(defaults[i]));
    ClipRegion::from_edges([sides[3], sides[0], sides[1], sides[2]])
}
