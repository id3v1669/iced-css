//! Bounded content height for caller-established block-containing BFC membership.
//! The caller proves membership and min-height zero/max-height none. The legacy
//! scalar entry point requires nonnegative margins and its designated first edge;
//! the signed entry points publish all normal edges and accept their extrema.
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
    /// Last normal margin bottom in the nonnegative primitive; extreme normal
    /// margin bottom in the signed primitive, distinct from its rebased endpoint.
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

/// Published normal margin edge, in top-before-bottom evaluation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignedNormalMarginEdge {
    Top,
    Bottom,
}

/// Finite principal border edges and signed used vertical margins.
#[derive(Debug, Clone, Copy)]
pub struct SignedNormalMarginInput {
    pub border_top: f32,
    pub border_bottom: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
}

/// Independent margin observations; top may exceed bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignedNormalMarginEdges {
    pub top: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignedNormalMarginError {
    InvalidInput(&'static str),
    Unrepresentable { edge: SignedNormalMarginEdge },
}

impl std::fmt::Display for SignedNormalMarginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "signed normal margin: {self:?}")
    }
}
impl std::error::Error for SignedNormalMarginError {}

/// Publish signed margin edges with Exact nearest/ties-even rounding and +0.
/// The caller establishes that this is a participating normal principal box.
/// # Errors
/// Validate border top, border bottom, margin top and margin bottom finiteness,
/// then border ordering, before arithmetic. Publish top before bottom.
pub fn publish_signed_normal_margin_edges(
    input: SignedNormalMarginInput,
) -> Result<SignedNormalMarginEdges, SignedNormalMarginError> {
    use SignedNormalMarginError as E;
    for (name, value) in [
        ("border_top", input.border_top),
        ("border_bottom", input.border_bottom),
        ("margin_top", input.margin_top),
        ("margin_bottom", input.margin_bottom),
    ] {
        if !value.is_finite() {
            return Err(E::InvalidInput(name));
        }
    }
    if input.border_bottom < input.border_top {
        return Err(E::InvalidInput("border_bottom_before_top"));
    }
    let top = Exact::from_f32(input.border_top)
        .sub(Exact::from_f32(input.margin_top))
        .to_f32()
        .map_err(|_| E::Unrepresentable {
            edge: SignedNormalMarginEdge::Top,
        })?;
    let bottom = Exact::from_f32(input.border_bottom)
        .add(Exact::from_f32(input.margin_bottom))
        .to_f32()
        .map_err(|_| E::Unrepresentable {
            edge: SignedNormalMarginEdge::Bottom,
        })?;
    Ok(SignedNormalMarginEdges { top, bottom })
}

/// Extrema of all participating normal margin edges, plus the separate float.
/// Membership is caller-established. The extrema may describe an inverted span.
#[derive(Debug, Clone, Copy)]
pub struct SignedBfcHeightInput {
    pub content_top: f32,
    pub normal_margin_top: f32,
    pub normal_margin_bottom: f32,
    pub float_bottom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignedBfcHeightStage {
    NormalHeight,
    NormalContentBottom,
    ExtendedHeight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignedBfcHeightError {
    InvalidInput(&'static str),
    Unrepresentable { stage: SignedBfcHeightStage },
}

impl std::fmt::Display for SignedBfcHeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "signed BFC content height: {self:?}")
    }
}
impl std::error::Error for SignedBfcHeightError {}

/// Floor the exact normal span at zero, rebase it onto the parent's established
/// content top, then extend strictly to the participating float bottom.
/// This publication policy is an engineering choice, not a CSS rounding rule.
/// `auto_extent.normal_margin_bottom` records the extreme normal bottom, which
/// may differ from the rebased normal endpoint and the final selected endpoint.
/// # Errors
/// Check finiteness in content-top, normal-top, normal-bottom, float-bottom order,
/// then float-bottom ordering against content top, before any arithmetic.
/// Publication failures identify normal height, normal endpoint or extension.
pub fn solve_signed_bfc_height(
    input: SignedBfcHeightInput,
) -> Result<BfcHeightPlacement, SignedBfcHeightError> {
    use SignedBfcHeightError as E;
    for (name, value) in [
        ("content_top", input.content_top),
        ("normal_margin_top", input.normal_margin_top),
        ("normal_margin_bottom", input.normal_margin_bottom),
        ("float_bottom", input.float_bottom),
    ] {
        if !value.is_finite() {
            return Err(E::InvalidInput(name));
        }
    }
    if input.float_bottom < input.content_top {
        return Err(E::InvalidInput("float_bottom_before_top"));
    }
    let span =
        Exact::from_f32(input.normal_margin_bottom).sub(Exact::from_f32(input.normal_margin_top));
    let normal_height = if span <= Exact::ZERO {
        0.0
    } else {
        span.to_f32().map_err(|_| E::Unrepresentable {
            stage: SignedBfcHeightStage::NormalHeight,
        })?
    };
    let normal_bottom = Exact::from_f32(input.content_top)
        .add(Exact::from_f32(normal_height))
        .to_f32()
        .map_err(|_| E::Unrepresentable {
            stage: SignedBfcHeightStage::NormalContentBottom,
        })?;
    let float_extended = input.float_bottom > normal_bottom;
    let (used_height, content_bottom) = if float_extended {
        let used_height = Exact::from_f32(input.float_bottom)
            .sub(Exact::from_f32(input.content_top))
            .to_f32()
            .map_err(|_| E::Unrepresentable {
                stage: SignedBfcHeightStage::ExtendedHeight,
            })?;
        let endpoint = if input.float_bottom == 0.0 {
            0.0
        } else {
            input.float_bottom
        };
        (used_height, endpoint)
    } else {
        (normal_height, normal_bottom)
    };
    Ok(BfcHeightPlacement {
        used_height,
        content_bottom,
        auto_extent: Some(AutoHeightExtent {
            normal_margin_bottom: if input.normal_margin_bottom == 0.0 {
                0.0
            } else {
                input.normal_margin_bottom
            },
            selected_bottom: content_bottom,
            float_extended,
        }),
    })
}
