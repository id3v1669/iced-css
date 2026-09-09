//! Import placement after statement classification, independently of import
//! grammar, media admission, URI resolution and resource acquisition.
use super::StatementContext;

/// The parser supplies whether a statement is ignored. Selector nonmatching or
/// an empty declaration result alone does not make a statement ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementDisposition {
    Ignored,
    Charset,
    Import,
    /// Any other non-ignored statement.
    Other,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementDecision {
    NotImport,
    Allowed,
    Nested,
    Late,
}
/// One syntax level of one stylesheet occurrence. A fresh stylesheet has its own
/// top-level window. State is fixed-size; observe performs constant work and never
/// allocates. This object does not determine whether any raw statement is valid.
#[derive(Debug)]
pub struct ImportPlacement {
    context: StatementContext,
    closed: bool,
}
impl ImportPlacement {
    pub const fn new(context: StatementContext) -> Self {
        Self {
            context,
            closed: false,
        }
    }
    pub fn observe(&mut self, disposition: StatementDisposition) -> PlacementDecision {
        match disposition {
            StatementDisposition::Ignored | StatementDisposition::Charset => {
                PlacementDecision::NotImport
            }
            StatementDisposition::Other => {
                self.closed = true;
                PlacementDecision::NotImport
            }
            StatementDisposition::Import => match self.context {
                StatementContext::Block => PlacementDecision::Nested,
                StatementContext::Stylesheet if self.closed => PlacementDecision::Late,
                StatementContext::Stylesheet => PlacementDecision::Allowed,
            },
        }
    }
}
