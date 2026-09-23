//! Bounded content height for caller-established block-containing BFC membership.
//! The caller proves noncollapsing nonnegative child margins, the designated first
//! margin-edge top, one participating float and min-height zero/max-height none.
//! Scalars cannot establish source eligibility or describe a general BFC tree.
use super::arithmetic::Exact;

/// Explicit height has no descendant extent inputs to evaluate.
#[derive(Debug, Clone, Copy)]
pub enum BfcHeightMode {
    Explicit(f32),
    Auto {
        last_in_flow_border_bottom: f32,
        last_in_flow_margin_bottom: f32,
        float_bottom: f32,
    },
}
/// Designated published content top and the applicable height branch.
#[derive(Debug, Clone, Copy)]
pub struct BfcHeightInput {
    pub content_top: f32,
    pub mode: BfcHeightMode,
}
/// Finite publication boundary, not an authored CSS validity classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BfcHeightStage {
    NormalMarginBottom,
    UsedHeight,
    ContentBottom,
}
/// Invalid primitive input or a finite-profile publication limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BfcHeightError {
    InvalidInput(&'static str),
    Unrepresentable { stage: BfcHeightStage },
}
impl std::fmt::Display for BfcHeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BFC content height: {self:?}")
    }
}
impl std::error::Error for BfcHeightError {}
/// Published Auto edges; float extension is strict after edge publication.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoHeightExtent {
    pub normal_margin_bottom: f32,
    pub selected_bottom: f32,
    pub float_extended: bool,
}
/// Independent finite height and endpoint observations.
/// Auto retains its selected edge even when top + rounded height differs.
/// An absent extent means unevaluated, not absence of descendants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BfcHeightPlacement {
    pub used_height: f32,
    pub content_bottom: f32,
    pub auto_extent: Option<AutoHeightExtent>,
}
/// Resolve this fixed profile with Exact nearest/ties-even publication and +0.
/// This endpoint policy is an engineering choice, not a CSS rounding rule.
/// # Errors
/// Checks top first; Explicit checks height. Auto checks normal, margin and float
/// domains, then normal/float ordering against top, before any arithmetic.
/// Publication failure identifies its first observable stage.
pub fn solve_bfc_height(input: BfcHeightInput) -> Result<BfcHeightPlacement, BfcHeightError> {
    use BfcHeightError as E;
    if !input.content_top.is_finite() {
        return Err(E::InvalidInput("content_top"));
    }
    match input.mode {
        BfcHeightMode::Explicit(height) => {
            if !height.is_finite() || height < 0. {
                return Err(E::InvalidInput("height"));
            }
            let used_height = if height == 0. { 0. } else { height };
            let content_bottom = Exact::from_f32(input.content_top)
                .add(Exact::from_f32(used_height))
                .to_f32()
                .map_err(|_| E::Unrepresentable {
                    stage: BfcHeightStage::ContentBottom,
                })?;
            Ok(BfcHeightPlacement {
                used_height,
                content_bottom,
                auto_extent: None,
            })
        }
        BfcHeightMode::Auto {
            last_in_flow_border_bottom,
            last_in_flow_margin_bottom,
            float_bottom,
        } => {
            if !last_in_flow_border_bottom.is_finite() {
                return Err(E::InvalidInput("last_in_flow_border_bottom"));
            }
            if !last_in_flow_margin_bottom.is_finite() || last_in_flow_margin_bottom < 0. {
                return Err(E::InvalidInput("last_in_flow_margin_bottom"));
            }
            if !float_bottom.is_finite() {
                return Err(E::InvalidInput("float_bottom"));
            }
            if last_in_flow_border_bottom < input.content_top {
                return Err(E::InvalidInput("normal_bottom_before_top"));
            }
            if float_bottom < input.content_top {
                return Err(E::InvalidInput("float_bottom_before_top"));
            }
            let normal_margin_bottom = Exact::from_f32(last_in_flow_border_bottom)
                .add(Exact::from_f32(last_in_flow_margin_bottom))
                .to_f32()
                .map_err(|_| E::Unrepresentable {
                    stage: BfcHeightStage::NormalMarginBottom,
                })?;
            let selected_bottom = normal_margin_bottom.max(float_bottom);
            let selected_bottom = if selected_bottom == 0. {
                0.
            } else {
                selected_bottom
            };
            let used_height = Exact::from_f32(selected_bottom)
                .sub(Exact::from_f32(input.content_top))
                .to_f32()
                .map_err(|_| E::Unrepresentable {
                    stage: BfcHeightStage::UsedHeight,
                })?;
            Ok(BfcHeightPlacement {
                used_height,
                content_bottom: selected_bottom,
                auto_extent: Some(AutoHeightExtent {
                    normal_margin_bottom,
                    selected_bottom,
                    float_extended: float_bottom > normal_margin_bottom,
                }),
            })
        }
    }
}
