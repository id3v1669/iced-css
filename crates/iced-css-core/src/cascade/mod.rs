//! CSS2.2 cascade ordering primitives. Matching and sheet membership are separate.
mod admission;
pub mod sheets;
pub use admission::{AdmissionError, AuthorAdmission, offer_author_sheet, offer_native_hint};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    UserAgent,
    User,
    Author,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    UserAgent,
    UserNormal,
    AuthorNormal,
    AuthorImportant,
    UserImportant,
}
impl Origin {
    /// CSS2.2 has five levels; it does not introduce a separate UA-important tier.
    pub fn level(self, important: bool) -> Level {
        match (self, important) {
            (Self::UserAgent, _) => Level::UserAgent,
            (Self::User, false) => Level::UserNormal,
            (Self::User, true) => Level::UserImportant,
            (Self::Author, false) => Level::AuthorNormal,
            (Self::Author, true) => Level::AuthorImportant,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Specificity([usize; 4]);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecificityError {
    Overflow,
    StyleAttribute,
}
impl std::fmt::Display for SpecificityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Overflow => "selector specificity count overflow",
            Self::StyleAttribute => {
                "style-attribute specificity cannot be combined with a selector"
            }
        })
    }
}
impl std::error::Error for SpecificityError {}
impl Specificity {
    pub const ZERO: Self = Self([0; 4]);
    pub const STYLE_ATTRIBUTE: Self = Self([1, 0, 0, 0]);
    pub fn selector(ids: usize, attributes: usize, types: usize) -> Self {
        Self([0, ids, attributes, types])
    }
    pub fn components(self) -> [usize; 4] {
        self.0
    }
    pub fn checked_selector_sum(self, other: Self) -> Result<Self, SpecificityError> {
        if self.0[0] != 0 || other.0[0] != 0 {
            return Err(SpecificityError::StyleAttribute);
        }
        let mut result = [0; 4];
        for (index, value) in result.iter_mut().enumerate().skip(1) {
            *value = self.0[index]
                .checked_add(other.0[index])
                .ok_or(SpecificityError::Overflow)?;
        }
        Ok(Self(result))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CascadeKey {
    pub level: Level,
    pub specificity: Specificity,
    pub order: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate<T> {
    pub key: CascadeKey,
    pub declaration: std::num::NonZeroU64,
    pub value: T,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinnerAdmission {
    Replaced,
    Retained,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictingOrder;
impl std::fmt::Display for ConflictingOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("equal cascade keys describe conflicting declarations")
    }
}
impl std::error::Error for ConflictingOrder {}
#[derive(Debug)]
pub struct Winner<T> {
    candidate: Option<Candidate<T>>,
}
impl<T: PartialEq> Winner<T> {
    pub fn new() -> Self {
        Self { candidate: None }
    }
    pub fn candidate(&self) -> Option<&Candidate<T>> {
        self.candidate.as_ref()
    }
    /// Offer a candidate already admitted for the same target/property. Equal
    /// keys are permitted only for repeated identical immutable declarations.
    /// Source occurrence/order allocation and whole-evaluation publication are
    /// separate responsibilities. A conflict leaves the current winner intact.
    pub fn offer(&mut self, candidate: Candidate<T>) -> Result<WinnerAdmission, ConflictingOrder> {
        if let Some(current) = &self.candidate {
            match candidate.key.cmp(&current.key) {
                std::cmp::Ordering::Less => return Ok(WinnerAdmission::Retained),
                std::cmp::Ordering::Equal => {
                    return if candidate.declaration == current.declaration
                        && candidate.value == current.value
                    {
                        Ok(WinnerAdmission::Retained)
                    } else {
                        Err(ConflictingOrder)
                    };
                }
                std::cmp::Ordering::Greater => {}
            }
        }
        self.candidate = Some(candidate);
        Ok(WinnerAdmission::Replaced)
    }
}
impl<T: PartialEq> Default for Winner<T> {
    fn default() -> Self {
        Self::new()
    }
}
mod defaulting;
pub use defaulting::{
    DefaultedValue, DefaultedValueRef, DefaultingError, default_value, default_value_ref,
};
pub mod rule;
pub mod winner_table;
