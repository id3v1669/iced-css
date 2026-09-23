//! Bounded noncollapsing own bottom edges from an authoritative content endpoint.
//! Callers establish CSS eligibility, including Auto vertical margin used zero;
//! scalar inputs cannot prove source membership or noncollapsing relationships.
use super::{arithmetic::Exact, block::AutoLength};

/// Content coordinate, nonnegative padding/border lengths, and signed margin.
#[derive(Debug, Clone, Copy)]
pub struct BlockBottomInput {
    pub content_bottom: f32,
    pub padding_bottom: f32,
    pub border_bottom: f32,
    pub margin_bottom: AutoLength,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomMarginResolution {
    AutoZero,
    Absolute,
}
/// Finite publication boundary, not authored CSS validity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockBottomStage {
    PaddingBottom,
    BorderBottom,
    MarginBottom,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockBottomError {
    InvalidInput(&'static str),
    Unrepresentable { stage: BlockBottomStage },
}
impl std::fmt::Display for BlockBottomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "block bottom: {self:?}")
    }
}
impl std::error::Error for BlockBottomError {}
/// Published padding, border and margin coordinates, with the used margin length.
/// A signed margin may place its edge before any other edge; no ordering is implied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockBottomPlacement {
    pub used_margin_bottom: f32,
    pub margin_resolution: BottomMarginResolution,
    pub padding_bottom: f32,
    pub border_bottom: f32,
    pub margin_bottom: f32,
}
/// Publish each two-term sum independently with Exact nearest/ties-even and +0.
/// Each stage starts from the prior published coordinate, without hidden carry.
/// # Errors
/// Validates content, padding, border and margin in that order before arithmetic.
/// Padding/border must be nonnegative; finite signed margins are accepted.
/// Reports the first unrepresentable edge, even if a later margin would cancel it.
pub fn place_block_bottom(
    input: BlockBottomInput,
) -> Result<BlockBottomPlacement, BlockBottomError> {
    use BlockBottomError as E;
    if !input.content_bottom.is_finite() {
        return Err(E::InvalidInput("content_bottom"));
    }
    if !input.padding_bottom.is_finite() || input.padding_bottom < 0. {
        return Err(E::InvalidInput("padding_bottom"));
    }
    if !input.border_bottom.is_finite() || input.border_bottom < 0. {
        return Err(E::InvalidInput("border_bottom"));
    }
    let (used_margin_bottom, margin_resolution) = match input.margin_bottom {
        AutoLength::Auto => (0., BottomMarginResolution::AutoZero),
        AutoLength::Value(value) => {
            if !value.is_finite() {
                return Err(E::InvalidInput("margin_bottom"));
            }
            (
                if value == 0. { 0. } else { value },
                BottomMarginResolution::Absolute,
            )
        }
    };
    let publish = |a, b, stage| {
        Exact::from_f32(a)
            .add(Exact::from_f32(b))
            .to_f32()
            .map_err(|_| E::Unrepresentable { stage })
    };
    let padding_bottom = publish(
        input.content_bottom,
        input.padding_bottom,
        BlockBottomStage::PaddingBottom,
    )?;
    let border_bottom = publish(
        padding_bottom,
        input.border_bottom,
        BlockBottomStage::BorderBottom,
    )?;
    let margin_bottom = publish(
        border_bottom,
        used_margin_bottom,
        BlockBottomStage::MarginBottom,
    )?;
    Ok(BlockBottomPlacement {
        used_margin_bottom,
        margin_resolution,
        padding_bottom,
        border_bottom,
        margin_bottom,
    })
}
