//! Bounded first-in-flow origin for caller-established noncollapsing membership.
//! The caller proves a nonreplaced normal block, absent clearance and applicability
//! of auto top margin's used zero. Scalars alone cannot establish those conditions.
use super::{arithmetic::Exact, block::AutoLength};

/// Upstream content top and actual computed child top-edge lengths.
#[derive(Debug, Clone, Copy)]
pub struct FirstChildOriginInput {
    pub containing_top: f32,
    pub margin_top: AutoLength,
    pub border_top: f32,
    pub padding_top: f32,
}
/// How the computed top margin produced its used value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopMarginResolution {
    AutoZero,
    Absolute,
}
/// Declared finite coordinate publication boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginCoordinateStage {
    BorderTop,
    ContentTop,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstChildOriginError {
    InvalidInput(&'static str),
    Unrepresentable { stage: OriginCoordinateStage },
}
impl std::fmt::Display for FirstChildOriginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "first child origin: {self:?}")
    }
}
impl std::error::Error for FirstChildOriginError {}
/// Used margin and the two separately rounded, finite coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstChildOriginPlacement {
    pub used_margin_top: f32,
    pub margin_resolution: TopMarginResolution,
    pub border_top: f32,
    pub content_top: f32,
}
/// Publish border top, then content top from that published coordinate.
/// Each fixed sum uses exact arithmetic and nearest/ties-even finite conversion.
/// This publication sequence is engineering policy, not a CSS rounding rule.
/// # Errors
/// Validates containing top, signed margin, nonnegative border and padding in that
/// order before arithmetic. Overflow names the first failed publication stage.
pub fn place_first_child_origin(
    input: FirstChildOriginInput,
) -> Result<FirstChildOriginPlacement, FirstChildOriginError> {
    use FirstChildOriginError as E;
    if !input.containing_top.is_finite() {
        return Err(E::InvalidInput("containing_top"));
    }
    let (used_margin_top, margin_resolution) = match input.margin_top {
        AutoLength::Auto => (0., TopMarginResolution::AutoZero),
        AutoLength::Value(value) => {
            if !value.is_finite() {
                return Err(E::InvalidInput("margin_top"));
            }
            (
                if value == 0. { 0. } else { value },
                TopMarginResolution::Absolute,
            )
        }
    };
    for (value, field) in [
        (input.border_top, "border_top"),
        (input.padding_top, "padding_top"),
    ] {
        if !value.is_finite() || value < 0. {
            return Err(E::InvalidInput(field));
        }
    }
    let border_top = Exact::from_f32(input.containing_top)
        .add(Exact::from_f32(used_margin_top))
        .to_f32()
        .map_err(|_| E::Unrepresentable {
            stage: OriginCoordinateStage::BorderTop,
        })?;
    let content_top = Exact::from_f32(border_top)
        .add(Exact::from_f32(input.border_top))
        .add(Exact::from_f32(input.padding_top))
        .to_f32()
        .map_err(|_| E::Unrepresentable {
            stage: OriginCoordinateStage::ContentTop,
        })?;
    Ok(FirstChildOriginPlacement {
        used_margin_top,
        margin_resolution,
        border_top,
        content_top,
    })
}
