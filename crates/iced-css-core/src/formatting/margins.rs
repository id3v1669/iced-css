//! Numerical extrema for an already established adjoining margin group.
//! The owner must retain member identities, eligibility and adjacency barriers.
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginError {
    Work,
    Nonfinite { index: usize },
}
impl std::fmt::Display for MarginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "margin collapse arithmetic: {self:?}")
    }
}
impl std::error::Error for MarginError {}
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MarginExtrema {
    positive: f32,
    negative: f32,
}
impl MarginExtrema {
    pub fn positive(self) -> f32 {
        self.positive
    }
    pub fn negative(self) -> f32 {
        self.negative
    }
    pub fn collapsed(self) -> f32 {
        // Opposite signs and finite operands cannot overflow their f32 domain.
        let result = self.positive + self.negative;
        if result == 0.0 { 0.0 } else { result }
    }
    /// Extend atomically, after the caller has established these margins adjoin.
    pub fn extend(&mut self, margins: &[f32], work: NonZeroUsize) -> Result<(), MarginError> {
        if margins.len() > work.get() {
            return Err(MarginError::Work);
        }
        let mut candidate = *self;
        for (index, margin) in margins.iter().copied().enumerate() {
            if !margin.is_finite() {
                return Err(MarginError::Nonfinite { index });
            }
            if margin > candidate.positive {
                candidate.positive = margin;
            }
            if margin < candidate.negative {
                candidate.negative = margin;
            }
        }
        *self = candidate;
        Ok(())
    }
    /// The caller establishes group adjacency; both original extrema survive.
    pub fn merge(&mut self, other: Self) {
        self.positive = self.positive.max(other.positive);
        self.negative = self.negative.min(other.negative);
    }
}
