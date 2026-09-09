use super::sheets::{AuthorSheets, SheetError, SheetId};
use super::{Candidate, Level, Specificity, Winner, WinnerAdmission};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorAdmission {
    Excluded,
    Admitted(WinnerAdmission),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionError {
    WrongOrigin,
    InvalidHint,
    Membership(SheetError),
    ConflictingOrder,
}
impl std::fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongOrigin => {
                f.write_str("author-sheet candidate has a non-author cascade level")
            }
            Self::InvalidHint => f.write_str("native hint requires author-normal zero specificity"),
            Self::Membership(error) => error.fmt(f),
            Self::ConflictingOrder => f.write_str("conflicting immutable declaration order"),
        }
    }
}
impl std::error::Error for AdmissionError {}

/// Gate an already matched target/property candidate using its owning author
/// sheet. Imported occurrences use the registered root sheet's identity. On a
/// membership change, the owner must rebuild winners; this cannot retract a
/// previously offered candidate or atomically publish a complete evaluation.
pub fn offer_author_sheet<T: PartialEq>(
    sheets: &AuthorSheets,
    owner: SheetId,
    winner: &mut Winner<T>,
    candidate: Candidate<T>,
) -> Result<AuthorAdmission, AdmissionError> {
    if !matches!(
        candidate.key.level,
        Level::AuthorNormal | Level::AuthorImportant
    ) {
        return Err(AdmissionError::WrongOrigin);
    }
    if !sheets.applies(owner).map_err(AdmissionError::Membership)? {
        return Ok(AuthorAdmission::Excluded);
    }
    winner
        .offer(candidate)
        .map(AuthorAdmission::Admitted)
        .map_err(|_| AdmissionError::ConflictingOrder)
}

/// Native hints participate in the normal cascade and share global author enable.
/// The order allocator must place them before author rules, as section 49 requires.
pub fn offer_native_hint<T: PartialEq>(
    sheets: &AuthorSheets,
    winner: &mut Winner<T>,
    candidate: Candidate<T>,
) -> Result<AuthorAdmission, AdmissionError> {
    if candidate.key.level != Level::AuthorNormal || candidate.key.specificity != Specificity::ZERO
    {
        return Err(AdmissionError::InvalidHint);
    }
    if !sheets.native_hints_apply() {
        return Ok(AuthorAdmission::Excluded);
    }
    winner
        .offer(candidate)
        .map(AuthorAdmission::Admitted)
        .map_err(|_| AdmissionError::ConflictingOrder)
}
